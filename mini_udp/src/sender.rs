use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    time::{Duration, Instant},
};

use crate::{
    packet::{MAX_PACKET_DATA_LEN, MAX_PACKET_LEN, Packet},
    prelude::*,
    ring_buffer::{RingBuffer, wrapping_gt},
};

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

pub trait CommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self;
    fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error>;
    fn send<Addr: SocketSendAddr>(
        &mut self,
        addr: &Addr,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error>;
}

pub trait SocketSendAddr {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error>;
}

pub trait Communicator<SEND: ByteRepr, RECV: ByteRepr> {
    fn write(&mut self, message: SEND);
    fn write_reliable(&mut self, message: SEND);
    fn write_ordered(&mut self, message: SEND);
    fn read(&mut self) -> Option<RECV>;
    fn read_ordered(&mut self) -> Option<RECV>;
    /// TODO! Remove where Debug
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug;
    /// Returns `true` if there are any pending messages to be send / packets to get acknowledged.
    fn has_work(&self) -> bool;
}

pub trait MultiCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    fn recv<CB>(&mut self, on_recv: CB)
    where
        CB: FnMut(SocketAddr, UdpCommunicatorMut<SEND, RECV>);
    fn broadcast(&mut self, message: SEND)
    where
        SEND: Clone;
    fn breadcast_except(&mut self, message: SEND, exception: SocketAddr)
    where
        SEND: Clone;
    fn tick<CB>(&mut self, on_recv: CB)
    where
        CB: FnMut(SocketAddr, UdpCommunicatorMut<SEND, RECV>, DelayedBroadcast<SEND>),
        SEND: Debug + Clone,
        RECV: Debug;
    fn get_mut<'a>(
        &'a mut self,
        client: &'a SocketAddr,
    ) -> Option<UdpCommunicatorMut<'a, SEND, RECV>>;
}

pub struct MultiUdpCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    socket: UdpCommunicatorSocket,
    coms: HashMap<SocketAddr, InnerUdpCommunicator<SEND, RECV>>,
}

pub struct UdpCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    socket: UdpCommunicatorSocket,
    pub(super) inner: InnerUdpCommunicator<SEND, RECV>,
}

pub struct UdpCommunicatorMut<'a, SEND: ByteRepr, RECV: ByteRepr> {
    socket: &'a mut UdpCommunicatorSocket,
    addr: &'a SocketAddr,
    inner: &'a mut InnerUdpCommunicator<SEND, RECV>,
}

pub struct UdpCommunicatorSocket {
    socket: UdpSocket,
    data_buffer: [u8; MAX_PACKET_LEN],
    #[cfg(debug_assertions)]
    fake_unreliable: bool,
    #[cfg(debug_assertions)]
    debug_logs: bool,
}

pub(crate) struct InnerUdpCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    pub(crate) reliable_send_packets: RingBuffer<(Instant, Packet<SEND>)>,
    pub(crate) reliable_ordered_send_packets: RingBuffer<(Instant, Packet<SEND>)>,
    pub(crate) reliable_received_packets: RingBuffer<()>,
    pub(crate) reliable_ordered_received_packets: RingBuffer<Packet<RECV>>,
    /// The sequence id of the next ordered packet to be read
    ordered_read_packet_head: u16,
    unreliable_send_packet_id: u16,
    unreliable_send_packets: VecDeque<Packet<SEND>>,
    reliable_send_queue: VecDeque<SEND>,
    reliable_ordered_send_queue: VecDeque<SEND>,
    unreliable_send_queue: VecDeque<SEND>,
    unordered_recv_queue: VecDeque<RECV>,
    ordered_recv_queue: VecDeque<RECV>,
    /// If this is `true`, a packet has been received more than once, potentially meaning that we
    /// have to send an ack to the other side.
    received_packet_duplicate: bool,
    #[cfg(test)]
    received_reliable_packet_ids: std::collections::HashSet<u16>,
    #[cfg(test)]
    received_ordered_packet_ids: std::collections::HashSet<u16>,
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
            #[cfg(test)]
            received_reliable_packet_ids: std::collections::HashSet::new(),
            #[cfg(test)]
            received_ordered_packet_ids: std::collections::HashSet::new(),
        }
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> Default for UdpCommunicator<SEND, RECV> {
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
            #[cfg(debug_assertions)]
            fake_unreliable: false,
            #[cfg(debug_assertions)]
            debug_logs: false,
        }
    }

    fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }

    fn send<Addr: SocketSendAddr>(
        &mut self,
        addr: &Addr,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        addr.send(self, slice_index)
    }
}

impl SocketSendAddr for () {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        socket.socket.send(&socket.data_buffer[slice_index])
    }
}

impl SocketSendAddr for SocketAddr {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        socket
            .socket
            .send_to(&socket.data_buffer[slice_index], self)
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> CommunicatorSocket for UdpCommunicator<SEND, RECV> {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind(addr),
            inner: InnerUdpCommunicator::default(),
        }
    }

    fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }

    fn send<Addr: SocketSendAddr>(
        &mut self,
        addr: &Addr,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        self.socket.send(addr, slice_index)
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> CommunicatorSocket for MultiUdpCommunicator<SEND, RECV> {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind(addr),
            coms: HashMap::new(),
        }
    }

    fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }

    fn send<Addr: SocketSendAddr>(
        &mut self,
        addr: &Addr,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        self.socket.send(addr, slice_index)
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> Communicator<SEND, RECV> for UdpCommunicator<SEND, RECV> {
    #[inline(always)]
    fn write(&mut self, message: SEND) {
        self.inner.unreliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_reliable(&mut self, message: SEND) {
        self.inner.reliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_ordered(&mut self, message: SEND) {
        self.inner.reliable_ordered_send_queue.push_back(message);
    }

    #[inline(always)]
    fn read(&mut self) -> Option<RECV> {
        self.inner.unordered_recv_queue.pop_front()
    }

    #[inline(always)]
    fn read_ordered(&mut self) -> Option<RECV> {
        self.inner.ordered_recv_queue.pop_front()
    }

    #[inline(always)]
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.inner.receive(&mut self.socket);
        self.inner.send(&(), &mut self.socket)
    }

    #[inline(always)]
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }
}

impl<'a, SEND: ByteRepr, RECV: ByteRepr> Communicator<SEND, RECV>
    for UdpCommunicatorMut<'a, SEND, RECV>
{
    #[inline(always)]
    fn write(&mut self, message: SEND) {
        self.inner.unreliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_reliable(&mut self, message: SEND) {
        self.inner.reliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_ordered(&mut self, message: SEND) {
        self.inner.reliable_ordered_send_queue.push_back(message);
    }

    #[inline(always)]
    fn read(&mut self) -> Option<RECV> {
        self.inner.unordered_recv_queue.pop_front()
    }

    #[inline(always)]
    fn read_ordered(&mut self) -> Option<RECV> {
        self.inner.ordered_recv_queue.pop_front()
    }

    #[inline(always)]
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.inner.send(self.addr, self.socket)
    }

    #[inline(always)]
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }
}

/// TODO: Remove where Debug
impl<SEND: ByteRepr + Debug, RECV: ByteRepr + Debug> MultiCommunicator<SEND, RECV>
    for MultiUdpCommunicator<SEND, RECV>
{
    fn recv<CB>(&mut self, mut on_recv: CB)
    where
        CB: FnMut(SocketAddr, UdpCommunicatorMut<SEND, RECV>),
    {
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            let com = self.coms.entry(addr).or_default();
            com.read_packet(n, &mut self.socket);
            on_recv(
                addr,
                UdpCommunicatorMut {
                    socket: &mut self.socket,
                    addr: &addr,
                    inner: com,
                },
            );
            if let Err(e) = com.send(&addr, &mut self.socket) {
                warn!("Failed to send after receive: {e}");
            }
        }
    }

    fn broadcast(&mut self, message: SEND)
    where
        SEND: Clone,
    {
        for com in self.coms.values_mut() {
            com.reliable_send_queue.push_back(message.clone());
        }
    }

    fn breadcast_except(&mut self, message: SEND, exception: SocketAddr)
    where
        SEND: Clone,
    {
        for (_, com) in self.coms.iter_mut().filter(|(addr, _)| **addr != exception) {
            com.reliable_send_queue.push_back(message.clone());
        }
    }

    fn tick<CB>(&mut self, mut on_recv: CB)
    where
        CB: FnMut(SocketAddr, UdpCommunicatorMut<SEND, RECV>, DelayedBroadcast<SEND>),
        SEND: Debug + Clone,
        RECV: Debug,
    {
        let mut broadcast = vec![];
        let mut broadcast_except = vec![];
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            let com = self.coms.entry(addr).or_default();
            com.read_packet(n, &mut self.socket);
            on_recv(
                addr,
                UdpCommunicatorMut {
                    socket: &mut self.socket,
                    addr: &addr,
                    inner: com,
                },
                DelayedBroadcast {
                    broadcast: &mut broadcast,
                    broadcast_except: &mut broadcast_except,
                },
            );
        }

        for message in broadcast {
            self.broadcast(message);
        }

        for (exception, message) in broadcast_except {
            self.breadcast_except(message, exception);
        }

        for (addr, inner) in &mut self.coms {
            if let Err(e) = {
                UdpCommunicatorMut {
                    socket: &mut self.socket,
                    addr,
                    inner,
                }
                .tick()
            } {
                warn!("Failed to tick Communicator to {addr:?}: {e}");
            }
        }
    }

    fn get_mut<'a>(
        &'a mut self,
        client: &'a SocketAddr,
    ) -> Option<UdpCommunicatorMut<'a, SEND, RECV>> {
        let inner = self.coms.get_mut(client)?;
        Some(UdpCommunicatorMut {
            socket: &mut self.socket,
            addr: client,
            inner,
        })
    }
}

pub struct DelayedBroadcast<'a, SEND: ByteRepr + Clone> {
    broadcast: &'a mut Vec<SEND>,
    broadcast_except: &'a mut Vec<(SocketAddr, SEND)>,
}

impl<'a, SEND: ByteRepr + Clone> DelayedBroadcast<'a, SEND> {
    pub fn broadcast(&mut self, message: SEND) {
        self.broadcast.push(message);
    }

    pub fn broadcast_except(&mut self, message: SEND, exception: SocketAddr) {
        self.broadcast_except.push((exception, message));
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> InnerUdpCommunicator<SEND, RECV> {
    /// TODO! Remove where Debug
    fn send<Addr>(
        &mut self,
        addr: &Addr,
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
    fn receive(&mut self, socket: &mut UdpCommunicatorSocket)
    where
        SEND: Debug,
        RECV: Debug,
    {
        while let Ok(n) = socket.socket.recv(&mut socket.data_buffer) {
            self.read_packet(n, socket);
        }
    }

    /// TODO! Remove where Debug
    fn read_packet(&mut self, n: usize, socket: &mut UdpCommunicatorSocket)
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
        if socket.fake_unreliable && rand::random_bool(0.2) {
            let corrupt_num = rand::random_range(0..n * 8);
            debug!("Corrupting {corrupt_num} bits of packet #{packet}",);
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
                if socket.fake_unreliable && rand::random_bool(0.5) {
                    return;
                }
                #[cfg(test)]
                debug!("Receiving packet #{}", packet.ack.sequence_id);
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
                if super::ring_buffer::wrapping_gt(
                    newest_index.wrapping_sub(31),
                    packet.ack.sequence_id,
                    64,
                ) {
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
                        debug!("packet head: {}, id = {id}", self.ordered_read_packet_head);
                        if id == self.ordered_read_packet_head {
                            debug!("extending with {} messages", packet.messages.len());
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
        addr: &Addr,
        socket: &mut UdpCommunicatorSocket,
    ) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
        Addr: SocketSendAddr,
    {
        for (last_send, packet) in self
            .reliable_send_packets
            .values_mut()
            .chain(self.reliable_ordered_send_packets.values_mut())
        {
            let send_cooldown = if cfg!(test) {
                Duration::from_millis(3)
            } else {
                Duration::from_millis(100)
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
                if let Err(e) = socket.send(addr, ..4 + packet.byte_len()) {
                    error!("Failed to send packet: {e}");
                }
            }
        }
        for packet in self.unreliable_send_packets.drain(..) {
            packet.write_to_bytes(&mut socket.data_buffer[4..])?;
            let crc = CRC.checksum(&socket.data_buffer[4..4 + packet.byte_len()]);
            socket.data_buffer[..4].copy_from_slice(&crc.to_le_bytes());
            if let Err(e) = socket.send(addr, ..4 + packet.byte_len()) {
                error!("Failed to send packet: {e}");
            }
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
            // TODO! creating a new ack for every iteration is pointless, wasted work
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

impl UdpCommunicatorSocket {
    #[cfg(debug_assertions)]
    pub fn with_fake_unreliablity(mut self) -> Self {
        self.fake_unreliable = true;
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_debug_logs(mut self) -> Self {
        self.debug_logs = true;
        self
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> UdpCommunicator<SEND, RECV> {
    #[cfg(debug_assertions)]
    pub fn with_fake_unreliablity(mut self) -> Self {
        self.socket = self.socket.with_fake_unreliablity();
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_debug_logs(mut self) -> Self {
        self.socket = self.socket.with_debug_logs();
        self
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> MultiUdpCommunicator<SEND, RECV> {
    #[cfg(debug_assertions)]
    pub fn with_fake_unreliablity(mut self) -> Self {
        self.socket = self.socket.with_fake_unreliablity();
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_debug_logs(mut self) -> Self {
        self.socket = self.socket.with_debug_logs();
        self
    }
}

#[cfg(test)]
pub(crate) fn test_init<SEND, RECV>(
    port_offset: u16,
) -> (UdpCommunicator<SEND, RECV>, UdpCommunicator<SEND, RECV>)
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    let _ = tracing_subscriber::FmtSubscriber::builder()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    let localhost = std::net::Ipv4Addr::new(127, 0, 0, 1);
    let localhost = std::net::IpAddr::V4(localhost);
    let addr1 = std::net::SocketAddr::new(localhost, port_offset);
    let addr2 = std::net::SocketAddr::new(localhost, port_offset + 1);
    let mut com1 = UdpCommunicator::<SEND, RECV>::bind(addr1);
    let mut com2 = UdpCommunicator::<SEND, RECV>::bind(addr2);
    assert!(com2.connect(addr1).is_ok());
    assert!(com1.connect(addr2).is_ok());
    (com1, com2)
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, time::Duration};

    use tracing::debug;

    use crate::{packet::InnerUdpMessage, prelude::*};

    #[test]
    fn packet_roundtrip() {
        let (mut com1, mut com2) = super::test_init::<InnerUdpMessage, InnerUdpMessage>(7200);
        let m1 = InnerUdpMessage::Hello;
        let m2 = InnerUdpMessage::Wave(1394);
        com2.write(m1);
        com2.write(m2);
        com2.tick().unwrap();
        com1.tick().unwrap();
        assert_eq!(com1.read(), Some(m1));
        assert_eq!(com1.read(), Some(m2));
        assert_eq!(com1.read(), None);
    }

    #[test]
    fn send_until_ack() {
        let (mut com1, mut com2) = super::test_init::<InnerUdpMessage, InnerUdpMessage>(7202);
        let m1 = InnerUdpMessage::Hello;
        com2.write_reliable(m1);
        com2.tick().unwrap();

        let mut i = 0;
        while com2.has_work() {
            i += 1;
            com1.tick().unwrap();
            if let Some(message) = com1.read() {
                assert_eq!(message, m1);
                debug!("com1 received: {message:?}");
                // Send a dummy packet back to acknowledge the received one
                com1.write(InnerUdpMessage::Wave(1));
            }
            com2.tick().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(i, 2);
    }

    #[test]
    fn test_reliability() {
        let (mut com1, mut com2) = super::test_init::<InnerUdpMessage, InnerUdpMessage>(7204);
        com1.socket = com1.socket.with_fake_unreliablity().with_debug_logs();
        com2.socket = com2.socket.with_fake_unreliablity();
        let mut send = HashSet::new();
        assert!(send.insert(InnerUdpMessage::Hello));
        for i in 0..20000 {
            assert!(send.insert(InnerUdpMessage::Wave(i)));
        }
        for m in &send {
            com1.write_reliable(*m);
        }
        com1.tick().unwrap();

        let mut received = HashSet::new();
        while com1.has_work() {
            com2.tick().unwrap();
            while let Some(message) = com2.read() {
                assert!(received.insert(message));
            }
            com1.tick().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }

        assert_eq!(received, send);
    }

    #[test]
    fn test_ordered_reliability() {
        let (mut com1, mut com2) = super::test_init::<InnerUdpMessage, InnerUdpMessage>(7206);
        com1.socket = com1.socket.with_fake_unreliablity().with_debug_logs();
        com2.socket = com2.socket.with_fake_unreliablity();
        let mut send = vec![];
        for i in 0..20000 {
            send.push(InnerUdpMessage::Wave(i));
        }
        for m in &send {
            com1.write_ordered(*m);
        }
        com1.tick().unwrap();

        let mut received = vec![];
        let mut i = 0;
        while com1.has_work() {
            com2.tick().unwrap();
            while let Some(message) = com2.read_ordered() {
                received.push(message);
                assert_eq!(message, InnerUdpMessage::Wave(i));
                i += 1;
            }
            com1.tick().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }

        assert_eq!(received, send);
    }
}
