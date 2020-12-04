//! embedded-sdmmc-rs - Useful macros for parsing SD/MMC structures.

macro_rules! access_field {
    ($self:expr, $offset:expr, $start_bit:expr, 1) => {
        ($self.data[$offset] & (1 << $start_bit)) != 0
    };
    ($self:expr, $offset:expr, $start:expr, $num_bits:expr) => {
        ($self.data[$offset] >> $start) & (((1u16 << $num_bits) - 1) as u8)
    };
}

macro_rules! define_field {
    ($name:ident, bool, $offset:expr, $bit:expr) => {
        /// Get the value from the $name field
        pub fn $name(&self) -> bool {
            access_field!(self, $offset, $bit, 1)
        }
    };
    ($name:ident, u8, $offset:expr, $start_bit:expr, $num_bits:expr) => {
        /// Get the value from the $name field
        pub fn $name(&self) -> u8 {
            access_field!(self, $offset, $start_bit, $num_bits)
        }
    };
    ($name:ident, $type:ty, [ $( ( $offset:expr, $start_bit:expr, $num_bits:expr ) ),+ ]) => {
        /// Gets the value from the $name field
        pub fn $name(&self) -> $type {
            let mut result = 0;
            $(
                    result <<= $num_bits;
                    let part = access_field!(self, $offset, $start_bit, $num_bits) as $type;
                    result |=  part;
            )+
            result
        }
    };

    ($name:ident, u8, $offset:expr) => {
        /// Get the value from the $name field
        pub fn $name(&self) -> u8 {
            self.data[$offset]
        }
    };

    ($name:ident, u16, $offset:expr) => {
        /// Get the value from the $name field
        pub fn $name(&self) -> u16 {
            u16::from_le_bytes([self.data[$offset], self.data[$offset + 1]])
        }
    };

    ($name:ident, u32, $offset:expr) => {
        /// Get the $name field
        pub fn $name(&self) -> u32 {
            u32::from_le_bytes([
                self.data[$offset],
                self.data[$offset + 1],
                self.data[$offset + 2],
                self.data[$offset + 3]
            ])
        }
    };
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
