use std::{
    collections::{HashMap, VecDeque},
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::Arc,
    time::Instant,
};

use crate::{
    ByteRepr,
    packet::{MAX_PACKET_LEN, Packet},
    ring_buffer::RingBuffer,
};

pub trait MaybeOwnedSocket {
    fn socket(&self) -> &UdpCommunicatorSocket;
}

impl MaybeOwnedSocket for UdpCommunicatorSocket {
    fn socket(&self) -> &UdpCommunicatorSocket {
        self
    }
}

impl MaybeOwnedSocket for Arc<UdpCommunicatorSocket> {
    fn socket(&self) -> &UdpCommunicatorSocket {
        self
    }
}

pub trait CommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self;
    fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error>;
}

pub trait Communicator<M: ByteRepr> {
    fn write(&mut self, message: M);
    fn read(&mut self) -> Option<M>;
}

pub struct MultiUdpCommunicator<M: ByteRepr> {
    socket: Arc<UdpCommunicatorSocket>,
    coms: HashMap<SocketAddr, UdpCommunicator<M, Arc<UdpCommunicatorSocket>>>,
}

pub struct UdpCommunicatorSocket {
    socket: UdpSocket,
    data_buffer: [u8; MAX_PACKET_LEN],
    #[cfg(test)]
    fake_unreliable: bool,
    #[cfg(test)]
    debug_logs: bool,
}

pub struct UdpCommunicator<M: ByteRepr, SOCKET: MaybeOwnedSocket = UdpCommunicatorSocket> {
    socket: SOCKET,
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

impl<M: ByteRepr> Default for UdpCommunicator<M> {
    fn default() -> Self {
        Self::bind("0.0.0.0:0")
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
            reliable_send_packets: RingBuffer::new(),
            unreliable_send_packet_id: 0,
            unreliable_send_packets: VecDeque::new(),
            received_packets: RingBuffer::new(),
            msg_send_queue: VecDeque::new(),
            msg_recv_queue: VecDeque::new(),
            received_packet_duplicate: false,
        }
    }

    fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }
}

impl<M: ByteRepr> CommunicatorSocket for MultiUdpCommunicator<M> {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: Arc::new(UdpCommunicatorSocket::bind(addr)),
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
        self.msg_send_queue.push_back(message);
    }

    #[inline(always)]
    fn read(&mut self) -> Option<M> {
        self.msg_recv_queue.pop_front()
    }
}

impl<M: ByteRepr> UdpCommunicator<M> {
    #[cfg(test)]
    fn with_fake_unreliablity(mut self) -> Self {
        self.socket.fake_unreliable = true;
        self
    }

    #[cfg(test)]
    fn with_debug_logs(mut self) -> Self {
        self.socket.debug_logs = true;
        self
    }
}

impl<M: ByteRepr> MultiUdpCommunicator<M> {
    #[cfg(test)]
    fn with_fake_unreliablity(mut self) -> Self {
        let mut socket = Arc::into_inner(self.socket).unwrap();
        socket.fake_unreliable = true;
        self.socket = Arc::new(socket);
        self
    }

    #[cfg(test)]
    fn with_debug_logs(mut self) -> Self {
        let mut socket = Arc::into_inner(self.socket).unwrap();
        socket.debug_logs = true;
        self.socket = Arc::new(socket);
        self
    }
}
