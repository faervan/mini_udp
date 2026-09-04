use crate::prelude2::*;

mod ack;
pub(crate) use ack::PacketAck;

mod priority;
pub use priority::Priority;

mod reliable;
pub use reliable::{Reliable, ReliablePacketHandler};

mod reliable_ordered;
pub use reliable_ordered::{ReliableOrdered, ReliableOrderedPacketHandler};

mod reliable_fragmented;
pub use reliable_fragmented::{ReliableFragmentation, ReliableFragmentationHandler};

mod unreliable;
pub use unreliable::{Unreliable, UnreliablePacketHandler};

mod unreliable_fragmented;
pub use unreliable_fragmented::{UnreliableFragmentation, UnreliableFragmentationHandler};

/// The maximum allowed length of the data part of a UDP packet.
/// The total maximum length ([`MAX_PACKET_LEN`]) is computed by adding the header length as well.
pub const MAX_PACKET_DATA_LEN: usize = 1024;
/// 4 bytes for the CRC, then the `PacketAck`, then 1 byte for the packet type,
///   finally 4 bytes for the amount of messages in the packet (this is not necessary as it can be
///   infered from the UDP packet length, but the [`ByteRepr`] derive currently always includes it)
///   or alternatively 4 bytes for chunk_id, fragment_id and fragment count.
pub const PACKET_HEADER_LEN: usize =
    // CRC
    4
    // ACK
    + PacketAck::BYTE_LEN
    // Packet type
    + 1
    // Num messages or chunk_id + num_fragments + fragment_id
    + 4;
/// The maximum allowed length of a UDP packet.
pub const MAX_PACKET_LEN: usize = PACKET_HEADER_LEN + MAX_PACKET_DATA_LEN;

#[derive(ByteRepr, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(super) struct Packet<M: ByteRepr> {
    pub(super) ack: PacketAck,
    pub(super) ty: PacketType<M>,
}

#[derive(ByteRepr, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug"), derive(strum::IntoStaticStr))]
pub(super) enum PacketType<M: ByteRepr> {
    Heartbeat,
    Unreliable {
        messages: Vec<M>,
    },
    UnreliableFragment {
        chunk_id: u16,
        num_fragments: u8,
        fragment_id: u8,
        data: [u8; MAX_PACKET_DATA_LEN],
    },
    ReliableOrdered {
        messages: Vec<M>,
    },
    ReliableOrderedFragment {
        chunk_id: u16,
        num_fragments: u8,
        fragment_id: u8,
        data: [u8; MAX_PACKET_DATA_LEN],
    },
    ReliableUnordered {
        messages: Vec<M>,
    },
}

#[derive(Debug, PartialEq)]
pub enum ReliablePacketKind {
    Ordered,
    Unordered,
    OrderedFragment,
}

impl<M: ByteRepr> Packet<M> {
    #[inline(always)]
    pub(super) fn heartbeat(ack: PacketAck) -> Self {
        Self {
            ack,
            ty: PacketType::Heartbeat,
        }
    }

    #[inline(always)]
    pub(super) fn get_reliable_kind(&self) -> Option<ReliablePacketKind> {
        match &self.ty {
            PacketType::ReliableOrdered { .. } => Some(ReliablePacketKind::Ordered),
            PacketType::ReliableUnordered { .. } => Some(ReliablePacketKind::Unordered),
            PacketType::ReliableOrderedFragment { .. } => Some(ReliablePacketKind::OrderedFragment),
            PacketType::Heartbeat
            | PacketType::Unreliable { .. }
            | PacketType::UnreliableFragment { .. } => None,
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::{packet::Packet, prelude2::*, ring_buffer::RingBuffer};

    #[derive(ByteRepr, Debug, PartialEq, Hash, Eq, Clone, Copy)]
    pub enum InnerUdpMessage {
        Hello,
        Wave(u16),
    }

    #[test]
    fn packet_byte_repr() {
        let packet = Packet {
            ack: PacketAck::new::<bool>(0, &RingBuffer::new(), &RingBuffer::new()),
            ty: PacketType::ReliableUnordered {
                messages: vec![
                    InnerUdpMessage::Wave(12),
                    InnerUdpMessage::Wave(9284),
                    InnerUdpMessage::Hello,
                ],
            },
        };
        assert_eq!(PacketAck::MIN_BYTE_LEN, 14);
        assert_eq!(Packet::<InnerUdpMessage>::MIN_BYTE_LEN, 15);
        assert_eq!(Packet::<InnerUdpMessage>::MAX_BYTE_LEN, 3019);
        let mut buf = [0; Packet::<InnerUdpMessage>::MAX_BYTE_LEN];
        assert!(packet.write_to_bytes(&mut buf).is_ok());
        assert_eq!(
            Packet::<InnerUdpMessage>::from_bytes(&buf[..packet.byte_len()]).unwrap(),
            packet
        );
    }
}
