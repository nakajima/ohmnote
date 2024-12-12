use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::Timer;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::geometry::OriginDimensions;
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::Point;
use embedded_graphics::Drawable;
use esp_hal::{i2c::master::I2c, Async};
use heapless::String;
use ssd1306::mode::DisplayConfig;
use ssd1306::{prelude::DisplayRotation, size::DisplaySize128x64, I2CDisplayInterface, Ssd1306};
use tinybmp::Bmp;
use u8g2_fonts::types::FontColor;
use u8g2_fonts::types::VerticalPosition;
use u8g2_fonts::{fonts, FontRenderer};

pub static READING: Watch<CriticalSectionRawMutex, Option<String<32>>, 1> = Watch::new();

#[task]
pub async fn init_display(i2c: I2c<'static, Async>) {
    log::info!("init_display");

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    while display.init().is_err() {
        log::warn!("init_display failed: {:?}, retrying", display.init().err());
        Timer::after_millis(50).await;
    }

    // Include the BMP file data.
    let bmp_data = include_bytes!("../ohm.bmp");

    // Parse the BMP file.
    let bmp: Bmp<BinaryColor> = Bmp::from_slice(bmp_data).unwrap();
    let bmp_size = bmp.size();

    loop {
        let font = FontRenderer::new::<fonts::u8g2_font_helvB24_te>();
        let reading = READING.receiver().unwrap().changed().await;

        // Clear the display buffer
        display.clear(BinaryColor::Off).unwrap();

        if let Some(reading) = reading {
            // Get the dimensions of the rendered text
            let text_dimensions = font
                .get_rendered_dimensions(
                    reading.as_str(),
                    Point::zero(),
                    VerticalPosition::Baseline,
                )
                .unwrap();

            let text_width = text_dimensions.bounding_box.unwrap().size.width as i32;
            let text_height = text_dimensions.bounding_box.unwrap().size.height as i32;

            // Get the dimensions of the ohm symbol
            let symbol_width = bmp_size.width as i32;
            let symbol_height = bmp_size.height as i32;

            // Define space between text and symbol
            let space_between = 5; // Adjust this value as needed

            // Total width of text, space, and symbol
            let total_width = text_width + space_between + symbol_width;

            // Calculate starting X position to center content
            let display_width = display.bounding_box().size.width as i32;
            let starting_x = (display_width - total_width) / 2;

            // Baseline Y position (adjust as needed)
            let baseline_y = (display.bounding_box().size.height as i32 + text_height) / 2;

            // Render the text
            font.render(
                reading.as_str(),
                Point::new(starting_x, baseline_y),
                VerticalPosition::Baseline,
                FontColor::Transparent(BinaryColor::On),
                &mut display,
            )
            .unwrap();

            // Render the ohm symbol next to the text with spacing
            let symbol_x = starting_x + text_width + space_between;
            let symbol_y = baseline_y - symbol_height;

            Image::new(&bmp, Point::new(symbol_x, symbol_y))
                .draw(&mut display)
                .unwrap();
        } else {
            let font = FontRenderer::new::<fonts::u8g2_font_helvB14_te>();
            font.render(
                "ohmnote",
                Point::new(24, display.bounding_box().size.height as i32 / 2 + 10),
                VerticalPosition::Baseline,
                FontColor::Transparent(BinaryColor::On),
                &mut display,
            )
            .unwrap();
        }

        // Flush the display buffer to the screen
        display.flush().unwrap();

        Timer::after_millis(100).await;
    }
}
