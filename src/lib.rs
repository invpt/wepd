#![no_std]
#![feature(generic_const_exprs)]

use core::fmt::Debug;

use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::{self, SpiDevice},
};

#[cfg(feature = "embedded-graphics")]
mod embedded_graphics;
mod geometry;
mod private {
    pub trait Internal {}
}

use geometry::*;
use private::*;
#[cfg(feature = "embedded-graphics")]
pub use embedded_graphics::*;

const WIDTH: usize = 200;
const HEIGHT: usize = 200;
const SCREEN_RECT: Rect = Rect {
    x: Span {
        lo: 0,
        hi: WIDTH as i16,
    },
    y: Span {
        lo: 0,
        hi: HEIGHT as i16,
    },
};

#[derive(Debug)]
pub enum DisplayError<Spi, Input, Output> {
    BusyTimeout,
    Spi(Spi),
    Input(Input),
    Output(Output),
}

impl<Spi, Input, Output> From<Spi> for DisplayError<Spi, Input, Output> {
    fn from(value: Spi) -> Self {
        Self::Spi(value)
    }
}

type Error<C> = DisplayError<
    <C as IsDisplayConfiguration>::SpiError,
    <C as IsDisplayConfiguration>::InputError,
    <C as IsDisplayConfiguration>::OutputError,
>;

/// A helper trait to avoid repeating type constraints. See [DisplayConfiguration].
pub trait IsDisplayConfiguration: Internal {
    type Spi: SpiDevice<Error = Self::SpiError>;
    type Dc: OutputPin<Error = Self::OutputError>;
    type Rst: OutputPin<Error = Self::OutputError>;
    type Busy: InputPin<Error = Self::InputError>;
    type Delay: DelayNs + Clone;
    type Wait: BusyWait;

    type SpiError: spi::Error;
    type OutputError: Debug;
    type InputError: Debug;

    fn get(
        self,
    ) -> DisplayConfiguration<
        Self::Spi,
        Self::Dc,
        Self::Rst,
        Self::Busy,
        Self::Delay,
        Self::Wait,
    >;
}

pub struct DisplayConfiguration<Spi, Dc, Rst, Busy, Delay, Wait> {
    pub spi: Spi,
    pub dc: Dc,
    pub rst: Rst,
    pub busy: Busy,
    pub delay: Delay,
    pub busy_wait: Wait,
}

impl<Spi, Dc, Rst, Busy, Delay, BusyCallback> Internal
    for DisplayConfiguration<Spi, Dc, Rst, Busy, Delay, BusyCallback>
{
}

impl<Spi, Dc, Rst, Busy, Delay, Wait, SpiError, OutputError, InputError>
    IsDisplayConfiguration for DisplayConfiguration<Spi, Dc, Rst, Busy, Delay, Wait>
where
    Spi: SpiDevice<Error = SpiError>,
    Dc: OutputPin<Error = OutputError>,
    Rst: OutputPin<Error = OutputError>,
    Busy: InputPin<Error = InputError>,
    Delay: DelayNs + Clone,
    Wait: BusyWait,
    SpiError: spi::Error,
    OutputError: Debug,
    InputError: Debug,
{
    type Spi = Spi;
    type Dc = Dc;
    type Rst = Rst;
    type Busy = Busy;
    type Delay = Delay;
    type Wait = Wait;
    type SpiError = SpiError;
    type OutputError = OutputError;
    type InputError = InputError;

    fn get(
        self,
    ) -> DisplayConfiguration<
        Self::Spi,
        Self::Dc,
        Self::Rst,
        Self::Busy,
        Self::Delay,
        Self::Wait,
    > {
        self
    }
}

#[derive(Debug)]
pub struct BusyTimeout;

pub trait BusyWait {
    fn poll_wait(&mut self) -> Result<(), BusyTimeout>;
}

pub struct DelayWaiter<Delay> {
    delay: Delay,
    delay_ms: u32,
    timeout_ms: u32,
}

impl<Delay> DelayWaiter<Delay>
where Delay: DelayNs {
    pub fn new(delay: Delay) -> Self {
        Self {
            delay,
            delay_ms: 1,
            timeout_ms: 100_000,
        }
    }

    pub fn with_delay_ms(self, ms: u32) -> Self {
        Self {
            delay_ms: ms,
            ..self
        }
    }

    pub fn with_timeout_ms(self, ms: u32) -> Self {
        Self {
            timeout_ms: ms,
            ..self
        }
    }
}

impl<Delay> BusyWait for DelayWaiter<Delay>
where Delay: DelayNs {
    fn poll_wait(&mut self) -> Result<(), BusyTimeout> {
        self.delay.delay_ms(self.delay_ms);
        
        if self.timeout_ms != 0 {
            match self.timeout_ms.checked_sub(self.delay_ms) {
                Some(new_timeout) => {
                    self.timeout_ms = new_timeout;
                    Ok(())
                }
                None => {
                    Err(BusyTimeout)
                }
            }
        } else {
            Ok(())
        }
    }
}

pub struct Display<C: IsDisplayConfiguration> {
    power_is_on: bool,
    initialized: bool,
    initial_refresh: bool,
    initial_write: bool,
    config:
        DisplayConfiguration<C::Spi, C::Dc, C::Rst, C::Busy, C::Delay, C::Wait>,
}

impl<C: IsDisplayConfiguration> Display<C> {
    pub fn new(config: C) -> Result<Self, Error<C>> {
        let mut config = config.get();

        do_output(config.dc.set_high())?;
        do_output(config.rst.set_high())?;

        Ok(Self {
            initialized: false,
            power_is_on: false,
            initial_refresh: true,
            initial_write: true,
            config,
        })
    }

    pub fn reset(&mut self) -> Result<(), Error<C>> {
        do_output(self.config.rst.set_low())?;
        self.config.delay.delay_ms(10);
        do_output(self.config.rst.set_high())?;
        self.config.delay.delay_ms(10);

        Ok(())
    }

    pub fn clear_screen(&mut self, value: u8) -> Result<(), Error<C>> {
        self.write_screen_buffer(value)?;
        self.refresh_all(true)?;
        self.write_screen_buffer_again(value)?;

        Ok(())
    }

    pub fn draw_image(
        &mut self,
        bitmap: &[u8],
        x_lo: i16,
        y_lo: i16,
        x_hi: i16,
        y_hi: i16,
    ) -> Result<(), Error<C>> {
        let rect = Rect {
            x: Span { lo: x_lo, hi: x_hi },
            y: Span { lo: y_lo, hi: y_hi },
        };
        self.write_image(bitmap, x_lo, y_lo, x_hi, y_hi)?;
        self.refresh(rect)?;
        self.write_image_again(bitmap, x_lo, y_lo, x_hi, y_hi)?;

        Ok(())
    }

    pub fn write_image(
        &mut self,
        bitmap: &[u8],
        x_lo: i16,
        y_lo: i16,
        x_hi: i16,
        y_hi: i16,
    ) -> Result<(), Error<C>> {
        let rect = Rect {
            x: Span { lo: x_lo, hi: x_hi },
            y: Span { lo: y_lo, hi: y_hi },
        };
        self.write_image_inner(0x24, bitmap, rect)?;
        Ok(())
    }

    fn write_image_again(
        &mut self,
        bitmap: &[u8],
        x_lo: i16,
        y_lo: i16,
        x_hi: i16,
        y_hi: i16,
    ) -> Result<(), Error<C>> {
        let rect = Rect {
            x: Span { lo: x_lo, hi: x_hi },
            y: Span { lo: y_lo, hi: y_hi },
        };
        self.write_image_inner(0x24, bitmap, rect)?;
        Ok(())
    }

    fn write_image_inner(
        &mut self,
        command: u8,
        bitmap: &[u8],
        rect: Rect,
    ) -> Result<(), Error<C>> {
        if self.initial_write {
            self.write_screen_buffer(0xFF)?;
        }

        let Some(screen_rect) = rect.intersection(SCREEN_RECT) else {
            return Ok(());
        };

        let aligned_rect = Rect {
            x: Span {
                lo: floor_multiple(screen_rect.x.lo, 8),
                hi: ceil_multiple(screen_rect.x.lo + screen_rect.x.size(), 8),
            },
            ..screen_rect
        };

        self.set_partial_ram_area(aligned_rect)?;

        self.transfer_command(command)?;
        self.config.spi.write(bitmap)?;

        Ok(())
    }

    fn init(&mut self) -> Result<(), Error<C>> {
        self.init_display()?;
        self.power_on()?;
        self.initialized = true;
        Ok(())
    }

    fn init_display(&mut self) -> Result<(), Error<C>> {
        // TODO:   if (_hibernating) _reset();

        self.transfer_command(0x01)?;
        self.config.spi.write(&[0xC7, 0x00, 0x00])?;

        // TODO: if(reduceBoosterTime) {...}

        self.transfer_command(0x18)?;
        self.config.spi.write(&[0x80])?;

        self.set_dark_border(false)?;

        self.set_partial_ram_area(SCREEN_RECT)?;

        Ok(())
    }

    fn power_on(&mut self) -> Result<(), Error<C>> {
        //TODO: if(waitingPowerOn)
        if self.power_is_on {
            return Ok(());
        }

        self.transfer_command(0x22)?;
        self.config.spi.write(&[0xf8])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;
        self.power_is_on = true;

        Ok(())
    }

    fn set_dark_border(&mut self, dark_border: bool) -> Result<(), Error<C>> {
        //TODO: if(_hibernating)return;
        self.transfer_command(0x3C)?;
        self.config
            .spi
            .write(&[if dark_border { 0x02 } else { 0x05 }])?;

        Ok(())
    }

    pub fn power_off(&mut self) -> Result<(), Error<C>> {
        if !self.power_is_on {
            return Ok(());
        }

        self.transfer_command(0x22)?;
        self.config.spi.write(&[0x83])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;
        self.power_is_on = false;
        self.initialized = false;

        Ok(())
    }

    fn refresh_all(&mut self, partial_update_mode: bool) -> Result<(), Error<C>> {
        if partial_update_mode {
            self.refresh(SCREEN_RECT)?;
        } else {
            self.update_full()?;
        }

        Ok(())
    }

    fn refresh(&mut self, rect: Rect) -> Result<(), Error<C>> {
        if self.initial_refresh {
            return self.refresh_all(false);
        }
        let rect = rect.intersection(SCREEN_RECT);
        let Some(rect) = rect else {
            return Ok(());
        };
        let rect = Rect {
            x: Span {
                lo: floor_multiple(rect.x.lo, 8),
                hi: ceil_multiple(rect.x.hi, 8),
            },
            y: rect.y,
        };
        if !self.initialized {
            self.init()?;
        }
        self.set_partial_ram_area(rect)?;
        self.update_part()?;

        Ok(())
    }

    fn update_full(&mut self) -> Result<(), Error<C>> {
        self.initial_refresh = false;

        self.transfer_command(0x22)?;
        self.config.spi.write(&[0xf4])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;

        Ok(())
    }

    fn update_part(&mut self) -> Result<(), Error<C>> {
        self.transfer_command(0x22)?;
        self.config.spi.write(&[0xfc])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;

        Ok(())
    }

    fn set_partial_ram_area(&mut self, rect: Rect) -> Result<(), Error<C>> {
        self.transfer_command(0x11)?;
        self.config.spi.write(&[0x03])?;
        self.transfer_command(0x44)?;
        self.config
            .spi
            .write(&[(rect.x.lo / 8) as u8, ((rect.x.hi - 1) / 8) as u8])?;
        self.transfer_command(0x45)?;
        self.config.spi.write(&[
            (rect.y.lo % 256) as u8,
            (rect.y.lo / 256) as u8,
            ((rect.y.hi - 1) % 256) as u8,
            ((rect.y.hi - 1) % 256) as u8,
        ])?;
        self.transfer_command(0x4e)?;
        self.config.spi.write(&[(rect.x.lo / 8) as u8])?;
        self.transfer_command(0x4f)?;
        self.config
            .spi
            .write(&[(rect.y.lo % 256) as u8, (rect.y.lo / 256) as u8])?;

        Ok(())
    }

    fn write_screen_buffer(&mut self, value: u8) -> Result<(), Error<C>> {
        if !self.initialized {
            self.init()?;
        }
        if self.initial_write {
            self.write_screen_buffer_inner(0x26, value)?;
        }
        self.write_screen_buffer_inner(0x24, value)?;
        self.initial_write = false;

        Ok(())
    }

    fn write_screen_buffer_again(&mut self, value: u8) -> Result<(), Error<C>> {
        if !self.initialized {
            self.init()?;
        }
        self.write_screen_buffer_inner(0x24, value)?;

        Ok(())
    }

    fn write_screen_buffer_inner(&mut self, command: u8, value: u8) -> Result<(), Error<C>> {
        self.transfer_command(command)?;
        for _ in 0..WIDTH * HEIGHT / 8 {
            self.config.spi.write(&[value])?;
        }

        Ok(())
    }

    fn wait_while_busy(&mut self) -> Result<(), Error<C>> {
        // Give some time for `busy` to be asserted by the display
        self.config.delay.delay_ms(1);

        while do_input(self.config.busy.is_high())? {
            match self.config.busy_wait.poll_wait() {
                Ok(()) => (),
                Err(BusyTimeout) => match do_input(self.config.busy.is_high())? {
                    true => return Err(DisplayError::BusyTimeout),
                    false => return Ok(()),
                },
            }
        }

        Ok(())
    }

    fn transfer_command(&mut self, value: u8) -> Result<(), Error<C>> {
        do_output(self.config.dc.set_low())?;
        self.config.spi.write(&[value])?;
        do_output(self.config.dc.set_high())?;
        Ok(())
    }
}

fn do_input<T, Spi, Input, Output>(
    r: Result<T, Input>,
) -> Result<T, DisplayError<Spi, Input, Output>> {
    match r {
        Ok(t) => Ok(t),
        Err(e) => Err(DisplayError::Input(e)),
    }
}

fn do_output<T, Spi, Input, Output>(
    r: Result<T, Output>,
) -> Result<T, DisplayError<Spi, Input, Output>> {
    match r {
        Ok(t) => Ok(t),
        Err(e) => Err(DisplayError::Output(e)),
    }
}

fn floor_multiple(n: i16, m: i16) -> i16 {
    n - n % m
}

fn ceil_multiple(n: i16, m: i16) -> i16 {
    n + if n % m > 0 { m - n % m } else { 0 }
}
