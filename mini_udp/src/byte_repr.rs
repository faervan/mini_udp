use std::{array::TryFromSliceError, fmt::Debug};

use mini_udp_derive::derive_for;
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
    /// Writes all items of `values` into `bytes`.
    /// Returns the number of items of `values` that fit into and have been written to `bytes`, as well
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
    /// Returns `(Vec<Self>, bytes_read)` on success
    fn read_many(bytes: &[u8]) -> Result<(Vec<Self>, usize), ByteReprError> {
        let mut ptr = 0;
        // TODO! Maybe allocate with capacity here
        let mut out = vec![];
        while bytes.len() > ptr && bytes.len() - ptr >= Self::MIN_BYTE_LEN {
            let v = Self::from_bytes(&bytes[ptr..])?;
            ptr += v.byte_len();
            out.push(v);
        }
        Ok((out, ptr))
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

derive_for!(());
derive_for!(bool);
derive_for!(u8);
derive_for!(i8);
derive_for!(u16);
derive_for!(i16);
derive_for!(u32);
derive_for!(i32);
derive_for!(f32);
derive_for!(u64);
derive_for!(i64);
derive_for!(f64);
derive_for!(usize);
derive_for!(isize);
derive_for!(u128);
derive_for!(i128);
derive_for!(String);
derive_for!(Box<T>, <T> where T: ByteRepr);
derive_for!(std::borrow::Cow<'a, T>, <'a, T> where T: ByteRepr + ToOwned<Owned = T>);
derive_for!(std::borrow::Cow<'a, str>, <'a>);
derive_for!(Option<T>, <T> where T: ByteRepr);
derive_for!([T; N], <T, const N: usize> where T: ByteRepr + Default);
derive_for!(Vec<T>, <T> where T: ByteRepr);

impl<T> StaticByteRepr for Box<T>
where
    T: StaticByteRepr,
{
    const BYTE_LEN: usize = T::BYTE_LEN;
}

impl<'a, T> StaticByteRepr for std::borrow::Cow<'a, T>
where
    T: StaticByteRepr + ToOwned<Owned = T>,
{
    const BYTE_LEN: usize = T::BYTE_LEN;
}

impl<T, const N: usize> StaticByteRepr for [T; N]
where
    T: StaticByteRepr,
{
    const BYTE_LEN: usize = T::BYTE_LEN * N;
}
