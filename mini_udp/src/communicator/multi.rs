use std::{
    collections::HashMap,
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
};

use crate::{communicator::UdpCommunicatorMut, prelude::*};

pub trait MultiCommunicator<SEND: ByteRepr, RECV: ByteRepr> {
    fn recv<M, CB>(&mut self, on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>;
    fn send(&mut self);
    /// Sadly not a real [Iterator]
    fn iter_mut(&mut self) -> IterMut<'_, SEND, RECV>;
    fn broadcast(&mut self, message: SEND)
    where
        SEND: Clone;
    fn broadcast_except(&mut self, message: SEND, exception: SocketAddr)
    where
        SEND: Clone;
    fn for_each<F>(&mut self, f: F)
    where
        SEND: Clone,
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>);
    fn retain<F>(&mut self, filter: F)
    where
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>) -> bool;
    fn remove(&mut self, addr: &SocketAddr) -> bool;
    fn tick<M, CB>(&mut self, on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>,
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

impl<SEND: ByteRepr, RECV: ByteRepr> Default for MultiUdpCommunicator<SEND, RECV> {
    #[inline(always)]
    fn default() -> Self {
        Self::bind("0.0.0.0:0")
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> CommunicatorSocket for MultiUdpCommunicator<SEND, RECV> {
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

/// TODO: Remove where Debug
impl<SEND: ByteRepr + Debug, RECV: ByteRepr + Debug> MultiCommunicator<SEND, RECV>
    for MultiUdpCommunicator<SEND, RECV>
{
    fn recv<M, CB>(&mut self, mut on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>,
    {
        let mut state = CB::prepare();
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            #[cfg(debug_assertions)]
            if self.socket.delay_packet(Some(addr), n) {
                continue;
            }
            let com = self.coms.entry(addr).or_default();
            com.read_packet(n, &mut self.socket);
            on_recv.on_recv(
                UdpCommunicatorMut {
                    socket: &mut self.socket,
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
                    socket: &mut self.socket,
                    addr,
                    inner: com,
                },
                &mut state,
            );
        }
        CB::finish(self, state);
    }

    fn send(&mut self) {
        for (addr, inner) in &mut self.coms {
            if let Err(e) = {
                UdpCommunicatorMut {
                    socket: &mut self.socket,
                    addr: *addr,
                    inner,
                }
                .tick()
            } {
                warn!("Failed to send Communicator of {addr:?}: {e}");
            }
        }
    }

    fn iter_mut(&mut self) -> IterMut<'_, SEND, RECV> {
        IterMut {
            socket: &mut self.socket,
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
        SEND: Clone,
        F: FnMut(UdpCommunicatorMut<'_, SEND, RECV>),
    {
        for (addr, com) in self.coms.iter_mut() {
            f(UdpCommunicatorMut {
                socket: &mut self.socket,
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
                socket: &mut self.socket,
                addr: *addr,
                inner,
            })
        });
    }

    fn remove(&mut self, addr: &SocketAddr) -> bool {
        self.coms.remove(addr).is_some()
    }

    fn tick<M, CB>(&mut self, mut on_recv: CB)
    where
        CB: OnReceiveCallback<M, SEND, RECV>,
        SEND: Debug + Clone,
        RECV: Debug,
    {
        let mut state = CB::prepare();
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            let com = self.coms.entry(addr).or_default();
            com.read_packet(n, &mut self.socket);
            on_recv.on_recv(
                UdpCommunicatorMut {
                    socket: &mut self.socket,
                    addr,
                    inner: com,
                },
                &mut state,
            );
        }
        CB::finish(self, state);

        self.send();
    }

    fn get_mut<'a>(
        &'a mut self,
        client: &'a SocketAddr,
    ) -> Option<UdpCommunicatorMut<'a, SEND, RECV>> {
        let inner = self.coms.get_mut(client)?;
        Some(UdpCommunicatorMut {
            socket: &mut self.socket,
            addr: *client,
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
    fn prepare() -> Self::State;
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State);
    #[allow(unused_variables)]
    fn finish(com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {}
}

impl<T, SEND, RECV> OnReceiveCallback<UdpCommunicatorMut<'_, SEND, RECV>, SEND, RECV> for T
where
    T: FnMut(UdpCommunicatorMut<SEND, RECV>),
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    type State = ();
    fn prepare() -> Self::State {}
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
    SEND: ByteRepr + Clone + Debug,
    RECV: ByteRepr + Debug,
{
    type State = Vec<SEND>;
    fn prepare() -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State) {
        self(com, DelayedBroadcast { broadcast: state });
    }
    fn finish(com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {
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
    SEND: ByteRepr + Clone + Debug,
    RECV: ByteRepr + Debug,
{
    type State = Vec<(SocketAddr, SEND)>;
    fn prepare() -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State) {
        self(com, DelayedBroadcastExcept { broadcast: state });
    }
    fn finish(com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {
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
    SEND: ByteRepr + Clone + Debug,
    RECV: ByteRepr + Debug,
{
    type State = Vec<DelayedForEachF<SEND, RECV>>;
    fn prepare() -> Self::State {
        vec![]
    }
    fn on_recv(&mut self, com: UdpCommunicatorMut<SEND, RECV>, state: &mut Self::State) {
        self(com, DelayedForEach { for_each: state });
    }
    fn finish(com: &mut MultiUdpCommunicator<SEND, RECV>, state: Self::State) {
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

pub struct DelayedForEach<'a, SEND: ByteRepr + Clone, RECV: ByteRepr> {
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

/// Sadly can't be a real [Iterator]
pub struct IterMut<'a, SEND, RECV>
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    socket: &'a mut UdpCommunicatorSocket,
    inner: std::collections::hash_map::IterMut<'a, SocketAddr, InnerUdpCommunicator<SEND, RECV>>,
}

impl<'a, SEND, RECV> IterMut<'a, SEND, RECV>
where
    SEND: ByteRepr,
    RECV: ByteRepr,
{
    #[allow(clippy::should_implement_trait)]
    pub fn next<'b>(&'b mut self) -> Option<UdpCommunicatorMut<'b, SEND, RECV>> {
        self.inner.next().map(|(addr, inner)| UdpCommunicatorMut {
            socket: self.socket,
            addr: *addr,
            inner,
        })
    }
}

impl<SEND: ByteRepr, RECV: ByteRepr> MultiUdpCommunicator<SEND, RECV> {
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
