use core::{fmt::Debug, marker::PhantomData};

use embedded_hal::{
    delay::DelayNs,
    digital::OutputPin,
    spi::{self, ErrorType, SpiBus, SpiDevice},
};

pub struct SingleDevice<Spi, Word, Delay, Cs> {
    pub(super) _phantom: PhantomData<Word>,
    pub(super) bus: Spi,
    pub(super) delay: Delay,
    pub(super) cs: Cs,
}

#[derive(Debug)]
pub enum SingleDeviceError<Spi, Output> {
    Spi(Spi),
    Output(Output),
}

impl<Spi, Output> spi::Error for SingleDeviceError<Spi, Output>
where
    Spi: spi::Error,
    Output: Debug,
{
    fn kind(&self) -> spi::ErrorKind {
        match self {
            Self::Spi(spi) => spi.kind(),
            Self::Output(_) => spi::ErrorKind::ChipSelectFault,
        }
    }
}

impl<Spi, Output> From<Spi> for SingleDeviceError<Spi, Output> {
    fn from(value: Spi) -> Self {
        Self::Spi(value)
    }
}

impl<Spi, Word, Delay, Cs, SpiError, OutputError> ErrorType for SingleDevice<Spi, Word, Delay, Cs>
where
    Spi: SpiBus<Word, Error = SpiError>,
    Word: Copy + 'static,
    Cs: OutputPin<Error = OutputError>,
    SpiError: spi::Error,
    OutputError: Debug,
{
    type Error = SingleDeviceError<SpiError, OutputError>;
}

impl<Spi, Word, Delay, Cs, SpiError> SpiDevice<Word> for SingleDevice<Spi, Word, Delay, Cs>
where
    Spi: SpiBus<Word, Error = SpiError>,
    Word: Copy + 'static,
    Delay: DelayNs,
    Cs: OutputPin,
    SpiError: spi::Error,
{
    fn transaction(
        &mut self,
        operations: &mut [embedded_hal::spi::Operation<'_, Word>],
    ) -> Result<(), Self::Error> {
        do_output(self.cs.set_low())?;

        for operation in operations {
            match operation {
                spi::Operation::Read(buf) => {
                    self.bus.read(buf)?;
                }
                spi::Operation::Write(buf) => {
                    self.bus.write(buf)?;
                }
                spi::Operation::Transfer(dst, src) => {
                    self.bus.transfer(dst, src)?;
                }
                spi::Operation::TransferInPlace(buf) => {
                    self.bus.transfer_in_place(buf)?;
                }
                spi::Operation::DelayNs(ns) => {
                    self.delay.delay_ns(*ns);
                }
            }
        }

        self.bus.flush()?;

        do_output(self.cs.set_high())?;

        Ok(())
    }
}

fn do_output<T, SpiError, OutputError>(
    r: Result<T, OutputError>,
) -> Result<T, SingleDeviceError<SpiError, OutputError>> {
    match r {
        Ok(t) => Ok(t),
        Err(e) => Err(SingleDeviceError::Output(e)),
    }
}
