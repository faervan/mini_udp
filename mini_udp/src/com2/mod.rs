use std::net::ToSocketAddrs;

use crate::prelude2::*;

mod msg_sink;
pub use msg_sink::*;

mod msg_trace;
pub use msg_trace::*;

mod connection;
pub use connection::*;

mod protection;
pub use protection::*;

mod inner;
pub(crate) use inner::*;

mod socket;
pub use socket::*;

pub trait Communicator<Config: MiniUdpConfig>: MightHaveWork {
    fn connection_state(
        &self,
    ) -> &ConnectionState<<Config::Context as MiniUdpContext>::ConnectionHandler>;
    /// Queue a message to be send unreliably.
    fn write(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, Config::UnreliablePacketHandler>;
    /// Queue a message to be send reliably, but not necessarily in order.
    // fn write_reliable(&mut self, message: <Config::Context as MiniUdpContext>::Send);
    /// Queue a message to be send reliably, in order. The receiver will make sure not to show this
    /// message before any previously send, ordered messages.
    fn write_ordered(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, Config::ReliableOrderedPacketHandler>;
    /// Add a heartbeat to the internal send queue. This behaves like the other `write*` methods,
    /// it does not actually send the packet yet.
    // fn write_heartbeat(&mut self);
    /// Try to read the next unreliable or reliable-unordered message received.
    // fn read(&mut self) -> Option<<Config::Context as MiniUdpContext>::Recv>;
    /// Try to read the reliable-ordered message received.
    fn read_ordered(&mut self) -> Option<<Config::Context as MiniUdpContext>::Recv>;
    /// Returns the [Instant] of time at which the last packet has been received.
    /// Before any packets have been received, this returns the [Instant] at which this
    /// [Communicator] has been constructed.
    fn last_seen(&self) -> Instant;
    /// Returns the [Instant] of time at which the last packet has been send.
    /// Before any packets have been send, this returns the [Instant] at which this [Communicator]
    /// has been constructed.
    fn last_send(&self) -> Instant;
}

pub struct UdpCommunicator<Config: MiniUdpConfig> {
    socket: UdpCommunicatorSocket<Config::Context>,
    inner: InnerCommunicator<Config>,
}

impl<Config: MiniUdpConfig> Communicator<Config> for UdpCommunicator<Config> {
    fn connection_state(
        &self,
    ) -> &ConnectionState<<<Config as MiniUdpConfig>::Context as MiniUdpContext>::ConnectionHandler>
    {
        self.inner.connection.state()
    }

    fn write(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, Config::UnreliablePacketHandler> {
        self.inner.unreliable.new_message(message)
    }

    fn write_ordered(
        &mut self,
        message: <<Config as MiniUdpConfig>::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, <Config as MiniUdpConfig>::ReliableOrderedPacketHandler> {
        self.inner.reliable_ordered.new_message(message)
    }

    fn read_ordered(
        &mut self,
    ) -> Option<<<Config as MiniUdpConfig>::Context as MiniUdpContext>::Recv> {
        self.inner.reliable_ordered.next()
    }

    fn last_seen(&self) -> Instant {
        self.inner.last_seen
    }

    fn last_send(&self) -> Instant {
        self.inner.last_send
    }
}

impl<Config> UdpCommunicator<Config>
where
    Config: MiniUdpConfig,
    Config::Context: MiniUdpContext<ConnectionHandler = InsecureConnection>,
{
    pub fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or(Error::NoSocketAddrYield)?;
        self.inner.connection.connect::<Config>(
            &mut self.inner.resend_handler,
            addr,
            (),
            &mut self.inner.reliable_ordered,
        )
    }
}

impl<Config: MiniUdpConfig> UdpCommunicator<Config> {
    pub fn recv(&mut self) {
        let _span =
            trace_span!("UdpCommunicator::recv", "bound to {:?}", self.local_addr()).entered();
        while let Ok((n, addr)) = self.socket.socket.recv_from(&mut self.socket.data_buffer) {
            match self
                .inner
                .connection
                .read_packet::<Config>(addr, &mut self.socket.data_buffer[..n])
            {
                Ok(Packet::Connection {
                    sequence_id,
                    ack,
                    con_data,
                }) => {
                    let _span = trace_span!("received Packet::Connection").entered();
                    match self.inner.reliable_ordered.read_packet(sequence_id, vec![]) {
                        // Packet has already been received.
                        Ok(true) => {
                            self.inner.received_packet_duplicate = true;
                            continue;
                        }
                        // Received a new packet.
                        Ok(false) => {
                            match self.inner.connection.incoming_connect::<_, Config>(
                                &mut self.inner.resend_handler,
                                addr,
                                con_data,
                                |_| true,
                                &mut self.inner.reliable_ordered,
                            ) {
                                Ok(true) => {
                                    // Connection established
                                    self.inner.acknowledge(ack);
                                }
                                Ok(false) => {
                                    // Connection establishment is proceeding
                                    self.inner.acknowledge(ack);
                                }
                                Err(e) => self.socket.handle_error(e),
                            }
                        }
                        // Error reading packet.
                        Err(e) => {
                            self.socket.handle_error(e);
                            continue;
                        }
                    }
                }
                Ok(Packet::Data {
                    sequence_id,
                    ack,
                    data,
                }) => {
                    let _span = trace_span!("received Packet::Data").entered();
                    todo!()
                }
                Err(e) => {
                    let _span = trace_span!("packet read error").entered();
                    self.socket.handle_error(e);
                }
            }
        }
    }

    pub fn send(&mut self) -> Result<(), Error> {
        let _span =
            trace_span!("UdpCommunicator::send", "bound to {:?}", self.local_addr()).entered();
        if self.inner.received_packet_duplicate {
            // TODO! Send heartbeat automatically?
            self.inner.received_packet_duplicate = false;
        }
        self.inner.flush_messages(
            #[cfg(any(test, feature = "debug"))]
            &self.socket,
        )?;
        self.inner.send_packets(&mut self.socket)
    }
}

pub trait MightHaveWork {
    fn has_work(&self) -> bool;
}

impl<Config: MiniUdpConfig> MightHaveWork for UdpCommunicator<Config> {
    /// Returns `true` if there are any pending messages to be send / packets to get acknowledged.
    fn has_work(&self) -> bool {
        self.inner.has_work()
    }
}

pub trait MessageSource<Config: MiniUdpConfig> {
    fn next(&mut self) -> Option<<Config::Context as MiniUdpContext>::Recv>;
}

impl<Config: MiniUdpConfig> CommunicatorSocket<Config::Context> for UdpCommunicator<Config> {
    fn bind_with<A: ToSocketAddrs>(
        addr: A,
        error_handler: <<Config::Context as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler,
    ) -> Self {
        Self {
            socket: UdpCommunicatorSocket::bind_with(addr, error_handler),
            inner: InnerCommunicator::new(),
        }
    }

    fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.socket.local_addr()
    }

    fn get_error_handler_mut(
        &mut self,
    ) -> &mut <<Config::Context as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler
    {
        self.socket.get_error_handler_mut()
    }

    fn get_resend_handler_mut(
        &mut self,
    ) -> &mut <Config::Context as MiniUdpContext>::ResendStrategy {
        self.socket.get_resend_handler_mut()
    }
}

#[cfg(test)]
mod test {
    use crate::prelude2::*;

    #[test]
    fn connect() {
        let _ = tracing_subscriber::FmtSubscriber::builder()
            .with_test_writer()
            .with_max_level(tracing::Level::TRACE)
            .try_init();
        #[derive(ByteRepr, Debug)]
        enum Msg {
            Hello,
            Bye,
        }
        type Cfg = UdpConfig<Msg, Msg, 0>;
        let mut com1 = UdpCommunicator::<Cfg>::bind_with("127.0.0.1:7500", ());
        let mut com2 = UdpCommunicator::<Cfg>::bind_with("127.0.0.1:7501", ());

        assert!(com1.connection_state().idle());
        com1.connect(com2.local_addr().unwrap()).unwrap();
        assert!(com1.connection_state().connecting());
        assert!(com2.connection_state().idle());

        for _ in 0..5 {
            com1.recv();
            com1.send().unwrap();
            com2.recv();
            com2.send().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        debug!("com1: {:?}", com1.connection_state());
        debug!("com1: {:?}", com2.connection_state());

        assert!(com1.connection_state().connected());
        assert!(com2.connection_state().connected());
    }
}
