//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.
//!
//! This is currently optimised for readability and debugability, not
//! performance.

use super::sdmmc_proto::*;
use super::{Block, BlockCount, BlockDevice, BlockIdx};
use core::cell::RefCell;

const DEFAULT_DELAY_COUNT: u32 = 32_000;

/// Represents an SD Card interface built from an SPI peripheral and a Chip
/// Select pin. We need Chip Select to be separate so we can clock out some
/// bytes without Chip Select asserted (which puts the card into SPI mode).
pub struct SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
{
    spi: RefCell<SPI>,
    cs: RefCell<CS>,
    card_type: CardType,
    state: State,
}

/// The possible errors `SdMmcSpi` can generate.
#[derive(Debug, Copy, Clone)]
pub enum Error {
    /// We got an error from the SPI peripheral
    Transport,
    /// We failed to enable CRC checking on the SD card
    CantEnableCRC,
    /// We didn't get a response when reading data from the card
    TimeoutReadBuffer,
    /// We didn't get a response when waiting for the card to not be busy
    TimeoutWaitNotBusy,
    /// We didn't get a response when executing this command
    TimeoutCommand(u8),
    /// We didn't get a response when executing this application-specific command
    TimeoutACommand(u8),
    /// We got a bad response from Command 58
    Cmd58Error,
    /// We failed to read the Card Specific Data register
    RegisterReadError,
    /// We got a CRC mismatch (card gave us, we calculated)
    CrcError(u16, u16),
    /// Error reading from the card
    ReadError,
    /// Error writing to the card
    WriteError,
    /// Can't perform this operation with the card in this state
    BadState,
    /// Couldn't find the card
    CardNotFound,
    /// Couldn't set a GPIO pin
    GpioError,
}

/// The possible states `SdMmcSpi` can be in.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    /// Card is not initialised
    NoInit,
    /// Card is in an error state
    Error,
    /// Card is initialised and idle
    Idle,
}

/// The different types of card we support.
#[derive(Debug, Copy, Clone, PartialEq)]
enum CardType {
    SD1,
    SD2,
    SDHC,
}

/// A terrible hack for busy-waiting the CPU while we wait for the card to
/// sort itself out.
///
/// @TODO replace this!
struct Delay(u32);

impl Delay {
    fn new() -> Delay {
        Delay(DEFAULT_DELAY_COUNT)
    }

    fn delay(&mut self, err: Error) -> Result<(), Error> {
        if self.0 == 0 {
            Err(err)
        } else {
            let dummy_var: u32 = 0;
            for _ in 0..100 {
                unsafe { core::ptr::read_volatile(&dummy_var) };
            }
            self.0 -= 1;
            Ok(())
        }
    }
}

impl<SPI, CS> SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
{
    /// Create a new SD/MMC controller using a raw SPI interface.
    pub fn new(spi: SPI, cs: CS) -> SdMmcSpi<SPI, CS> {
        SdMmcSpi {
            spi: RefCell::new(spi),
            cs: RefCell::new(cs),
            card_type: CardType::SD1,
            state: State::NoInit,
        }
    }

    /// Get a temporary borrow on the underlying SPI device. Useful if you
    /// need to re-clock the SPI after performing `init()`.
    pub fn spi(&mut self) -> core::cell::RefMut<SPI> {
        self.spi.borrow_mut()
    }

    fn cs_high(&self) -> Result<(), Error> {
        self.cs
            .borrow_mut()
            .try_set_high()
            .map_err(|_| Error::GpioError)
    }

    fn cs_low(&self) -> Result<(), Error> {
        self.cs
            .borrow_mut()
            .try_set_low()
            .map_err(|_| Error::GpioError)
    }

    /// This routine must be performed with an SPI clock speed of around 100 - 400 kHz.
    /// Afterwards you may increase the SPI clock speed.
    pub fn init(&mut self) -> Result<(), Error> {
        let f = |s: &mut Self| {
            // Assume it hasn't worked
            s.state = State::Error;
            // Supply minimum of 74 clock cycles without CS asserted.
            s.cs_high()?;
            for _ in 0..10 {
                s.send(0xFF)?;
            }
            // Assert CS
            s.cs_low()?;
            // Enter SPI mode
            let mut attempts = 32;
            while attempts > 0 {
                match s.card_command(CMD0, 0) {
                    Err(Error::TimeoutCommand(0)) => {
                        // Try again?
                        attempts -= 1;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(R1_IDLE_STATE) => {
                        break;
                    }
                    Ok(_) => {
                        // Try again
                    }
                }
            }
            if attempts == 0 {
                return Err(Error::CardNotFound);
            }
            // Enable CRC
            if s.card_command(CMD59, 1)? != R1_IDLE_STATE {
                return Err(Error::CantEnableCRC);
            }
            // Check card version
            let mut delay = Delay::new();
            loop {
                if s.card_command(CMD8, 0x1AA)? == (R1_ILLEGAL_COMMAND | R1_IDLE_STATE) {
                    s.card_type = CardType::SD1;
                    break;
                }
                s.receive()?;
                s.receive()?;
                s.receive()?;
                let status = s.receive()?;
                if status == 0xAA {
                    s.card_type = CardType::SD2;
                    break;
                }
                delay.delay(Error::TimeoutCommand(CMD8))?;
            }

            let arg = match s.card_type {
                CardType::SD1 => 0,
                CardType::SD2 | CardType::SDHC => 0x4000_0000,
            };

            let mut delay = Delay::new();
            while s.card_acmd(ACMD41, arg)? != R1_READY_STATE {
                delay.delay(Error::TimeoutACommand(ACMD41))?;
            }

            if s.card_type == CardType::SD2 {
                if s.card_command(CMD58, 0)? != 0 {
                    return Err(Error::Cmd58Error);
                }
                if (s.receive()? & 0xC0) == 0xC0 {
                    s.card_type = CardType::SDHC;
                }
                // Discard other three bytes
                s.receive()?;
                s.receive()?;
                s.receive()?;
            }
            s.state = State::Idle;
            Ok(())
        };
        let result = f(self);
        self.cs_high()?;
        let _ = self.receive();
        result
    }

    /// De-init the card so it can't be used
    pub fn deinit(&mut self) {
        self.state = State::NoInit;
    }

    /// Return the usable size of this SD card in bytes.
    pub fn card_size_bytes(&self) -> Result<u64, Error> {
        self.check_state()?;
        self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            match csd {
                Csd::V1(ref contents) => Ok(contents.card_capacity_bytes()),
                Csd::V2(ref contents) => Ok(contents.card_capacity_bytes()),
            }
        })
    }

    /// Erase some blocks on the card.
    pub fn erase(&mut self, _first_block: BlockIdx, _last_block: BlockIdx) -> Result<(), Error> {
        self.check_state()?;
        unimplemented!();
    }

    /// Can this card erase single blocks?
    pub fn erase_single_block_enabled(&self) -> Result<bool, Error> {
        self.check_state()?;
        self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            match csd {
                Csd::V1(ref contents) => Ok(contents.erase_single_block_enabled()),
                Csd::V2(ref contents) => Ok(contents.erase_single_block_enabled()),
            }
        })
    }

    /// Return an error if we're not in  `State::Idle`. It probably means
    /// they haven't called `begin()`.
    fn check_state(&self) -> Result<(), Error> {
        if self.state != State::Idle {
            Err(Error::BadState)
        } else {
            Ok(())
        }
    }

    /// Perform a function that might error with the chipselect low.
    /// Always releases the chipselect, even if the function errors.
    fn with_chip_select_mut<F, T>(&self, func: F) -> Result<T, Error>
    where
        F: FnOnce(&Self) -> Result<T, Error>,
    {
        self.cs_low()?;
        let result = func(self);
        self.cs_high()?;
        result
    }

    /// Perform a function that might error with the chipselect low.
    /// Always releases the chipselect, even if the function errors.
    fn with_chip_select<F, T>(&self, func: F) -> Result<T, Error>
    where
        F: FnOnce(&Self) -> Result<T, Error>,
    {
        self.cs_low()?;
        let result = func(self);
        self.cs_high()?;
        result
    }

    /// Read the 'card specific data' block.
    fn read_csd(&self) -> Result<Csd, Error> {
        match self.card_type {
            CardType::SD1 => {
                let mut csd = CsdV1::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::RegisterReadError);
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V1(csd))
            }
            CardType::SD2 | CardType::SDHC => {
                let mut csd = CsdV2::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::RegisterReadError);
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V2(csd))
            }
        }
    }

    /// Read an arbitrary number of bytes from the card. Always fills the
    /// given buffer, so make sure it's the right size.
    fn read_data(&self, buffer: &mut [u8]) -> Result<(), Error> {
        // Get first non-FF byte.
        let mut delay = Delay::new();
        let status = loop {
            let s = self.receive()?;
            if s != 0xFF {
                break s;
            }
            delay.delay(Error::TimeoutReadBuffer)?;
        };
        if status != DATA_START_BLOCK {
            return Err(Error::ReadError);
        }

        for b in buffer.iter_mut() {
            *b = self.receive()?;
        }

        let mut crc = u16::from(self.receive()?);
        crc <<= 8;
        crc |= u16::from(self.receive()?);

        let calc_crc = crc16(buffer);
        if crc != calc_crc {
            return Err(Error::CrcError(crc, calc_crc));
        }

        Ok(())
    }

    /// Write an arbitrary number of bytes to the card.
    fn write_data(&self, token: u8, buffer: &[u8]) -> Result<(), Error> {
        let calc_crc = crc16(buffer);
        self.send(token)?;
        for &b in buffer.iter() {
            self.send(b)?;
        }
        self.send((calc_crc >> 8) as u8)?;
        self.send(calc_crc as u8)?;
        let status = self.receive()?;
        if (status & DATA_RES_MASK) != DATA_RES_ACCEPTED {
            Err(Error::WriteError)
        } else {
            Ok(())
        }
    }

    /// Perform an application-specific command.
    fn card_acmd(&self, command: u8, arg: u32) -> Result<u8, Error> {
        self.card_command(CMD55, 0)?;
        self.card_command(command, arg)
    }

    /// Perform a command.
    fn card_command(&self, command: u8, arg: u32) -> Result<u8, Error> {
        self.wait_not_busy()?;
        let mut buf = [
            0x40 | command,
            (arg >> 24) as u8,
            (arg >> 16) as u8,
            (arg >> 8) as u8,
            arg as u8,
            0,
        ];
        buf[5] = crc7(&buf[0..5]);

        for b in buf.iter() {
            self.send(*b)?;
        }

        // skip stuff byte for stop read
        if command == CMD12 {
            let _result = self.receive()?;
        }

        for _ in 0..512 {
            let result = self.receive()?;
            if (result & 0x80) == ERROR_OK {
                return Ok(result);
            }
        }

        Err(Error::TimeoutCommand(command))
    }

    /// Receive a byte from the SD card by clocking in an 0xFF byte.
    fn receive(&self) -> Result<u8, Error> {
        self.transfer(0xFF)
    }

    /// Send a byte from the SD card.
    fn send(&self, out: u8) -> Result<(), Error> {
        let _ = self.transfer(out)?;
        Ok(())
    }

    /// Send one byte and receive one byte.
    fn transfer(&self, out: u8) -> Result<u8, Error> {
        let mut spi = self.spi.borrow_mut();
        spi.try_transfer(&mut [out])
            .map(|b| b[0])
            .map_err(|_e| Error::Transport)
    }

    /// Spin until the card returns 0xFF, or we spin too many times and
    /// timeout.
    fn wait_not_busy(&self) -> Result<(), Error> {
        let mut delay = Delay::new();
        loop {
            let s = self.receive()?;
            if s == 0xFF {
                break;
            }
            delay.delay(Error::TimeoutWaitNotBusy)?;
        }
        Ok(())
    }
}

impl<SPI, CS> BlockDevice for SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
    CS: embedded_hal::digital::OutputPin,
{
    type Error = Error;

    /// Read one or more blocks, starting at the given block index.
    fn read(
        &self,
        blocks: &mut [Block],
        start_block_idx: BlockIdx,
        _reason: &str,
    ) -> Result<(), Self::Error> {
        self.check_state()?;
        let start_idx = match self.card_type {
            CardType::SD1 | CardType::SD2 => start_block_idx.0 * 512,
            CardType::SDHC => start_block_idx.0,
        };
        self.with_chip_select(|s| {
            if blocks.len() == 1 {
                // Start a single-block read
                s.card_command(CMD17, start_idx)?;
                s.read_data(&mut blocks[0].contents)?;
            } else {
                // Start a multi-block read
                s.card_command(CMD18, start_idx)?;
                for block in blocks.iter_mut() {
                    s.read_data(&mut block.contents)?;
                }
                // Stop the read
                s.card_command(CMD12, 0)?;
            }
            Ok(())
        })
    }

    /// Write one or more blocks, starting at the given block index.
    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        self.check_state()?;
        let start_idx = match self.card_type {
            CardType::SD1 | CardType::SD2 => start_block_idx.0 * 512,
            CardType::SDHC => start_block_idx.0,
        };
        self.with_chip_select_mut(|s| {
            if blocks.len() == 1 {
                // Start a single-block write
                s.card_command(CMD24, start_idx)?;
                s.write_data(DATA_START_BLOCK, &blocks[0].contents)?;
                s.wait_not_busy()?;
                if s.card_command(CMD13, 0)? != 0x00 {
                    return Err(Error::WriteError);
                }
                if s.receive()? != 0x00 {
                    return Err(Error::WriteError);
                }
            } else {
                // Start a multi-block write
                s.card_command(CMD25, start_idx)?;
                for block in blocks.iter() {
                    s.wait_not_busy()?;
                    s.write_data(WRITE_MULTIPLE_TOKEN, &block.contents)?;
                }
                // Stop the write
                s.wait_not_busy()?;
                s.send(STOP_TRAN_TOKEN)?;
            }
            Ok(())
        })
    }

    /// Determine how many blocks this device can hold.
    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        let num_bytes = self.card_size_bytes()?;
        let num_blocks = (num_bytes / 512) as u32;
        Ok(BlockCount(num_blocks))
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
