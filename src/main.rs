use dotenv::dotenv;
use embedded_hal::delay::DelayNs;
use embedded_svc::http::{client::Client, Headers, Status};
use esp_idf_hal::{
    delay::{Delay, FreeRtos},
    gpio::{IOPin, PinDriver},
    io::Read,
    peripherals::Peripherals,
    sys::esp_crt_bundle_attach,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::client::{Configuration as HttpConfig, EspHttpConnection},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use models::CurrentlyPlaying;
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

    let check_playback = thread::Builder::new().stack_size(64 * 1024).spawn(|| {
        loop {
            let httpconnection = EspHttpConnection::new(&HttpConfig {
                // use_global_ca_store: true,
                crt_bundle_attach: Some(esp_crt_bundle_attach),
                ..Default::default()
            })
            .expect("Could not establish http connection");

            let mut httpclient = Client::wrap(httpconnection);
            let url_root = include_str!("../api_url.txt").replace("\n", "");
            let formatted_url = std::format!("{}/current_playback", url_root);

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
    });

    loop {
        if btn_pin1.is_high() && btn1_status == ButtonStatus::Low {
            println!("Button 1 Pressed");
            // Using a button lock to make sure register one button input at a time
            btn_lock = false;
            btn1_status = ButtonStatus::High;
        } else if btn_pin1.is_low() && !btn_lock {
            btn_lock = true;
            btn1_status = ButtonStatus::Low;
        }

        if btn_pin2.is_high() && btn2_status == ButtonStatus::Low {
            println!("Button 2 Pressed");
            btn_lock = false;
            btn2_status = ButtonStatus::High;
        } else if btn_pin2.is_low() && !btn_lock {
            btn_lock = true;
            btn2_status = ButtonStatus::Low;
        }

        if btn_pin3.is_high() && btn3_status == ButtonStatus::Low {
            println!("Button 3 Pressed");
            btn_lock = false;
            btn3_status = ButtonStatus::High;
        } else if btn_pin3.is_low() && !btn_lock {
            btn_lock = true;
            btn3_status = ButtonStatus::Low;
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn wifi(wifi_driver: &mut BlockingWifi<EspWifi>) {
    // TODO: Make wifi work for enterprise networks -> I will need it for college wifi
    let wifi_ssid = include_str!("../wifi_ssid.txt");
    let wifi_password = include_str!("../wifi_password.txt");
    let auth_token = include_str!("../auth_token.txt");

    wifi_driver
        .set_configuration(&embedded_svc::wifi::Configuration::Client(
            embedded_svc::wifi::ClientConfiguration {
                ssid: heapless::String::try_from(wifi_ssid).unwrap(),
                password: heapless::String::try_from(wifi_password).unwrap(),
                ..Default::default()
            },
        ))
        .unwrap();

    wifi_driver.start().unwrap();
    wifi_driver.connect().unwrap();

    println!("Waiting for wifi connection");
    wifi_driver.wait_netif_up().unwrap();

    println!(
        "Connected to IP {:?}",
        wifi_driver.wifi().sta_netif().get_ip_info()
    );
}
