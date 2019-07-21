//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.
//!
//! This is currently optimised for readability and debugability, not
//! performance.

use crate::sdmmc_proto::*;
use crate::{Block, BlockCount, BlockDevice, BlockIdx};
use core::cell::UnsafeCell;
use nb::block;
use crate::sdmmc::{Error, State, Delay, Transport};

/// Represents an SD Card interface built from an SPI peripheral and a Chip
/// Select pin. We need Chip Select to be separate so we can clock out some
/// bytes without Chip Select asserted (which puts the card into SPI mode).
pub struct SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
{
    spi: UnsafeCell<SPI>,
    cs: UnsafeCell<CS>,
    card_type: CardType,
    state: State,
}

impl<SPI, CS> SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
{
    /// Create a new SD/MMC controller using a raw SPI interface.
    pub fn new(spi: SPI, cs: CS) -> SdMmcSpi<SPI, CS> {
        SdMmcSpi {
            spi: UnsafeCell::new(spi),
            cs: UnsafeCell::new(cs),
            card_type: CardType::SD1,
            state: State::NoInit,
        }
    }

    /// Get a temporary borrow on the underlying SPI device. Useful if you
    /// need to re-clock the SPI after performing `init()`.
    pub fn spi(&mut self) -> &mut SPI {
        unsafe { &mut *self.spi.get() }
    }

    fn cs_high(&self) {
        let cs = unsafe { &mut *self.cs.get() };
        cs.set_high();
    }

    fn cs_low(&self) {
        let cs = unsafe { &mut *self.cs.get() };
        cs.set_low();
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
    fn with_chip_select_mut<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&Self) -> T,
    {
        self.cs_low();
        let result = func(self);
        self.cs_high();
        result
    }

    /// Perform a function that might error with the chipselect low.
    /// Always releases the chipselect, even if the function errors.
    fn with_chip_select<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&Self) -> T,
    {
        self.cs_low();
        let result = func(self);
        self.cs_high();
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

    /// Send one byte and receive one byte.
    fn transfer(&self, out: u8) -> Result<u8, Error> {
        let spi = unsafe { &mut *self.spi.get() };
        block!(spi.send(out)).map_err(|_e| Error::Transport)?;
        block!(spi.read()).map_err(|_e| Error::Transport)
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

impl<SPI, CS> Transport for SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
    CS: embedded_hal::digital::OutputPin,
{

    /// This routine must be performed with an SPI clock speed of around 100 - 400 kHz.
    /// Afterwards you may increase the SPI clock speed.
    fn init(&mut self) -> Result<(), Error> {
        let f = |s: &mut Self| {
            // Assume it hasn't worked
            s.state = State::Error;
            // Supply minimum of 74 clock cycles without CS asserted.
            s.cs_high();
            for _ in 0..10 {
                s.send(0xFF)?;
            }
            // Assert CS
            s.cs_low();
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
        self.cs_high();
        let _ = self.receive();
        result
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
            if (result & 0x80) == 0 {
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

}

impl<SPI, CS> BlockDevice for SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
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
