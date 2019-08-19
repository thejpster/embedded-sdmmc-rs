//! embedded-sdmmc-rs - Generic File System structures
//!
//! Implements generic file system components. These should be applicable to
//! most (if not all) supported filesystems.

// ****************************************************************************
//
// Imports
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Public Types
//
// ****************************************************************************

/// Things that impl this can tell you the current time.
pub trait TimeSource {
    /// Returns the current time
    fn get_timestamp(&self) -> Timestamp;
}

/// Represents a cluster on disk.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cluster(pub(crate) u32);

/// Represents a directory entry, which tells you about
/// other files and directories.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DirEntry {
    /// The name of the file
    pub name: ShortFileName,
    /// When the file was last modified
    pub mtime: Timestamp,
    /// When the file was first created
    pub ctime: Timestamp,
    /// The file attributes (Read Only, Archive, etc)
    pub attributes: Attributes,
    /// The starting cluster of the file. The FAT tells us the following Clusters.
    pub cluster: Cluster,
    /// The size of the file in bytes.
    pub size: u32,
}

/// An MS-DOS 8.3 filename. 7-bit ASCII only. All lower-case is converted to
/// upper-case by default.
#[derive(PartialEq, Eq, Clone)]
pub struct ShortFileName {
    pub(crate) contents: [u8; 11],
}

/// Represents an instant in time, in the local time zone. TODO: Consider
/// replacing this with POSIX time as a `u32`, which would save two bytes at
/// the expense of some maths.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Timestamp {
    /// Add 1970 to this file to get the calendar year
    pub year_since_1970: u8,
    /// Add one to this value to get the calendar month
    pub zero_indexed_month: u8,
    /// Add one to this value to get the calendar day
    pub zero_indexed_day: u8,
    /// The number of hours past midnight
    pub hours: u8,
    /// The number of minutes past the hour
    pub minutes: u8,
    /// The number of seconds past the minute
    pub seconds: u8,
}

/// Indicates whether a directory entry is read-only, a directory, a volume
/// label, etc.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Attributes(u8);

/// Represents an open file on disk.
#[derive(Debug, PartialEq)]
pub struct OpenFileState {
    /// The starting point of the file.
    pub(crate) starting_cluster: Cluster,
    /// The current cluster, and how many bytes that short-cuts us
    pub(crate) current_cluster: (u32, Cluster),
    /// How far through the file we've read (in bytes).
    pub(crate) current_offset: u32,
    /// The length of the file, in bytes.
    pub(crate) length: u32,
    /// What mode the file was opened in
    pub(crate) mode: Mode,
}

/// Represents an open directory on disk.
#[derive(Debug, PartialEq, Clone)]
pub struct OpenDirState {
    /// The starting point of the directory listing.
    pub(crate) cluster: Cluster,
}

/// The different ways we can open a file.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Mode {
    /// Open a file for reading, if it exists.
    ReadOnly,
    /// Open a file for appending (writing to the end of the existing file), if it exists.
    ReadWriteAppend,
    /// Open a file and remove all contents, before writing to the start of the existing file, if it exists.
    ReadWriteTruncate,
    /// Create a new empty file. Fail if it exists.
    ReadWriteCreate,
    /// Create a new empty file, or truncate an existing file.
    ReadWriteCreateOrTruncate,
    /// Create a new empty file, or append to an existing file.
    ReadWriteCreateOrAppend,
}

/// Various filename related errors that can occur.
#[derive(Debug, Clone)]
pub enum FilenameError {
    /// Tried to create a file with an invalid character.
    InvalidCharacter,
    /// Tried to create a file with no file name.
    FilenameEmpty,
    /// Given name was too long (we are limited to 8.3).
    NameTooLong,
    /// Can't start a file with a period, or after 8 characters.
    MisplacedPeriod,
}

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

// None

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

impl Cluster {
    /// Magic value indicating an invalid cluster value.
    pub const INVALID: Cluster = Cluster(0xFFFF_FFFF);
    /// Magic value indicating a bad cluster.
    pub const BAD: Cluster = Cluster(0xFFFF_FFFE);
    /// Magic value indicating a empty cluster.
    pub const EMPTY: Cluster = Cluster(0xFFFF_FFFD);
    /// Magic value indicating the cluster holding the root directory (which
    /// doesn't have a number in FAT16 as there's a reserved region).
    pub const ROOT_DIR: Cluster = Cluster(0xFFFF_FFFC);
}

// impl DirEntry

impl ShortFileName {
    const FILENAME_BASE_MAX_LEN: usize = 8;
    const FILENAME_EXT_MAX_LEN: usize = 3;
    const FILENAME_MAX_LEN: usize = 11;

    /// Create a new MS-DOS 8.3 space-padded file name as stored in the directory entry.
    pub fn create_from_str(name: &str) -> Result<ShortFileName, FilenameError> {
        let mut sfn = ShortFileName {
            contents: [b' '; Self::FILENAME_MAX_LEN],
        };
        let mut idx = 0;
        let mut seen_dot = false;
        for ch in name.bytes() {
            match ch {
                // Microsoft say these are the invalid characters
                0x00...0x1F
                | 0x20
                | 0x22
                | 0x2A
                | 0x2B
                | 0x2C
                | 0x2F
                | 0x3A
                | 0x3B
                | 0x3C
                | 0x3D
                | 0x3E
                | 0x3F
                | 0x5B
                | 0x5C
                | 0x5D
                | 0x7C => {
                    return Err(FilenameError::InvalidCharacter);
                }
                // Denotes the start of the file extension
                b'.' => {
                    if idx >= 1 && idx <= Self::FILENAME_BASE_MAX_LEN {
                        idx = Self::FILENAME_BASE_MAX_LEN;
                        seen_dot = true;
                    } else {
                        return Err(FilenameError::MisplacedPeriod);
                    }
                }
                _ => {
                    let ch = if ch >= b'a' && ch <= b'z' {
                        // Uppercase characters only
                        ch - 32
                    } else {
                        ch
                    };
                    if seen_dot {
                        if idx >= Self::FILENAME_BASE_MAX_LEN && idx < Self::FILENAME_MAX_LEN {
                            sfn.contents[idx] = ch;
                        } else {
                            return Err(FilenameError::NameTooLong);
                        }
                    } else if idx < Self::FILENAME_BASE_MAX_LEN {
                        sfn.contents[idx] = ch;
                    } else {
                        return Err(FilenameError::NameTooLong);
                    }
                    idx += 1;
                }
            }
        }
        if idx == 0 {
            return Err(FilenameError::FilenameEmpty);
        }
        Ok(sfn)
    }

    /// Create a new MS-DOS 8.3 space-padded file name as stored in the directory entry.
    /// Use this for volume labels with mixed case.
    pub fn create_from_str_mixed_case(name: &str) -> Result<ShortFileName, FilenameError> {
        let mut sfn = ShortFileName {
            contents: [b' '; Self::FILENAME_MAX_LEN],
        };
        let mut idx = 0;
        let mut seen_dot = false;
        for ch in name.bytes() {
            match ch {
                // Microsoft say these are the invalid characters
                0x00...0x1F
                | 0x20
                | 0x22
                | 0x2A
                | 0x2B
                | 0x2C
                | 0x2F
                | 0x3A
                | 0x3B
                | 0x3C
                | 0x3D
                | 0x3E
                | 0x3F
                | 0x5B
                | 0x5C
                | 0x5D
                | 0x7C => {
                    return Err(FilenameError::InvalidCharacter);
                }
                // Denotes the start of the file extension
                b'.' => {
                    if idx >= 1 && idx <= Self::FILENAME_BASE_MAX_LEN {
                        idx = Self::FILENAME_BASE_MAX_LEN;
                        seen_dot = true;
                    } else {
                        return Err(FilenameError::MisplacedPeriod);
                    }
                }
                _ => {
                    if seen_dot {
                        if idx >= Self::FILENAME_BASE_MAX_LEN && idx < Self::FILENAME_MAX_LEN {
                            sfn.contents[idx] = ch;
                        } else {
                            return Err(FilenameError::NameTooLong);
                        }
                    } else if idx < Self::FILENAME_BASE_MAX_LEN {
                        sfn.contents[idx] = ch;
                    } else {
                        return Err(FilenameError::NameTooLong);
                    }
                    idx += 1;
                }
            }
        }
        if idx == 0 {
            return Err(FilenameError::FilenameEmpty);
        }
        Ok(sfn)
    }
}

impl core::fmt::Display for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut printed = 0;
        for (i, &c) in self.contents.iter().enumerate() {
            if c != b' ' {
                if i == Self::FILENAME_BASE_MAX_LEN {
                    write!(f, ".")?;
                    printed += 1;
                }
                write!(f, "{}", c as char)?;
                printed += 1;
            }
        }
        if let Some(mut width) = f.width() {
            if width > printed {
                width -= printed;
                for _ in 0..width {
                    write!(f, "{}", f.fill())?;
                }
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ShortFileName(\"{}\")", self)
    }
}

impl Timestamp {
    const MONTH_LOOKUP: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

    /// Create a `Timestamp` from the 16-bit FAT date and time fields.
    pub fn from_fat(date: u16, time: u16) -> Timestamp {
        let year = (1980 + (date >> 9)) as u16;
        let month = ((date >> 5) & 0x000F) as u8;
        let day = (date & 0x001F) as u8;
        let hours = ((time >> 11) & 0x001F) as u8;
        let minutes = ((time >> 5) & 0x0003F) as u8;
        let seconds = ((time << 1) & 0x0003F) as u8;
        // Volume labels have a zero for month/day, so tolerate that...
        Timestamp {
            year_since_1970: (year - 1970) as u8,
            zero_indexed_month: if month == 0 { 0 } else { month - 1 },
            zero_indexed_day: if day == 0 { 0 } else { day - 1 },
            hours,
            minutes,
            seconds,
        }
    }

    /// Create a `Timestamp` from year/month/day/hour/minute/second.
    ///
    /// Values should be given as you'd write then (i.e. 1980, 01, 01, 13, 30,
    /// 05) is 1980-Jan-01, 1:30:05pm.
    pub fn from_calendar(
        year: u16,
        month: u8,
        day: u8,
        hours: u8,
        minutes: u8,
        seconds: u8,
    ) -> Result<Timestamp, &'static str> {
        Ok(Timestamp {
            year_since_1970: if year >= 1970 && year <= (1970 + 255) {
                (year - 1970) as u8
            } else {
                return Err("Bad year");
            },
            zero_indexed_month: if month >= 1 && month <= 12 {
                month - 1
            } else {
                return Err("Bad month");
            },
            zero_indexed_day: if day >= 1 && day <= 31 {
                day - 1
            } else {
                return Err("Bad day");
            },
            hours: if hours <= 23 {
                hours
            } else {
                return Err("Bad hours");
            },
            minutes: if minutes <= 59 {
                minutes
            } else {
                return Err("Bad minutes");
            },
            seconds: if seconds <= 59 {
                seconds
            } else {
                return Err("Bad seconds");
            },
        })
    }
}

impl core::fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Timestamp({})", self)
    }
}

impl core::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{}-{:02}-{:02} {:02}:{:02}:{:02}",
            u16::from(self.year_since_1970) + 1970,
            self.zero_indexed_month + 1,
            self.zero_indexed_day + 1,
            self.hours,
            self.minutes,
            self.seconds
        )
    }
}

impl Attributes {
    /// Indicates this file cannot be written.
    pub const READ_ONLY: u8 = 0x01;
    /// Indicates the file is hidden.
    pub const HIDDEN: u8 = 0x02;
    /// Indicates this is a system file.
    pub const SYSTEM: u8 = 0x04;
    /// Indicates this is a volume label.
    pub const VOLUME: u8 = 0x08;
    /// Indicates this is a directory.
    pub const DIRECTORY: u8 = 0x10;
    /// Indicates this file needs archiving (i.e. has been modified since last
    /// archived).
    pub const ARCHIVE: u8 = 0x20;
    /// This set of flags indicates the file is actually a long file name
    /// fragment.
    pub const LFN: u8 = Self::READ_ONLY | Self::HIDDEN | Self::SYSTEM | Self::VOLUME;

    /// Create a `Attributes` value from the `u8` stored in a FAT16/FAT32
    /// Directory Entry.
    pub(crate) fn create_from_fat(value: u8) -> Attributes {
        Attributes(value)
    }

    /// Does this file has the read-only attribute set?
    pub fn is_read_only(self) -> bool {
        (self.0 & Self::READ_ONLY) == Self::READ_ONLY
    }

    /// Does this file has the hidden attribute set?
    pub fn is_hidden(self) -> bool {
        (self.0 & Self::HIDDEN) == Self::HIDDEN
    }

    /// Does this file has the system attribute set?
    pub fn is_system(self) -> bool {
        (self.0 & Self::SYSTEM) == Self::SYSTEM
    }

    /// Does this file has the volume attribute set?
    pub fn is_volume(self) -> bool {
        (self.0 & Self::VOLUME) == Self::VOLUME
    }

    /// Does this entry point at a directory?
    pub fn is_directory(self) -> bool {
        (self.0 & Self::DIRECTORY) == Self::DIRECTORY
    }

    /// Does this need archiving?
    pub fn is_archive(self) -> bool {
        (self.0 & Self::ARCHIVE) == Self::ARCHIVE
    }

    /// Is this a long file name fragment?
    pub fn is_lfn(self) -> bool {
        (self.0 & Self::LFN) == Self::LFN
    }
}

impl core::fmt::Debug for Attributes {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.is_lfn() {
            write!(f, "LFN")?;
        } else {
            if self.is_directory() {
                write!(f, "D")?;
            } else {
                write!(f, "F")?;
            }
            if self.is_read_only() {
                write!(f, "R")?;
            }
            if self.is_hidden() {
                write!(f, "H")?;
            }
            if self.is_system() {
                write!(f, "S")?;
            }
            if self.is_volume() {
                write!(f, "V")?;
            }
            if self.is_archive() {
                write!(f, "A")?;
            }
        }
        Ok(())
    }
}

impl OpenFileState {
    /// Create a new file handle.
    pub(crate) fn new(cluster: Cluster, length: u32, mode: Mode) -> OpenFileState {
        OpenFileState {
            starting_cluster: cluster,
            current_cluster: (0, cluster),
            mode,
            length,
            current_offset: 0,
        }
    }

    /// Are we at the end of the file?
    pub fn eof(&self) -> bool {
        self.current_offset == self.length
    }

    /// How long is the file?
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Seek to a new position in the file, relative to the start of the file.
    pub fn seek_from_start(&mut self, offset: u32) -> Result<(), ()> {
        if offset <= self.length {
            self.current_offset = offset;
            if offset < self.current_cluster.0 {
                // Back to start
                self.current_cluster = (0, self.starting_cluster);
            }
            Ok(())
        } else {
            Err(())
        }
    }

    /// Seek to a new position in the file, relative to the end of the file.
    pub fn seek_from_end(&mut self, offset: u32) -> Result<(), ()> {
        if offset <= self.length {
            self.current_offset = self.length - offset;
            if offset < self.current_cluster.0 {
                // Back to start
                self.current_cluster = (0, self.starting_cluster);
            }
            Ok(())
        } else {
            Err(())
        }
    }

    /// Seek to a new position in the file, relative to the current position.
    pub fn seek_from_current(&mut self, offset: i32) -> Result<(), ()> {
        let new_offset = i64::from(self.current_offset) + i64::from(offset);
        if new_offset >= 0 && new_offset <= i64::from(self.length) {
            self.current_offset = new_offset as u32;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Amount of file left to read.
    pub fn left(&self) -> u32 {
        self.length - self.current_offset
    }
}

impl OpenDirState {}

impl FilenameError {}

// ****************************************************************************
//
// Private Functions / Impl for Priate Types
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn filename_no_extension() {
        let sfn = ShortFileName {
            contents: *b"HELLO      ",
        };
        assert_eq!(format!("{}", &sfn), "HELLO");
        assert_eq!(sfn, ShortFileName::create_from_str("HELLO").unwrap());
        assert_eq!(sfn, ShortFileName::create_from_str("hello").unwrap());
        assert_eq!(sfn, ShortFileName::create_from_str("HeLlO").unwrap());
        assert_eq!(sfn, ShortFileName::create_from_str("HELLO.").unwrap());
    }

    #[test]
    fn filename_extension() {
        let sfn = ShortFileName {
            contents: *b"HELLO   TXT",
        };
        assert_eq!(format!("{}", &sfn), "HELLO.TXT");
        assert_eq!(sfn, ShortFileName::create_from_str("HELLO.TXT").unwrap());
    }

    #[test]
    fn filename_fulllength() {
        let sfn = ShortFileName {
            contents: *b"12345678TXT",
        };
        assert_eq!(format!("{}", &sfn), "12345678.TXT");
        assert_eq!(sfn, ShortFileName::create_from_str("12345678.TXT").unwrap());
    }

    #[test]
    fn filename_short_extension() {
        let sfn = ShortFileName {
            contents: *b"12345678C  ",
        };
        assert_eq!(format!("{}", &sfn), "12345678.C");
        assert_eq!(sfn, ShortFileName::create_from_str("12345678.C").unwrap());
    }

    #[test]
    fn filename_short() {
        let sfn = ShortFileName {
            contents: *b"1       C  ",
        };
        assert_eq!(format!("{}", &sfn), "1.C");
        assert_eq!(sfn, ShortFileName::create_from_str("1.C").unwrap());
    }

    #[test]
    fn filename_bad() {
        assert!(ShortFileName::create_from_str("").is_err());
        assert!(ShortFileName::create_from_str(" ").is_err());
        assert!(ShortFileName::create_from_str("123456789").is_err());
        assert!(ShortFileName::create_from_str("12345678.ABCD").is_err());
    }

}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
