#![no_std]

use core::{fmt::Debug, marker::PhantomData};

use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::{self, SpiBus, SpiDevice},
};

mod single_device;

pub use single_device::*;

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

#[derive(Clone, Copy, Debug)]
struct Span {
    pub lo: i16,
    pub hi: i16,
}

impl Span {
    /// Returns the size of the span, calculated as `hi - lo`.
    pub fn size(self) -> i16 {
        self.hi - self.lo
    }

    /// Computes the intersection of two spans.
    /// Returns `None` if there is no intersection, otherwise returns `Some(Span)`.
    pub fn intersection(self, other: Span) -> Option<Span> {
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);

        if lo <= hi {
            Some(Span { lo, hi })
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: Span,
    y: Span,
}

impl Rect {
    /// Computes the intersection of two rectangles.
    /// Returns `None` if there is no intersection, otherwise returns `Some(Rect)`.
    fn intersection(self, other: Rect) -> Option<Rect> {
        let x = self.x.intersection(other.x)?;
        let y = self.y.intersection(other.y)?;

        Some(Rect { x, y })
    }
}

#[derive(Debug)]
pub enum DisplayError<Spi, Input, Output> {
    Spi(Spi),
    Input(Input),
    Output(Output),
}

impl<Spi, Input, Output> From<Spi> for DisplayError<Spi, Input, Output> {
    fn from(value: Spi) -> Self {
        Self::Spi(value)
    }
}

pub struct Display<Delay, Spi, Dc, Busy, Rst, Wait, CurrentTimeMs> {
    power_is_on: bool,
    initialized: bool,
    initial_refresh: bool,
    initial_write: bool,
    delay: Delay,
    spi: Spi,
    dc: Dc,
    busy: Busy,
    rst: Rst,
    wait: Wait,
    current_time_ms: CurrentTimeMs,
}

impl<Delay, Spi, Dc, Cs, Busy, Rst, Wait, CurrentTimeMs, BusError, InputError, OutputError>
    Display<Delay, SingleDevice<Spi, u8, Delay, Cs>, Dc, Busy, Rst, Wait, CurrentTimeMs>
where
    Delay: DelayNs + Clone,
    Spi: SpiBus<Error = BusError>,
    Dc: OutputPin<Error = OutputError>,
    Cs: OutputPin<Error = OutputError>,
    Busy: InputPin<Error = InputError>,
    Rst: OutputPin<Error = OutputError>,
    Wait: FnMut(),
    CurrentTimeMs: FnMut() -> u64,
    BusError: spi::Error,
    OutputError: Debug,
{
    pub fn new_on_bus(
        delay: Delay,
        spi: Spi,
        dc: Dc,
        mut cs: Cs,
        busy: Busy,
        rst: Rst,
        wait: Wait,
        current_time_ms: CurrentTimeMs,
    ) -> Result<Self, DisplayError<BusError, InputError, OutputError>> {
        do_output(cs.set_high())?;

        let spi = SingleDevice {
            _phantom: PhantomData,
            bus: spi,
            delay: delay.clone(),
            cs,
        };

        match Display::new(delay, spi, dc, busy, rst, wait, current_time_ms) {
            Ok(d) => Ok(d),
            Err(e) => match e {
                DisplayError::Spi(e) => match e {
                    SingleDeviceError::Spi(e) => Err(DisplayError::Spi(e)),
                    SingleDeviceError::Output(e) => Err(DisplayError::Output(e)),
                },
                DisplayError::Input(e) => Err(DisplayError::Input(e)),
                DisplayError::Output(e) => Err(DisplayError::Output(e)),
            },
        }
    }
}

impl<Delay, Spi, Dc, Busy, Rst, Wait, CurrentTimeMs, SpiError, InputError, OutputError>
    Display<Delay, Spi, Dc, Busy, Rst, Wait, CurrentTimeMs>
where
    Delay: DelayNs,
    Spi: SpiDevice<Error = SpiError>,
    Dc: OutputPin<Error = OutputError>,
    Busy: InputPin<Error = InputError>,
    Rst: OutputPin<Error = OutputError>,
    Wait: FnMut(),
    CurrentTimeMs: FnMut() -> u64,
{
    pub fn new(
        delay: Delay,
        spi: Spi,
        mut dc: Dc,
        busy: Busy,
        mut rst: Rst,
        wait: Wait,
        current_time_ms: CurrentTimeMs,
    ) -> Result<Self, DisplayError<SpiError, InputError, OutputError>> {
        do_output(dc.set_high())?;
        do_output(rst.set_high())?;

        Ok(Self {
            initialized: false,
            power_is_on: false,
            initial_refresh: true,
            initial_write: true,
            delay,
            spi,
            dc,
            busy,
            rst,
            wait,
            current_time_ms,
        })
    }

    pub fn reset(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        do_output(self.rst.set_low())?;
        self.delay.delay_ms(10);
        do_output(self.rst.set_high())?;
        self.delay.delay_ms(10);

        Ok(())
    }

    pub fn clear_screen(
        &mut self,
        value: u8,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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
        self.spi.write(bitmap)?;

        Ok(())
    }

    fn init(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        self.init_display()?;
        self.power_on()?;
        self.initialized = true;
        Ok(())
    }

    fn init_display(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        // TODO:   if (_hibernating) _reset();

        self.transfer_command(0x01)?;
        self.spi.write(&[0xC7, 0x00, 0x00])?;

        // TODO: if(reduceBoosterTime) {...}

        self.transfer_command(0x18)?;
        self.spi.write(&[0x80])?;

        self.set_dark_border(false)?;

        self.set_partial_ram_area(SCREEN_RECT)?;

        Ok(())
    }

    fn power_on(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        //TODO: if(waitingPowerOn)
        if self.power_is_on {
            return Ok(());
        }

        self.transfer_command(0x22)?;
        self.spi.write(&[0xf8])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;
        self.power_is_on = true;

        Ok(())
    }

    fn set_dark_border(
        &mut self,
        dark_border: bool,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        //TODO: if(_hibernating)return;
        self.transfer_command(0x3C)?;
        self.spi.write(&[if dark_border { 0x02 } else { 0x05 }])?;

        Ok(())
    }

    pub fn power_off(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        if !self.power_is_on {
            return Ok(());
        }

        self.transfer_command(0x22)?;
        self.spi.write(&[0x83])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;
        self.power_is_on = false;
        self.initialized = false;

        Ok(())
    }

    fn refresh_all(
        &mut self,
        partial_update_mode: bool,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        if partial_update_mode {
            self.refresh(SCREEN_RECT)?;
        } else {
            self.update_full()?;
        }

        Ok(())
    }

    fn refresh(
        &mut self,
        rect: Rect,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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

    fn update_full(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        self.initial_refresh = false;

        self.transfer_command(0x22)?;
        self.spi.write(&[0xf4])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;

        Ok(())
    }

    fn update_part(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        self.transfer_command(0x22)?;
        self.spi.write(&[0xfc])?;
        self.transfer_command(0x20)?;
        self.wait_while_busy()?;

        Ok(())
    }

    fn set_partial_ram_area(
        &mut self,
        rect: Rect,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        self.transfer_command(0x11)?;
        self.spi.write(&[0x03])?;
        self.transfer_command(0x44)?;
        self.spi
            .write(&[(rect.x.lo / 8) as u8, ((rect.x.hi - 1) / 8) as u8])?;
        self.transfer_command(0x45)?;
        self.spi.write(&[
            (rect.y.lo % 256) as u8,
            (rect.y.lo / 256) as u8,
            ((rect.y.hi - 1) % 256) as u8,
            ((rect.y.hi - 1) % 256) as u8,
        ])?;
        self.transfer_command(0x4e)?;
        self.spi.write(&[(rect.x.lo / 8) as u8])?;
        self.transfer_command(0x4f)?;
        self.spi
            .write(&[(rect.y.lo % 256) as u8, (rect.y.lo / 256) as u8])?;

        Ok(())
    }

    fn write_screen_buffer(
        &mut self,
        value: u8,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
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

    fn write_screen_buffer_again(
        &mut self,
        value: u8,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        if !self.initialized {
            self.init()?;
        }
        self.write_screen_buffer_inner(0x24, value)?;

        Ok(())
    }

    fn write_screen_buffer_inner(
        &mut self,
        command: u8,
        value: u8,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        self.transfer_command(command)?;
        for _ in 0..WIDTH * HEIGHT / 8 {
            self.spi.write(&[value])?;
        }

        Ok(())
    }

    fn wait_while_busy(&mut self) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        self.delay.delay_ms(1);
        let start = (self.current_time_ms)();
        loop {
            if do_input(self.busy.is_low())? {
                break;
            }

            (self.wait)();

            if do_input(self.busy.is_low())? {
                break;
            }

            let busy_timeout = 10000;
            if (self.current_time_ms)() - start > busy_timeout {
                break;
            }
        }

        Ok(())
    }

    fn transfer_command(
        &mut self,
        value: u8,
    ) -> Result<(), DisplayError<SpiError, InputError, OutputError>> {
        do_output(self.dc.set_low())?;
        self.spi.write(&[value])?;
        do_output(self.dc.set_high())?;
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
