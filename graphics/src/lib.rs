use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, Point},
    pixelcolor::{PixelColor, Rgb565},
    primitives::{Primitive, PrimitiveStyle, Rectangle, StyledDrawable},
};
use esp_idf_hal::gpio::{Gpio16, Output, OutputPin, PinDriver};
use ili9341::Ili9341;

fn rgb888_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
    let red = (r >> 3) as u16;
    let green = (g >> 2) as u16;
    let blue = (b >> 3) as u16;

    (red << 11) | (green << 5) | blue
}

pub fn convert_vec_rgb888_to_rgb565(rgb888_vec: &Vec<u8>) -> Vec<u8> {
    let mut rgb565_vec = Vec::with_capacity((rgb888_vec.len() / 3) * 2);

    for chunk in rgb888_vec.chunks_exact(3) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let rgb565 = rgb888_to_rgb565(r, g, b);
        rgb565_vec.push((rgb565 >> 8) as u8); // High byte
        rgb565_vec.push(rgb565 as u8); // Low byte
    }

    rgb565_vec
}

pub fn fill_display<T>(display: &mut T, color: Rgb565)
where
    T: DrawTarget<Color = Rgb565>,
{
    let display_area = Rectangle::new(Point::new(0, 0), display.bounding_box().size);

    let fill_style = PrimitiveStyle::with_fill(color);

    display_area.draw_styled(&fill_style, display);
}