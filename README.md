# wepd

The Watchy e-paper driver used in [`wable`](https://github.com/invpt/wable).

## Usage

With `embedded-hal-bus` to provide the `ExclusiveDevice` implementation of `SpiDevice` (note - values are for Watchy V2):
```rust
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
    current_millis: current_millis,
    wait: || delay.delay(1.millis()),
})
.unwrap();
```

## State

This was a quick port from the original implementation that directly used APIs exposed by `esp-hal`.
