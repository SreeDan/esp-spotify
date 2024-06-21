use dotenv::dotenv;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition,
    wifi::EspWifi,
};
use std::{thread::sleep, time::Duration};

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    dotenv().ok();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let wifi_driver = EspWifi::new(
        peripherals.modem,
        sys_loop,
        Some(nvs)
    ).unwrap();

    wifi(wifi_driver);
}

fn wifi(mut wifi_driver: EspWifi) {
    // TODO: Make wifi work for enterprise networks -> I will need it for college wifi
    let wifi_ssid = include_str!("../wifi_ssid.txt");
    let wifi_password = include_str!("../wifi_password.txt");
    let auth_token = include_str!("../auth_token.txt");

    wifi_driver
        .set_configuration(&embedded_svc::wifi::Configuration::Client(embedded_svc::wifi::ClientConfiguration {
            ssid: heapless::String::try_from(wifi_ssid).unwrap(),
            password: heapless::String::try_from(wifi_password).unwrap(),
            ..Default::default()
        }))
        .unwrap();

    wifi_driver.start().unwrap();
    wifi_driver.connect().unwrap();
    let mut seconds = 1;

    while !wifi_driver.is_connected().unwrap() {
        let config = wifi_driver.get_configuration().unwrap();
        println!(
            "Waiting for station {:?}, waiting {seconds} seconds",
            config
        );
        sleep(Duration::from_secs(seconds));
        seconds *= 2;
    }

    // let mut client = HttpClient::wrap(EspHttpConnection::new(&Default::default())?);

    loop {
        println!(
            "Connected to IP {:?}",
            wifi_driver.sta_netif().get_ip_info().unwrap()
        );
        sleep(Duration::from_secs(1));
    }
}