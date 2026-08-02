use crate::prelude::*;

#[derive(Debug, ByteRepr)]
#[cfg_attr(test, derive(PartialEq))]
pub(super) struct PacketAck {
    /// The id of the [`super::Packet`] with which this [`PacketAck`] is send
    pub(super) sequence_id: u16,
    /// The id of the most recent received packet.
    newest_received: u16,
    /// Bitflags indicating which of the previous 31 packets were received
    ack_bits: u32,
}

impl StaticByteRepr for PacketAck {
    const BYTE_LEN: usize = PacketAck::MIN_BYTE_LEN;
}

impl<SEND: ByteRepr, RECV: ByteRepr> InnerUdpCommunicator<SEND, RECV> {
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
    use crate::{packet::InnerUdpMessage, prelude::*};

    #[test]
    fn acknowledge() {
        let (mut com1, mut com2) = crate::sender::test_init::<_, bool>(7300);
        com1.write(InnerUdpMessage::Hello);
        assert_eq!(com1.inner.reliable_send_packets.iter().count(), 0);
        com1.tick().unwrap();
        com1.write(InnerUdpMessage::Hello);
        com1.tick().unwrap();
        com1.write(InnerUdpMessage::Wave(1083));
        com1.tick().unwrap();
        com1.write(InnerUdpMessage::Hello);
        com1.tick().unwrap();
        com1.write(InnerUdpMessage::Wave(56000));
        com1.tick().unwrap();
        assert_eq!(com1.inner.reliable_send_packets.iter().count(), 5);
        assert_eq!(com2.inner.received_packets.iter().count(), 0);
        std::thread::sleep(std::time::Duration::from_millis(350));
        com1.tick().unwrap();
        com2.tick().unwrap();
        assert_eq!(com2.inner.received_packets.iter().count(), 5);
        com1.tick().unwrap();
        assert_eq!(com1.inner.reliable_send_packets.iter().count(), 0);
    }
}
