use crate::prelude2::*;

#[derive(Debug, ByteRepr, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub struct PacketAck<UnorderedBits: MaxConcurrentPackets, OrderedBits: MaxConcurrentPackets> {
    /// The id of the most recent reliable received packet.
    reliable_newest_received: u16,
    /// Bitflags indicating which of the previous `max_packets - 1` reliable packets were received.
    reliable_ack_bits: UnorderedBits,
    /// The id of the most recent reliable ordered received packet.
    ordered_newest_received: u16,
    /// Bitflags indicating which of the previous `max_packets - 1` reliable ordered packets were
    /// received.
    ordered_ack_bits: OrderedBits,
}

impl<Config: MiniUdpConfig> InnerCommunicator<Config> {
    pub(crate) fn create_ack(
        &self,
    ) -> PacketAck<
        <Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets,
        <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets,
    > {
        // TODO! Create ack for unordered
        let (reliable_newest_received, reliable_ack_bits) = (0, <Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets::UNSET);
        let (ordered_newest_received, ordered_ack_bits) = self.reliable_ordered.get_ack();
        PacketAck {
            reliable_newest_received,
            reliable_ack_bits,
            ordered_newest_received,
            ordered_ack_bits,
        }
    }

    pub(crate) fn acknowledge(
        &mut self,
        ack: PacketAck<
            <Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets,
            <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets,
        >,
    ) {
        // TODO! ack reliable unordered
        self.reliable_ordered
            .acknowledge(ack.ordered_newest_received, ack.ordered_ack_bits);
    }
}

impl<UnorderedBits: MaxConcurrentPackets, OrderedBits: MaxConcurrentPackets>
    PacketAck<UnorderedBits, OrderedBits>
{
    pub fn from_reliable_ordered_ack(ack: PartialPacketAck<OrderedBits>) -> Self {
        Self {
            reliable_newest_received: 0,
            reliable_ack_bits: UnorderedBits::UNSET,
            ordered_newest_received: ack.newest_received,
            ordered_ack_bits: ack.ack_bits,
        }
    }
}

pub struct PartialPacketAck<MaxPackets: MaxConcurrentPackets> {
    pub newest_received: u16,
    pub ack_bits: MaxPackets,
}

impl<MaxPackets: MaxConcurrentPackets> PartialPacketAck<MaxPackets> {
    #[inline(always)]
    pub fn new((newest_received, ack_bits): (u16, MaxPackets)) -> Self {
        Self {
            newest_received,
            ack_bits,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{packet::test::InnerUdpMessage, prelude::*};

    #[test]
    fn acknowledge() {
        let (mut com1, mut com2) = crate::communicator::test_init::<UdpContext<_, bool, 1>>(7302);
        com1.write_reliable(InnerUdpMessage::Hello);
        assert_eq!(com1.inner.reliable_send_packets.iter().count(), 0);
        com1.tick().unwrap();
        com1.write_reliable(InnerUdpMessage::Hello);
        com1.tick().unwrap();
        com1.write_reliable(InnerUdpMessage::Wave(1083));
        com1.tick().unwrap();
        com1.write_reliable(InnerUdpMessage::Hello);
        com1.tick().unwrap();
        com1.write_reliable(InnerUdpMessage::Wave(56000));
        com1.tick().unwrap();
        assert_eq!(com1.inner.reliable_send_packets.iter().count(), 5);
        assert_eq!(com2.inner.reliable_received_packets.iter().count(), 0);
        std::thread::sleep(Duration::from_millis(350));
        com1.tick().unwrap();
        com2.tick().unwrap();
        assert_eq!(com2.inner.reliable_received_packets.iter().count(), 5);
        com1.tick().unwrap();
        assert_eq!(com1.inner.reliable_send_packets.iter().count(), 0);
    }
}
