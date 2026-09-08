use thiserror::Error;

use crate::prelude2::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error("ByteRepr: {0}")]
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
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("The communicator is not connected")]
    NotConnected,
    #[error("Incoming connection request refused: Already connected")]
    AlreadyConnected,
    #[error(
        "Received an unexpected message.\n\
        May be either a data message while not connected or a connection message \
        that does not align with the current connection state. \
        The latter should generally not happen, because connection messages are sent ordered."
    )]
    UnexpectedMessage,
    #[error("A received packet was determined not to belong to the current connection.")]
    PacketSessionMismatch,
    #[error("Failed to write new packet because the send buffer is full.")]
    PacketSendBufferFull,
    #[error("The provided ToSocketAddr returned an empty iterator.")]
    NoSocketAddrYield,
}

/// As [`std::io::Error`] does not implement [`PartialEq`], two [`Error::Io`] values never compare
/// as equal.
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::ByteRepr(e1) => {
                if let Self::ByteRepr(e2) = other
                    && e1 == e2
                {
                    return true;
                }
            }
            Self::CrcFailed => return matches!(other, Self::CrcFailed),
            Self::PacketLengthLessThanCrcBytes => {
                return matches!(other, Self::PacketLengthLessThanCrcBytes);
            }
            Self::MessageTooBig => return matches!(other, Self::MessageTooBig),
            Self::PacketTooOld {
                sequence_id: s1,
                newest_id: n1,
            } => {
                if let Self::PacketTooOld {
                    sequence_id: s2,
                    newest_id: n2,
                } = other
                    && s1 == s2
                    && n1 == n2
                {
                    return true;
                }
            }
            Self::Io(_) => return false,
            Self::NotConnected => return matches!(other, Self::NotConnected),
            Self::AlreadyConnected => return matches!(other, Self::AlreadyConnected),
            Self::UnexpectedMessage => return matches!(other, Self::UnexpectedMessage),
            Self::PacketSessionMismatch => return matches!(other, Self::PacketSessionMismatch),
            Self::PacketSendBufferFull => return matches!(other, Self::PacketSendBufferFull),
            Self::NoSocketAddrYield => return matches!(other, Self::NoSocketAddrYield),
        }
        false
    }
}
