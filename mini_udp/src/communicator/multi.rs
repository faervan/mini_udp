use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs},
};

use crate::{communicator::UdpCommunicatorMut, prelude::*};

pub trait MultiCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    /// Create a connection to the provided `addr`.
    fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<UdpCommunicatorMut<'_, SEND, RECV>, std::io::Error>;
    /// Receive all new packets. If no "connection" exists for the [`SocketAddr`] a new packet was
    /// received from, one will be created.
    /// For each packet that was received, the provided `on_recv` callback will be called, giving
    /// you access to the connection that the packet was received from. You can read all received
    /// messages from that packet using [`UdpCommunicatorMut::read`] or
    /// [`UdpCommunicatorMut::read_ordered`] and queue responses using
    /// [`UdpCommunicatorMut::write*`](UdpCommunicatorMut).
    ///
    /// **Example**
    /// ```
    /// use mini_udp::prelude::*;
    ///
    /// let mut multi = MultiUdpCommunicator::<(), u8>::bind("0.0.0.0:7000");
    /// let mut com = UdpCommunicator::<u8, ()>::default().connect("0.0.0.0:7000").unwrap();
    /// com.write(200);
    /// com.send().unwrap();
    /// let mut received_response = false;
    /// multi.recv(|mut com: UdpCommunicatorMut<_, _>| {
    ///     if let Some(msg) = com.read() && msg == 200 {
    ///         received_response = true;
    ///     }
    /// });
    /// assert!(received_response);
    /// ```
    fn recv<M, CB>(&mut self, on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>;
    /// Send all pending messages. This will also resend reliable, unacknowledged packets if the
    /// configured resend interval has been reached.
    /// If new packets have been received since the last time this method was called and there are
    /// no pending messages to be send or packets to be resend, this will send an empty heartbeat
    /// packet to the connected receiver.
    fn send(&mut self);
    /// Iterate over all client connections. A client connection is automatically created when
    /// [`recv`](Self::recv) reads a packet from a [`SocketAddr`] that does not already have a
    /// connection. Connections are never automatically deleted by [`mini_udp`].
    fn iter_mut(&mut self) -> IterMut<'_, SEND, RECV>;
    /// Broadcast the provided `message` to all connections, reliably.
    fn broadcast(&mut self, message: SEND)
    where
        SEND: Clone;
    /// Broadcast the provided `message` to all connections except `exception`, reliably.
    fn broadcast_except(&mut self, message: SEND, exception: SocketAddr)
    where
        SEND: Clone;
    /// Execute `f` for each connection.
    fn for_each<F>(&mut self, f: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>);
    /// Remove all connections for which the provided `filter` closure returns `false`.
    fn retain<F>(&mut self, filter: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>) -> bool;
    /// Remove the connection to `addr`, returning whether a connection to `addr` existed.
    fn remove(&mut self, addr: &SocketAddr) -> bool;
    /// Get the connection to `addr`, if it exists, so you can read or queue messages.
    fn get_mut<'a>(
        &'a mut self,
        addr: &'a SocketAddr,
    ) -> Option<UdpCommunicatorMut<'a, SEND, RECV>>;
    #[inline(always)]
    /// A shorthand for [`self.recv()`](Self::recv) followed by [`self.send()`](Self::send).
    fn tick<M, CB>(&mut self, on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>,
    {
        self.recv(on_recv);
        self.send();
    }
}

/// A wrapper around [`std::net::UdpSocket`] that will handle message (de)serialization,
/// reliability, and ordering when connected to one or many other
/// [`UdpCommunicators`](UdpCommunicator) or [`MultiUdpCommunicators`](MultiUdpCommunicator).
///
/// **Example**
/// ```rust
/// use mini_udp::prelude::*;
///
/// #[derive(ByteRepr, Debug, Clone, PartialEq)]
/// enum Message {
///     Hello {
///         greeting: String
///     },
///     Number(Option<i32>),
///     Bye,
/// }
///
/// // Bind to "0.0.0.0:0", which lets the OS decide the port.
/// let mut multi1 = MultiUdpCommunicator::default();
/// let mut multi2 = MultiUdpCommunicator::bind("0.0.0.0:7005");
///
/// // Add a connection to multi1. This will not send any data to the provided address,
/// // but enables us to start sending messages to it.
/// let mut com: UdpCommunicatorMut<_, _> = multi1.connect("0.0.0.0:7005").unwrap();
/// assert_eq!(com.addr.port(), 7005);
///
/// com.write_reliable(Message::Hello { greeting: String::from("Hello world!") });
/// // The `broadcast*` methods send the provided message to all connections of this
/// // [`MultiUdpCommunicator`], reliably but not necessarily in order.
/// multi1.broadcast(Message::Number(Some(-500)));
/// // Send all queued messages of all connections. Because the combined byte size of `Hello` and
/// // `Number` is below the maximum packet data size of 1024, they will be added to the same packet
/// // and are thus guaranteed to preserve their order.
/// multi1.send();
///
/// // We have connected multi1 to multi2, but multi2 does not know about it yet!
/// assert_eq!(multi2.iter_mut().count(), 0);
/// // Receive all new packets.
/// multi2.recv(());
/// // When multi2 receives the packet we have just send to it from multi1, it will automatically
/// // create a connection to multi1.
/// assert_eq!(multi2.iter_mut().count(), 1);
///
/// multi2.for_each(|mut com| {
///     assert_eq!(com.read(), Some(Message::Hello { greeting: String::from("Hello world!") }));
///     assert_eq!(com.read(), Some(Message::Number(Some(-500))));
///     com.write(Message::Bye);
/// });
/// multi2.send();
///
/// multi1.recv(|mut com: UdpCommunicatorMut<_, _>| {
///     assert_eq!(com.read(), Some(Message::Bye));
/// });
/// ```
pub struct MultiUdpCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    socket: UdpCommunicatorSocket,
    coms: HashMap<SocketAddr, InnerUdpCommunicator<SEND, RECV>>,
}

impl<SEND: ByteRepr, RECV: ByteRepr> Default for MultiUdpCommunicator<SEND, RECV> {
    #[inline(always)]
    fn default() -> Self {
        Self::bind("0.0.0.0:0")
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> CommunicatorSocket for MultiUdpCommunicator<SEND, RECV> {
    /// Create a new [`MultiUdpCommunicator`], binding it to the provided `addr`.
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind(addr),
            coms: HashMap::new(),
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
}

impl<SEND: ByteRepr, RECV: ByteRepr> MultiCommunicator<SEND, RECV>
    for MultiUdpCommunicator<SEND, RECV>
{
    fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<UdpCommunicatorMut<'_, SEND, RECV>, std::io::Error> {
        let addr = addr.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::other("The provided `addr` parses into an empty SocketAddr iterator")
        })?;
        Ok(UdpCommunicatorMut {
            socket: &self.socket,
            addr,
            inner: self.coms.entry(addr).or_default(),
        })
    }

    fn recv<M, CB>(&mut self, mut on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>,
    {
        let mut state = on_recv.prepare();
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            #[cfg(debug_assertions)]
            if self.socket.delay_packet(Some(addr), n) {
                continue;
            }
            let com = self.coms.entry(addr).or_default();
            com.read_packet(n, &mut self.socket);
            on_recv.on_recv(
                UdpCommunicatorMut {
                    socket: &self.socket,
                    addr,
                    inner: com,
                },
                &mut state,
            );
        }
        #[cfg(debug_assertions)]
        while let Some((n, Some(addr))) = self.socket.read_delayed() {
            let com = self.coms.entry(addr).or_default();
            com.read_packet(n, &mut self.socket);
            on_recv.on_recv(
                UdpCommunicatorMut {
                    socket: &self.socket,
                    addr,
                    inner: com,
                },
                &mut state,
            );
        }
        on_recv.finish(self, state);
    }

    fn send(&mut self) {
        for (addr, inner) in &mut self.coms {
            if let Err(e) = inner.send(*addr, &mut self.socket) {
                warn!("Failed to send Communicator of {addr:?}: {e}");
            }
        }
    }

    fn iter_mut(&mut self) -> IterMut<'_, SEND, RECV> {
        IterMut {
            socket: &self.socket,
            inner: self.coms.iter_mut(),
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

    fn broadcast_except(&mut self, message: SEND, exception: SocketAddr)
    where
        SEND: Clone,
    {
        for (_, com) in self.coms.iter_mut().filter(|(addr, _)| **addr != exception) {
            com.reliable_send_queue.push_back(message.clone());
        }
    }

    fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>),
    {
        for (addr, com) in self.coms.iter_mut() {
            f(UdpCommunicatorMut {
                socket: &self.socket,
                addr: *addr,
                inner: com,
            });
        }
    }

    fn retain<F>(&mut self, mut filter: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>) -> bool,
    {
        self.coms.retain(|addr, inner| {
            filter(UdpCommunicatorMut {
                socket: &self.socket,
                addr: *addr,
                inner,
            })
        });
    }

    fn remove(&mut self, addr: &SocketAddr) -> bool {
        self.coms.remove(addr).is_some()
    }

    fn get_mut<'a>(
        &'a mut self,
        addr: &'a SocketAddr,
    ) -> Option<UdpCommunicatorMut<'a, SEND, RECV>> {
        let inner = self.coms.get_mut(addr)?;
        Some(UdpCommunicatorMut {
            socket: &self.socket,
            addr: *addr,
            inner,
        })
    }
}

pub trait OnReceiveCallback<M, SEND, RECV>
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    type State;
    /// Will be called at the beginning of each [`MultiCommunicator::recv`] invocation.
    fn prepare(&mut self) -> Self::State;
    /// Will be called for each packet that has been received during a [`MultiCommunicator::recv`]
    /// invocation.
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State);
    #[allow(unused_variables)]
    /// Will be called at the end of each [`MultiCommunicator::recv`] invocation.
    fn finish(&mut self, com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {}
}

impl<SEND, RECV> OnReceiveCallback<(), SEND, RECV> for ()
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    type State = ();
    fn prepare(&mut self) -> Self::State {}
    fn on_recv(&mut self, _com: UdpCommunicatorMut<SEND, RECV>, _state: &mut Self::State) {}
}

impl<T, SEND, RECV> OnReceiveCallback<UdpCommunicatorMut<'_, SEND, RECV>, SEND, RECV> for T
where
    T: FnMut(UdpCommunicatorMut<SEND, RECV>),
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    type State = ();
    fn prepare(&mut self) -> Self::State {}
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, _state: &mut Self::State) {
        self(com);
    }
}

impl<T, SEND, RECV>
    OnReceiveCallback<
        (
            UdpCommunicatorMut<'_, SEND, RECV>,
            DelayedBroadcast<'_, SEND>,
        ),
        SEND,
        RECV,
    > for T
where
    T: FnMut(UdpCommunicatorMut<SEND, RECV>, DelayedBroadcast<SEND>),
    SEND: ByteRepr + Clone,
    RECV: ByteRepr,
{
    type State = Vec<SEND>;
    fn prepare(&mut self) -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State) {
        self(com, DelayedBroadcast { broadcast: state });
    }
    fn finish(&mut self, com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {
        for message in state {
            com.broadcast(message);
        }
    }
}

impl<T, SEND, RECV>
    OnReceiveCallback<
        (
            UdpCommunicatorMut<'_, SEND, RECV>,
            DelayedBroadcastExcept<'_, SEND>,
        ),
        SEND,
        RECV,
    > for T
where
    T: FnMut(UdpCommunicatorMut<SEND, RECV>, DelayedBroadcastExcept<SEND>),
    SEND: ByteRepr + Clone,
    RECV: ByteRepr,
{
    type State = Vec<(SocketAddr, SEND)>;
    fn prepare(&mut self) -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State) {
        self(com, DelayedBroadcastExcept { broadcast: state });
    }
    fn finish(&mut self, com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {
        for (exception, message) in state {
            com.broadcast_except(message, exception);
        }
    }
}

impl<T, SEND, RECV>
    OnReceiveCallback<
        (
            UdpCommunicatorMut<'_, SEND, RECV>,
            DelayedForEach<'_, SEND, RECV>,
        ),
        SEND,
        RECV,
    > for T
where
    T: FnMut(UdpCommunicatorMut<SEND, RECV>, DelayedForEach<SEND, RECV>),
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    type State = Vec<DelayedForEachF<SEND, RECV>>;
    fn prepare(&mut self) -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State) {
        self(com, DelayedForEach { for_each: state });
    }
    fn finish(&mut self, com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {
        for f in state {
            com.for_each(f);
        }
    }
}

pub struct DelayedBroadcast<'a, SEND: ByteRepr + Clone> {
    broadcast: &'a mut Vec<SEND>,
}

impl<'a, SEND: ByteRepr + Clone> DelayedBroadcast<'a, SEND> {
    pub fn broadcast(&mut self, message: SEND) {
        self.broadcast.push(message);
    }
}

pub struct DelayedBroadcastExcept<'a, SEND: ByteRepr + Clone> {
    broadcast: &'a mut Vec<(SocketAddr, SEND)>,
}

impl<'a, SEND: ByteRepr + Clone> DelayedBroadcastExcept<'a, SEND> {
    pub fn broadcast_except(&mut self, message: SEND, exception: SocketAddr) {
        self.broadcast.push((exception, message));
    }
}

type DelayedForEachF<SEND, RECV> = Box<dyn FnMut(UdpCommunicatorMut<'_, SEND, RECV>)>;

pub struct DelayedForEach<'a, SEND: ByteRepr, RECV: ByteRepr> {
    for_each: &'a mut Vec<DelayedForEachF<SEND, RECV>>,
}

impl<'a, SEND, RECV> DelayedForEach<'a, SEND, RECV>
where
    SEND: ByteRepr + Clone,
    RECV: ByteRepr,
{
    pub fn for_each<F>(&mut self, f: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>) + 'static,
    {
        self.for_each.push(Box::new(f));
    }
}

pub struct IterMut<'a, SEND, RECV>
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    socket: &'a UdpCommunicatorSocket,
    inner: std::collections::hash_map::IterMut<'a, SocketAddr, InnerUdpCommunicator<SEND, RECV>>,
}

impl<'a, SEND, RECV> Iterator for IterMut<'a, SEND, RECV>
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    type Item = UdpCommunicatorMut<'a, SEND, RECV>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(addr, inner)| UdpCommunicatorMut {
            socket: self.socket,
            addr: *addr,
            inner,
        })
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> MultiUdpCommunicator<SEND, RECV> {
    #[cfg(debug_assertions)]
    /// Simulate fake UDP unreliability by randomly dropping packets according to the provided
    /// probability.
    /// This is currently only available on debug builds.
    pub fn with_fake_drop(mut self, drop_probability: f64) -> Self {
        self.socket = self.socket.with_fake_drop(drop_probability);
        self
    }

    #[cfg(debug_assertions)]
    /// Simulate fake UDP unreliability by randomly corrupting bits of packets according to the
    /// provided probability (the probability determines how likely it is for a packet to be
    /// corrupted, not how many bits will be flipped).
    /// This is currently only available on debug builds.
    pub fn with_fake_corruption(mut self, corruption_probability: f64) -> Self {
        self.socket = self.socket.with_fake_corruption(corruption_probability);
        self
    }

    #[cfg(debug_assertions)]
    /// Add an extra delay to packet receiving by a random amount of milliseconds in the range of
    /// the provided `delay_ms`.
    /// Only packet receiving is affected by this, not sending.
    /// This is currently only available on debug builds.
    pub fn with_fake_delay(mut self, delay_ms: std::ops::Range<u64>) -> Self {
        self.socket = self.socket.with_fake_delay(delay_ms);
        self
    }

    #[cfg(debug_assertions)]
    /// Enable debug logs like notifications when a packet has been artificially corrupted by
    /// [`Self::with_fake_corruption`].
    /// This is currently only available on debug builds.
    pub fn with_debug_logs(mut self) -> Self {
        self.socket = self.socket.with_debug_logs();
        self
    }
}

#[cfg(test)]
mod test {
    use crate::prelude::*;

    #[test]
    fn multi_udp_communicator_iter_mut() {
        let mut multi = MultiUdpCommunicator::<(), ()>::bind("0.0.0.0:7400");
        let mut com1 = UdpCommunicator::<(), ()>::bind("0.0.0.0:7401")
            .connect("0.0.0.0:7400")
            .unwrap();
        let mut com2 = UdpCommunicator::<(), ()>::bind("0.0.0.0:7402")
            .connect("0.0.0.0:7400")
            .unwrap();
        let mut com3 = UdpCommunicator::<(), ()>::bind("0.0.0.0:7403")
            .connect("0.0.0.0:7400")
            .unwrap();
        com1.write_heartbeat();
        com2.write_heartbeat();
        com3.write_heartbeat();
        com1.send().unwrap();
        com2.send().unwrap();
        com3.send().unwrap();
        multi.recv(|_com: UdpCommunicatorMut<_, _>| {});
        let mut ports = vec![7401, 7402, 7403];
        for com in multi.iter_mut() {
            let port = com.addr.port();
            let i = ports.iter().position(|p| *p == port).unwrap();
            ports.remove(i);
        }
        assert!(ports.is_empty());
    }
}
