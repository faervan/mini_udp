use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs},
};

use crate::{communicator::UdpCommunicatorMut, prelude::*};

pub trait MultiCommunicator<CTX: MiniUdpContext, PacketHandling: PacketHandler> {
    /// Create a connection to the provided `addr`.
    fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<UdpCommunicatorMut<'_, CTX, PacketHandling>, std::io::Error>;
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
    /// const PROTOCOL_VERSION: u32 = 109;
    /// type ClientCtx = UdpContext<u8, (), PROTOCOL_VERSION>;
    /// type ServerCtx = <ClientCtx as MiniUdpContext>::Reverse;
    ///
    /// let mut multi = MultiUdpCommunicator::<ServerCtx>::bind("0.0.0.0:7000");
    /// let mut com =
    ///     UdpCommunicator::<ClientCtx>::default().connect("0.0.0.0:7000").unwrap();
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
        CB: OnReceiveCallback<M, CTX, PacketHandling>;
    /// Send all pending messages. This will also resend reliable, unacknowledged packets if the
    /// configured resend interval has been reached.
    /// If new packets have been received since the last time this method was called and there are
    /// no pending messages to be send or packets to be resend, this will send an empty heartbeat
    /// packet to the connected receiver.
    fn send(&mut self);
    /// Iterate over all client connections. A client connection is automatically created when
    /// [`recv`](Self::recv) reads a packet from a [`SocketAddr`] that does not already have a
    /// connection. Connections are never automatically deleted by [`mini_udp`].
    fn iter_mut(&mut self) -> IterMut<'_, CTX, PacketHandling>;
    /// Broadcast the provided `message` to all connections, reliably.
    fn broadcast(&mut self, message: CTX::Send)
    where
        CTX::Send: Clone;
    /// Broadcast the provided `message` to all connections except `exception`, reliably.
    fn broadcast_except(&mut self, message: CTX::Send, exception: SocketAddr)
    where
        CTX::Send: Clone;
    /// Execute `f` for each connection.
    fn for_each<F>(&mut self, f: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, CTX, PacketHandling>);
    /// Remove all connections for which the provided `filter` closure returns `false`.
    fn retain<F>(&mut self, filter: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, CTX, PacketHandling>) -> bool;
    /// Remove the connection to `addr`, returning whether a connection to `addr` existed.
    fn remove(&mut self, addr: &SocketAddr) -> bool;
    /// Get the connection to `addr`, if it exists, so you can read or queue messages.
    fn get_mut<'a>(
        &'a mut self,
        addr: &'a SocketAddr,
    ) -> Option<UdpCommunicatorMut<'a, CTX, PacketHandling>>;
    #[inline(always)]
    /// A shorthand for [`self.recv()`](Self::recv) followed by [`self.send()`](Self::send).
    fn tick<M, CB>(&mut self, on_recv: CB)
    where
        CB: OnReceiveCallback<M, CTX, PacketHandling>,
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
/// const PROTOCOL_VERSION: u32 = 17;
/// type UdpCtx = UdpContext<Message, Message, PROTOCOL_VERSION>;
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
/// let mut multi1 = MultiUdpCommunicator::<_, DefaultPacketHandler>::default();
/// let mut multi2 = MultiUdpCommunicator::<UdpCtx>::bind("0.0.0.0:7005");
///
/// // Add a connection to multi1. This will not send any data to the provided address,
/// // but enables us to start sending messages to it.
/// let mut com: UdpCommunicatorMut<UdpCtx, _> =
///     multi1.connect("0.0.0.0:7005").unwrap();
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
pub struct MultiUdpCommunicator<
    CTX: MiniUdpContext,
    PacketHandling: PacketHandler = DefaultPacketHandler,
> {
    pub(super) socket: UdpCommunicatorSocket<CTX>,
    coms: HashMap<SocketAddr, InnerUdpCommunicator<CTX, PacketHandling>>,
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> Default
    for MultiUdpCommunicator<CTX, PacketHandling>
where
    <CTX::ErrorHandling as ErrorHandlingStrategy>::Handler: Default,
{
    #[inline(always)]
    fn default() -> Self {
        Self::bind("0.0.0.0:0")
    }
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> MultiUdpCommunicator<CTX, PacketHandling>
where
    <CTX::ErrorHandling as ErrorHandlingStrategy>::Handler: Default,
{
    /// Create a new [`MultiUdpCommunicator`], binding it to the provided `addr`.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind_with(addr, Default::default()),
            coms: HashMap::new(),
        }
    }
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> CommunicatorSocket<CTX>
    for MultiUdpCommunicator<CTX, PacketHandling>
{
    fn bind_with<A: ToSocketAddrs>(
        addr: A,
        error_handler: <<CTX as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler,
    ) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind_with(addr, error_handler),
            coms: HashMap::new(),
        }
    }

    #[inline(always)]
    fn get_error_handler_mut(
        &mut self,
    ) -> &mut <<CTX as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler {
        self.socket.get_error_handler_mut()
    }

    #[inline(always)]
    fn get_resend_handler_mut(&mut self) -> &mut <CTX as MiniUdpContext>::ResendStrategy {
        self.socket.get_resend_handler_mut()
    }
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> MultiCommunicator<CTX, PacketHandling>
    for MultiUdpCommunicator<CTX, PacketHandling>
{
    fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<UdpCommunicatorMut<'_, CTX, PacketHandling>, std::io::Error> {
        let addr = addr.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::other("The provided `addr` parses into an empty SocketAddr iterator")
        })?;
        Ok(UdpCommunicatorMut {
            #[cfg(any(test, feature = "debug"))]
            socket: &self.socket,
            addr,
            inner: self.coms.entry(addr).or_default(),
        })
    }

    fn recv<M, CB>(&mut self, mut on_recv: CB)
    where
        CB: OnReceiveCallback<M, CTX, PacketHandling>,
    {
        let mut state = on_recv.prepare();
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            #[cfg(any(test, feature = "debug"))]
            if self.socket.delay_packet(Some(addr), n) {
                continue;
            }
            let com = self.coms.entry(addr).or_default();
            if let Err(error) = com.read_packet(n, &mut self.socket) {
                CTX::ErrorHandling::handle_error(&mut self.socket.error_handler, error);
            }
            on_recv.on_recv(
                UdpCommunicatorMut {
                    #[cfg(any(test, feature = "debug"))]
                    socket: &self.socket,
                    addr,
                    inner: com,
                },
                &mut state,
            );
        }
        #[cfg(any(test, feature = "debug"))]
        while let Some((n, Some(addr))) = self.socket.read_delayed() {
            let com = self.coms.entry(addr).or_default();
            if let Err(error) = com.read_packet(n, &mut self.socket) {
                CTX::ErrorHandling::handle_error(&mut self.socket.error_handler, error);
            }
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
            if let Err(error) = inner.send(*addr, &mut self.socket) {
                // TODO! the communicator addr should be propagated to the error handler
                #[cfg(any(test, feature = "debug"))]
                warn!("Failed to send Communicator of {addr:?}: {error}");
                CTX::ErrorHandling::handle_error(&mut self.socket.error_handler, error);
            }
        }
    }

    fn iter_mut(&mut self) -> IterMut<'_, CTX, PacketHandling> {
        IterMut {
            #[cfg(any(test, feature = "debug"))]
            socket: &self.socket,
            inner: self.coms.iter_mut(),
        }
    }

    fn broadcast(&mut self, message: CTX::Send)
    where
        CTX::Send: Clone,
    {
        for com in self.coms.values_mut() {
            com.reliable_send_queue.push(message.clone());
        }
    }

    fn broadcast_except(&mut self, message: CTX::Send, exception: SocketAddr)
    where
        CTX::Send: Clone,
    {
        for (_, com) in self.coms.iter_mut().filter(|(addr, _)| **addr != exception) {
            com.reliable_send_queue.push(message.clone());
        }
    }

    fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, CTX, PacketHandling>),
    {
        for (addr, com) in self.coms.iter_mut() {
            f(UdpCommunicatorMut {
                #[cfg(any(test, feature = "debug"))]
                socket: &self.socket,
                addr: *addr,
                inner: com,
            });
        }
    }

    fn retain<F>(&mut self, mut filter: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, CTX, PacketHandling>) -> bool,
    {
        self.coms.retain(|addr, inner| {
            filter(UdpCommunicatorMut {
                #[cfg(any(test, feature = "debug"))]
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
    ) -> Option<UdpCommunicatorMut<'a, CTX, PacketHandling>> {
        let inner = self.coms.get_mut(addr)?;
        Some(UdpCommunicatorMut {
            #[cfg(any(test, feature = "debug"))]
            socket: &self.socket,
            addr: *addr,
            inner,
        })
    }
}

pub trait OnReceiveCallback<M, CTX: MiniUdpContext, PacketHandling: PacketHandler> {
    type State;
    /// Will be called at the beginning of each [`MultiCommunicator::recv`] invocation.
    fn prepare(&mut self) -> Self::State;
    /// Will be called for each packet that has been received during a [`MultiCommunicator::recv`]
    /// invocation.
    fn on_recv(&mut self, com: UdpCommunicatorMut<CTX, PacketHandling>, state: &mut Self::State);
    #[allow(unused_variables)]
    /// Will be called at the end of each [`MultiCommunicator::recv`] invocation.
    fn finish(&mut self, com: &mut MultiUdpCommunicator<CTX, PacketHandling>, state: Self::State) {}
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> OnReceiveCallback<(), CTX, PacketHandling>
    for ()
{
    type State = ();
    fn prepare(&mut self) -> Self::State {}
    fn on_recv(&mut self, _com: UdpCommunicatorMut<CTX, PacketHandling>, _state: &mut Self::State) {
    }
}

impl<T, CTX: MiniUdpContext, PacketHandling: PacketHandler>
    OnReceiveCallback<UdpCommunicatorMut<'_, CTX, PacketHandling>, CTX, PacketHandling> for T
where
    T: FnMut(UdpCommunicatorMut<CTX, PacketHandling>),
{
    type State = ();
    fn prepare(&mut self) -> Self::State {}
    fn on_recv(&mut self, com: UdpCommunicatorMut<CTX, PacketHandling>, _state: &mut Self::State) {
        self(com);
    }
}

impl<T, CTX: MiniUdpContext, PacketHandling: PacketHandler>
    OnReceiveCallback<
        (
            UdpCommunicatorMut<'_, CTX, PacketHandling>,
            DelayedBroadcast<'_, CTX::Send>,
        ),
        CTX,
        PacketHandling,
    > for T
where
    T: FnMut(UdpCommunicatorMut<CTX, PacketHandling>, DelayedBroadcast<CTX::Send>),
    CTX::Send: Clone,
{
    type State = Vec<CTX::Send>;
    fn prepare(&mut self) -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<CTX, PacketHandling>, state: &mut Self::State) {
        self(com, DelayedBroadcast { broadcast: state });
    }
    fn finish(&mut self, com: &mut MultiUdpCommunicator<CTX, PacketHandling>, state: Self::State) {
        for message in state {
            com.broadcast(message);
        }
    }
}

impl<T, CTX: MiniUdpContext, PacketHandling: PacketHandler>
    OnReceiveCallback<
        (
            UdpCommunicatorMut<'_, CTX, PacketHandling>,
            DelayedBroadcastExcept<'_, CTX::Send>,
        ),
        CTX,
        PacketHandling,
    > for T
where
    T: FnMut(UdpCommunicatorMut<CTX, PacketHandling>, DelayedBroadcastExcept<CTX::Send>),
    CTX::Send: Clone,
{
    type State = Vec<(SocketAddr, CTX::Send)>;
    fn prepare(&mut self) -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<CTX, PacketHandling>, state: &mut Self::State) {
        self(com, DelayedBroadcastExcept { broadcast: state });
    }
    fn finish(&mut self, com: &mut MultiUdpCommunicator<CTX, PacketHandling>, state: Self::State) {
        for (exception, message) in state {
            com.broadcast_except(message, exception);
        }
    }
}

impl<T, CTX: MiniUdpContext, PacketHandling: PacketHandler>
    OnReceiveCallback<
        (
            UdpCommunicatorMut<'_, CTX, PacketHandling>,
            DelayedForEach<'_, CTX, PacketHandling>,
        ),
        CTX,
        PacketHandling,
    > for T
where
    T: FnMut(UdpCommunicatorMut<CTX, PacketHandling>, DelayedForEach<CTX, PacketHandling>),
    CTX::Send: Clone,
{
    type State = Vec<DelayedForEachF<CTX, PacketHandling>>;
    fn prepare(&mut self) -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<CTX, PacketHandling>, state: &mut Self::State) {
        self(com, DelayedForEach { for_each: state });
    }
    fn finish(&mut self, com: &mut MultiUdpCommunicator<CTX, PacketHandling>, state: Self::State) {
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

type DelayedForEachF<CTX, PacketHandling> =
    Box<dyn FnMut(UdpCommunicatorMut<'_, CTX, PacketHandling>)>;

pub struct DelayedForEach<'a, CTX: MiniUdpContext, PacketHandling: PacketHandler> {
    for_each: &'a mut Vec<DelayedForEachF<CTX, PacketHandling>>,
}

impl<'a, CTX: MiniUdpContext, PacketHandling: PacketHandler>
    DelayedForEach<'a, CTX, PacketHandling>
{
    pub fn for_each<F>(&mut self, f: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, CTX, PacketHandling>) + 'static,
    {
        self.for_each.push(Box::new(f));
    }
}

pub struct IterMut<'a, CTX: MiniUdpContext, PacketHandling: PacketHandler> {
    #[cfg(any(test, feature = "debug"))]
    socket: &'a UdpCommunicatorSocket<CTX>,
    inner: std::collections::hash_map::IterMut<
        'a,
        SocketAddr,
        InnerUdpCommunicator<CTX, PacketHandling>,
    >,
}

impl<'a, CTX: MiniUdpContext, PacketHandling: PacketHandler> Iterator
    for IterMut<'a, CTX, PacketHandling>
{
    type Item = UdpCommunicatorMut<'a, CTX, PacketHandling>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(addr, inner)| UdpCommunicatorMut {
            #[cfg(any(test, feature = "debug"))]
            socket: self.socket,
            addr: *addr,
            inner,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::prelude::*;

    type UdpCtx = UdpContext<(), (), 1>;

    #[test]
    fn multi_udp_communicator_iter_mut() {
        let mut multi = MultiUdpCommunicator::<UdpCtx>::bind("0.0.0.0:7400");
        let mut com1 = UdpCommunicator::<UdpCtx>::bind("0.0.0.0:7401")
            .connect("0.0.0.0:7400")
            .unwrap();
        let mut com2 = UdpCommunicator::<UdpCtx>::bind("0.0.0.0:7402")
            .connect("0.0.0.0:7400")
            .unwrap();
        let mut com3 = UdpCommunicator::<UdpCtx>::bind("0.0.0.0:7403")
            .connect("0.0.0.0:7400")
            .unwrap();
        com1.write_heartbeat();
        com2.write_heartbeat();
        com3.write_heartbeat();
        com1.send().unwrap();
        com2.send().unwrap();
        com3.send().unwrap();
        multi.recv(|_com: UdpCommunicatorMut<UdpCtx, _>| {});
        let mut ports = vec![7401, 7402, 7403];
        for com in multi.iter_mut() {
            let port = com.addr.port();
            let i = ports.iter().position(|p| *p == port).unwrap();
            ports.remove(i);
        }
        assert!(ports.is_empty());
    }
}
