use std::array::TryFromSliceError;

use thiserror::Error;

pub trait BitRepr: Sized {
    const MIN_BIT_LEN: usize;
    const MAX_BIT_LEN: usize;
    fn bit_len(&self) -> usize;
    fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), BitReprError>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, BitReprError>;
}

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
