use crate::prelude2::*;

mod msg_sink;
pub use msg_sink::*;

mod msg_trace;
pub use msg_trace::*;

mod connection;
pub use connection::*;

mod security;
pub use security::*;

mod inner;
pub(crate) use inner::*;

mod socket;
pub use socket::*;

pub trait Communicator<Config: MiniUdpConfig>: MightHaveWork {
    /// Queue a message to be send unreliably.
    fn write(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, Config::UnreliablePacketHandler>;
    /// Queue a message to be send reliably, but not necessarily in order.
    // fn write_reliable(&mut self, message: <Config::Context as MiniUdpContext>::Send);
    /// Queue a message to be send reliably, in order. The receiver will make sure not to show this
    /// message before any previously send, ordered messages.
    // fn write_ordered(&mut self, message: <Config::Context as MiniUdpContext>::Send);
    /// Add a heartbeat to the internal send queue. This behaves like the other `write*` methods,
    /// it does not actually send the packet yet.
    // fn write_heartbeat(&mut self);
    /// Try to read the next unreliable or reliable-unordered message received.
    // fn read(&mut self) -> Option<<Config::Context as MiniUdpContext>::Recv>;
    /// Try to read the reliable-ordered message received.
    // fn read_ordered(&mut self) -> Option<<Config::Context as MiniUdpContext>::Recv>;
    /// Returns the [Instant] of time at which the last packet has been received.
    /// Before any packets have been received, this returns the [Instant] at which this
    /// [Communicator] has been constructed.
    fn last_seen(&self) -> Instant;
    /// Returns the [Instant] of time at which the last packet has been send.
    /// Before any packets have been send, this returns the [Instant] at which this [Communicator]
    /// has been constructed.
    fn last_send(&self) -> Instant;
}

pub trait MightHaveWork {
    fn has_work(&self) -> bool;
}

pub struct UdpCommunicator<Config: MiniUdpConfig> {
    socket: UdpCommunicatorSocket<Config::Context>,
    inner: InnerCommunicator<Config>,
}

impl<Config: MiniUdpConfig> Communicator<Config> for UdpCommunicator<Config> {
    fn write(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, Config::UnreliablePacketHandler> {
        self.inner.unreliable.new_message(message)
    }

    fn last_seen(&self) -> Instant {
        self.inner.last_seen
    }

    fn last_send(&self) -> Instant {
        self.inner.last_send
    }
}

impl<Config: MiniUdpConfig> MightHaveWork for UdpCommunicator<Config> {
    /// Returns `true` if there are any pending messages to be send / packets to get acknowledged.
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }
}

impl<Config: MiniUdpConfig> UdpCommunicator<Config> {
    pub fn recv(&mut self) {
        todo!()
    }

    pub fn send(&mut self) -> Result<(), Error> {
        todo!()
    }
}
