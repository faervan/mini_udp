use crate::prelude::*;

/// The maximum allowed length of the data part of a UDP packet.
/// The total maximum length is computed by adding the header length as well.
pub(super) const MAX_PACKET_DATA_LEN: usize = 1024;
/// 4 bytes for the CRC, then the [`PacketAck`], then 1 byte extra metadata (reliable, ordered)
/// Currently 2 byte extra because booleans do not get combined yet!
pub(super) const PACKET_HEADER_LEN: usize = 4 + PacketAck::BYTE_LEN + 2; //1;
/// The maximum allowed length of a UDP packet.
pub(super) const MAX_PACKET_LEN: usize = PACKET_HEADER_LEN + MAX_PACKET_DATA_LEN;

const PROTOCOL_VERSION: u32 = 0x00_00_00_01;
/// [`crc::CRC_32_BZIP2`] with `init` set to [`PROTOCOL_VERSION`]
const CRC_ALGORITHM: crc::Algorithm<u32> = crc::Algorithm {
    width: 32,
    poly: 0x04c11db7,
    init: PROTOCOL_VERSION,
    refin: false,
    refout: false,
    xorout: 0xffffffff,
    check: 0xfc891918,
    residue: 0xc704dd7b,
};
const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&CRC_ALGORITHM);

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
    pub fn new(ack: PacketAck, messages: impl IntoIterator<Item = M>) -> Self {
        Self {
            ack,
            reliable: true,
            ordered: false,
            messages: messages.into_iter().collect(),
        }
    }

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

#[derive(ByteRepr, Debug, PartialEq, Hash, Eq, Clone, Copy)]
pub enum InnerUdpMessage {
    Hello,
    Wave(u16),
}

#[cfg(test)]
mod test {
    use crate::{
        packet::{InnerUdpMessage, Packet},
        prelude::*,
    };

    #[test]
    fn packet_byte_repr() {
        let com = UdpCommunicator::<InnerUdpMessage>::default();
        let packet = Packet::new(
            com.inner.create_ack(0),
            [
                InnerUdpMessage::Wave(12),
                InnerUdpMessage::Wave(9284),
                InnerUdpMessage::Hello,
            ],
        );
        assert_eq!(crate::PacketAck::MIN_BYTE_LEN, 8);
        assert_eq!(Packet::<InnerUdpMessage>::MIN_BYTE_LEN, 14);
        assert_eq!(Packet::<InnerUdpMessage>::MAX_BYTE_LEN, 3014);
        let mut buf = [0; Packet::<InnerUdpMessage>::MAX_BYTE_LEN];
        assert!(packet.write_to_bytes(&mut buf).is_ok());
        assert_eq!(
            Packet::<InnerUdpMessage>::from_bytes(&buf[..packet.byte_len()]).unwrap(),
            packet
        );
    }
}
