use dotenv::dotenv;
use embedded_hal::delay::DelayNs;
use embedded_svc::http::{client::Client, Headers, Status};
use esp_idf_hal::{
    delay::{Delay, FreeRtos},
    gpio::{IOPin, PinDriver},
    io::Read,
    peripherals::Peripherals,
    sys::{esp_crt_bundle_attach, esp_get_free_heap_size, esp_get_minimum_free_heap_size},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::client::{Configuration as HttpConfig, EspHttpConnection},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use log::{error, info};
use models::CurrentlyPlaying;
use once_cell::sync::Lazy;
use serde_json::json;
use std::{
    thread::{self, sleep},
    time::Duration,
};

#[derive(PartialEq, Eq)]
enum ButtonStatus {
    Low,
    High,
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    dotenv().ok();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let mut btn1_status = ButtonStatus::High;
    let mut btn2_status = ButtonStatus::High;
    let mut btn3_status = ButtonStatus::High;
    let mut btn_pin1 = PinDriver::input(peripherals.pins.gpio5.downgrade()).unwrap();
    let mut btn_pin2 = PinDriver::input(peripherals.pins.gpio1.downgrade()).unwrap();
    let mut btn_pin3 = PinDriver::input(peripherals.pins.gpio17.downgrade()).unwrap();
    btn_pin1.set_pull(esp_idf_hal::gpio::Pull::Up).unwrap();
    btn_pin2.set_pull(esp_idf_hal::gpio::Pull::Up).unwrap();
    btn_pin3.set_pull(esp_idf_hal::gpio::Pull::Up).unwrap();
    let mut btn_lock = false;

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

    let check_playback = thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
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
                let mut image_buf = vec![0u8; 300 * 300];

                response.read_exact(&mut playing_buf).unwrap();

                let response_str = std::str::from_utf8(&playing_buf);

                let playing_json: Result<Option<CurrentlyPlaying>, serde_json::Error> =
                    serde_json::from_slice(&playing_buf);

                if let Err(_) = playing_json {
                    Delay::new_default().delay_ms(5000);
                    continue;
                }

                // println!("{:?}", response_str);
                // println!("{:#?}", playing_json);

                Delay::new_default().delay_ms(1000);
            }
        })
        .unwrap();

    loop {
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

        print_memory_info();

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
