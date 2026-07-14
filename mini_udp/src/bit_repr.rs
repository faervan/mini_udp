use std::array::TryFromSliceError;

use thiserror::Error;

pub trait StaticBitRepr {
    const BIT_LEN: usize;
}

pub trait BitRepr: Sized {
    const MIN_BIT_LEN: usize;
    const MAX_BIT_LEN: usize;
    fn bit_len(&self) -> usize;
    /// The number of bytes written on success is equal to `self.bit_len()`
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), BitReprError>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, BitReprError>;
}

pub trait BitReprExt: BitRepr {
    /// Writes all items of `v` into `bytes`.
    /// Returns the number of items of `v` that fit into and have been written to `bytes`, as well
    /// as the length of all those items combined in bytes.
    /// `(num_items_written, num_bytes_written)`
    fn write_many<'a, I>(values: I, bytes: &mut [u8]) -> (usize, usize)
    where
        I: IntoIterator<Item = &'a Self>,
        Self: 'a,
    {
        let mut ptr = 0;
        let mut i = 0;
        for v in values {
            if bytes.len() - ptr < v.bit_len() || v.write_to_bytes(&mut bytes[ptr..]).is_err() {
                return (i, ptr);
            }
            ptr += v.bit_len();
            i += 1;
        }
        (i, ptr)
    }
    /// Returns `(Vec<Self>, bytes_read)`
    fn read_many(bytes: &[u8]) -> (Vec<Self>, usize) {
        let mut ptr = 0;
        let mut out = vec![];
        while bytes.len() > ptr {
            match Self::from_bytes(&bytes[ptr..]) {
                Ok(v) => {
                    ptr += v.bit_len();
                    out.push(v);
                }
                Err(e) => {
                    // TODO! make this better
                    eprintln!("Failed to read packet: {e}");
                    return (out, ptr);
                }
            }
        }
        (out, ptr)
    }
}

impl<T: BitRepr> BitReprExt for T {}

#[derive(Error, Debug)]
pub enum BitReprError {
    #[error("The provided byte slice is too short")]
    SliceTooShort,
    #[error("Encountered an unexpected value")]
    InvalidValue,
}

impl From<TryFromSliceError> for BitReprError {
    fn from(_value: TryFromSliceError) -> Self {
        Self::SliceTooShort
    }
}
