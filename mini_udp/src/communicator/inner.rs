use std::collections::VecDeque;

use crate::{
    communicator::SocketSendAddr,
    prelude::*,
    ring_buffer::{RingBuffer, wrapping_gt},
};

pub(crate) struct InnerUdpCommunicator<CTX: MiniUdpContext, PacketHandling: PacketHandler> {
    pub reliable_send_packets: RingBuffer<PendingPacket<CTX>>,
    pub reliable_ordered_send_packets: RingBuffer<PendingPacket<CTX>>,
    pub reliable_received_packets: RingBuffer<()>,
    pub reliable_ordered_received_packets: RingBuffer<Vec<CTX::Recv>>,
    /// TODO!
    pub unreliable_received_fragments: PacketHandling::UnreliableFragmentationHandler,
    /// The sequence id of the next ordered packet to be read
    pub ordered_read_packet_head: u16,
    pub unreliable_send_packet_id: u16,
    pub unreliable_send_packets: VecDeque<Packet<CTX::Send>>,
    pub reliable_send_queue: Vec<CTX::Send>,
    pub reliable_ordered_send_queue: Vec<CTX::Send>,
    pub unreliable_send_queue: Vec<CTX::Send>,
    pub unordered_recv_queue: VecDeque<CTX::Recv>,
    pub ordered_recv_queue: VecDeque<CTX::Recv>,
    /// If this is `true`, a packet has been received more than once, potentially meaning that we
    /// have to send an ack to the other side.
    pub received_packet_duplicate: bool,
    pub last_seen: Instant,
    pub last_send: Instant,
    #[cfg(test)]
    pub received_reliable_packet_ids: std::collections::HashSet<u16>,
    #[cfg(test)]
    pub received_ordered_packet_ids: std::collections::HashSet<u16>,
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> Default
    for InnerUdpCommunicator<CTX, PacketHandling>
{
    fn default() -> Self {
        Self {
            reliable_send_packets: RingBuffer::new(),
            reliable_ordered_send_packets: RingBuffer::new(),
            reliable_received_packets: RingBuffer::new(),
            reliable_ordered_received_packets: RingBuffer::new(),
            unreliable_received_fragments: PacketHandling::UnreliableFragmentationHandler::default(
            ),
            ordered_read_packet_head: 0,
            unreliable_send_packet_id: 0,
            unreliable_send_packets: VecDeque::new(),
            reliable_send_queue: Vec::new(),
            reliable_ordered_send_queue: Vec::new(),
            unreliable_send_queue: Vec::new(),
            unordered_recv_queue: VecDeque::new(),
            ordered_recv_queue: VecDeque::new(),
            received_packet_duplicate: false,
            last_seen: Instant::now(),
            last_send: Instant::now(),
            #[cfg(test)]
            received_reliable_packet_ids: std::collections::HashSet::new(),
            #[cfg(test)]
            received_ordered_packet_ids: std::collections::HashSet::new(),
        }
    }
}

pub(crate) struct PendingPacket<CTX: MiniUdpContext> {
    resend_context: <CTX::ResendStrategy as ResendStrategy>::PacketContext,
    packet: Packet<CTX::Send>,
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> InnerUdpCommunicator<CTX, PacketHandling> {
    /// [`crc::CRC_32_BZIP2`] with `init` set to [`PROTOCOL_VERSION`]
    const CRC_ALGORITHM: crc::Algorithm<u32> = crc::Algorithm {
        width: 32,
        poly: 0x04c11db7,
        init: CTX::PROTOCOL_VERSION,
        refin: false,
        refout: false,
        xorout: 0xffffffff,
        check: 0xfc891918,
        residue: 0xc704dd7b,
    };
    const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&Self::CRC_ALGORITHM);

    pub fn send<Addr>(
        &mut self,
        addr: Addr,
        socket: &mut UdpCommunicatorSocket<CTX>,
    ) -> Result<(), Error>
    where
        Addr: SocketSendAddr,
    {
        if self.received_packet_duplicate
            && self.reliable_send_queue.is_empty()
            && self.reliable_ordered_send_queue.is_empty()
            && self.unreliable_send_queue.is_empty()
        {
            #[cfg(feature = "debug")]
            if socket.debug_logs {
                debug!("Sending heartbeat");
            }
            let sequence_id = self.unreliable_send_packet_id;
            self.unreliable_send_packet_id = self.unreliable_send_packet_id.wrapping_add(1);
            let packet = Packet::heartbeat(PacketAck::new(
                sequence_id,
                &self.reliable_received_packets,
                &self.reliable_ordered_received_packets,
            ));
            #[cfg(feature = "debug")]
            if socket.debug_logs {
                debug!("Constructed new hearbeat packet with id(unreliable): #{sequence_id}");
            }
            self.unreliable_send_packets.push_back(packet);
        }
        self.received_packet_duplicate = false;
        self.flush_messages(socket);
        self.send_packets(addr, socket)
    }

    pub fn receive(&mut self, socket: &mut UdpCommunicatorSocket<CTX>) {
        while let Ok(n) = socket.socket.recv(&mut socket.data_buffer) {
            #[cfg(any(test, feature = "debug"))]
            if socket.delay_packet(None, n) {
                continue;
            }
            if let Err(error) = self.read_packet(n, socket) {
                CTX::ErrorHandling::handle_error(&mut socket.error_handler, error);
            }
        }
        #[cfg(any(test, feature = "debug"))]
        while let Some((n, _)) = socket.read_delayed() {
            if let Err(error) = self.read_packet(n, socket) {
                CTX::ErrorHandling::handle_error(&mut socket.error_handler, error);
            }
        }
    }

    pub fn read_packet(
        &mut self,
        n: usize,
        socket: &mut UdpCommunicatorSocket<CTX>,
    ) -> Result<(), Error> {
        #[cfg(test)]
        let packet = Packet::<CTX::Recv>::from_bytes(&socket.data_buffer[4..n])
            .unwrap()
            .ack
            .sequence_id;
        #[cfg(feature = "debug")]
        // Fake UDP unreliability
        if let Some(p) = socket.corruption_probability
            && rand::random_bool(p)
        {
            let corrupt_num = rand::random_range(0..n * 8);
            if socket.debug_logs {
                #[cfg(test)]
                debug!("Corrupting {corrupt_num} bits of packet #{packet}");
                #[cfg(not(test))]
                debug!("Corrupting {corrupt_num} bits of received packet");
            }
            for i in 0..corrupt_num {
                socket.data_buffer[i / 8] ^= 1 << (i % 8);
            }
        }
        let Ok(crc_bytes) = socket.data_buffer[..4].try_into() else {
            return Err(Error::PacketLengthLessThanCrcBytes);
        };
        let crc = u32::from_le_bytes(crc_bytes);
        if Self::CRC.checksum(&socket.data_buffer[4..n]) != crc {
            #[cfg(test)]
            warn!("CRC check failed, packet: {packet:#?}");
            return Err(Error::CrcFailed);
        }
        match Packet::<CTX::Recv>::from_bytes(&socket.data_buffer[4..n]) {
            Ok(packet) => {
                #[cfg(feature = "debug")]
                // Fake UDP unreliability
                if let Some(p) = socket.drop_probability
                    && rand::random_bool(p)
                {
                    return Ok(());
                }
                #[cfg(test)]
                debug!("Receiving packet #{}", packet.ack.sequence_id);
                #[cfg(test)]
                let ty_name = <&str>::from(&packet.ty);

                self.last_seen = Instant::now();
                match packet.ty {
                    PacketType::Heartbeat => {
                        #[cfg(test)]
                        debug!("Received heartbeat packet #{}", packet.ack.sequence_id);
                        self.acknowledge(&packet.ack);
                    }
                    PacketType::Unreliable { messages } => {
                        self.unordered_recv_queue.extend(messages);
                        self.acknowledge(&packet.ack);
                    }
                    PacketType::UnreliableFragment { .. } => todo!(),
                    PacketType::ReliableOrdered { messages } => {
                        if self
                            .reliable_ordered_received_packets
                            .get(packet.ack.sequence_id)
                            .is_some()
                        {
                            self.received_packet_duplicate = true;
                            #[cfg(test)]
                            debug!(
                                "Received duplicate packet #{} (ty = {ty_name})",
                                packet.ack.sequence_id,
                            );
                            return Ok(());
                        }

                        let newest_index =
                            self.reliable_ordered_received_packets.get_newest_index();
                        if wrapping_gt(newest_index.wrapping_sub(31), packet.ack.sequence_id, 64) {
                            self.received_packet_duplicate = true;
                            return Err(Error::PacketTooOld {
                                sequence_id: packet.ack.sequence_id,
                                newest_id: newest_index,
                            });
                        }

                        #[cfg(test)]
                        assert!(
                            self.received_ordered_packet_ids
                                .insert(packet.ack.sequence_id)
                        );

                        self.acknowledge(&packet.ack);

                        self.reliable_ordered_received_packets
                            .insert(packet.ack.sequence_id, messages);
                        for (id, messages) in self.reliable_ordered_received_packets.iter_mut() {
                            if id == self.ordered_read_packet_head {
                                self.ordered_read_packet_head =
                                    self.ordered_read_packet_head.wrapping_add(1);
                                self.ordered_recv_queue.extend(messages.drain(..));
                            } else if wrapping_gt(id, self.ordered_read_packet_head, 32) {
                                break;
                            }
                        }
                    }
                    PacketType::ReliableUnordered { messages } => {
                        if self
                            .reliable_received_packets
                            .get(packet.ack.sequence_id)
                            .is_some()
                        {
                            self.received_packet_duplicate = true;
                            #[cfg(test)]
                            debug!(
                                "Received duplicate packet #{} (ty = {ty_name})",
                                packet.ack.sequence_id,
                            );
                            return Ok(());
                        }

                        let newest_index = self.reliable_received_packets.get_newest_index();
                        if wrapping_gt(newest_index.wrapping_sub(31), packet.ack.sequence_id, 64) {
                            self.received_packet_duplicate = true;
                            return Err(Error::PacketTooOld {
                                sequence_id: packet.ack.sequence_id,
                                newest_id: newest_index,
                            });
                        }

                        #[cfg(test)]
                        assert!(
                            self.received_reliable_packet_ids
                                .insert(packet.ack.sequence_id)
                        );

                        self.acknowledge(&packet.ack);

                        self.reliable_received_packets
                            .insert(packet.ack.sequence_id, ());
                        self.unordered_recv_queue.extend(messages);
                    }
                    PacketType::ReliableOrderedFragment { .. } => todo!(),
                }
                Ok(())
            }
            Err(e) => {
                #[cfg(any(test, feature = "debug"))]
                warn!("Received invalid packet: {e}");
                Err(Error::ByteRepr(e))
            }
        }
    }

    pub fn write_heartbeat(
        &mut self,
        #[cfg(any(test, feature = "debug"))] socket: &UdpCommunicatorSocket<CTX>,
    ) {
        let sequence_id = self.unreliable_send_packet_id;
        self.unreliable_send_packet_id = self.unreliable_send_packet_id.wrapping_add(1);
        let packet = Packet::heartbeat(PacketAck::new(
            sequence_id,
            &self.reliable_received_packets,
            &self.reliable_ordered_received_packets,
        ));
        #[cfg(any(test, feature = "debug"))]
        if socket.debug_logs {
            debug!("Constructed new hearbeat packet with id(unreliable): #{sequence_id}");
        }
        self.unreliable_send_packets.push_back(packet);
    }

    fn flush_messages(&mut self, socket: &mut UdpCommunicatorSocket<CTX>) {
        flush_messages::<false, _>(
            socket,
            &mut self.reliable_send_packets,
            &self.reliable_received_packets,
            &self.reliable_ordered_received_packets,
            &mut self.reliable_send_queue,
        );
        flush_messages::<true, _>(
            socket,
            &mut self.reliable_ordered_send_packets,
            &self.reliable_received_packets,
            &self.reliable_ordered_received_packets,
            &mut self.reliable_ordered_send_queue,
        );
        while !self.unreliable_send_queue.is_empty() {
            let mut available_bytes = MAX_PACKET_DATA_LEN;
            let mut included_msgs = 0;
            for msg in self.unreliable_send_queue.iter() {
                if msg.byte_len() <= available_bytes {
                    available_bytes -= msg.byte_len();
                    included_msgs += 1;
                } else {
                    // TODO! Maybe include other messages here that are small enough
                    break;
                }
            }
            if included_msgs == 0 {
                panic!(
                    "Msg {:#?} is too large (byte_len = {}), the max packet size is {}",
                    self.unreliable_send_queue[0],
                    self.unreliable_send_queue[0].byte_len(),
                    MAX_PACKET_DATA_LEN
                );
            }
            let sequence_id = self.unreliable_send_packet_id;
            self.unreliable_send_packet_id = self.unreliable_send_packet_id.wrapping_add(1);
            let packet = Packet {
                ack: PacketAck::new(
                    sequence_id,
                    &self.reliable_received_packets,
                    &self.reliable_ordered_received_packets,
                ),
                ty: PacketType::Unreliable {
                    messages: self.unreliable_send_queue.drain(..included_msgs).collect(),
                },
            };
            #[cfg(feature = "debug")]
            if socket.debug_logs {
                debug!(
                    "Constructed new unreliable packet #{sequence_id} with {included_msgs} messages"
                );
            }
            self.unreliable_send_packets.push_back(packet);
        }
    }

    fn send_packets<Addr>(
        &mut self,
        addr: Addr,
        socket: &mut UdpCommunicatorSocket<CTX>,
    ) -> Result<(), Error>
    where
        Addr: SocketSendAddr,
    {
        let mut any_send = false;

        self.reliable_send_packets.retain(|_, packet| {
            !resend_if_needed::<false, _, _>(packet, socket, addr, Self::CRC, &mut any_send)
        });
        self.reliable_ordered_send_packets.retain(|_, packet| {
            !resend_if_needed::<true, _, _>(packet, socket, addr, Self::CRC, &mut any_send)
        });

        for packet in self.unreliable_send_packets.drain(..) {
            packet.write_to_bytes(&mut socket.data_buffer[4..])?;
            let crc = Self::CRC.checksum(&socket.data_buffer[4..4 + packet.byte_len()]);
            socket.data_buffer[..4].copy_from_slice(&crc.to_le_bytes());
            if let Err(e) = addr.send(socket, ..4 + packet.byte_len()) {
                error!("Failed to send packet: {e}");
            } else {
                any_send = true;
            }
        }

        if any_send {
            self.last_send = Instant::now();
        }

        Ok(())
    }

    pub fn has_work(&self) -> bool {
        !(self.reliable_send_packets.is_empty()
            && self.reliable_ordered_send_packets.is_empty()
            && self.unreliable_send_packets.is_empty()
            && self.reliable_send_queue.is_empty()
            && self.reliable_ordered_send_queue.is_empty()
            && self.unreliable_send_queue.is_empty())
    }
}

fn flush_messages<const ORDERED: bool, CTX: MiniUdpContext>(
    socket: &mut UdpCommunicatorSocket<CTX>,
    send_packets: &mut RingBuffer<PendingPacket<CTX>>,
    reliable_received: &RingBuffer<()>,
    ordered_received: &RingBuffer<Vec<CTX::Recv>>,
    send_queue: &mut Vec<CTX::Send>,
) {
    while !send_packets.push_will_override() && !send_queue.is_empty() {
        let mut available_bytes = MAX_PACKET_DATA_LEN;
        let mut included_msgs = 0;
        for msg in send_queue.iter() {
            if msg.byte_len() <= available_bytes {
                available_bytes -= msg.byte_len();
                included_msgs += 1;
            } else {
                // TODO! Include other messages here that are small enough, if the packet is not
                // ordered.
                break;
            }
        }
        if included_msgs == 0 {
            panic!(
                "Msg {:#?} is too large (byte_len = {}), the max packet size is {}",
                send_queue[0],
                send_queue[0].byte_len(),
                MAX_PACKET_DATA_LEN
            );
        }
        let sequence_id = send_packets.get_next_index();
        let messages = send_queue.drain(..included_msgs).collect();
        let packet = Packet {
            // TODO! Creating a new ack for every iteration is pointless, wasted work if there are
            // many packets to be send
            ack: PacketAck::new(sequence_id, reliable_received, ordered_received),
            ty: match ORDERED {
                true => PacketType::ReliableOrdered { messages },
                false => PacketType::ReliableUnordered { messages },
            },
        };
        #[cfg(any(test, feature = "debug"))]
        if socket.debug_logs {
            debug!(
                "Constructed new reliable {} packet #{sequence_id} with {included_msgs} messages",
                if ORDERED { "ordered" } else { "unordered" }
            );
        }
        send_packets.push(PendingPacket {
            resend_context: socket.resend_handler.new_packet(
                packet
                    .get_reliable_kind()
                    .expect("all packets here are reliable"),
            ),
            packet,
        });
    }
}

/// Returns `true` if the packet should not be resend again
fn resend_if_needed<const ORDERED: bool, CTX, ADDR>(
    PendingPacket {
        resend_context,
        packet,
    }: &mut PendingPacket<CTX>,
    socket: &mut UdpCommunicatorSocket<CTX>,
    addr: ADDR,
    crc: crc::Crc<u32>,
    any_send: &mut bool,
) -> bool
where
    CTX: MiniUdpContext,
    ADDR: SocketSendAddr,
{
    let drop_packet = match socket.resend_handler.resend(resend_context) {
        ResendAction::Resend => false,
        ResendAction::ResendThenDrop => true,
        ResendAction::DoNotResend => return false,
    };
    if let Err(error) = packet.write_to_bytes(&mut socket.data_buffer[4..]) {
        socket
            .resend_handler
            .handle_send_error::<CTX::ErrorHandling>(
                resend_context,
                Error::ByteRepr(error),
                &mut socket.error_handler,
            );
        return drop_packet;
    }
    let crc = crc.checksum(&socket.data_buffer[4..4 + packet.byte_len()]);
    socket.data_buffer[..4].copy_from_slice(&crc.to_le_bytes());
    if let Err(error) = addr.send(socket, ..4 + packet.byte_len()) {
        socket
            .resend_handler
            .handle_send_error::<CTX::ErrorHandling>(
                resend_context,
                error,
                &mut socket.error_handler,
            );
        return drop_packet;
    }
    *any_send = true;
    drop_packet
}
