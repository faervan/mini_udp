#[doc(hidden)]
pub use mini_udp_derive::ByteRepr;

pub use crate::byte_repr::{ByteRepr, ByteReprError, ByteReprExt as _, StaticByteRepr};
pub use crate::sender::{MultiUdpCommunicator, UdpCommunicator};

pub(crate) use crate::packet_ack::PacketAck;
