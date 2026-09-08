use crate::prelude2::*;

pub(crate) struct InnerCommunicator<Config: MiniUdpConfig> {
    pub connection: <Config::Context as MiniUdpContext>::ConnectionHandler,
    pub resend_handler: <Config::Context as MiniUdpContext>::ResendStrategy,
    pub unreliable: Config::UnreliablePacketHandler,
    pub reliable: Config::ReliablePacketHandler,
    pub reliable_ordered: Config::ReliableOrderedPacketHandler,
    pub unreliable_fragmented: Config::UnreliableFragmentationHandler,
    pub reliable_fragmented: Config::ReliableFragmentationHandler,
    /// If this is `true`, a packet has been received more than once, potentially meaning that we
    /// have to send an ack to the other side.
    pub received_packet_duplicate: bool,
    pub last_seen: Instant,
    pub last_send: Instant,
}

impl<Config: MiniUdpConfig> InnerCommunicator<Config> {
    pub fn new() -> Self {
        Self {
            connection: Default::default(),
            resend_handler: Default::default(),
            unreliable: Default::default(),
            reliable: Default::default(),
            reliable_ordered: Default::default(),
            unreliable_fragmented: Default::default(),
            reliable_fragmented: Default::default(),
            received_packet_duplicate: false,
            last_seen: Instant::now(),
            last_send: Instant::now(),
        }
    }

    pub fn flush_messages(
        &mut self,
        #[cfg(any(test, feature = "debug"))] socket: &UdpCommunicatorSocket<Config::Context>,
    ) -> Result<(), Error> {
        let _span = trace_span!("InnerCommunicator::flush_messages").entered();
        let ack = self.create_ack();
        // TODO! Flush unreliable
        // TODO! Flush reliable unordered
        self.reliable_ordered.flush_messages(
            &mut self.connection,
            &mut self.resend_handler,
            ack,
            #[cfg(any(test, feature = "debug"))]
            socket,
        )?;
        Ok(())
    }

    pub fn send_packets(
        &mut self,
        socket: &mut UdpCommunicatorSocket<Config::Context>,
    ) -> Result<(), Error> {
        let _span = trace_span!("InnerCommunicator::send_packets").entered();
        let mut any_send = false;

        let Some(addr) = self.connection.send_addr() else {
            trace!("Not connected, skipping send.");
            return Ok(());
        };
        self.reliable_ordered.send(socket, *addr, &mut any_send);

        if any_send {
            self.last_send = Instant::now();
        }

        Ok(())
    }

    pub fn has_work(&self) -> bool {
        self.unreliable.has_work()
    }
}
