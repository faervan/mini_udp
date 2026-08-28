use std::net::{SocketAddr, ToSocketAddrs};

use crate::prelude::*;

mod inner;
pub(crate) use inner::*;

mod multi;
pub use multi::*;

mod socket;
pub use socket::*;

#[cfg(feature = "debug")]
mod debug;
#[cfg(feature = "debug")]
#[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
pub use debug::MiniUdpDebugExt;

pub trait Communicator<CTX: MiniUdpContext> {
    /// Queue a message to be send unreliably.
    fn write(&mut self, message: CTX::SEND);
    /// Queue a message to be send reliably, but not necessarily in order.
    fn write_reliable(&mut self, message: CTX::SEND);
    /// Queue a message to be send reliably, in order. The receiver will make sure not to show this
    /// message before any previously send, ordered messages.
    fn write_ordered(&mut self, message: CTX::SEND);
    /// Add a heartbeat to the internal send queue. This behaves like the other `write*` methods,
    /// it does not actually send the packet yet.
    fn write_heartbeat(&mut self);
    /// Try to read the next unreliable or reliable-unordered message received.
    fn read(&mut self) -> Option<CTX::RECV>;
    /// Try to read the reliable-ordered message received.
    fn read_ordered(&mut self) -> Option<CTX::RECV>;
    /// Returns `true` if there are any pending messages to be send / packets to get acknowledged.
    fn has_work(&self) -> bool;
    /// Returns the [Instant] of time at which the last packet has been received.
    /// Before any packets have been received, this returns the [Instant] at which this
    /// [Communicator] has been constructed.
    fn last_seen(&self) -> &Instant;
    /// Returns the [Instant] of time at which the last packet has been send.
    /// Before any packets have been send, this returns the [Instant] at which this [Communicator]
    /// has been constructed.
    fn last_send(&self) -> &Instant;
}

/// A wrapper around [`std::net::UdpSocket`] that will handle message (de)serialization,
/// reliability, and ordering when connected to another [`UdpCommunicator`] or
/// [`MultiUdpCommunicator`].
///
/// **Example**
/// ```rust
/// use mini_udp::prelude::*;
/// use std::borrow::Cow;
///
/// const PROTOCOL_VERSION: u32 = 2;
/// type SenderCtx<'a> = UdpContext<Message<'a>, (), PROTOCOL_VERSION>;
/// type ReceiverCtx<'a> = <SenderCtx<'a> as MiniUdpContext>::REVERSE;
///
/// #[derive(ByteRepr, Debug, PartialEq)]
/// struct Message<'a>(Cow<'a, str>);
///
/// let mut com1 =
///     UdpCommunicator::<SenderCtx>::bind("0.0.0.0:7002")
///         .connect("0.0.0.0:7003")
///         .unwrap();
/// let mut com2 =
///     UdpCommunicator::<ReceiverCtx>::bind("0.0.0.0:7003")
///         .connect("0.0.0.0:7002")
///         .unwrap();
/// let message = "hello udp";
/// com1.write(Message(Cow::Borrowed(message)));
/// com1.send().unwrap();
/// com2.recv();
/// assert_eq!(com2.read(), Some(Message(Cow::Owned(String::from(message)))));
/// ```
pub struct UdpCommunicator<CTX: MiniUdpContext> {
    socket: UdpCommunicatorSocket,
    pub(super) inner: InnerUdpCommunicator<CTX>,
}

impl<CTX: MiniUdpContext> CommunicatorSocket for UdpCommunicator<CTX> {
    /// Create a new [`UdpCommunicator`], binding it to the provided `addr`.
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind(addr),
            inner: InnerUdpCommunicator::default(),
        }
    }

    #[inline(always)]
    fn with_reliable_unordered_resend_interval(mut self, interval: Duration) -> Self {
        self.socket = self
            .socket
            .with_reliable_unordered_resend_interval(interval);
        self
    }

    #[inline(always)]
    fn with_reliable_ordered_resend_interval(mut self, interval: Duration) -> Self {
        self.socket = self.socket.with_reliable_ordered_resend_interval(interval);
        self
    }

    #[inline(always)]
    fn with_max_reliable_unordered_retries(mut self, retries: usize) -> Self {
        self.socket = self.socket.with_max_reliable_unordered_retries(retries);
        self
    }

    #[inline(always)]
    fn with_max_reliable_ordered_retries(mut self, retries: usize) -> Self {
        self.socket = self.socket.with_max_reliable_ordered_retries(retries);
        self
    }
}

/// A connection of an [`MultiUdpCommunicator`].
pub struct UdpCommunicatorMut<'a, CTX: MiniUdpContext> {
    #[cfg(feature = "debug")]
    socket: &'a UdpCommunicatorSocket,
    pub addr: SocketAddr,
    inner: &'a mut InnerUdpCommunicator<CTX>,
}

impl<CTX: MiniUdpContext> Default for UdpCommunicator<CTX> {
    fn default() -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind("0.0.0.0:0"),
            inner: InnerUdpCommunicator::default(),
        }
    }
}

impl<CTX: MiniUdpContext> Communicator<CTX> for UdpCommunicator<CTX> {
    #[inline(always)]
    fn write(&mut self, message: CTX::SEND) {
        self.inner.unreliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_reliable(&mut self, message: CTX::SEND) {
        self.inner.reliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_ordered(&mut self, message: CTX::SEND) {
        self.inner.reliable_ordered_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_heartbeat(&mut self) {
        self.inner.write_heartbeat(
            #[cfg(feature = "debug")]
            &self.socket,
        );
    }

    #[inline(always)]
    fn read(&mut self) -> Option<CTX::RECV> {
        self.inner.unordered_recv_queue.pop_front()
    }

    #[inline(always)]
    fn read_ordered(&mut self) -> Option<CTX::RECV> {
        self.inner.ordered_recv_queue.pop_front()
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

impl<'a, CTX: MiniUdpContext> Communicator<CTX> for UdpCommunicatorMut<'a, CTX> {
    #[inline(always)]
    fn write(&mut self, message: CTX::SEND) {
        self.inner.unreliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_reliable(&mut self, message: CTX::SEND) {
        self.inner.reliable_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_ordered(&mut self, message: CTX::SEND) {
        self.inner.reliable_ordered_send_queue.push_back(message);
    }

    #[inline(always)]
    fn write_heartbeat(&mut self) {
        self.inner.write_heartbeat(
            #[cfg(feature = "debug")]
            self.socket,
        );
    }

    #[inline(always)]
    fn read(&mut self) -> Option<CTX::RECV> {
        self.inner.unordered_recv_queue.pop_front()
    }

    #[inline(always)]
    fn read_ordered(&mut self) -> Option<CTX::RECV> {
        self.inner.ordered_recv_queue.pop_front()
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

impl<CTX: MiniUdpContext> UdpCommunicator<CTX> {
    #[inline(always)]
    /// Connect to the provided `addr`.
    ///
    /// This does not actually send anything to the remote address, see
    /// [`std::net::UdpSocket::connect`] for more info.
    pub fn connect<A: ToSocketAddrs>(self, addr: A) -> Result<Self, std::io::Error> {
        self.socket.socket.connect(addr)?;
        Ok(self)
    }

    #[inline(always)]
    /// Receive all new packets and deserialize them into messages.
    /// You can read the new messages using [`UdpCommunicator::read`] and [`UdpCommunicator::read_ordered`].
    pub fn recv(&mut self) {
        self.inner.receive(&mut self.socket);
    }

    #[inline(always)]
    /// Send all pending messages. This will also resend reliable, unacknowledged packets if the
    /// configured resend interval has been reached.
    /// If new packets have been received since the last time this method was called and there are
    /// no pending messages to be send or packets to be resend, this will send an empty heartbeat
    /// packet to the connected receiver.
    pub fn send(&mut self) -> Result<(), ByteReprError> {
        self.inner.send((), &mut self.socket)
    }

    #[inline(always)]
    /// A shorthand for [`self.recv()`](Self::recv) followed by [`self.send()`](Self::send).
    pub fn tick(&mut self) -> Result<(), ByteReprError> {
        self.recv();
        self.send()
    }
}

#[cfg(test)]
pub(crate) fn test_init<CTX: MiniUdpContext>(
    port_offset: u16,
) -> (UdpCommunicator<CTX>, UdpCommunicator<CTX>) {
    let _ = tracing_subscriber::FmtSubscriber::builder()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    let localhost = std::net::Ipv4Addr::new(127, 0, 0, 1);
    let localhost = std::net::IpAddr::V4(localhost);
    let addr1 = std::net::SocketAddr::new(localhost, port_offset);
    let addr2 = std::net::SocketAddr::new(localhost, port_offset + 1);
    let com1 = UdpCommunicator::<CTX>::bind(addr1).connect(addr2).unwrap();
    let com2 = UdpCommunicator::<CTX>::bind(addr2).connect(addr1).unwrap();
    (com1, com2)
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use tracing::debug;

    use crate::{packet::test::InnerUdpMessage, prelude::*};

    type UdpCtx = UdpContext<InnerUdpMessage, InnerUdpMessage, 0>;

    #[test]
    fn packet_roundtrip() {
        let (mut com1, mut com2) = super::test_init::<UdpCtx>(7200);
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
        let (mut com1, mut com2) = super::test_init::<UdpCtx>(7202);
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
        let (mut com1, mut com2) = super::test_init::<UdpCtx>(7204);
        #[cfg(feature = "debug")]
        {
            com1.socket = com1
                .socket
                .with_fake_drop(0.4)
                .with_fake_corruption(0.1)
                .with_debug_logs();
            com2.socket = com2.socket.with_fake_drop(0.6).with_fake_corruption(0.2);
        }
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
        let (mut com1, mut com2) = super::test_init::<UdpCtx>(7206);
        #[cfg(feature = "debug")]
        {
            com1.socket = com1
                .socket
                .with_fake_drop(0.4)
                .with_fake_corruption(0.1)
                .with_debug_logs();
            com2.socket = com2.socket.with_fake_drop(0.6).with_fake_corruption(0.2);
        }
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
    #[cfg(feature = "debug")]
    fn test_fake_delay() {
        let (mut com1, mut com2) = super::test_init::<UdpCtx>(7208);
        com1.socket = com1.socket.with_debug_logs();
        com2.socket = com2.socket.with_fake_delay(70..80);
        let msg = InnerUdpMessage::Wave(u16::MAX);
        com1.write_reliable(msg);
        let start = Instant::now();
        com1.send().unwrap();
        loop {
            use std::thread::sleep;

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
    #[cfg(feature = "debug")]
    fn test_fake_delay_multi() {
        let _ = tracing_subscriber::FmtSubscriber::builder()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();
        let mut multi_com = MultiUdpCommunicator::<UdpContext<(), isize, 1>>::bind("0.0.0.0:7210")
            .with_debug_logs()
            .with_fake_delay(25..26);
        let mut com = UdpCommunicator::<UdpContext<isize, (), 1>>::default()
            .connect("0.0.0.0:7210")
            .unwrap();
        let msg = -240_594;
        com.write_ordered(msg);
        let start = Instant::now();
        com.send().unwrap();
        let mut break_loop = false;
        loop {
            use std::thread::sleep;

            sleep(Duration::from_millis(1));
            multi_com.recv(|mut com: UdpCommunicatorMut<_>| {
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

    #[test]
    fn test_protocol_version_check() {
        let mut com1 = UdpCommunicator::<UdpContext<String, (), 1>>::default()
            .connect("0.0.0.0:7212")
            .unwrap();
        let mut com2 = UdpCommunicator::<UdpContext<(), String, 2>>::bind("0.0.0.0:7212");
        com1.write(String::from("Can you hear me?"));
        com1.send().unwrap();
        com2.recv();
        // Sender send with a protocol version of 1, receiver has a version of 2, so the CRC check
        // failed and no messages are available.
        assert_eq!(com2.read(), None);
    }
}
