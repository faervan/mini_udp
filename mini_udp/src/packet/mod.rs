use crate::prelude2::*;

mod ack;
pub(crate) use ack::{PacketAck, PartialPacketAck};

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
    // TODO! Use `BYTE_LEN` here once ByteRepr derive derives StaticByteRepr as well.
    + PacketAck::<u128, u128>::MAX_BYTE_LEN
    // Packet type
    + 1
    // Num messages or chunk_id + num_fragments + fragment_id
    + 4;
/// The maximum allowed length of a UDP packet.
pub const MAX_PACKET_LEN: usize = PACKET_HEADER_LEN + MAX_PACKET_DATA_LEN;

pub enum Packet<M: ByteRepr, Config: MiniUdpConfig, ConHandler: ConnectionHandler> {
    Connection {
        sequence_id: u16,
        ack: PacketAck<<Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets, <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets>,
        con_data: ConHandler::ConnectionEstablishmentData,
    },
    Data {
        sequence_id: u16,
        ack: PacketAck<<Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets, <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets>,
        data: PacketData<M>,
    },
}

#[derive(ByteRepr, Debug)]
pub enum PacketHeader<Config: MiniUdpConfig, ConHandler: ConnectionHandler> {
    Unprotected {
        sequence_id: u16,
        ack: PacketAck<<Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets, <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets>,
        con_data: ConHandler::ConnectionEstablishmentData,
    },
    Protected {
        sequence_id: u16,
        ack: PacketAck<<Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets, <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets>,
        header: <ConHandler::Protection as PacketProtection>::Header,
    },
}

#[derive(ByteRepr, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug"), derive(strum::IntoStaticStr))]
pub enum PacketData<M: ByteRepr> {
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

impl<M: ByteRepr> PacketData<M> {
    #[inline(always)]
    pub(super) fn get_reliable_kind(&self) -> Option<ReliablePacketKind> {
        match self {
            PacketData::ReliableOrdered { .. } => Some(ReliablePacketKind::Ordered),
            PacketData::ReliableUnordered { .. } => Some(ReliablePacketKind::Unordered),
            PacketData::ReliableOrderedFragment { .. } => Some(ReliablePacketKind::OrderedFragment),
            PacketData::Heartbeat
            | PacketData::Unreliable { .. }
            | PacketData::UnreliableFragment { .. } => None,
        }
    }
}

pub trait MaxConcurrentPackets: StaticByteRepr + Copy {
    type RingBuffer<T: Debug>: RingBufferApi<T> + Debug + Default;
    type Array<T: Debug>: std::borrow::BorrowMut<[T]> + Debug;

    const UNSET: Self;

    fn new_array<T>(t: T) -> Self::Array<T>
    where
        T: Debug + Copy;
    fn index_array_by_ringbuffer_index<T: Debug>(array: &mut Self::Array<T>, index: u16) -> &mut T;
    fn set_ack_flag(&mut self, packet_index: u16);
    fn iter_acknowledged(self) -> impl Iterator<Item = u16>;
}

macro_rules! impl_max_concurrent_packets {
    ($ty:ty, $size:expr) => {
        impl MaxConcurrentPackets for $ty {
            type RingBuffer<T: Debug> = RingBuffer<T, $size>;
            type Array<T: Debug> = [T; $size];

            const UNSET: Self = 0;

            fn new_array<T>(t: T) -> Self::Array<T>
            where
                T: Debug + Copy,
            {
                [t; _]
            }
            fn index_array_by_ringbuffer_index<T: Debug>(
                array: &mut Self::Array<T>,
                index: u16,
            ) -> &mut T {
                &mut array[(index % $size) as usize]
            }
            #[inline(always)]
            fn set_ack_flag(&mut self, packet_index: u16) {
                *self |= 1 << packet_index as $ty;
            }
            #[inline(always)]
            fn iter_acknowledged(self) -> impl Iterator<Item = u16> {
                (0..$size).filter(move |i| self & 1 << i != 0)
            }
        }
    };
}

impl_max_concurrent_packets!(u8, 8);
impl_max_concurrent_packets!(u16, 16);
impl_max_concurrent_packets!(u32, 32);
impl_max_concurrent_packets!(u64, 64);
impl_max_concurrent_packets!(u128, 128);

#[cfg(test)]
pub mod test {
    use crate::prelude2::*;

    #[derive(ByteRepr, Debug, PartialEq, Hash, Eq, Clone, Copy)]
    pub enum InnerUdpMessage {
        Hello,
        Wave(u16),
    }

    // #[test]
    // fn packet_byte_repr() {
    //     let packet = Packet {
    //         ack: PacketAck::new::<bool>(0, &RingBuffer::new(), &RingBuffer::new()),
    //         ty: PacketType::ReliableUnordered {
    //             messages: vec![
    //                 InnerUdpMessage::Wave(12),
    //                 InnerUdpMessage::Wave(9284),
    //                 InnerUdpMessage::Hello,
    //             ],
    //         },
    //     };
    //     assert_eq!(PacketAck::MIN_BYTE_LEN, 14);
    //     assert_eq!(
    //         Packet::<InnerUdpMessage, InsecureConnection>::MIN_BYTE_LEN,
    //         15
    //     );
    //     assert_eq!(
    //         Packet::<InnerUdpMessage, InsecureConnection>::MAX_BYTE_LEN,
    //         3019
    //     );
    //     let mut buf = [0; Packet::<InnerUdpMessage, InsecureConnection>::MAX_BYTE_LEN];
    //     assert!(packet.write_to_bytes(&mut buf).is_ok());
    //     assert_eq!(
    //         Packet::<InnerUdpMessage, InsecureConnection>::from_bytes(&buf[..packet.byte_len()])
    //             .unwrap(),
    //         packet
    //     );
    // }
}
