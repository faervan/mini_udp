use std::{array::TryFromSliceError, fmt::Debug};

use thiserror::Error;

pub trait StaticByteRepr {
    const BYTE_LEN: usize;
}

// TODO! maybe remove the Debug bound in the future
/// You may use the [derive macro](macro@crate::prelude::ByteRepr) to implement this trait.
///
/// **Example**
/// ```rust
/// use mini_udp::prelude::*;
///
/// #[derive(ByteRepr, PartialEq, Debug)]
/// struct A {
///     list: [f32; 3],
/// }
/// assert_eq!(A::MIN_BYTE_LEN, 12);
/// assert_eq!(A::MAX_BYTE_LEN, 12);
///
/// let a = A { list: [1397.201, -0.0, -4010401.32914] };
/// assert_eq!(a.byte_len(), A::MIN_BYTE_LEN);
///
/// let mut buf = [0; A::MAX_BYTE_LEN];
/// assert!(a.write_to_bytes(&mut buf).is_ok());
/// assert_eq!(A::from_bytes(&buf).unwrap(), a);
/// ```
pub trait ByteRepr: Debug + Sized {
    const MIN_BYTE_LEN: usize;
    const MAX_BYTE_LEN: usize;
    fn byte_len(&self) -> usize;
    /// The number of bytes written on success is equal to `self.byte_len()`
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError>;
    /// The number of bytes read on success is equal to `self.byte_len()`
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

impl ByteRepr for () {
    const MIN_BYTE_LEN: usize = 0;
    const MAX_BYTE_LEN: usize = 0;
    fn byte_len(&self) -> usize {
        0
    }
    fn write_to_bytes(&self, _bytes: &mut [u8]) -> Result<(), ByteReprError> {
        Ok(())
    }
    fn from_bytes(_bytes: &[u8]) -> Result<Self, ByteReprError> {
        Ok(())
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

impl ByteRepr for String {
    const MIN_BYTE_LEN: usize = 4;
    const MAX_BYTE_LEN: usize = 1024;
    fn byte_len(&self) -> usize {
        4 + self.len()
    }
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ByteReprError> {
        bytes
            .get_mut(..4)
            .ok_or(ByteReprError::SliceTooShort)?
            .copy_from_slice(&(self.len() as u32).to_le_bytes());
        bytes
            .get_mut(4..4 + self.len())
            .ok_or(ByteReprError::SliceTooShort)?
            .copy_from_slice(self.as_bytes());
        Ok(())
    }
    fn from_bytes(bytes: &[u8]) -> Result<Self, ByteReprError> {
        let len = u32::from_le_bytes(
            TryInto::<[u8; 4]>::try_into(bytes.get(..4).ok_or(ByteReprError::SliceTooShort)?)
                .map_err(|_| ByteReprError::SliceTooShort)?,
        ) as usize;
        Ok(
            Self::from_utf8_lossy(bytes.get(4..4 + len).ok_or(ByteReprError::SliceTooShort)?)
                .into_owned(),
        )
    }
}

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

impl_byte_repr_multi_byte_primitives!(u16, 2);
impl_byte_repr_multi_byte_primitives!(i16, 2);
impl_byte_repr_multi_byte_primitives!(u32, 4);
impl_byte_repr_multi_byte_primitives!(i32, 4);
impl_byte_repr_multi_byte_primitives!(f32, 4);
impl_byte_repr_multi_byte_primitives!(u64, 8);
impl_byte_repr_multi_byte_primitives!(i64, 8);
impl_byte_repr_multi_byte_primitives!(f64, 8);
#[cfg(target_pointer_width = "32")]
impl_byte_repr_multi_byte_primitives!(usize, 4);
#[cfg(target_pointer_width = "32")]
impl_byte_repr_multi_byte_primitives!(isize, 4);
#[cfg(target_pointer_width = "64")]
impl_byte_repr_multi_byte_primitives!(usize, 8);
#[cfg(target_pointer_width = "64")]
impl_byte_repr_multi_byte_primitives!(isize, 8);
impl_byte_repr_multi_byte_primitives!(u128, 16);
impl_byte_repr_multi_byte_primitives!(i128, 16);
