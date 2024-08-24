# wepd

The Watchy e-paper driver used in [`wable`](https://github.com/invpt/wable).

## Usage

If you're using `esp-hal`, you'll probably want to use `Display::new_on_bus`. For the Watchy V2, that looks something like this:

```rust
let spi = Spi::new(
    peripherals.SPI2,
    HertzU32::Hz(20000000),
    SpiMode::Mode0,
    &clocks,
)
.with_mosi(io.pins.gpio23)
.with_sck(io.pins.gpio18);

let mut display = Display::new_on_bus(
    delay,
    spi,
    Output::new(io.pins.gpio10, Level::High),
    Output::new(io.pins.gpio5, Level::High),
    Input::new(io.pins.gpio19, Pull::None),
    Output::new(io.pins.gpio9, Level::High),
    || delay.delay(1.millis()),
    current_millis,
)
.unwrap();
```

`esp-idf-hal` users may want to use `Display::new` in conjunction with [`SpiSoftCsDeviceDriver`](https://docs.esp-rs.org/esp-idf-hal/esp_idf_hal/spi/struct.SpiSoftCsDeviceDriver.html), but I cannot give any guidance on this as I have not used `esp-idf-hal`.

## State

This was a quick port from the original implementation that directly used APIs exposed by `esp-hal`. It's not perfect but it works for me; I hope to do some cleanup to make it look nicer later.
