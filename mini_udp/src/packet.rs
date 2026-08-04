use crate::prelude::*;

/// The maximum allowed length of the data part of a UDP packet.
/// The total maximum length is computed by adding the header length as well.
pub(super) const MAX_PACKET_DATA_LEN: usize = 1024;
/// 4 bytes for the CRC, then the [`PacketAck`], then 1 byte extra metadata (reliable, ordered),
///   finally 4 bytes for the amount of messages in the packet (this is not necessary as it can be
///   infered from the UDP packet length, but the [`ByteRepr`] derive currently always includes it).
/// Currently 2 byte extra because booleans do not get combined yet! (TODO!)
pub(super) const PACKET_HEADER_LEN: usize = 4 + PacketAck::BYTE_LEN + 2 + 4;
/// The maximum allowed length of a UDP packet.
pub(super) const MAX_PACKET_LEN: usize = PACKET_HEADER_LEN + MAX_PACKET_DATA_LEN;

#[derive(ByteRepr, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Packet<M: ByteRepr> {
    pub(super) ack: PacketAck,
    /// TODO!
    pub(super) reliable: bool,
    /// TODO!
    pub(super) ordered: bool,
    /// If `messages.is_empty()`, then this was send as a heartbeat packet
    pub(super) messages: Vec<M>,
}

impl<M: ByteRepr> Packet<M> {
    #[inline(always)]
    pub fn heartbeat(ack: PacketAck) -> Self {
        Self {
            ack,
            reliable: false,
            ordered: false,
            messages: vec![],
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::{packet::Packet, prelude::*, ring_buffer::RingBuffer};

    #[derive(ByteRepr, Debug, PartialEq, Hash, Eq, Clone, Copy)]
    pub enum InnerUdpMessage {
        Hello,
        Wave(u16),
    }

    #[test]
    fn packet_byte_repr() {
        let packet = Packet {
            ack: PacketAck::new::<bool>(0, &RingBuffer::new(), &RingBuffer::new()),
            reliable: true,
            ordered: false,
            messages: vec![
                InnerUdpMessage::Wave(12),
                InnerUdpMessage::Wave(9284),
                InnerUdpMessage::Hello,
            ],
        };
        assert_eq!(PacketAck::MIN_BYTE_LEN, 14);
        assert_eq!(Packet::<InnerUdpMessage>::MIN_BYTE_LEN, 20);
        assert_eq!(Packet::<InnerUdpMessage>::MAX_BYTE_LEN, 3020);
        let mut buf = [0; Packet::<InnerUdpMessage>::MAX_BYTE_LEN];
        assert!(packet.write_to_bytes(&mut buf).is_ok());
        assert_eq!(
            Packet::<InnerUdpMessage>::from_bytes(&buf[..packet.byte_len()]).unwrap(),
            packet
        );
    }
}
