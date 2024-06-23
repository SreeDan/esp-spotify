use embedded_graphics::{image::ImageRaw, pixelcolor::Rgb565, prelude::*};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};
use image::io::Reader as ImageReader;
use image::{DynamicImage, RgbImage};
use reqwest::blocking::get;
use std::io::Cursor;

fn main() -> Result<(), core::convert::Infallible> {
    let mut display = SimulatorDisplay::<Rgb565>::new(Size::new(240, 320));

    // URL of the Spotify album image
    let url = "https://i.scdn.co/image/ab67616d00001e02ff9ca10b55ce82ae553c8228";

    // Fetch and decode the image
    let image_bytes = get(url).ok().unwrap().bytes().ok().unwrap();

    let img = ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .expect("Failed to guess image format")
        .decode()
        .expect("Failed to decode image");

    let rgb_imag: RgbImage = img.into_rgb8();
    let resized_image = DynamicImage::ImageRgb8(rgb_imag)
        .resize(150, 150, image::imageops::FilterType::Nearest)
        .to_rgb8();

    let raw = resized_image.clone().into_raw();
    let rgb565: Vec<u8> = graphics::convert_vec_rgb888_to_rgb565(&raw);

    let out: ImageRaw<Rgb565> = ImageRaw::new(&rgb565, resized_image.width());
    out.draw(&mut display)?;

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    Window::new("Hello world", &output_settings).show_static(&display);

    Ok(())
}
