use embedded_graphics_core::prelude::{Dimensions, DrawTarget, PixelColor};

use super::*;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Color {
    Black,
    White,
}

impl PixelColor for Color {
    type Raw = ();
}

pub struct Framebuffer {
    framebuffer: [u8; WIDTH * HEIGHT / 8],
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            framebuffer: [0xFF; WIDTH * HEIGHT / 8],
        }
    }

    pub fn flush<C: IsDisplayConfiguration>(
        &mut self,
        display: &mut Display<C>,
    ) -> Result<(), Error<C>> {
        display.draw_image(&self.framebuffer, 0, 0, 200, 200)
    }
}

impl Dimensions for Framebuffer {
    fn bounding_box(&self) -> embedded_graphics_core::primitives::Rectangle {
        embedded_graphics_core::primitives::Rectangle {
            top_left: embedded_graphics_core::geometry::Point { x: 0, y: 0 },
            size: embedded_graphics_core::geometry::Size {
                width: WIDTH as u32,
                height: HEIGHT as u32,
            },
        }
    }
}

impl DrawTarget for Framebuffer {
    type Color = Color;

    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics_core::Pixel<Self::Color>>,
    {
        for embedded_graphics_core::Pixel(point, color) in pixels {
            if point.x < 0 || point.x >= WIDTH as i32 || point.y < 0 || point.y >= HEIGHT as i32 {
                continue;
            }
            let x = point.x as usize;
            let y = point.y as usize;
            let byte_index = x / 8 + y * WIDTH / 8;
            let byte = &mut self.framebuffer[byte_index];
            let bit_index = 7 - x % 8;

            match color {
                Color::White => {
                    *byte |= 0b1 << bit_index;
                }
                Color::Black => {
                    *byte &= !(0b1 << bit_index);
                }
            }
        }

        Ok(())
    }
}
