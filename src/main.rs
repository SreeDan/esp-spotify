use dotenv::dotenv;
use embedded_hal::delay::DelayNs;
use embedded_svc::http::{client::Client, Headers, Status};
use esp_idf_hal::{delay::Delay, io::Read, peripherals::Peripherals, sys::esp_crt_bundle_attach};
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

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    dotenv().ok();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    // let mut wifi_driver = EspWifi::new(peripherals.modem, sys_loop, Some(nvs)).unwrap();
    let mut wifi_driver = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
    )
    .unwrap();

    wifi(&mut wifi_driver);

    let httpconnection = EspHttpConnection::new(&HttpConfig {
        // use_global_ca_store: true,
        crt_bundle_attach: Some(esp_crt_bundle_attach),
        ..Default::default()
    })
    .expect("Could not establish http connection");

    let mut httpclient = Client::wrap(httpconnection);

    let check_playback = thread::spawn(|| {
        loop {
            let httpconnection = EspHttpConnection::new(&HttpConfig {
                // use_global_ca_store: true,
                crt_bundle_attach: Some(esp_crt_bundle_attach),
                ..Default::default()
            })
            .expect("Could not establish http connection");

            let mut httpclient = Client::wrap(httpconnection);
            let url_root = include_str!("../api_url.txt");
            let formatted_url = std::format!("{}/current_playback", url_root);

            let request = httpclient
                .get(&formatted_url)
                .expect("could not send current_playback request");

            let mut response = request.submit().expect("could not get response");

            let mut buf = vec![0u8; response.content_len().unwrap() as usize];
            response.read_exact(&mut buf).unwrap();

            let response_str = std::str::from_utf8(&buf);
            let playing_json: Result<Option<CurrentlyPlaying>> =
                serde_json::from_slice(&buf).unwrap();

            if let Err(_) = playing_json {
                Delay::new_default().delay_ms(5000);
                continue;
            }

            println!("{:?}", playing_json);

            Delay::new_default().delay_ms(1000);
        }
    });

    loop {
        Delay::new_default().delay_ms(1);
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
    let mut seconds = 1;

    println!("Waiting for wifi connection");
    wifi_driver.wait_netif_up().unwrap();

    println!(
        "Connected to IP {:?}",
        wifi_driver.wifi().sta_netif().get_ip_info()
    );
}
