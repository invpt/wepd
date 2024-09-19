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

pub struct Framebuffer<const FB_WIDTH: usize, const FB_HEIGHT: usize>
where
    [(); FB_WIDTH * FB_HEIGHT / 8]:,
{
    framebuffer: [u8; FB_WIDTH * FB_HEIGHT / 8],
}

impl<const FB_WIDTH: usize, const FB_HEIGHT: usize> Framebuffer<{ FB_WIDTH }, { FB_HEIGHT }>
where
    [(); FB_WIDTH * FB_HEIGHT / 8]:,
{
    pub fn new() -> Self {
        // https://github.com/nvzqz/static-assertions/issues/40
        // Not yet
        // assert!(FB_WIDTH % 8 == 0);
        // assert!(FB_HEIGHT % 8 == 0);
        // This should be handled in another way
        Self {
            framebuffer: [0xFF; FB_WIDTH * FB_HEIGHT / 8],
        }
    }

    pub fn flush<C: IsDisplayConfiguration>(
        &mut self,
        display: &mut Display<C>,
        x_lo: i16,
        y_lo: i16,
    ) -> Result<(), Error<C>> {
        display.draw_image(
            &self.framebuffer,
            x_lo,
            y_lo,
            x_lo + FB_WIDTH as i16,
            y_lo + FB_HEIGHT as i16,
        )
    }    
}

impl<const FB_WIDTH: usize, const FB_HEIGHT: usize> Dimensions
    for Framebuffer<{ FB_WIDTH }, { FB_HEIGHT }>
where
    [(); FB_WIDTH * FB_HEIGHT / 8]:,
{
    fn bounding_box(&self) -> embedded_graphics_core::primitives::Rectangle {
        embedded_graphics_core::primitives::Rectangle {
            top_left: embedded_graphics_core::geometry::Point { x: 0, y: 0 },
            size: embedded_graphics_core::geometry::Size {
                width: FB_WIDTH as u32,
                height: FB_HEIGHT as u32,
            },
        }
    }
}

impl<const FB_WIDTH: usize, const FB_HEIGHT: usize> DrawTarget
    for Framebuffer<{ FB_WIDTH }, { FB_HEIGHT }>
where
    [(); FB_WIDTH * FB_HEIGHT / 8]:,
{
    type Color = Color;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), ()>
    where
        I: IntoIterator<Item = embedded_graphics_core::Pixel<Color>>,
    {
        for embedded_graphics_core::Pixel(point, color) in pixels {
            // Maybe implement it for the enum?
            let color_value = match color {
                Color::White => 1,
                Color::Black => 0,
            };

            let x = point.x as usize;
            let y = point.y as usize;

            if x >= FB_WIDTH || y >= FB_HEIGHT {
                continue;
            }

            let index = (y * FB_WIDTH + x) / 8;
            let bit = 7 - (x % 8);

            if color_value == 1 {
                // White
                self.framebuffer[index] |= 1 << bit;
            } else {
                // Black
                self.framebuffer[index] &= !(1 << bit);
            }
        }
        Ok(())
    }
}
