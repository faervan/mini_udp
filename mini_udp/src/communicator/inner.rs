use std::{collections::VecDeque, fmt::Debug};

use crate::{
    communicator::{CRC, SocketSendAddr},
    prelude::*,
    ring_buffer::{RingBuffer, wrapping_gt},
};

pub(crate) struct InnerUdpCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    pub reliable_send_packets: RingBuffer<(Instant, Packet<SEND>)>,
    pub reliable_ordered_send_packets: RingBuffer<(Instant, Packet<SEND>)>,
    pub reliable_received_packets: RingBuffer<()>,
    pub reliable_ordered_received_packets: RingBuffer<Packet<RECV>>,
    /// The sequence id of the next ordered packet to be read
    pub ordered_read_packet_head: u16,
    pub unreliable_send_packet_id: u16,
    pub unreliable_send_packets: VecDeque<Packet<SEND>>,
    pub reliable_send_queue: VecDeque<SEND>,
    pub reliable_ordered_send_queue: VecDeque<SEND>,
    pub unreliable_send_queue: VecDeque<SEND>,
    pub unordered_recv_queue: VecDeque<RECV>,
    pub ordered_recv_queue: VecDeque<RECV>,
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

impl<SEND: ByteRepr, RECV: ByteRepr> Default for InnerUdpCommunicator<SEND, RECV> {
    fn default() -> Self {
        Self {
            reliable_send_packets: RingBuffer::new(),
            reliable_ordered_send_packets: RingBuffer::new(),
            reliable_received_packets: RingBuffer::new(),
            reliable_ordered_received_packets: RingBuffer::new(),
            ordered_read_packet_head: 0,
            unreliable_send_packet_id: 0,
            unreliable_send_packets: VecDeque::new(),
            reliable_send_queue: VecDeque::new(),
            reliable_ordered_send_queue: VecDeque::new(),
            unreliable_send_queue: VecDeque::new(),
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

impl<SEND: ByteRepr, RECV: ByteRepr> InnerUdpCommunicator<SEND, RECV> {
    /// TODO! Remove where Debug
    pub fn send<Addr>(
        &mut self,
        addr: Addr,
        socket: &mut UdpCommunicatorSocket,
    ) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
        Addr: SocketSendAddr,
    {
        if self.received_packet_duplicate
            && self.reliable_send_queue.is_empty()
            && self.reliable_ordered_send_queue.is_empty()
            && self.unreliable_send_queue.is_empty()
        {
            #[cfg(debug_assertions)]
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
            #[cfg(debug_assertions)]
            if socket.debug_logs {
                debug!("Constructed new hearbeat packet with id(unreliable): #{sequence_id}");
            }
            self.unreliable_send_packets.push_back(packet);
        }
        self.received_packet_duplicate = false;
        self.flush_messages(
            #[cfg(debug_assertions)]
            socket,
        );
        self.send_packets(addr, socket)
    }

    /// TODO! Remove where Debug
    pub fn receive(&mut self, socket: &mut UdpCommunicatorSocket)
    where
        SEND: Debug,
        RECV: Debug,
    {
        while let Ok(n) = socket.socket.recv(&mut socket.data_buffer) {
            #[cfg(debug_assertions)]
            if socket.delay_packet(None, n) {
                continue;
            }
            self.read_packet(n, socket);
        }
        #[cfg(debug_assertions)]
        while let Some((n, _)) = socket.read_delayed() {
            self.read_packet(n, socket);
        }
    }

    /// TODO! Remove where Debug
    pub fn read_packet(&mut self, n: usize, socket: &mut UdpCommunicatorSocket)
    where
        SEND: Debug,
        RECV: Debug,
    {
        #[cfg(debug_assertions)]
        let packet = Packet::<RECV>::from_bytes(&socket.data_buffer[4..n])
            .unwrap()
            .ack
            .sequence_id;
        #[cfg(debug_assertions)]
        // Fake UDP unreliability
        if let Some(p) = socket.corruption_probability
            && rand::random_bool(p)
        {
            let corrupt_num = rand::random_range(0..n * 8);
            if socket.debug_logs {
                debug!("Corrupting {corrupt_num} bits of packet #{packet}",);
            }
            for i in 0..corrupt_num {
                socket.data_buffer[i / 8] ^= 1 << (i % 8);
            }
        }
        let Ok(crc_bytes) = socket.data_buffer[..4].try_into() else {
            return;
        };
        let crc = u32::from_le_bytes(crc_bytes);
        if CRC.checksum(&socket.data_buffer[4..n]) != crc {
            #[cfg(test)]
            warn!("CRC check failed, packet: {packet:#?}");
            return;
        }
        match Packet::<RECV>::from_bytes(&socket.data_buffer[4..n]) {
            Ok(packet) => {
                #[cfg(debug_assertions)]
                // Fake UDP unreliability
                if let Some(p) = socket.drop_probability
                    && rand::random_bool(p)
                {
                    return;
                }
                #[cfg(test)]
                debug!("Receiving packet #{}", packet.ack.sequence_id);
                self.last_seen = Instant::now();
                if !packet.reliable {
                    if packet.messages.is_empty() {
                        // Heartbeat
                        #[cfg(test)]
                        debug!("Received heartbeat packet #{}", packet.ack.sequence_id);
                    } else {
                        self.unordered_recv_queue.extend(packet.messages);
                    }
                    self.acknowledge(&packet.ack);
                    return;
                }

                if (packet.ordered
                    && self
                        .reliable_ordered_received_packets
                        .get(packet.ack.sequence_id)
                        .is_some())
                    || (!packet.ordered
                        && self
                            .reliable_received_packets
                            .get(packet.ack.sequence_id)
                            .is_some())
                {
                    self.received_packet_duplicate = true;
                    #[cfg(test)]
                    debug!(
                        "Received duplicate packet #{} (ordered = {})",
                        packet.ack.sequence_id, packet.ordered
                    );
                    return;
                }
                let newest_index = if packet.ordered {
                    self.reliable_ordered_received_packets.get_newest_index()
                } else {
                    self.reliable_received_packets.get_newest_index()
                };
                if wrapping_gt(newest_index.wrapping_sub(31), packet.ack.sequence_id, 64) {
                    self.received_packet_duplicate = true;
                    #[cfg(test)]
                    debug!(
                        "Received too old packet #{} (ordered = {})",
                        packet.ack.sequence_id, packet.ordered
                    );
                    return;
                }

                #[cfg(test)]
                assert!(
                    if packet.ordered {
                        &mut self.received_ordered_packet_ids
                    } else {
                        &mut self.received_reliable_packet_ids
                    }
                    .insert(packet.ack.sequence_id)
                );

                self.acknowledge(&packet.ack);
                if packet.ordered {
                    self.reliable_ordered_received_packets
                        .insert(packet.ack.sequence_id, packet);
                    for (id, packet) in self.reliable_ordered_received_packets.iter_mut() {
                        if id == self.ordered_read_packet_head {
                            self.ordered_read_packet_head =
                                self.ordered_read_packet_head.wrapping_add(1);
                            self.ordered_recv_queue.extend(packet.messages.drain(..));
                        } else if wrapping_gt(id, self.ordered_read_packet_head, 32) {
                            break;
                        }
                    }
                } else {
                    self.reliable_received_packets
                        .insert(packet.ack.sequence_id, ());
                    self.unordered_recv_queue.extend(packet.messages);
                }
            }
            Err(e) => warn!("Received invalid packet: {e}"),
        }
    }

    pub fn write_heartbeat(&mut self, #[cfg(debug_assertions)] socket: &UdpCommunicatorSocket) {
        let sequence_id = self.unreliable_send_packet_id;
        self.unreliable_send_packet_id = self.unreliable_send_packet_id.wrapping_add(1);
        let packet = Packet::heartbeat(PacketAck::new(
            sequence_id,
            &self.reliable_received_packets,
            &self.reliable_ordered_received_packets,
        ));
        #[cfg(debug_assertions)]
        if socket.debug_logs {
            debug!("Constructed new hearbeat packet with id(unreliable): #{sequence_id}");
        }
        self.unreliable_send_packets.push_back(packet);
    }

    /// TODO! Remove where Debug
    fn flush_messages(&mut self, #[cfg(debug_assertions)] socket: &UdpCommunicatorSocket)
    where
        SEND: Debug,
        RECV: Debug,
    {
        flush_messages(
            #[cfg(debug_assertions)]
            socket,
            &mut self.reliable_send_packets,
            &self.reliable_received_packets,
            &self.reliable_ordered_received_packets,
            &mut self.reliable_send_queue,
            false,
        );
        flush_messages(
            #[cfg(debug_assertions)]
            socket,
            &mut self.reliable_ordered_send_packets,
            &self.reliable_received_packets,
            &self.reliable_ordered_received_packets,
            &mut self.reliable_ordered_send_queue,
            true,
        );
        while !self.unreliable_send_queue.is_empty() {
            let mut available_bytes = MAX_PACKET_DATA_LEN;
            let mut included_msgs = 0;
            for msg in self.unreliable_send_queue.iter() {
                if msg.byte_len() <= available_bytes {
                    available_bytes -= msg.byte_len();
                    included_msgs += 1;
                } else {
                    // TODO! Maybe include other messages here that are small enough, but that
                    // would make message ordering arbitrary
                    break;
                }
            }
            if included_msgs == 0 {
                error!(
                    "Msg {:#?} is too large to fit {} bytes, but the max packet size is {}",
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
                reliable: false,
                ordered: false,
                messages: self.unreliable_send_queue.drain(..included_msgs).collect(),
            };
            #[cfg(debug_assertions)]
            if socket.debug_logs {
                debug!(
                    "Constructed new unreliable packet #{sequence_id} with {included_msgs} messages"
                );
            }
            self.unreliable_send_packets.push_back(packet);
        }
    }

    /// TODO! Remove where Debug
    fn send_packets<Addr>(
        &mut self,
        addr: Addr,
        socket: &mut UdpCommunicatorSocket,
    ) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
        Addr: SocketSendAddr,
    {
        let mut any_send = false;
        for (last_send, packet) in self
            .reliable_send_packets
            .values_mut()
            .chain(self.reliable_ordered_send_packets.values_mut())
        {
            let send_cooldown = if cfg!(test) {
                Duration::from_millis(3)
            } else if packet.ordered {
                socket.reliable_ordered_resend_interval
            } else {
                socket.reliable_unordered_resend_interval
            };
            if last_send.elapsed() > send_cooldown {
                *last_send = Instant::now();
                if let Err(e) = packet.write_to_bytes(&mut socket.data_buffer[4..]) {
                    panic!(
                        "{e}: {:?}\npacket len: {}\npacket max len: {}\ndatabuffer len: {}",
                        *packet,
                        packet.byte_len(),
                        Packet::<SEND>::MAX_BYTE_LEN,
                        socket.data_buffer.len()
                    );
                }
                let crc = CRC.checksum(&socket.data_buffer[4..4 + packet.byte_len()]);
                socket.data_buffer[..4].copy_from_slice(&crc.to_le_bytes());
                if let Err(e) = addr.send(socket, ..4 + packet.byte_len()) {
                    error!("Failed to send packet: {e}");
                } else {
                    any_send = true;
                }
            }
        }
        for packet in self.unreliable_send_packets.drain(..) {
            packet.write_to_bytes(&mut socket.data_buffer[4..])?;
            let crc = CRC.checksum(&socket.data_buffer[4..4 + packet.byte_len()]);
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

fn flush_messages<SEND: ByteRepr + Debug, RECV: ByteRepr>(
    #[cfg(debug_assertions)] socket: &UdpCommunicatorSocket,
    send_packets: &mut RingBuffer<(Instant, Packet<SEND>)>,
    reliable_received: &RingBuffer<()>,
    ordered_received: &RingBuffer<Packet<RECV>>,
    send_queue: &mut VecDeque<SEND>,
    ordered: bool,
) {
    while !send_packets.push_will_override() && !send_queue.is_empty() {
        let mut available_bytes = MAX_PACKET_DATA_LEN;
        let mut included_msgs = 0;
        for msg in send_queue.iter() {
            if msg.byte_len() <= available_bytes {
                available_bytes -= msg.byte_len();
                included_msgs += 1;
            } else {
                // TODO! Maybe include other messages here that are small enough, but that
                // would make message ordering arbitrary
                break;
            }
        }
        if included_msgs == 0 {
            error!(
                "Msg {:#?} is too large to fit {} bytes, but the max packet size is {}",
                send_queue[0],
                send_queue[0].byte_len(),
                MAX_PACKET_DATA_LEN
            );
        }
        let sequence_id = send_packets.get_next_index();
        let packet = Packet {
            // TODO! Creating a new ack for every iteration is pointless, wasted work
            ack: PacketAck::new(sequence_id, reliable_received, ordered_received),
            reliable: true,
            ordered,
            messages: send_queue.drain(..included_msgs).collect(),
        };
        #[cfg(debug_assertions)]
        if socket.debug_logs {
            debug!(
                "Constructed new reliable {} packet #{sequence_id} with {included_msgs} messages",
                if ordered { "ordered" } else { "unordered" }
            );
        }
        send_packets.push((Instant::now() - Duration::from_secs(1), packet));
    }
}
