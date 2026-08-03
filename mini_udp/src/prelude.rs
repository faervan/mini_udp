#[doc(hidden)]
pub use mini_udp_derive::ByteRepr;

pub use crate::byte_repr::{ByteRepr, ByteReprError, ByteReprExt, StaticByteRepr};
pub use crate::communicator::*;

pub(crate) use crate::communicator::{InnerUdpCommunicator, UdpCommunicatorSocket};
pub(crate) use crate::packet::{MAX_PACKET_DATA_LEN, MAX_PACKET_LEN, Packet};
pub(crate) use crate::packet_ack::PacketAck;

pub(crate) use std::time::Instant;

#[cfg(debug_assertions)]
pub use tracing::debug;
pub use tracing::{error, warn};
