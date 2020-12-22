//! # embedded-sdmmc
//!
//! > An SD/MMC Library written in Embedded Rust
//!
//! This crate is intended to allow you to read/write files on a FAT formatted SD
//! card on your Rust Embedded device, as easily as using the `SdFat` Arduino
//! library. It is written in pure-Rust, is `#![no_std]` and does not use `alloc`
//! or `collections` to keep the memory footprint low. In the first instance it is
//! designed for readability and simplicity over performance.
//!
//! ## Using the crate
//!
//! You will need something that implements the `BlockDevice` trait, which can read and write the 512-byte blocks (or sectors) from your card. If you were to implement this over USB Mass Storage, there's no reason this crate couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice` suitable for reading SD and SDHC cards over SPI.
//!
//! ```rust
//! # struct DummySpi;
//! # struct DummyCsPin;
//! # struct DummyUart;
//! # struct DummyTimeSource;
//! # impl embedded_hal::blocking::spi::Transfer<u8> for  DummySpi {
//! #   type Error = ();
//! #   fn try_transfer<'w>(&mut self, data: &'w mut [u8]) -> Result<&'w [u8], ()> { Ok(&[0]) }
//! # }
//! # impl embedded_hal::digital::OutputPin for DummyCsPin {
//! #   type Error = ();
//! #   fn try_set_low(&mut self) -> Result<(), ()> { Ok(()) }
//! #   fn try_set_high(&mut self) -> Result<(), ()> { Ok(()) }
//! # }
//! # impl embedded_sdmmc::TimeSource for DummyTimeSource {
//! #   fn get_timestamp(&self) -> embedded_sdmmc::Timestamp { embedded_sdmmc::Timestamp::from_fat(0, 0) }
//! # }
//! # impl std::fmt::Write for DummyUart { fn write_str(&mut self, s: &str) -> std::fmt::Result { Ok(()) } }
//! # use std::fmt::Write;
//! # let mut uart = DummyUart;
//! # let mut sdmmc_spi = DummySpi;
//! # let mut sdmmc_cs = DummyCsPin;
//! # let time_source = DummyTimeSource;
//! let mut cont = embedded_sdmmc::Controller::new(embedded_sdmmc::SdMmcSpi::new(sdmmc_spi, sdmmc_cs), time_source);
//! write!(uart, "Init SD card...").unwrap();
//! match cont.device().init() {
//!     Ok(_) => {
//!         write!(uart, "OK!\nCard size...").unwrap();
//!         match cont.device().card_size_bytes() {
//!             Ok(size) => writeln!(uart, "{}", size).unwrap(),
//!             Err(e) => writeln!(uart, "Err: {:?}", e).unwrap(),
//!         }
//!         write!(uart, "Volume 0...").unwrap();
//!         match cont.get_volume(embedded_sdmmc::VolumeIdx(0)) {
//!             Ok(v) => writeln!(uart, "{:?}", v).unwrap(),
//!             Err(e) => writeln!(uart, "Err: {:?}", e).unwrap(),
//!         }
//!     }
//!     Err(e) => writeln!(uart, "{:?}!", e).unwrap(),
//! }
//! ```

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

// ****************************************************************************
//
// Imports
//
// ****************************************************************************

#[cfg(test)]
#[macro_use]
extern crate hex_literal;

use byteorder::{ByteOrder, LittleEndian};
use core::convert::TryFrom;
use log::debug;

#[macro_use]
mod structure;

pub mod blockdevice;
pub mod fat;
pub mod filesystem;
pub mod sdmmc;
pub mod sdmmc_proto;

pub use crate::blockdevice::{Block, BlockCount, BlockDevice, BlockIdx};
pub use crate::fat::FatVolume;
use crate::fat::RESERVED_ENTRIES;
pub use crate::filesystem::{
    Attributes, Cluster, DirEntry, Directory, File, FilenameError, Mode, ShortFileName, TimeSource,
    Timestamp, MAX_FILE_SIZE,
};
pub use crate::sdmmc::Error as SdMmcError;
pub use crate::sdmmc::SdMmcSpi;

// ****************************************************************************
//
// Public Types
//
// ****************************************************************************

/// Represents all the ways the functions in this crate can fail.
#[derive(Debug, Clone)]
pub enum Error<E>
where
    E: core::fmt::Debug,
{
    /// The underlying block device threw an error.
    DeviceError(E),
    /// The filesystem is badly formatted (or this code is buggy).
    FormatError(&'static str),
    /// The given `VolumeIdx` was bad,
    NoSuchVolume,
    /// The given filename was bad
    FilenameError(FilenameError),
    /// Out of memory opening directories
    TooManyOpenDirs,
    /// Out of memory opening files
    TooManyOpenFiles,
    /// That file doesn't exist
    FileNotFound,
    /// You can't open a file twice
    FileAlreadyOpen,
    /// You can't open a directory twice
    DirAlreadyOpen,
    /// You can't open a directory as a file
    OpenedDirAsFile,
    /// You can't delete a directory as a file
    DeleteDirAsFile,
    /// You can't delete an open file
    FileIsOpen,
    /// We can't do that yet
    Unsupported,
    /// Tried to read beyond end of file
    EndOfFile,
    /// Found a bad cluster
    BadCluster,
    /// Error while converting types
    ConversionError,
    /// The device does not have enough space for the operation
    NotEnoughSpace,
    /// Cluster was not properly allocated by the library
    AllocationError,
    /// Jumped to free space during fat traversing
    JumpedFree,
    /// Tried to open Read-Only file with write mode
    ReadOnly,
    /// Tried to create an existing file
    FileAlreadyExists,
    /// Bad block size - only 512 byte blocks supported
    BadBlockSize(u16),
    /// Entry not found in the block
    NotInBlock,
}

/// We have to track what directories are open to prevent users from modifying
/// open directories (like creating a file when we have an open iterator).
pub const MAX_OPEN_DIRS: usize = 4;

/// We have to track what files and directories are open to prevent users from
/// deleting open files (like Windows does).
pub const MAX_OPEN_FILES: usize = 4;

/// A `Controller` wraps a block device and gives access to the volumes within it.
pub struct Controller<D, T>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    block_device: D,
    timesource: T,
    open_dirs: [(VolumeIdx, Cluster); MAX_OPEN_DIRS],
    open_files: [(VolumeIdx, Cluster); MAX_OPEN_DIRS],
}

/// Represents a partition with a filesystem within it.
#[derive(Debug, PartialEq, Eq)]
pub struct Volume {
    idx: VolumeIdx,
    volume_type: VolumeType,
}

/// This enum holds the data for the various different types of filesystems we
/// support.
#[derive(Debug, PartialEq, Eq)]
pub enum VolumeType {
    /// FAT16/FAT32 formatted volumes.
    Fat(FatVolume),
}

/// A `VolumeIdx` is a number which identifies a volume (or partition) on a
/// disk. `VolumeIdx(0)` is the first primary partition on an MBR partitioned
/// disk.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct VolumeIdx(pub usize);

// ****************************************************************************
//
// Public Data
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Private Types
//
// ****************************************************************************

/// Marker for a FAT32 partition. Sometimes also use for FAT16 formatted
/// partitions.
const PARTITION_ID_FAT32_LBA: u8 = 0x0C;
/// Marker for a FAT16 partition with LBA. Seen on a Raspberry Pi SD card.
const PARTITION_ID_FAT16_LBA: u8 = 0x0E;
/// Marker for a FAT16 partition. Seen on a card formatted with the official
/// SD-Card formatter.
const PARTITION_ID_FAT16: u8 = 0x06;
/// Marker for a FAT32 partition. What Macosx disk utility (and also SD-Card formatter?)
/// use.
const PARTITION_ID_FAT32_CHS_LBA: u8 = 0x0B;

// ****************************************************************************
//
// Private Data
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Public Functions / Impl for Public Types
//
// ****************************************************************************

impl<D, T> Controller<D, T>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    /// Create a new Disk Controller using a generic `BlockDevice`. From this
    /// controller we can open volumes (partitions) and with those we can open
    /// files.
    pub fn new(block_device: D, timesource: T) -> Controller<D, T> {
        debug!("Creating new embedded-sdmmc::Controller");
        Controller {
            block_device,
            timesource,
            open_dirs: [(VolumeIdx(0), Cluster::INVALID); 4],
            open_files: [(VolumeIdx(0), Cluster::INVALID); 4],
        }
    }

    /// Temporarily get access to the underlying block device.
    pub fn device(&mut self) -> &mut D {
        &mut self.block_device
    }

    /// Get a volume (or partition) based on entries in the Master Boot
    /// Record. We do not support GUID Partition Table disks. Nor do we
    /// support any concept of drive letters - that is for a higher layer to
    /// handle.
    pub fn get_volume(&mut self, volume_idx: VolumeIdx) -> Result<Volume, Error<D::Error>> {
        const PARTITION1_START: usize = 446;
        const PARTITION2_START: usize = PARTITION1_START + PARTITION_INFO_LENGTH;
        const PARTITION3_START: usize = PARTITION2_START + PARTITION_INFO_LENGTH;
        const PARTITION4_START: usize = PARTITION3_START + PARTITION_INFO_LENGTH;
        const FOOTER_START: usize = 510;
        const FOOTER_VALUE: u16 = 0xAA55;
        const PARTITION_INFO_LENGTH: usize = 16;
        const PARTITION_INFO_STATUS_INDEX: usize = 0;
        const PARTITION_INFO_TYPE_INDEX: usize = 4;
        const PARTITION_INFO_LBA_START_INDEX: usize = 8;
        const PARTITION_INFO_NUM_BLOCKS_INDEX: usize = 12;

        let (part_type, lba_start, num_blocks) = {
            let mut blocks = [Block::new()];
            self.block_device
                .read(&mut blocks, BlockIdx(0), "read_mbr")
                .map_err(Error::DeviceError)?;
            let block = &blocks[0];
            // We only support Master Boot Record (MBR) partitioned cards, not
            // GUID Partition Table (GPT)
            if LittleEndian::read_u16(&block[FOOTER_START..FOOTER_START + 2]) != FOOTER_VALUE {
                return Err(Error::FormatError("Invalid MBR signature"));
            }
            let partition = match volume_idx {
                VolumeIdx(0) => {
                    &block[PARTITION1_START..(PARTITION1_START + PARTITION_INFO_LENGTH)]
                }
                VolumeIdx(1) => {
                    &block[PARTITION2_START..(PARTITION2_START + PARTITION_INFO_LENGTH)]
                }
                VolumeIdx(2) => {
                    &block[PARTITION3_START..(PARTITION3_START + PARTITION_INFO_LENGTH)]
                }
                VolumeIdx(3) => {
                    &block[PARTITION4_START..(PARTITION4_START + PARTITION_INFO_LENGTH)]
                }
                _ => {
                    return Err(Error::NoSuchVolume);
                }
            };
            // Only 0x80 and 0x00 are valid (bootable, and non-bootable)
            if (partition[PARTITION_INFO_STATUS_INDEX] & 0x7F) != 0x00 {
                return Err(Error::FormatError("Invalid partition status"));
            }
            let lba_start = LittleEndian::read_u32(
                &partition[PARTITION_INFO_LBA_START_INDEX..(PARTITION_INFO_LBA_START_INDEX + 4)],
            );
            let num_blocks = LittleEndian::read_u32(
                &partition[PARTITION_INFO_NUM_BLOCKS_INDEX..(PARTITION_INFO_NUM_BLOCKS_INDEX + 4)],
            );
            (
                partition[PARTITION_INFO_TYPE_INDEX],
                BlockIdx(lba_start),
                BlockCount(num_blocks),
            )
        };
        match part_type {
            PARTITION_ID_FAT32_CHS_LBA
            | PARTITION_ID_FAT32_LBA
            | PARTITION_ID_FAT16_LBA
            | PARTITION_ID_FAT16 => {
                let volume = fat::parse_volume(self, lba_start, num_blocks)?;
                Ok(Volume {
                    idx: volume_idx,
                    volume_type: volume,
                })
            }
            _ => Err(Error::FormatError("Partition type not supported")),
        }
    }

    /// Open a directory. You can then read the directory entries in a random
    /// order using `get_directory_entry`.
    ///
    /// TODO: Work out how to prevent damage occuring to the file system while
    /// this directory handle is open. In particular, stop this directory
    /// being unlinked.
    pub fn open_root_dir(&mut self, volume: &Volume) -> Result<Directory, Error<D::Error>> {
        // Find a free directory entry, and check the root dir isn't open. As
        // we already know the root dir's magic cluster number, we can do both
        // checks in one loop.
        let mut open_dirs_row = None;
        for (i, d) in self.open_dirs.iter().enumerate() {
            if *d == (volume.idx, Cluster::ROOT_DIR) {
                return Err(Error::DirAlreadyOpen);
            }
            if d.1 == Cluster::INVALID {
                open_dirs_row = Some(i);
                break;
            }
        }
        let open_dirs_row = open_dirs_row.ok_or(Error::TooManyOpenDirs)?;
        // Remember this open directory
        self.open_dirs[open_dirs_row] = (volume.idx, Cluster::ROOT_DIR);
        Ok(Directory {
            cluster: Cluster::ROOT_DIR,
            entry: None,
        })
    }

    /// Open a directory. You can then read the directory entries in a random
    /// order using `get_directory_entry`.
    ///
    /// TODO: Work out how to prevent damage occuring to the file system while
    /// this directory handle is open. In particular, stop this directory
    /// being unlinked.
    pub fn open_dir(
        &mut self,
        volume: &Volume,
        parent_dir: &Directory,
        name: &str,
    ) -> Result<Directory, Error<D::Error>> {
        // Find a free open directory table row
        let mut open_dirs_row = None;
        for (i, d) in self.open_dirs.iter().enumerate() {
            if d.1 == Cluster::INVALID {
                open_dirs_row = Some(i);
            }
        }
        let open_dirs_row = open_dirs_row.ok_or(Error::TooManyOpenDirs)?;

        // Open the directory
        let dir_entry = match &volume.volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(self, parent_dir, name)?,
        };

        if !dir_entry.attributes.is_directory() {
            return Err(Error::OpenedDirAsFile);
        }

        // Check it's not already open
        for (_i, dir_table_row) in self.open_dirs.iter().enumerate() {
            if *dir_table_row == (volume.idx, dir_entry.cluster) {
                return Err(Error::DirAlreadyOpen);
            }
        }
        // Remember this open directory
        self.open_dirs[open_dirs_row] = (volume.idx, dir_entry.cluster);
        Ok(Directory {
            cluster: dir_entry.cluster,
            entry: Some(dir_entry),
        })
    }

    /// Close a directory. You cannot perform operations on an open directory
    /// and so must close it if you want to do something with it.
    pub fn close_dir(&mut self, volume: &Volume, dir: Directory) {
        let target = (volume.idx, dir.cluster);
        for d in self.open_dirs.iter_mut() {
            if *d == target {
                d.1 = Cluster::INVALID;
                break;
            }
        }
        drop(dir);
    }

    /// Look in a directory for a named file.
    pub fn find_directory_entry(
        &mut self,
        volume: &Volume,
        dir: &Directory,
        name: &str,
    ) -> Result<DirEntry, Error<D::Error>> {
        match &volume.volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(self, dir, name),
        }
    }

    /// Call a callback function for each directory entry in a directory.
    pub fn iterate_dir<F>(
        &mut self,
        volume: &Volume,
        dir: &Directory,
        func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
    {
        match &volume.volume_type {
            VolumeType::Fat(fat) => fat.iterate_dir(self, dir, func),
        }
    }

    /// Open a file from DirEntry. This is obtained by calling iterate_dir. A file can only be opened once.
    pub fn open_dir_entry(
        &mut self,
        volume: &mut Volume,
        dir_entry: DirEntry,
        mode: Mode,
    ) -> Result<File, Error<D::Error>> {
        let open_files_row = self.get_open_files_row()?;
        // Check it's not already open
        for dir_table_row in self.open_files.iter() {
            if *dir_table_row == (volume.idx, dir_entry.cluster) {
                return Err(Error::DirAlreadyOpen);
            }
        }
        if dir_entry.attributes.is_directory() {
            return Err(Error::OpenedDirAsFile);
        }
        if dir_entry.attributes.is_read_only() && mode != Mode::ReadOnly {
            return Err(Error::ReadOnly);
        }

        let mode = solve_mode_variant(mode, true);
        let file = match mode {
            Mode::ReadOnly => File {
                starting_cluster: dir_entry.cluster,
                current_cluster: (0, dir_entry.cluster),
                current_offset: 0,
                length: dir_entry.size,
                mode,
                entry: dir_entry,
            },
            Mode::ReadWriteAppend => {
                let mut file = File {
                    starting_cluster: dir_entry.cluster,
                    current_cluster: (0, dir_entry.cluster),
                    current_offset: 0,
                    length: dir_entry.size,
                    mode,
                    entry: dir_entry,
                };
                // seek_from_end with 0 can't fail
                file.seek_from_end(0).ok();
                file
            }
            Mode::ReadWriteTruncate => {
                let mut file = File {
                    starting_cluster: dir_entry.cluster,
                    current_cluster: (0, dir_entry.cluster),
                    current_offset: 0,
                    length: dir_entry.size,
                    mode,
                    entry: dir_entry,
                };
                match &mut volume.volume_type {
                    VolumeType::Fat(fat) => {
                        fat.truncate_cluster_chain(self, file.starting_cluster)?
                    }
                };
                file.update_length(0);
                // TODO update entry Timestamps
                match &volume.volume_type {
                    VolumeType::Fat(fat) => {
                        let fat_type = fat.get_fat_type();
                        self.write_entry_to_disk(fat_type, &file.entry)?;
                    }
                };

                file
            }
            _ => return Err(Error::Unsupported),
        };
        // Remember this open file
        self.open_files[open_files_row] = (volume.idx, file.starting_cluster);
        Ok(file)
    }

    /// Open a file with the given full path. A file can only be opened once.
    pub fn open_file_in_dir(
        &mut self,
        volume: &mut Volume,
        dir: &Directory,
        name: &str,
        mode: Mode,
    ) -> Result<File, Error<D::Error>> {
        let dir_entry = match &volume.volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(self, dir, name),
        };

        let open_files_row = self.get_open_files_row()?;
        let dir_entry = match dir_entry {
            Ok(entry) => Some(entry),
            Err(_)
                if (mode == Mode::ReadWriteCreate)
                    | (mode == Mode::ReadWriteCreateOrTruncate)
                    | (mode == Mode::ReadWriteCreateOrAppend) =>
            {
                None
            }
            _ => return Err(Error::FileNotFound),
        };

        let mode = solve_mode_variant(mode, dir_entry.is_some());

        match mode {
            Mode::ReadWriteCreate => {
                if dir_entry.is_some() {
                    return Err(Error::FileAlreadyExists);
                }
                let file_name =
                    ShortFileName::create_from_str(name).map_err(Error::FilenameError)?;
                let att = Attributes::create_from_fat(0);
                let entry = match &mut volume.volume_type {
                    VolumeType::Fat(fat) => {
                        fat.write_new_directory_entry(self, dir, file_name, att)?
                    }
                };

                let file = File {
                    starting_cluster: entry.cluster,
                    current_cluster: (0, entry.cluster),
                    current_offset: 0,
                    length: entry.size,
                    mode,
                    entry,
                };
                // Remember this open file
                self.open_files[open_files_row] = (volume.idx, file.starting_cluster);
                Ok(file)
            }
            _ => {
                // Safe to unwrap, since we actually have an entry if we got here
                let dir_entry = dir_entry.unwrap();
                self.open_dir_entry(volume, dir_entry, mode)
            }
        }
    }

    /// Get the next entry in open_files list
    fn get_open_files_row(&self) -> Result<usize, Error<D::Error>> {
        // Find a free directory entry
        let mut open_files_row = None;
        for (i, d) in self.open_files.iter().enumerate() {
            if d.1 == Cluster::INVALID {
                open_files_row = Some(i);
            }
        }
        open_files_row.ok_or(Error::TooManyOpenDirs)
    }

    /// Delete a closed file with the given full path, if exists.
    pub fn delete_file_in_dir(
        &mut self,
        volume: &Volume,
        dir: &Directory,
        name: &str,
    ) -> Result<(), Error<D::Error>> {
        debug!(
            "delete_file(volume={:?}, dir={:?}, filename={:?}",
            volume, dir, name
        );
        let dir_entry = match &volume.volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(self, dir, name),
        }?;

        if dir_entry.attributes.is_directory() {
            return Err(Error::DeleteDirAsFile);
        }

        let target = (volume.idx, dir_entry.cluster);
        for d in self.open_files.iter_mut() {
            if *d == target {
                return Err(Error::FileIsOpen);
            }
        }

        match &volume.volume_type {
            VolumeType::Fat(fat) => return fat.delete_directory_entry(self, dir, name),
        };
    }

    /// Read from an open file.
    pub fn read(
        &mut self,
        volume: &Volume,
        file: &mut File,
        buffer: &mut [u8],
    ) -> Result<usize, Error<D::Error>> {
        // Calculate which file block the current offset lies within
        // While there is more to read, read the block and copy in to the buffer.
        // If we need to find the next cluster, walk the FAT.
        let mut space = buffer.len();
        let mut read = 0;
        while space > 0 && !file.eof() {
            let (block_idx, block_offset, block_avail) =
                self.find_data_on_disk(volume, &mut file.current_cluster, file.current_offset)?;
            let mut blocks = [Block::new()];
            self.block_device
                .read(&mut blocks, block_idx, "read")
                .map_err(Error::DeviceError)?;
            let block = &blocks[0];
            let to_copy = block_avail.min(space).min(file.left() as usize);
            assert!(to_copy != 0);
            buffer[read..read + to_copy]
                .copy_from_slice(&block[block_offset..block_offset + to_copy]);
            read += to_copy;
            space -= to_copy;
            file.seek_from_current(to_copy as i32).unwrap();
        }
        Ok(read)
    }

    /// Write to a open file.
    pub fn write(
        &mut self,
        volume: &mut Volume,
        file: &mut File,
        buffer: &[u8],
    ) -> Result<usize, Error<D::Error>> {
        debug!(
            "write(volume={:?}, file={:?}, buffer={:x?}",
            volume, file, buffer
        );
        if file.mode == Mode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        if file.starting_cluster.0 < RESERVED_ENTRIES {
            // file doesn't have a valid allocated cluster (possible zero-length file), allocate one
            file.starting_cluster = match &mut volume.volume_type {
                VolumeType::Fat(fat) => fat.alloc_cluster(self, None, false)?,
            };
            file.entry.cluster = file.starting_cluster;
            debug!("Alloc first cluster {:?}", file.starting_cluster);
        }
        if (file.current_cluster.1).0 < file.starting_cluster.0 {
            debug!("Rewinding to start");
            file.current_cluster = (0, file.starting_cluster);
        }
        let bytes_until_max = usize::try_from(MAX_FILE_SIZE - file.current_offset)
            .map_err(|_| Error::ConversionError)?;
        let bytes_to_write = core::cmp::min(buffer.len(), bytes_until_max);
        let mut written = 0;

        while written < bytes_to_write {
            let mut current_cluster = file.current_cluster;
            debug!(
                "Have written bytes {}/{}, finding cluster {:?}",
                written, bytes_to_write, current_cluster
            );
            let (block_idx, block_offset, block_avail) =
                match self.find_data_on_disk(volume, &mut current_cluster, file.current_offset) {
                    Ok(vars) => {
                        debug!(
                            "Found block_idx={:?}, block_offset={:?}, block_avail={}",
                            vars.0, vars.1, vars.2
                        );
                        vars
                    }
                    Err(Error::EndOfFile) => {
                        debug!("Extending file");
                        match &mut volume.volume_type {
                            VolumeType::Fat(ref mut fat) => {
                                if fat
                                    .alloc_cluster(self, Some(current_cluster.1), false)
                                    .is_err()
                                {
                                    return Ok(written);
                                }
                                debug!("Allocated new FAT cluster, finding offsets...");
                                let new_offset = self
                                    .find_data_on_disk(
                                        volume,
                                        &mut current_cluster,
                                        file.current_offset,
                                    )
                                    .map_err(|_| Error::AllocationError)?;
                                debug!("New offset {:?}", new_offset);
                                new_offset
                            }
                        }
                    }
                    Err(e) => return Err(e),
                };
            let mut blocks = [Block::new()];
            let to_copy = core::cmp::min(block_avail, bytes_to_write - written);
            if block_offset != 0 {
                debug!("Partial block write");
                self.block_device
                    .read(&mut blocks, block_idx, "read")
                    .map_err(Error::DeviceError)?;
            }
            let block = &mut blocks[0];
            block[block_offset..block_offset + to_copy]
                .copy_from_slice(&buffer[written..written + to_copy]);
            debug!("Writing block {:?}", block_idx);
            self.block_device
                .write(&blocks, block_idx)
                .map_err(Error::DeviceError)?;
            written += to_copy;
            file.current_cluster = current_cluster;
            let to_copy = i32::try_from(to_copy).map_err(|_| Error::ConversionError)?;
            // TODO: Should we do this once when the whole file is written?
            file.update_length(file.length + (to_copy as u32));
            file.seek_from_current(to_copy).unwrap();
            file.entry.attributes.set_archive(true);
            file.entry.mtime = self.timesource.get_timestamp();
            debug!("Updating FAT info sector");
            match &mut volume.volume_type {
                VolumeType::Fat(fat) => {
                    fat.update_info_sector(self)?;
                    debug!("Updating dir entry");
                    self.write_entry_to_disk(fat.get_fat_type(), &file.entry)?;
                }
            }
        }
        Ok(written)
    }

    /// Close a file with the given full path.
    pub fn close_file(&mut self, volume: &Volume, file: File) -> Result<(), Error<D::Error>> {
        let target = (volume.idx, file.starting_cluster);
        for d in self.open_files.iter_mut() {
            if *d == target {
                d.1 = Cluster::INVALID;
                break;
            }
        }
        drop(file);
        Ok(())
    }

    /// This function turns `desired_offset` into an appropriate block to be
    /// read. It either calculates this based on the start of the file, or
    /// from the last cluster we read - whichever is better.
    fn find_data_on_disk(
        &mut self,
        volume: &Volume,
        start: &mut (u32, Cluster),
        desired_offset: u32,
    ) -> Result<(BlockIdx, usize, usize), Error<D::Error>> {
        let bytes_per_cluster = match &volume.volume_type {
            VolumeType::Fat(fat) => fat.bytes_per_cluster(),
        };
        // How many clusters forward do we need to go?
        let offset_from_cluster = desired_offset - start.0;
        let num_clusters = offset_from_cluster / bytes_per_cluster;
        for _ in 0..num_clusters {
            start.1 = match &volume.volume_type {
                VolumeType::Fat(fat) => fat.next_cluster(self, start.1)?,
            };
            start.0 += bytes_per_cluster;
        }
        // How many blocks in are we?
        let offset_from_cluster = desired_offset - start.0;
        assert!(offset_from_cluster < bytes_per_cluster);
        let num_blocks = BlockCount(offset_from_cluster / Block::LEN_U32);
        let block_idx = match &volume.volume_type {
            VolumeType::Fat(fat) => fat.cluster_to_block(start.1),
        } + num_blocks;
        let block_offset = (desired_offset % Block::LEN_U32) as usize;
        let available = Block::LEN - block_offset;
        Ok((block_idx, block_offset, available))
    }

    /// Writes a Directory Entry to the disk
    fn write_entry_to_disk(
        &mut self,
        fat_type: fat::FatType,
        entry: &DirEntry,
    ) -> Result<(), Error<D::Error>> {
        let mut blocks = [Block::new()];
        self.block_device
            .read(&mut blocks, entry.entry_block, "read")
            .map_err(Error::DeviceError)?;
        let block = &mut blocks[0];

        let start = usize::try_from(entry.entry_offset).map_err(|_| Error::ConversionError)?;
        block[start..start + 32].copy_from_slice(&entry.serialize(fat_type)[..]);

        self.block_device
            .write(&blocks, entry.entry_block)
            .map_err(Error::DeviceError)?;
        Ok(())
    }
}

// ****************************************************************************
//
// Private Functions / Impl for Private Types
//
// ****************************************************************************

/// Transform mode variants (ReadWriteCreate_Or_Append) to simple modes ReadWriteAppend or
/// ReadWriteCreate
fn solve_mode_variant(mode: Mode, dir_entry_is_some: bool) -> Mode {
    let mut mode = mode;
    if mode == Mode::ReadWriteCreateOrAppend {
        if dir_entry_is_some {
            mode = Mode::ReadWriteAppend;
        } else {
            mode = Mode::ReadWriteCreate;
        }
    } else if mode == Mode::ReadWriteCreateOrTruncate {
        if dir_entry_is_some {
            mode = Mode::ReadWriteTruncate;
        } else {
            mode = Mode::ReadWriteCreate;
        }
    }
    mode
}

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyBlockDevice;

    struct Clock;

    #[derive(Debug)]
    enum Error {
        Unknown,
    }

    impl TimeSource for Clock {
        fn get_timestamp(&self) -> Timestamp {
            // TODO: Return actual time
            Timestamp {
                year_since_1970: 0,
                zero_indexed_month: 0,
                zero_indexed_day: 0,
                hours: 0,
                minutes: 0,
                seconds: 0,
            }
        }
    }

    impl BlockDevice for DummyBlockDevice {
        type Error = Error;

        /// Read one or more blocks, starting at the given block index.
        fn read(
            &self,
            blocks: &mut [Block],
            start_block_idx: BlockIdx,
            _reason: &str,
        ) -> Result<(), Self::Error> {
            // Actual blocks taken from an SD card, except I've changed the start and length of partition 0.
            static BLOCKS: [Block; 3] = [
                Block {
                    contents: [
                        0xfa, 0xb8, 0x00, 0x10, 0x8e, 0xd0, 0xbc, 0x00, 0xb0, 0xb8, 0x00, 0x00,
                        0x8e, 0xd8, 0x8e, 0xc0, // 0x000
                        0xfb, 0xbe, 0x00, 0x7c, 0xbf, 0x00, 0x06, 0xb9, 0x00, 0x02, 0xf3, 0xa4,
                        0xea, 0x21, 0x06, 0x00, // 0x010
                        0x00, 0xbe, 0xbe, 0x07, 0x38, 0x04, 0x75, 0x0b, 0x83, 0xc6, 0x10, 0x81,
                        0xfe, 0xfe, 0x07, 0x75, // 0x020
                        0xf3, 0xeb, 0x16, 0xb4, 0x02, 0xb0, 0x01, 0xbb, 0x00, 0x7c, 0xb2, 0x80,
                        0x8a, 0x74, 0x01, 0x8b, // 0x030
                        0x4c, 0x02, 0xcd, 0x13, 0xea, 0x00, 0x7c, 0x00, 0x00, 0xeb, 0xfe, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x040
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x050
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x060
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x070
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x080
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x090
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0A0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0B0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0C0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0F0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x100
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x110
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x120
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x130
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x140
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x150
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x160
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x170
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x180
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x190
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1A0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4c, 0xca, 0xde, 0x06,
                        0x00, 0x00, 0x00, 0x04, // 0x1B0
                        0x01, 0x04, 0x0c, 0xfe, 0xc2, 0xff, 0x01, 0x00, 0x00, 0x00, 0x33, 0x22,
                        0x11, 0x00, 0x00, 0x00, // 0x1C0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x55, 0xaa, // 0x1F0
                    ],
                },
                Block {
                    contents: [
                        0xeb, 0x58, 0x90, 0x6d, 0x6b, 0x66, 0x73, 0x2e, 0x66, 0x61, 0x74, 0x00,
                        0x02, 0x08, 0x20, 0x00, // 0x000
                        0x02, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x00, 0x00, 0x10, 0x00, 0x04, 0x00,
                        0x00, 0x08, 0x00, 0x00, // 0x010
                        0x00, 0x20, 0x76, 0x00, 0x80, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x02, 0x00, 0x00, 0x00, // 0x020
                        0x01, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x030
                        0x80, 0x01, 0x29, 0x0b, 0xa8, 0x89, 0x27, 0x50, 0x69, 0x63, 0x74, 0x75,
                        0x72, 0x65, 0x73, 0x20, // 0x040
                        0x20, 0x20, 0x46, 0x41, 0x54, 0x33, 0x32, 0x20, 0x20, 0x20, 0x0e, 0x1f,
                        0xbe, 0x77, 0x7c, 0xac, // 0x050
                        0x22, 0xc0, 0x74, 0x0b, 0x56, 0xb4, 0x0e, 0xbb, 0x07, 0x00, 0xcd, 0x10,
                        0x5e, 0xeb, 0xf0, 0x32, // 0x060
                        0xe4, 0xcd, 0x16, 0xcd, 0x19, 0xeb, 0xfe, 0x54, 0x68, 0x69, 0x73, 0x20,
                        0x69, 0x73, 0x20, 0x6e, // 0x070
                        0x6f, 0x74, 0x20, 0x61, 0x20, 0x62, 0x6f, 0x6f, 0x74, 0x61, 0x62, 0x6c,
                        0x65, 0x20, 0x64, 0x69, // 0x080
                        0x73, 0x6b, 0x2e, 0x20, 0x20, 0x50, 0x6c, 0x65, 0x61, 0x73, 0x65, 0x20,
                        0x69, 0x6e, 0x73, 0x65, // 0x090
                        0x72, 0x74, 0x20, 0x61, 0x20, 0x62, 0x6f, 0x6f, 0x74, 0x61, 0x62, 0x6c,
                        0x65, 0x20, 0x66, 0x6c, // 0x0A0
                        0x6f, 0x70, 0x70, 0x79, 0x20, 0x61, 0x6e, 0x64, 0x0d, 0x0a, 0x70, 0x72,
                        0x65, 0x73, 0x73, 0x20, // 0x0B0
                        0x61, 0x6e, 0x79, 0x20, 0x6b, 0x65, 0x79, 0x20, 0x74, 0x6f, 0x20, 0x74,
                        0x72, 0x79, 0x20, 0x61, // 0x0C0
                        0x67, 0x61, 0x69, 0x6e, 0x20, 0x2e, 0x2e, 0x2e, 0x20, 0x0d, 0x0a, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0F0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x100
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x110
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x120
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x130
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x140
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x150
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x160
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x170
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x180
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x190
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1A0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1B0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1C0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x55, 0xaa, // 0x1F0
                    ],
                },
                Block {
                    contents: hex!(
                        "52 52 61 41 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
                         00 00 00 00 72 72 41 61 FF FF FF FF FF FF FF FF
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 55 AA"
                    ),
                },
            ];
            println!(
                "Reading block {} to {}",
                start_block_idx.0,
                start_block_idx.0 as usize + blocks.len()
            );
            for (idx, block) in blocks.iter_mut().enumerate() {
                let block_idx = start_block_idx.0 as usize + idx;
                if block_idx < BLOCKS.len() {
                    *block = BLOCKS[block_idx].clone();
                } else {
                    return Err(Error::Unknown);
                }
            }
            Ok(())
        }

        /// Write one or more blocks, starting at the given block index.
        fn write(&self, _blocks: &[Block], _start_block_idx: BlockIdx) -> Result<(), Self::Error> {
            unimplemented!();
        }

        /// Determine how many blocks this device can hold.
        fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
            Ok(BlockCount(2))
        }
    }

    #[test]
    fn partition0() {
        let mut c = Controller::new(DummyBlockDevice, Clock);
        let v = c.get_volume(VolumeIdx(0)).unwrap();
        assert_eq!(
            v,
            Volume {
                idx: VolumeIdx(0),
                volume_type: VolumeType::Fat(FatVolume {
                    lba_start: BlockIdx(1),
                    num_blocks: BlockCount(0x0011_2233),
                    blocks_per_cluster: 8,
                    first_data_block: BlockCount(15136),
                    fat_start: BlockCount(32),
                    name: fat::VolumeName::new(*b"Pictures   "),
                    free_clusters_count: None,
                    next_free_cluster: None,
                    cluster_count: 965_788,
                    fat_specific_info: fat::FatSpecificInfo::Fat32(fat::Fat32Info {
                        first_root_dir_cluster: Cluster(2),
                        info_location: BlockIdx(1) + BlockCount(1),
                    })
                })
            }
        );
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
