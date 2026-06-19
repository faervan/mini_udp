use byteable::IntoByteArray;

use crate::{BitRepr, bit_repr::StaticBitRepr, sender::UdpCommunicator};

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(feature = "byteable", derive(byteable::Byteable))]
pub(super) struct PacketAck {
    /// The id of the [`super::Packet`] with which this [`PacketAck`] is send
    pub(super) sequence_id: u16,
    /// The id of the most recent received packet.
    newest_received: u16,
    /// Bitflags indicating which of the previous 31 packets were received
    ack_bits: u32,
}

impl StaticBitRepr for PacketAck {
    const BIT_LEN: usize = PacketAck::BYTE_SIZE;
}

impl<M: BitRepr> UdpCommunicator<M> {
    pub(super) fn acknowledge(&mut self, ack: PacketAck) {
        for i in 0..32 {
            if ack.ack_bits & 1 << i != 0 {
                let index = ack.newest_received.wrapping_sub(i);
                self.reliable_send_packets.take(index);
            }
        }
    }

    pub(super) fn create_ack(&self, sequence_id: u16) -> PacketAck {
        let mut ack_bits = 0;
        let newest_received = self.received_packets.get_newest_index();
        for i in self.received_packets.keys() {
            ack_bits |= 1 << newest_received.wrapping_sub(i) as u32;
        }
        PacketAck {
            sequence_id,
            newest_received,
            ack_bits,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{packet::InnerUdpMessage, sender::UdpCommunicator};

    #[test]
    fn acknowledge() {
        let mut com = UdpCommunicator::<InnerUdpMessage>::default();
        // Those two are overridden immediately
        com.received_packets.push(());
        com.received_packets.insert(3, ());
        //
        com.received_packets.insert(u16::MAX - 30, ());
        com.received_packets.insert(u16::MAX, ());
        com.received_packets.push(());
        let ack = com.create_ack(0);
        assert_eq!(com.received_packets.iter().count(), 3);
        com.acknowledge(ack);
        // TODO! Fix this test
        // assert_eq!(com.received_packets.iter().count(), 0);
    }
}
