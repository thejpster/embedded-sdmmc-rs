//! embedded-sdmmc-rs - Constants from the SD Specifications
//!
//! Based on SdFat, under the following terms:
//!
//! > Copyright (c) 2011-2018 Bill Greiman
//! > This file is part of the SdFat library for SD memory cards.
//! >
//! > MIT License
//! >
//! > Permission is hereby granted, free of charge, to any person obtaining a
//! > copy of this software and associated documentation files (the "Software"),
//! > to deal in the Software without restriction, including without limitation
//! > the rights to use, copy, modify, merge, publish, distribute, sublicense,
//! > and/or sell copies of the Software, and to permit persons to whom the
//! > Software is furnished to do so, subject to the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be included
//! > in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//! > OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! > FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! > AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! > LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! > FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! > DEALINGS IN THE SOFTWARE.

//==============================================================================

// Possible errors the SD card can return

/// Card indicates last operation was a success
pub const ERROR_OK: u8 = 0x00;

//==============================================================================

// SD Card Commands

/// GO_IDLE_STATE - init card in spi mode if CS low
pub const CMD0_GO_IDLE_STATE: u8 = 0;
/// SEND_IF_COND - verify SD Memory Card interface operating condition.*/
pub const CMD8_SEND_IF_COND: u8 = 8;
/// SEND_CSD - read the Card Specific Data (CSD register)
pub const CMD9_SEND_CSD: u8 = 9;
/// STOP_TRANSMISSION - end multiple block read sequence
pub const CMD12_STOP_TRANSMISSION: u8 = 12;
/// SEND_STATUS - read the card status register
pub const CMD13_SEND_STATUS: u8 = 13;
/// READ_SINGLE_BLOCK - read a single data block from the card
pub const CMD17_READ_SINGLE_BLOCK: u8 = 17;
/// READ_MULTIPLE_BLOCK - read a multiple data blocks from the card
pub const CMD18_READ_MULTIPLE_BLOCK: u8 = 18;
/// WRITE_BLOCK - write a single data block to the card
pub const CMD24_WRITE_BLOCK: u8 = 24;
/// WRITE_MULTIPLE_BLOCK - write blocks of data until a STOP_TRANSMISSION
pub const CMD25_WRITE_MULTIPLE_BLOCK: u8 = 25;
/// APP_CMD - escape for application specific command
pub const CMD55_APP_CMD: u8 = 55;
/// READ_OCR - read the OCR register of a card
pub const CMD58_READ_OCR: u8 = 58;
/// CRC_ON_OFF - enable or disable CRC checking
pub const CMD59_CRC_ON_OFF: u8 = 59;
/// SD_SEND_OP_COND - Sends host capacity support information and activates
/// the card's initialization process
pub const ACMD41_SD_SEND_OP_COND: u8 = 41;

//==============================================================================

/// status for card in the ready state
pub const R1_READY_STATE: u8 = 0x00;

/// status for card in the idle state
pub const R1_IDLE_STATE: u8 = 0x01;

/// status bit for illegal command
pub const R1_ILLEGAL_COMMAND: u8 = 0x04;

/// start data token for read or write single block*/
pub const DATA_START_BLOCK: u8 = 0xFE;

/// stop token for write multiple blocks*/
pub const STOP_TRAN_TOKEN: u8 = 0xFD;

/// start data token for write multiple blocks*/
pub const WRITE_MULTIPLE_TOKEN: u8 = 0xFC;

/// mask for data response tokens after a write block operation
pub const DATA_RES_MASK: u8 = 0x1F;

/// write data accepted token
pub const DATA_RES_ACCEPTED: u8 = 0x05;

/// A test pattern to use with CMD8_SEND_IF_COND
pub const SD_CHECK_PATTERN: u32 = 0x0000_01AA;

/// Card Specific Data, version 1
#[derive(Default)]
pub struct CsdV1 {
    /// The 16-bytes of data in this Card Specific Data block
    pub data: [u8; 16],
}

/// Card Specific Data, version 2
#[derive(Default)]
pub struct CsdV2 {
    /// The 16-bytes of data in this Card Specific Data block
    pub data: [u8; 16],
}

/// Card Specific Data
pub enum Csd {
    /// A version 1 CSD
    V1(CsdV1),
    /// A version 2 CSD
    V2(CsdV2),
}

impl CsdV1 {
    /// Create a new, empty, CSD
    pub fn new() -> CsdV1 {
        CsdV1::default()
    }

    define_field!(csd_ver, u8, 0, 6, 2);
    define_field!(data_read_access_time1, u8, 1, 0, 8);
    define_field!(data_read_access_time2, u8, 2, 0, 8);
    define_field!(max_data_transfer_rate, u8, 3, 0, 8);
    define_field!(card_command_classes, u16, [(4, 0, 8), (5, 4, 4)]);
    define_field!(read_block_length, u8, 5, 0, 4);
    define_field!(read_partial_blocks, bool, 6, 7);
    define_field!(write_block_misalignment, bool, 6, 6);
    define_field!(read_block_misalignment, bool, 6, 5);
    define_field!(dsr_implemented, bool, 6, 4);
    define_field!(device_size, u32, [(6, 0, 2), (7, 0, 8), (8, 6, 2)]);
    define_field!(max_read_current_vdd_max, u8, 8, 0, 3);
    define_field!(max_read_current_vdd_min, u8, 8, 3, 3);
    define_field!(max_write_current_vdd_max, u8, 9, 2, 3);
    define_field!(max_write_current_vdd_min, u8, 9, 5, 3);
    define_field!(device_size_multiplier, u8, [(9, 0, 2), (10, 7, 1)]);
    define_field!(erase_single_block_enabled, bool, 10, 6);
    define_field!(erase_sector_size, u8, [(10, 0, 6), (11, 7, 1)]);
    define_field!(write_protect_group_size, u8, 11, 0, 7);
    define_field!(write_protect_group_enable, bool, 12, 7);
    define_field!(write_speed_factor, u8, 12, 2, 3);
    define_field!(max_write_data_length, u8, [(12, 0, 2), (13, 6, 2)]);
    define_field!(write_partial_blocks, bool, 13, 5);
    define_field!(file_format, u8, 14, 2, 2);
    define_field!(temporary_write_protection, bool, 14, 4);
    define_field!(permanent_write_protection, bool, 14, 5);
    define_field!(copy_flag_set, bool, 14, 6);
    define_field!(file_format_group_set, bool, 14, 7);
    define_field!(crc, u8, 15, 0, 8);

    /// Returns the card capacity in bytes
    pub fn card_capacity_bytes(&self) -> u64 {
        let multiplier = self.device_size_multiplier() + self.read_block_length() + 2;
        (u64::from(self.device_size()) + 1) << multiplier
    }

    /// Returns the card capacity in 512-byte blocks
    pub fn card_capacity_blocks(&self) -> u32 {
        let multiplier = self.device_size_multiplier() + self.read_block_length() - 7;
        (self.device_size() + 1) << multiplier
    }
}

impl CsdV2 {
    /// Create a new, empty, CSD
    pub fn new() -> CsdV2 {
        CsdV2::default()
    }

    define_field!(csd_ver, u8, 0, 6, 2);
    define_field!(data_read_access_time1, u8, 1, 0, 8);
    define_field!(data_read_access_time2, u8, 2, 0, 8);
    define_field!(max_data_transfer_rate, u8, 3, 0, 8);
    define_field!(card_command_classes, u16, [(4, 0, 8), (5, 4, 4)]);
    define_field!(read_block_length, u8, 5, 0, 4);
    define_field!(read_partial_blocks, bool, 6, 7);
    define_field!(write_block_misalignment, bool, 6, 6);
    define_field!(read_block_misalignment, bool, 6, 5);
    define_field!(dsr_implemented, bool, 6, 4);
    define_field!(device_size, u32, [(7, 0, 6), (8, 0, 8), (9, 0, 8)]);
    define_field!(erase_single_block_enabled, bool, 10, 6);
    define_field!(erase_sector_size, u8, [(10, 0, 6), (11, 7, 1)]);
    define_field!(write_protect_group_size, u8, 11, 0, 7);
    define_field!(write_protect_group_enable, bool, 12, 7);
    define_field!(write_speed_factor, u8, 12, 2, 3);
    define_field!(max_write_data_length, u8, [(12, 0, 2), (13, 6, 2)]);
    define_field!(write_partial_blocks, bool, 13, 5);
    define_field!(file_format, u8, 14, 2, 2);
    define_field!(temporary_write_protection, bool, 14, 4);
    define_field!(permanent_write_protection, bool, 14, 5);
    define_field!(copy_flag_set, bool, 14, 6);
    define_field!(file_format_group_set, bool, 14, 7);
    define_field!(crc, u8, 15, 0, 8);

    /// Returns the card capacity in bytes
    pub fn card_capacity_bytes(&self) -> u64 {
        (u64::from(self.device_size()) + 1) * 512 * 1024
    }

    /// Returns the card capacity in 512-byte blocks
    pub fn card_capacity_blocks(&self) -> u32 {
        (self.device_size() + 1) * 1024
    }
}

/// Perform the 7-bit CRC used on the SD card
pub fn crc7(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for mut d in data.iter().cloned() {
        for _bit in 0..8 {
            crc <<= 1;
            if ((d & 0x80) ^ (crc & 0x80)) != 0 {
                crc ^= 0x09;
            }
            d <<= 1;
        }
    }
    (crc << 1) | 1
}

/// Perform the X25 CRC calculation, as used for data blocks.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &byte in data {
        crc = ((crc >> 8) & 0xFF) | (crc << 8);
        crc ^= u16::from(byte);
        crc ^= (crc & 0xFF) >> 4;
        crc ^= crc << 12;
        crc ^= (crc & 0xFF) << 5;
    }
    crc
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_crc7() {
        const DATA: [u8; 15] = [
            0x00, 0x26, 0x00, 0x32, 0x5f, 0x59, 0x83, 0xc8, 0xad, 0xdb, 0xcf, 0xff, 0xd2, 0x40,
            0x40,
        ];
        assert_eq!(crc7(&DATA), 0xA5);
    }

    #[test]
    fn test_crc16() {
        // An actual CSD read from an SD card
        const DATA: [u8; 16] = [
            0x00, 0x26, 0x00, 0x32, 0x5f, 0x5a, 0x83, 0xae, 0xfe, 0xfb, 0xcf, 0xff, 0x92, 0x80,
            0x40, 0xdf,
        ];
        assert_eq!(crc16(&DATA), 0x9fc5);
    }

    #[test]
    fn test_csdv1b() {
        const EXAMPLE: CsdV1 = CsdV1 {
            data: [
                0x00, 0x26, 0x00, 0x32, 0x5f, 0x59, 0x83, 0xc8, 0xad, 0xdb, 0xcf, 0xff, 0xd2, 0x40,
                0x40, 0xa5,
            ],
        };

        // CSD Structure: describes version of CSD structure
        // 0b00 [Interpreted: Version 1.0]
        assert_eq!(EXAMPLE.csd_ver(), 0x00);

        // Data Read Access Time 1: defines Asynchronous part of the read
        // access time 0x26 [Interpreted: 1.5 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x26);

        // Data Read Access Time 2: worst case clock dependent factor for data
        // access time 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes: 0x5f5 [Interpreted: Class 0: Yes. Class 1:
        // No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6:
        // Yes. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class
        // 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes(), 0x5f5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(EXAMPLE.read_block_length(), 0x09);

        // Partial Blocks for Read Allowed:
        // 0b1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.read_partial_blocks(), true);

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_block_misalignment(), false);

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.read_block_misalignment(), false);

        // DSR Implemented: indicates configurable driver stage integrated on
        // card 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.dsr_implemented(), false);

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)*device size multiplier*max read data block
        // length) bytes 0xf22 [Decimal: 3874]
        assert_eq!(EXAMPLE.device_size(), 3874);

        // Max Read Current @ VDD Min:
        // 0x5 [Interpreted: 35mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_min(), 5);

        // Max Read Current @ VDD Max:
        // 0x5 [Interpreted: 80mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_max(), 5);

        // Max Write Current @ VDD Min:
        // 0x6 [Interpreted: 60mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_min(), 6);

        // Max Write Current @ VDD Max::
        // 0x6 [Interpreted: 200mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_max(), 6);

        // Device Size Multiplier:
        // 0x7 [Interpreted: x512]
        assert_eq!(EXAMPLE.device_size_multiplier(), 7);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.erase_single_block_enabled(), true);

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x1f [Interpreted: 32 blocks]
        assert_eq!(EXAMPLE.erase_sector_size(), 0x1F);

        // Write Protect Group Size:
        // 0x7f [Interpreted: 128 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size(), 0x7f);

        // Write Protect Group Enable:
        // 0x1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.write_protect_group_enable(), true);

        // Write Speed Factor: block program time as multiple of read access time
        // 0x4 [Interpreted: x16]
        assert_eq!(EXAMPLE.write_speed_factor(), 0x4);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(EXAMPLE.max_write_data_length(), 0x9);

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_partial_blocks(), false);

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert_eq!(EXAMPLE.file_format_group_set(), false);

        // Copy Flag:
        // 0b1 [Interpreted: Non-Original]
        assert_eq!(EXAMPLE.copy_flag_set(), true);

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.permanent_write_protection(), false);

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.temporary_write_protection(), false);

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format(), 0x00);

        // CRC7 Checksum + always 1 in LSB:
        // 0xa5
        assert_eq!(EXAMPLE.crc(), 0xa5);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 1_015_808_000);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 1_984_000);
    }

    #[test]
    fn test_csdv1() {
        const EXAMPLE: CsdV1 = CsdV1 {
            data: [
                0x00, 0x7f, 0x00, 0x32, 0x5b, 0x5a, 0x83, 0xaf, 0x7f, 0xff, 0xcf, 0x80, 0x16, 0x80,
                0x00, 0x6f,
            ],
        };
        // CSD Structure: describes version of CSD structure
        // 0b00 [Interpreted: Version 1.0]
        assert_eq!(EXAMPLE.csd_ver(), 0x00);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x7f [Interpreted: 8.0 x 10ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x7F);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes(), 0x5b5);

        // Max Read Data Block Length:
        // 0xa [Interpreted: 1024 Bytes]
        assert_eq!(EXAMPLE.read_block_length(), 0x0a);

        // Partial Blocks for Read Allowed:
        // 0b1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.read_partial_blocks(), true);

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_block_misalignment(), false);

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.read_block_misalignment(), false);

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.dsr_implemented(), false);

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)*device size multiplier*max read data block
        // length) bytes 0xebd [Decimal: 3773]
        assert_eq!(EXAMPLE.device_size(), 3773);

        // Max Read Current @ VDD Min:
        // 0x7 [Interpreted: 100mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_min(), 7);

        // Max Read Current @ VDD Max:
        // 0x7 [Interpreted: 200mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_max(), 7);

        // Max Write Current @ VDD Min:
        // 0x7 [Interpreted: 100mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_min(), 7);

        // Max Write Current @ VDD Max::
        // 0x7 [Interpreted: 200mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_max(), 7);

        // Device Size Multiplier:
        // 0x7 [Interpreted: x512]
        assert_eq!(EXAMPLE.device_size_multiplier(), 7);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.erase_single_block_enabled(), true);

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x1f [Interpreted: 32 blocks]
        assert_eq!(EXAMPLE.erase_sector_size(), 0x1F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_protect_group_enable(), false);

        // Write Speed Factor: block program time as multiple of read access time
        // 0x5 [Interpreted: x32]
        assert_eq!(EXAMPLE.write_speed_factor(), 0x5);

        // Max Write Data Block Length:
        // 0xa [Interpreted: 1024 Bytes]
        assert_eq!(EXAMPLE.max_write_data_length(), 0xa);

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_partial_blocks(), false);

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert_eq!(EXAMPLE.file_format_group_set(), false);

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert_eq!(EXAMPLE.copy_flag_set(), false);

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.permanent_write_protection(), false);

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.temporary_write_protection(), false);

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format(), 0x00);

        // CRC7 Checksum + always 1 in LSB:
        // 0x6f
        assert_eq!(EXAMPLE.crc(), 0x6F);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 1_978_662_912);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 3_864_576);
    }

    #[test]
    fn test_csdv2() {
        const EXAMPLE: CsdV2 = CsdV2 {
            data: [
                0x40, 0x0e, 0x00, 0x32, 0x5b, 0x59, 0x00, 0x00, 0x1d, 0x69, 0x7f, 0x80, 0x0a, 0x40,
                0x00, 0x8b,
            ],
        };
        // CSD Structure: describes version of CSD structure
        // 0b01 [Interpreted: Version 2.0 SDHC]
        assert_eq!(EXAMPLE.csd_ver(), 0x01);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x0e [Interpreted: 1.0 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x0E);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes(), 0x5b5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(EXAMPLE.read_block_length(), 0x09);

        // Partial Blocks for Read Allowed:
        // 0b0 [Interpreted: Yes]
        assert_eq!(EXAMPLE.read_partial_blocks(), false);

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_block_misalignment(), false);

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.read_block_misalignment(), false);

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.dsr_implemented(), false);

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)* 512kbytes
        // 0x001d69 [Decimal: 7529]
        assert_eq!(EXAMPLE.device_size(), 7529);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.erase_single_block_enabled(), true);

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x7f [Interpreted: 128 blocks]
        assert_eq!(EXAMPLE.erase_sector_size(), 0x7F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_protect_group_enable(), false);

        // Write Speed Factor: block program time as multiple of read access time
        // 0x2 [Interpreted: x4]
        assert_eq!(EXAMPLE.write_speed_factor(), 0x2);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(EXAMPLE.max_write_data_length(), 0x9);

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_partial_blocks(), false);

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert_eq!(EXAMPLE.file_format_group_set(), false);

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert_eq!(EXAMPLE.copy_flag_set(), false);

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.permanent_write_protection(), false);

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.temporary_write_protection(), false);

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format(), 0x00);

        // CRC7 Checksum + always 1 in LSB:
        // 0x8b
        assert_eq!(EXAMPLE.crc(), 0x8b);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 3_947_888_640);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 7_710_720);
    }

    #[test]
    fn test_csdv2b() {
        const EXAMPLE: CsdV2 = CsdV2 {
            data: [
                0x40, 0x0e, 0x00, 0x32, 0x5b, 0x59, 0x00, 0x00, 0x3a, 0x91, 0x7f, 0x80, 0x0a, 0x40,
                0x00, 0x05,
            ],
        };
        // CSD Structure: describes version of CSD structure
        // 0b01 [Interpreted: Version 2.0 SDHC]
        assert_eq!(EXAMPLE.csd_ver(), 0x01);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x0e [Interpreted: 1.0 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x0E);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes(), 0x5b5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(EXAMPLE.read_block_length(), 0x09);

        // Partial Blocks for Read Allowed:
        // 0b0 [Interpreted: Yes]
        assert_eq!(EXAMPLE.read_partial_blocks(), false);

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_block_misalignment(), false);

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.read_block_misalignment(), false);

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.dsr_implemented(), false);

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)* 512kbytes
        // 0x003a91 [Decimal: 7529]
        assert_eq!(EXAMPLE.device_size(), 14993);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert_eq!(EXAMPLE.erase_single_block_enabled(), true);

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x7f [Interpreted: 128 blocks]
        assert_eq!(EXAMPLE.erase_sector_size(), 0x7F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_protect_group_enable(), false);

        // Write Speed Factor: block program time as multiple of read access time
        // 0x2 [Interpreted: x4]
        assert_eq!(EXAMPLE.write_speed_factor(), 0x2);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(EXAMPLE.max_write_data_length(), 0x9);

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert_eq!(EXAMPLE.write_partial_blocks(), false);

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert_eq!(EXAMPLE.file_format_group_set(), false);

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert_eq!(EXAMPLE.copy_flag_set(), false);

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.permanent_write_protection(), false);

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert_eq!(EXAMPLE.temporary_write_protection(), false);

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format(), 0x00);

        // CRC7 Checksum + always 1 in LSB:
        // 0x05
        assert_eq!(EXAMPLE.crc(), 0x05);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 7_861_174_272);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 15_353_856);
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
