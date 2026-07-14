use std::array::TryFromSliceError;

use thiserror::Error;

pub trait StaticByteRepr {
    const BYTE_LEN: usize;
}

pub trait ByteRepr: Sized {
    const MIN_BYTE_LEN: usize;
    const MAX_BYTE_LEN: usize;
    fn byte_len(&self) -> usize;
    /// The number of bytes written on success is equal to `self.byte_len()`
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, ByteReprError>;
}

pub trait ByteReprExt: ByteRepr {
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
            if bytes.len() - ptr < v.byte_len() || v.write_to_bytes(&mut bytes[ptr..]).is_err() {
                return (i, ptr);
            }
            ptr += v.byte_len();
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
                    ptr += v.byte_len();
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

impl<T: ByteRepr> ByteReprExt for T {}

#[derive(Error, Debug)]
pub enum ByteReprError {
    #[error("The provided byte slice is too short")]
    SliceTooShort,
    #[error("Encountered an unexpected value")]
    InvalidValue,
}

impl From<TryFromSliceError> for ByteReprError {
    fn from(_value: TryFromSliceError) -> Self {
        Self::SliceTooShort
    }
}
