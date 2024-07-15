use embedded_canvas::CanvasAt;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, Point, Size},
    image::{ImageDrawable, ImageRaw},
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::{Rgb565, RgbColor},
    primitives::{PointsIter, PrimitiveStyle, Rectangle, StyledDrawable},
    text::Text,
    Drawable,
};
use embedded_layout::{layout::linear::LinearLayout, prelude::*};
use ili9341::DisplayError;

pub fn rgb888_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
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
    let display_area = Rectangle::new(
        embedded_graphics::geometry::Point::new(0, 0),
        display.bounding_box().size,
    );

    let fill_style = PrimitiveStyle::with_fill(color);

    display_area.draw_styled(&fill_style, display);
}

fn draw_canvas<T>(display: &mut T, canvas: CanvasAt<Rgb565>, background: Rgb565)
where
    T: DrawTarget<Color = Rgb565>,
{
    display.fill_contiguous(
        &canvas.bounding_box(),
        canvas
            .bounding_box()
            .points()
            .map(|point| canvas.get_pixel(point).unwrap_or(background)),
    );
}

pub fn draw_album_cover<T>(display: &mut T, image_bytes: Option<&[u8]>)
where
    T: DrawTarget<Color = Rgb565>,
{
    let mut album_canvas = CanvasAt::new(Point::zero(), Size::new(240, 240));

    let out: ImageRaw<Rgb565> = ImageRaw::new(&image_bytes.unwrap(), 240);
    out.draw(&mut album_canvas).expect("Could not draw image");

    draw_canvas(display, album_canvas, Rgb565::BLACK);
}

pub fn draw_title_and_artist<T>(display: &mut T, title: String, artist: String)
where
    T: DrawTarget<Color = Rgb565, Error = DisplayError>,
{
    let mut displayed_title = title.clone();
    if displayed_title.len() > 23 {
        displayed_title = title[..22].to_string();
        displayed_title.push_str("..")
    }

    let mut displayed_artist = artist.clone();
    if displayed_artist.len() > 23 {
        displayed_artist = artist[..22].to_string();
        displayed_artist.push_str("..");
    }

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    let mut combined_canvas = CanvasAt::<Rgb565>::new(Point::new(0, 240), Size::new(240, 50));
    LinearLayout::vertical(
        embedded_layout::object_chain::Chain::new(Text::new(
            &displayed_title,
            Point::zero(),
            text_style,
        ))
        .append(Text::new(&displayed_artist, Point::zero(), text_style)),
    )
    .with_alignment(horizontal::Left)
    .arrange()
    .align_to(
        &combined_canvas.bounding_box(),
        horizontal::Left,
        vertical::Center,
    )
    .draw(&mut combined_canvas)
    .unwrap();

    draw_canvas(display, combined_canvas, Rgb565::BLACK);
}

pub fn something<T>(display: &mut T)
where
    T: DrawTarget<Color = Rgb565>,
{
    // Create a new character style
    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);

    // Create a text at position (20, 30) and draw it using the previously defined style
    Text::with_alignment(
        "First line\nSecond line",
        Point::new(20, 30),
        style,
        embedded_graphics::text::Alignment::Center,
    )
    .draw(display);
}
