use display_interface_spi::SPIInterfaceNoCS;
use dotenv::dotenv;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_svc::http::{client::Client, Headers};
use esp_idf_hal::{
    delay::Delay,
    gpio::{IOPin, PinDriver},
    io::Read,
    peripherals::Peripherals,
    spi::{self, SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI2},
    sys::{esp_crt_bundle_attach, esp_get_free_heap_size, esp_get_minimum_free_heap_size},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::client::{Configuration as HttpConfig, EspHttpConnection},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use ili9341::Ili9341;
use log::{error, info};
use models::CurrentlyPlaying;
use once_cell::sync::Lazy;
use std::{
    sync::{Arc, Mutex},
    thread::{self},
    time::Duration,
};

#[derive(PartialEq, Eq)]
enum ButtonStatus {
    Low,
    High,
}

enum Signal {
    ChangeSong(Option<CurrentlyPlaying>),
    UpdateProgress(Option<u32>),
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    dotenv().ok();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let pins = peripherals.pins;

    let mut btn1_status = ButtonStatus::High;
    let mut btn2_status = ButtonStatus::High;
    let mut btn3_status = ButtonStatus::High;
    let mut btn_pin1 = PinDriver::input(pins.gpio5.downgrade()).unwrap();
    let mut btn_pin2 = PinDriver::input(pins.gpio1.downgrade()).unwrap();
    let mut btn_pin3 = PinDriver::input(pins.gpio2.downgrade()).unwrap();
    btn_pin1.set_pull(esp_idf_hal::gpio::Pull::Up).unwrap();
    btn_pin2.set_pull(esp_idf_hal::gpio::Pull::Up).unwrap();
    btn_pin3.set_pull(esp_idf_hal::gpio::Pull::Up).unwrap();
    let mut btn_lock = false;

    let sclk = pins.gpio39;
    let mosi = pins.gpio11;

    // let cs = PinDriver::output(pins.gpio17).unwrap();
    let miso = pins.gpio13;
    let dc = PinDriver::output(pins.gpio15).unwrap();
    let rst = PinDriver::output(pins.gpio16).unwrap();

    let spi = peripherals.spi2;
    let spidispplayinterface = {
        let driver =
            SpiDriver::new::<SPI2>(spi, sclk, mosi, Some(miso), &SpiDriverConfig::new()).unwrap();
        let config =
            spi::config::Config::default().baudrate(esp_idf_hal::units::Hertz(10 * 1_000_000));

        let spi_device = SpiDeviceDriver::new(driver, Some(pins.gpio17), &config).unwrap();

        SPIInterfaceNoCS::new(spi_device, dc)
    };

    let mut display = Ili9341::new(
        spidispplayinterface,
        rst,
        &mut esp_idf_hal::delay::FreeRtos,
        ili9341::Orientation::Portrait,
        ili9341::DisplaySize240x320,
    )
    .expect("Failed to initialize LCD ILI9341.");

    graphics::fill_display(&mut display, Rgb565::BLACK);

    let mut wifi_driver = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
    )
    .unwrap();

    wifi(&mut wifi_driver);

    static API_URL_ROOT: Lazy<String> = Lazy::new(|| {
        include_str!("../api_url.txt")
            .trim()
            .replace("\n", "")
            .to_string()
    });

    static AUTH_TOKEN: Lazy<String> = Lazy::new(|| {
        include_str!("../auth_token.txt")
            .trim()
            .replace("\n", "")
            .to_string()
    });

    // Using crossbeam channel instead of normal channels because it avoids blocking indefinitely
    // and keeps watchdog happy :)
    let (transmitter, receiver) = crossbeam_channel::unbounded();

    // Used instead of song name because two songs can have the same name, but a url acts like an id
    let stored_song_url: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let stored_song_position: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));

    let check_playback = thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            loop {
                let httpconnection = EspHttpConnection::new(&HttpConfig {
                    // use_global_ca_store: true,
                    crt_bundle_attach: Some(esp_crt_bundle_attach),
                    ..Default::default()
                })
                .expect("Could not establish http connection");

                let mut httpclient = Client::wrap(httpconnection);
                let formatted_url = std::format!("{}/current_playback", *API_URL_ROOT);

                let request = httpclient
                    .get(&formatted_url)
                    .expect("could not send current_playback request");

                let mut response = request.submit().expect("could not get response");

                let mut playing_buf = vec![0u8; response.content_len().unwrap() as usize];
                // let mut image_buf = vec![0u8; 300 * 300];

                response.read_exact(&mut playing_buf).unwrap();

                let playing_json: Result<Option<CurrentlyPlaying>, serde_json::Error> =
                    serde_json::from_slice(&playing_buf);

                if let Err(_) = playing_json {
                    Delay::new_default().delay_ms(5000);
                    continue;
                }

                let mut prev_song_url = stored_song_url.lock().unwrap();
                let mut prev_song_position = stored_song_position.lock().unwrap();

                let playing_data = match playing_json {
                    Ok(Some(data)) => data,
                    _ => {
                        if prev_song_url.is_some() || prev_song_position.is_some() {
                            *prev_song_url = None;
                            *prev_song_position = None;
                            transmitter.send(Signal::ChangeSong(None));
                            transmitter.send(Signal::UpdateProgress(None));
                        }
                        Delay::new_default().delay_ms(5000);
                        continue;
                    }
                };

                if prev_song_url.is_none()
                    || (prev_song_url.is_some()
                        && *prev_song_url != Some(playing_data.track.name.clone()))
                {
                    *prev_song_url = Some(playing_data.track.name.clone());
                    transmitter.send(Signal::ChangeSong(Some(playing_data.clone())));
                }

                if prev_song_position.is_none()
                    || (prev_song_position.is_some()
                        && *prev_song_position != Some(playing_data.progress_secs))
                {
                    *prev_song_position = Some(playing_data.progress_secs);
                    transmitter.send(Signal::UpdateProgress(Some(playing_data.progress_secs)));
                }

                Delay::new_default().delay_ms(1000);
            }
        })
        .unwrap();

    let control_playback_thread = thread::Builder::new()
        .stack_size(8 * 1024)
        .spawn(move || loop {
            if btn_pin1.is_high() && btn1_status == ButtonStatus::Low {
                info!("Button 1 Pressed - Attempting to skip track");
                // Using a button lock to make sure register one button input at a time
                btn_lock = false;
                btn1_status = ButtonStatus::High;

                if !previous_track(&*API_URL_ROOT, &*AUTH_TOKEN) {
                    error!("could not go to previous track");
                }
            } else if btn_pin1.is_low() && !btn_lock {
                btn_lock = true;
                btn1_status = ButtonStatus::Low;
            }

            if btn_pin2.is_high() && btn2_status == ButtonStatus::Low {
                info!("Button 2 Pressed - Attempting to toggle playback");
                btn_lock = false;
                btn2_status = ButtonStatus::High;

                if !toggle_playback(&*API_URL_ROOT, &*AUTH_TOKEN) {
                    error!("could not toggle playback");
                }
            } else if btn_pin2.is_low() && !btn_lock {
                btn_lock = true;
                btn2_status = ButtonStatus::Low;
            }

            if btn_pin3.is_high() && btn3_status == ButtonStatus::Low {
                info!("Button 3 Pressed - Attempting to skip track");
                btn_lock = false;
                btn3_status = ButtonStatus::High;

                if !skip_track(&*API_URL_ROOT, &*AUTH_TOKEN) {
                    error!("could not go to next track");
                }
            } else if btn_pin3.is_low() && !btn_lock {
                btn_lock = true;
                btn3_status = ButtonStatus::Low;
            }

            Delay::new_default().delay_ms(100);
        })
        .unwrap();

    loop {
        match receiver.recv() {
            Ok(received_signal) => match received_signal {
                Signal::ChangeSong(optional_currently_playing) => {
                    match optional_currently_playing {
                        Some(currently_playing) => {
                            println!("Attempting to draw image ");
                            graphics::draw_title_and_artist(
                                &mut display,
                                currently_playing.track.name,
                                currently_playing.track.artists[0].name.clone(),
                            );
                        }
                        None => {
                            println!("Nothing");
                        }
                    }
                }
                Signal::UpdateProgress(optional_position) => {}
            },
            Err(_) => {
                error!("Error receiving signal");
                continue;
            }
        }

        // print_memory_info();

        thread::sleep(Duration::from_millis(100));
    }
}

fn wifi(wifi_driver: &mut BlockingWifi<EspWifi>) {
    // TODO: Make wifi work for enterprise networks -> I will need it for college wifi
    let wifi_ssid = include_str!("../wifi_ssid.txt");
    let wifi_password = include_str!("../wifi_password.txt");

    std::thread::sleep(Duration::from_millis(4000));
    wifi_driver
        .set_configuration(&embedded_svc::wifi::Configuration::Client(
            embedded_svc::wifi::ClientConfiguration {
                ssid: heapless::String::try_from(wifi_ssid).unwrap(),
                password: heapless::String::try_from(wifi_password).unwrap(),
                auth_method: esp_idf_svc::wifi::AuthMethod::WPA2Personal,
                ..Default::default()
            },
        ))
        .unwrap();

    std::thread::sleep(Duration::from_millis(4000));
    wifi_driver.start().unwrap();
    let connection_info = wifi_driver.connect();

    if let Err(e) = connection_info {
        error!("something failed in connection info: {:?}", e);
    }

    std::thread::sleep(Duration::from_millis(4000));
    info!("Waiting for wifi connection");
    let waiting_info = wifi_driver.wait_netif_up();

    if let Err(e) = waiting_info {
        error!("Failed to connect to WiFi: {:?}", e);
    }

    info!(
        "Connected to IP {:?}",
        wifi_driver.wifi().sta_netif().get_ip_info()
    );
}

fn toggle_playback(api_url_root: &String, auth_token: &String) -> bool {
    let httpconnection = EspHttpConnection::new(&HttpConfig {
        // use_global_ca_store: true,
        crt_bundle_attach: Some(esp_crt_bundle_attach),
        ..Default::default()
    })
    .expect("Could not establish http connection");

    let mut httpclient = Client::wrap(httpconnection);
    let formatted_url = std::format!("{}/toggle_playback?auth_token={}", api_url_root, auth_token);
    let request = httpclient
        .get(&formatted_url)
        .expect("could not send current_playback request");

    let mut response = request.submit().expect("could not get response");
    if response.status() != 200 {
        error!("status code was {}", response.status());
    }
    return response.status() == 200;
}

fn skip_track(api_url_root: &String, auth_token: &String) -> bool {
    let httpconnection = EspHttpConnection::new(&HttpConfig {
        // use_global_ca_store: true,
        crt_bundle_attach: Some(esp_crt_bundle_attach),
        ..Default::default()
    })
    .expect("Could not establish http connection");

    let mut httpclient = Client::wrap(httpconnection);
    let formatted_url = std::format!("{}/next_track?auth_token={}", api_url_root, auth_token);
    let request = httpclient
        .get(&formatted_url)
        .expect("could not send next_track request");

    let mut response = request.submit().expect("could not get response");
    if response.status() != 200 {
        error!("status code was {}", response.status());
    }
    return response.status() == 200;
}

fn previous_track(api_url_root: &String, auth_token: &String) -> bool {
    let httpconnection = EspHttpConnection::new(&HttpConfig {
        // use_global_ca_store: true,
        crt_bundle_attach: Some(esp_crt_bundle_attach),
        ..Default::default()
    })
    .expect("Could not establish http connection");

    let mut httpclient = Client::wrap(httpconnection);
    let formatted_url = std::format!("{}/previous_track?auth_token={}", api_url_root, auth_token);
    let request = httpclient
        .get(&formatted_url)
        .expect("could not send previous_track request");

    let mut response = request.submit().expect("could not get response");
    if response.status() != 200 {
        error!("status code was {}", response.status());
    }
    return response.status() == 200;
}

fn print_memory_info() {
    println!("Total free heap size: {} bytes", unsafe {
        esp_get_free_heap_size()
    });
    println!("Minimum free heap size ever: {} bytes", unsafe {
        esp_get_minimum_free_heap_size()
    });
}
