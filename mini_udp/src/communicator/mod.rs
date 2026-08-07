use std::{
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
};

use crate::prelude::*;

mod inner;
pub(crate) use inner::*;

mod multi;
pub use multi::*;

mod socket;
pub use socket::*;

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

pub trait Communicator<SEND: ByteRepr, RECV: ByteRepr> {
    fn write(&mut self, message: SEND);
    fn write_reliable(&mut self, message: SEND);
    fn write_ordered(&mut self, message: SEND);
    /// Add a heartbeat to the internal send queue. This behaves like the other `write*` methods,
    /// it does not actually send the packet yet.
    fn write_heartbeat(&mut self);
    fn read(&mut self) -> Option<RECV>;
    fn read_ordered(&mut self) -> Option<RECV>;
    /// TODO! Remove where Debug
    fn send(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug;
    /// TODO! Remove where Debug
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug;
    /// Returns `true` if there are any pending messages to be send / packets to get acknowledged.
    fn has_work(&self) -> bool;
    /// Returns the [Instant] of time at which the last packet has been received.
    /// Before any packets have been received, this returns the [Instant] at which [Communicator]
    /// has been constructed.
    fn last_seen(&self) -> &Instant;
    fn last_send(&self) -> &Instant;
}

pub struct UdpCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    socket: UdpCommunicatorSocket,
    pub(super) inner: InnerUdpCommunicator<SEND, RECV>,
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
}

pub struct UdpCommunicatorMut<'a, SEND: ByteRepr, RECV: ByteRepr> {
    socket: &'a mut UdpCommunicatorSocket,
    pub addr: SocketAddr,
    inner: &'a mut InnerUdpCommunicator<SEND, RECV>,
}

impl<SEND: ByteRepr, RECV: ByteRepr> Default for UdpCommunicator<SEND, RECV> {
    fn default() -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind("0.0.0.0:0"),
            inner: InnerUdpCommunicator::default(),
        }
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
    fn write_heartbeat(&mut self) {
        self.inner.write_heartbeat(
            #[cfg(debug_assertions)]
            &self.socket,
        );
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
    fn send(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.inner.send((), &mut self.socket)
    }

    #[inline(always)]
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.recv();
        self.send()
    }

    #[inline(always)]
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }

    #[inline(always)]
    fn last_seen(&self) -> &Instant {
        &self.inner.last_seen
    }

    #[inline(always)]
    fn last_send(&self) -> &Instant {
        &self.inner.last_send
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
    fn write_heartbeat(&mut self) {
        self.inner.write_heartbeat(
            #[cfg(debug_assertions)]
            self.socket,
        );
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
    fn send(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.inner.send(self.addr, self.socket)
    }

    #[inline(always)]
    fn tick(&mut self) -> Result<(), ByteReprError>
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.send()
    }

    #[inline(always)]
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }

    #[inline(always)]
    fn last_seen(&self) -> &Instant {
        &self.inner.last_seen
    }

    #[inline(always)]
    fn last_send(&self) -> &Instant {
        &self.inner.last_send
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> UdpCommunicator<SEND, RECV> {
    #[inline(always)]
    pub fn recv(&mut self)
    where
        SEND: Debug,
        RECV: Debug,
    {
        self.inner.receive(&mut self.socket);
    }

    #[cfg(debug_assertions)]
    pub fn with_fake_drop(mut self, drop_probability: f64) -> Self {
        self.socket = self.socket.with_fake_drop(drop_probability);
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_fake_corruption(mut self, corruption_probability: f64) -> Self {
        self.socket = self.socket.with_fake_corruption(corruption_probability);
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_fake_delay(mut self, delay_ms: std::ops::Range<u64>) -> Self {
        self.socket = self.socket.with_fake_delay(delay_ms);
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
    use std::{collections::HashSet, thread::sleep, time::Duration};

    use tracing::debug;

    use crate::{packet::test::InnerUdpMessage, prelude::*};

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
        com1.socket = com1
            .socket
            .with_fake_drop(0.4)
            .with_fake_corruption(0.1)
            .with_debug_logs();
        com2.socket = com2.socket.with_fake_drop(0.6).with_fake_corruption(0.2);
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
        com1.socket = com1
            .socket
            .with_fake_drop(0.4)
            .with_fake_corruption(0.1)
            .with_debug_logs();
        com2.socket = com2.socket.with_fake_drop(0.6).with_fake_corruption(0.2);
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

    #[test]
    fn test_fake_delay() {
        let (mut com1, mut com2) = super::test_init::<InnerUdpMessage, InnerUdpMessage>(7208);
        com1.socket = com1.socket.with_debug_logs();
        com2.socket = com2.socket.with_fake_delay(70..80);
        let msg = InnerUdpMessage::Wave(u16::MAX);
        com1.write_reliable(msg);
        let start = Instant::now();
        com1.send().unwrap();
        loop {
            sleep(Duration::from_millis(1));
            com2.recv();
            if let Some(received) = com2.read() {
                assert_eq!(received, msg);
                assert!(start.elapsed().as_millis() > 70);
                assert!(start.elapsed().as_millis() < 82);
                break;
            }
        }
    }

    #[test]
    fn test_fake_delay_multi() {
        let _ = tracing_subscriber::FmtSubscriber::builder()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();
        let mut multi_com = MultiUdpCommunicator::<(), isize>::bind("0.0.0.0:7210")
            .with_debug_logs()
            .with_fake_delay(25..26);
        let mut com = UdpCommunicator::<isize, ()>::default();
        com.connect("0.0.0.0:7210").unwrap();
        let msg = -240_594;
        com.write_ordered(msg);
        let start = Instant::now();
        com.send().unwrap();
        let mut break_loop = false;
        loop {
            sleep(Duration::from_millis(1));
            multi_com.recv(|mut com: UdpCommunicatorMut<_, _>| {
                assert_eq!(com.read_ordered().unwrap(), msg);
                assert!(start.elapsed().as_millis() > 24);
                assert!(start.elapsed().as_millis() < 28);
                break_loop = true;
            });
            if break_loop {
                break;
            }
        }
    }
}
