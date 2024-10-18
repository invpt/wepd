# wepd

The Watchy e-paper driver used in [`wable`](https://github.com/invpt/wable).

## Usage example

With `embedded-hal-bus` to provide the `ExclusiveDevice` implementation of `SpiDevice` (note - values are for Watchy V2):
```rust
#![no_std]
#![no_main]

use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    delay::Delay,
    gpio::{Input, Io, Level, Output, Pull},
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
};
use fugit::HertzU32;
use wepd::{DelayWaiter, Display, DisplayConfiguration};

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();
    let delay = Delay::new(&clocks);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let bus = Spi::new(
        peripherals.SPI2,
        HertzU32::Hz(20000000),
        SpiMode::Mode0,
        &clocks,
    )
    .with_mosi(io.pins.gpio23)
    .with_sck(io.pins.gpio18);

    let mut display = Display::new(DisplayConfiguration {
        spi: ExclusiveDevice::new(bus, Output::new(io.pins.gpio5, Level::High), delay).unwrap(),
        dc: Output::new(io.pins.gpio10, Level::High),
        rst: Output::new(io.pins.gpio9, Level::High),
        busy: Input::new(io.pins.gpio19, Pull::None),
        delay,
        busy_wait: DelayWaiter::new(delay)
            .with_timeout_ms(100_000)
            .with_delay_ms(1),
    })
    .unwrap();

    display.reset().unwrap();

    display.clear_screen(0xFF).unwrap();

    display
        .draw_image(include_bytes!("../image.bin"), 0, 0, 200, 200)
        .unwrap();

    display.power_off().unwrap();

    loop {}
}
```

## Embedded Graphics Examples
Make sure to have the `embedded-graphics` feature flag set. For embedded graphics `BinaryColor::Off` is a black pixel and `BinaryColor::On` is a white pixel on the display.

### Text
```rust
    //Creates a frame buffer for embedded graphics
    let mut fb = wepd::Framebuffer::new();
    //Create your embedded text
    let style = MonoTextStyle::new(&ascii::FONT_10X20, BinaryColor::Off);
    Text::new("Hello world", Point { x: 5, y: 15 }, style)
        .draw(&mut fb)
        .unwrap();
    //Write the frame buffer to the display struct made earlier
    fb.flush(&mut display).unwrap();
```

### Images using tinybmp
```rust
    //Creates a frame buffer for embedded graphics
    let mut fb = wepd::Framebuffer::new();
    //Have bmp under 200x200 pixels in your project directory and include it
    let bmp_data = include_bytes!("../ferris.bmp");
    let bmp: Bmp<BinaryColor> = Bmp::from_slice(bmp_data).unwrap();
    Image::new(&bmp, Point::new(50, 50)).draw(&mut fb).unwrap();
    //Write the frame buffer to the display struct made earlier
    fb.flush(&mut display).unwrap();
```

## State

This was a quick port from my original implementation that directly used APIs exposed by `esp-hal`.
