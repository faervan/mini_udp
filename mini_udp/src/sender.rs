use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    ops::Deref,
    time::{Duration, Instant},
};

use crate::{
    packet::{MAX_PACKET_DATA_LEN, MAX_PACKET_LEN, Packet},
    prelude::*,
    ring_buffer::RingBuffer,
};

pub trait CommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self;
    fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error>;
}

pub trait Communicator<M: ByteRepr> {
    fn write(&mut self, message: M);
    fn read(&mut self) -> Option<M>;
    /// TODO! Remove where Debug
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        M: Debug;
    /// Returns `true` if there are any pending messages to be send / packets to get acknowledged.
    fn has_work(&self) -> bool;
}

pub struct MultiUdpCommunicator<M: ByteRepr> {
    socket: UdpCommunicatorSocket,
    coms: HashMap<SocketAddr, InnerUdpCommunicator<M>>,
}

pub struct UdpCommunicator<M: ByteRepr> {
    socket: UdpCommunicatorSocket,
    pub(super) inner: InnerUdpCommunicator<M>,
}

pub struct UdpCommunicatorSocket {
    socket: UdpSocket,
    data_buffer: [u8; MAX_PACKET_LEN],
    #[cfg(test)]
    fake_unreliable: bool,
    #[cfg(test)]
    debug_logs: bool,
}

impl Deref for UdpCommunicatorSocket {
    type Target = UdpSocket;
    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

pub(crate) struct InnerUdpCommunicator<M: ByteRepr> {
    pub(crate) reliable_send_packets: RingBuffer<(Instant, Packet<M>)>,
    unreliable_send_packet_id: u16,
    unreliable_send_packets: VecDeque<Packet<M>>,
    pub(crate) received_packets: RingBuffer<()>,
    msg_send_queue: VecDeque<M>,
    msg_recv_queue: VecDeque<M>,
    /// If this is `true`, a packet has been received more than once, potentially meaning that we
    /// have to send an ack to the other side.
    received_packet_duplicate: bool,
}

impl<M: ByteRepr> Default for InnerUdpCommunicator<M> {
    fn default() -> Self {
        Self {
            reliable_send_packets: RingBuffer::new(),
            unreliable_send_packet_id: 0,
            unreliable_send_packets: VecDeque::new(),
            received_packets: RingBuffer::new(),
            msg_send_queue: VecDeque::new(),
            msg_recv_queue: VecDeque::new(),
            received_packet_duplicate: false,
        }
    }
}

impl<M: ByteRepr> Default for UdpCommunicator<M> {
    fn default() -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind("0.0.0.0:0"),
            inner: InnerUdpCommunicator::default(),
        }
    }
}

impl CommunicatorSocket for UdpCommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        let socket = UdpSocket::bind(addr).expect("Failed to bind to udp socket");
        socket
            .set_nonblocking(true)
            .expect("Failed to set udp socket to nonblocking mode");
        Self {
            socket,
            data_buffer: [0; MAX_PACKET_LEN],
            #[cfg(test)]
            fake_unreliable: false,
            #[cfg(test)]
            debug_logs: false,
        }
    }

    fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }
}

impl<M: ByteRepr> CommunicatorSocket for UdpCommunicator<M> {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind(addr),
            inner: InnerUdpCommunicator::default(),
        }
    }

    fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }
}

impl<M: ByteRepr> CommunicatorSocket for MultiUdpCommunicator<M> {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind(addr),
            coms: HashMap::new(),
        }
    }

    fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }
}

impl<M: ByteRepr> Communicator<M> for UdpCommunicator<M> {
    #[inline(always)]
    fn write(&mut self, message: M) {
        self.inner.msg_send_queue.push_back(message);
    }

    #[inline(always)]
    fn read(&mut self) -> Option<M> {
        self.inner.msg_recv_queue.pop_front()
    }

    #[inline(always)]
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        M: Debug,
    {
        self.inner.tick(&mut self.socket)
    }

    #[inline(always)]
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }
}

impl<M: ByteRepr> InnerUdpCommunicator<M> {
    /// TODO! Remove where Debug
    fn tick(&mut self, socket: &mut UdpCommunicatorSocket) -> Result<(), ByteReprError>
    where
        M: Debug,
    {
        self.receive(socket);
        if self.received_packet_duplicate && self.msg_send_queue.is_empty() {
            self.received_packet_duplicate = false;
            let sequence_id = self.unreliable_send_packet_id;
            self.unreliable_send_packet_id = self.unreliable_send_packet_id.wrapping_add(1);
            let packet = Packet::heartbeat(self.create_ack(sequence_id));
            #[cfg(test)]
            if socket.debug_logs {
                debug!("Constructed new hearbeat packet #{sequence_id}");
            }
            self.unreliable_send_packets.push_back(packet);
        }
        self.flush_messages(
            #[cfg(test)]
            socket,
        );
        self.send_packets(socket)
    }

    /// TODO! Remove where Debug
    fn receive(&mut self, socket: &mut UdpCommunicatorSocket)
    where
        M: Debug,
    {
        while let Ok(n) = socket.socket.recv(&mut socket.data_buffer) {
            #[cfg(test)]
            // Fake UDP unreliability
            if socket.fake_unreliable && rand::random_bool(0.2) {
                let corrupt_num = rand::random_range(0..n * 8);
                debug!("Corrupting {corrupt_num} bits");
                for i in 0..corrupt_num {
                    socket.data_buffer[i / 8] ^= 1 << i % 8;
                }
            }
            match Packet::<M>::from_bytes(&socket.data_buffer[..n]) {
                Ok(packet) => {
                    #[cfg(test)]
                    // Fake UDP unreliability
                    if socket.fake_unreliable && rand::random_bool(0.5) {
                        continue;
                    }
                    if !packet.reliable {
                        if packet.messages.is_empty() {
                            // Heartbeat
                            #[cfg(test)]
                            debug!("Received heartbeat packet #{}", packet.ack.sequence_id);
                        } else {
                            self.msg_recv_queue.extend(packet.messages);
                        }
                        self.acknowledge(packet.ack);
                        continue;
                    }

                    if self.received_packets.get(packet.ack.sequence_id).is_some() {
                        self.received_packet_duplicate = true;
                        #[cfg(test)]
                        debug!("Received duplicate packet #{}", packet.ack.sequence_id);
                        continue;
                    }
                    if super::ring_buffer::wrapping_gt(
                        self.received_packets.get_newest_index().wrapping_sub(31),
                        packet.ack.sequence_id,
                        64,
                    ) {
                        self.received_packet_duplicate = true;
                        #[cfg(test)]
                        debug!("Received too old packet #{}", packet.ack.sequence_id);
                        continue;
                    }
                    self.received_packets.insert(packet.ack.sequence_id, ());
                    self.msg_recv_queue.extend(packet.messages);
                    self.acknowledge(packet.ack);
                }
                Err(e) => warn!("Received invalid packet: {e}"),
            }
        }
    }

    /// TODO! Remove where Debug
    fn flush_messages(&mut self, #[cfg(test)] socket: &UdpCommunicatorSocket)
    where
        M: Debug,
    {
        while !self.reliable_send_packets.push_will_override() && !self.msg_send_queue.is_empty() {
            let mut available_bytes = MAX_PACKET_DATA_LEN;
            let mut included_msgs = 0;
            for msg in self.msg_send_queue.iter() {
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
                    self.msg_send_queue[0],
                    self.msg_send_queue[0].byte_len(),
                    MAX_PACKET_DATA_LEN
                );
            }
            let sequence_id = self.reliable_send_packets.get_next_index();
            let packet = Packet {
                ack: self.create_ack(sequence_id),
                reliable: true,
                ordered: true,
                messages: self.msg_send_queue.drain(..included_msgs).collect(),
            };
            #[cfg(test)]
            if socket.debug_logs {
                debug!("Constructed new packet #{sequence_id} with {included_msgs} messages");
            }
            self.reliable_send_packets
                .push((Instant::now() - Duration::from_secs(1), packet));
        }
    }

    /// TODO! Remove where Debug
    fn send_packets(&mut self, socket: &mut UdpCommunicatorSocket) -> Result<(), ByteReprError>
    where
        M: Debug,
    {
        for (last_send, packet) in self.reliable_send_packets.iter_mut() {
            let send_cooldown = if cfg!(test) {
                Duration::from_millis(3)
            } else {
                Duration::from_millis(100)
            };
            if last_send.elapsed() > send_cooldown {
                *last_send = Instant::now();
                if let Err(e) = packet.write_to_bytes(&mut socket.data_buffer) {
                    panic!(
                        "{e}: {:#?}\npacket len: {}\npacket max len: {}\ndatabuffer len: {}",
                        *packet,
                        packet.byte_len(),
                        Packet::<M>::MAX_BYTE_LEN,
                        socket.data_buffer.len()
                    );
                }
                if let Err(e) = socket.socket.send(&socket.data_buffer[..packet.byte_len()]) {
                    error!("Failed to send packet: {e}");
                }
            }
        }
        for packet in self.unreliable_send_packets.drain(..) {
            packet.write_to_bytes(&mut socket.data_buffer)?;
            if let Err(e) = socket.socket.send(&socket.data_buffer[..packet.byte_len()]) {
                error!("Failed to send packet: {e}");
            }
        }

        Ok(())
    }

    pub fn has_work(&self) -> bool {
        !(self.reliable_send_packets.is_empty()
            && self.unreliable_send_packets.is_empty()
            && self.msg_send_queue.is_empty())
    }
}

impl UdpCommunicatorSocket {
    #[cfg(test)]
    fn with_fake_unreliablity(mut self) -> Self {
        self.fake_unreliable = true;
        self
    }

    #[cfg(test)]
    fn with_debug_logs(mut self) -> Self {
        self.debug_logs = true;
        self
    }
}
