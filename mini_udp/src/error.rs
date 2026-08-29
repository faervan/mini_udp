use thiserror::Error;

use crate::prelude::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    ByteRepr(#[from] ByteReprError),
    #[error("The CRC failed")]
    CrcFailed,
    #[error("The received packet is shorter than 4 bytes, but the CRC alone needs 4 bytes")]
    PacketLengthLessThanCrcBytes,
    #[error("The message is larger than the maximum size of {}", MAX_PACKET_DATA_LEN * 256)]
    MessageTooBig,
    #[error(
        "The received packed has id {sequence_id}, which is too old because \
        the newest received packet had id {newest_id}"
    )]
    PacketTooOld { sequence_id: u16, newest_id: u16 },
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
