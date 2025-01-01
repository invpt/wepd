# wepd

The Watchy e-paper driver used in [`wable`](https://github.com/invpt/wable).

## Usage example

With `embedded-hal-bus` to provide the `ExclusiveDevice` implementation of `SpiDevice`:
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

    // Watchy V2 display pins
    let mosi = io.pins.gpio23; // 23: Master-Out/Slave-In
    let cs   = io.pins.gpio5;  //  5: Chip Select
    let rst  = io.pins.gpio9;  //  9: Reset
    let busy = io.pins.gpio19; // 19: Busy
    let sclk = io.pins.gpio18; // 18: Serial Clock
    let dc   = io.pins.gpio10; // 10: Direct Current

    // Watchy V3 display pins
    /*
    let mosi = io.pins.gpio48; // 48: Master-Out/Slave-In
    let cs   = io.pins.gpio33; // 33: Chip Select
    let rst  = io.pins.gpio35; // 35: Reset
    let busy = io.pins.gpio36; // 36: Busy
    let sclk = io.pins.gpio47; // 47: Serial Clock
    let dc   = io.pins.gpio34; // 34: Direct Current
    */

    let bus = Spi::new(
        peripherals.SPI2,
        HertzU32::Hz(20000000),
        SpiMode::Mode0,
        &clocks,
    )
    .with_mosi(mosi)
    .with_sck(sclk);

    let mut display = Display::new(DisplayConfiguration {
        spi: ExclusiveDevice::new(bus, Output::new(cs, Level::High), delay).unwrap(),
        dc: Output::new(dc, Level::High),
        rst: Output::new(rst, Level::High),
        busy: Input::new(busy, Pull::None),
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
