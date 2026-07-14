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

impl ByteRepr for bool {
    const MIN_BYTE_LEN: usize = 1;
    const MAX_BYTE_LEN: usize = 1;
    fn byte_len(&self) -> usize {
        1
    }
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError> {
        *bytes.get_mut(0).ok_or(ByteReprError::SliceTooShort)? = if *self { 1 } else { 0 };
        Ok(())
    }
    fn from_bytes(bytes: &[u8]) -> Result<Self, ByteReprError> {
        if *bytes.first().ok_or(ByteReprError::SliceTooShort)? == 1 {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl ByteRepr for u8 {
    const MIN_BYTE_LEN: usize = 1;
    const MAX_BYTE_LEN: usize = 1;
    fn byte_len(&self) -> usize {
        1
    }
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError> {
        *bytes.get_mut(0).ok_or(ByteReprError::SliceTooShort)? = *self;
        Ok(())
    }
    fn from_bytes(bytes: &[u8]) -> Result<Self, ByteReprError> {
        bytes.first().copied().ok_or(ByteReprError::SliceTooShort)
    }
}

impl ByteRepr for i8 {
    const MIN_BYTE_LEN: usize = 1;
    const MAX_BYTE_LEN: usize = 1;
    fn byte_len(&self) -> usize {
        1
    }
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError> {
        *bytes.get_mut(0).ok_or(ByteReprError::SliceTooShort)? = *self as u8;
        Ok(())
    }
    fn from_bytes(bytes: &[u8]) -> Result<Self, ByteReprError> {
        bytes
            .first()
            .map(|v| *v as i8)
            .ok_or(ByteReprError::SliceTooShort)
    }
}

#[macro_export]
macro_rules! impl_byte_repr_multi_byte_primitives {
    ($ty:ty, $len:literal) => {
        impl ByteRepr for $ty {
            const MIN_BYTE_LEN: usize = $len;
            const MAX_BYTE_LEN: usize = $len;
            fn byte_len(&self) -> usize {
                $len
            }
            fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError> {
                bytes[..$len].copy_from_slice(&self.to_le_bytes());
                Ok(())
            }
            fn from_bytes(bytes: &[u8]) -> Result<Self, ByteReprError> {
                Ok(Self::from_le_bytes(
                    TryInto::<[u8; $len]>::try_into(&bytes[..$len])
                        .map_err(|_| ByteReprError::SliceTooShort)?,
                ))
            }
        }
    };
}

crate::impl_byte_repr_multi_byte_primitives!(u16, 2);
crate::impl_byte_repr_multi_byte_primitives!(i16, 2);
crate::impl_byte_repr_multi_byte_primitives!(u32, 4);
crate::impl_byte_repr_multi_byte_primitives!(i32, 4);
crate::impl_byte_repr_multi_byte_primitives!(f32, 4);
crate::impl_byte_repr_multi_byte_primitives!(u64, 8);
crate::impl_byte_repr_multi_byte_primitives!(i64, 8);
crate::impl_byte_repr_multi_byte_primitives!(f64, 8);
#[cfg(target_pointer_width = "32")]
crate::impl_byte_repr_multi_byte_primitives!(usize, 4);
#[cfg(target_pointer_width = "32")]
crate::impl_byte_repr_multi_byte_primitives!(isize, 4);
#[cfg(target_pointer_width = "64")]
crate::impl_byte_repr_multi_byte_primitives!(usize, 8);
#[cfg(target_pointer_width = "64")]
crate::impl_byte_repr_multi_byte_primitives!(isize, 8);
crate::impl_byte_repr_multi_byte_primitives!(u128, 16);
crate::impl_byte_repr_multi_byte_primitives!(i128, 16);
