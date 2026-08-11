use crate::{packet::Packet, prelude::*, ring_buffer::RingBuffer};

#[derive(Debug, ByteRepr)]
#[cfg_attr(test, derive(PartialEq))]
pub(super) struct PacketAck {
    /// The id of the [`super::packet::Packet`] with which this [`PacketAck`] is send
    pub(super) sequence_id: u16,
    /// The id of the most recent reliable received packet.
    reliable_newest_received: u16,
    /// Bitflags indicating which of the previous 31 reliable packets were received
    reliable_ack_bits: u32,
    /// The id of the most recent reliable ordered received packet.
    ordered_newest_received: u16,
    /// Bitflags indicating which of the previous 31 reliable ordered packets were received
    ordered_ack_bits: u32,
}

impl<SEND: ByteRepr, RECV: ByteRepr, const PROTOCOL_VERSION: u32>
    InnerUdpCommunicator<SEND, RECV, PROTOCOL_VERSION>
{
    pub(super) fn acknowledge(&mut self, ack: &PacketAck) {
        for i in 0..32 {
            if ack.reliable_ack_bits & 1 << i != 0 {
                let index = ack.reliable_newest_received.wrapping_sub(i);
                self.reliable_send_packets.take(index);
            }
            if ack.ordered_ack_bits & 1 << i != 0 {
                let index = ack.ordered_newest_received.wrapping_sub(i);
                self.reliable_ordered_send_packets.take(index);
            }
        }
    }
}

impl PacketAck {
    pub(super) fn new<RECV: ByteRepr>(
        sequence_id: u16,
        reliable_received: &RingBuffer<()>,
        ordered_received: &RingBuffer<Packet<RECV>>,
    ) -> Self {
        let mut reliable_ack_bits = 0;
        let reliable_newest_received = reliable_received.get_newest_index();
        for i in reliable_received.keys() {
            reliable_ack_bits |= 1 << reliable_newest_received.wrapping_sub(i) as u32;
        }

        let mut ordered_ack_bits = 0;
        let ordered_newest_received = ordered_received.get_newest_index();
        for i in ordered_received.keys() {
            ordered_ack_bits |= 1 << ordered_newest_received.wrapping_sub(i) as u32;
        }

        PacketAck {
            sequence_id,
            reliable_newest_received,
            reliable_ack_bits,
            ordered_newest_received,
            ordered_ack_bits,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{packet::test::InnerUdpMessage, prelude::*};

    #[test]
    fn acknowledge() {
        let (mut com1, mut com2) = crate::communicator::test_init::<_, bool>(7300);
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
