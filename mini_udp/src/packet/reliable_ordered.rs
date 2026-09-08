use crate::prelude2::*;

pub trait ReliableOrderedPacketHandler<Config: MiniUdpConfig>:
    MessageSink<Config> + MessageSource<Config> + MightHaveWork + Debug + Default
{
    type MaxPackets: MaxConcurrentPackets;

    fn write_packet<'a>(
        &'a mut self,
        resend_handler: &'a mut <Config::Context as MiniUdpContext>::ResendStrategy,
        priority: Priority,
    ) -> Result<PacketWriteGuard<'a, Self::MaxPackets, impl FnOnce(usize) + 'a>, Error>;

    /// Returns `received_packet_duplicate` on success.
    /// That is:
    /// `Ok(false)` means "Successfully read the packet."
    /// `Ok(true)` means "The packet has already been read!"
    fn read_packet(
        &mut self,
        sequence_id: u16,
        messages: Vec<<Config::Context as MiniUdpContext>::Recv>,
    ) -> Result<bool, Error>;

    fn get_ack(&self) -> (u16, Self::MaxPackets);

    fn acknowledge(&mut self, newest_received: u16, ack_bits: Self::MaxPackets);

    fn flush_messages(
        &mut self,
        connection: &mut <Config::Context as MiniUdpContext>::ConnectionHandler,
        resend_handler: &mut <Config::Context as MiniUdpContext>::ResendStrategy,
        ack: PacketAck<
            <Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets,
            <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets,
        >,
        #[cfg(any(test, feature = "debug"))] socket: &UdpCommunicatorSocket<Config::Context>,
    ) -> Result<(), Error>;

    fn send<Addr>(
        &mut self,
        socket: &mut UdpCommunicatorSocket<Config::Context>,
        addr: Addr,
        any_send: &mut bool,
    ) where
        Addr: SocketSendAddr;
}

pub struct PacketWriteGuard<'a, MaxPackets: MaxConcurrentPackets, F>
where
    F: FnOnce(usize) + 'a,
{
    pub sequence_id: u16,
    pub partial_ack: PartialPacketAck<MaxPackets>,
    buffer: &'a mut [u8; BUFFER_SIZE],
    f: F,
}

impl<'a, MaxPackets, F> PacketWriteGuard<'a, MaxPackets, F>
where
    MaxPackets: MaxConcurrentPackets,
    F: FnOnce(usize) + 'a,
{
    pub fn write_to_connection<Config, M, P>(
        self,
        con: &mut <Config::Context as MiniUdpContext>::ConnectionHandler,
        create_packet: P,
    ) -> Result<(), Error>
    where
        Config: MiniUdpConfig,
        M: ByteRepr,
        P: FnOnce(
            u16,
            PartialPacketAck<MaxPackets>,
        )
            -> Packet<M, Config, <Config::Context as MiniUdpContext>::ConnectionHandler>,
    {
        let byte_len = con.write_packet(
            self.buffer,
            create_packet(self.sequence_id, self.partial_ack),
        )?;
        (self.f)(byte_len);
        Ok(())
    }
}

type MaybeMsgTrace = Option<MessageTracePacketUpdate<ReliablePacketState>>;

#[derive(Debug)]
/// `MaxPackets` specifies the maximum amount of packets that can be send in one direction
/// concurrently.
/// If `MaxPackets` is `u32`, then the sender can send 32 packets to the other side, but
/// will wait with sending the 33th packet until it receives an acknowledgement that the first
/// packet it send was actually received.
pub struct ReliableOrdered<Config: MiniUdpConfig, MaxPackets: MaxConcurrentPackets> {
    send_queue: VecDeque<(
        <Config::Context as MiniUdpContext>::Send,
        Priority,
        MaybeMsgTrace,
    )>,
    recv_queue: VecDeque<<Config::Context as MiniUdpContext>::Recv>,
    pending:
        MaxPackets::RingBuffer<PendingPacket<<Config::Context as MiniUdpContext>::ResendStrategy>>,
    send_buffer: MaxPackets::Array<[u8; BUFFER_SIZE]>,
    recv_buffer: MaxPackets::RingBuffer<Vec<<Config::Context as MiniUdpContext>::Recv>>,
    read_packet_head: u16,
    #[cfg(test)]
    received_packet_ids: std::collections::HashSet<u16>,
}

impl<Config: MiniUdpConfig, MaxPackets: MaxConcurrentPackets> Default
    for ReliableOrdered<Config, MaxPackets>
{
    fn default() -> Self {
        Self {
            send_queue: VecDeque::new(),
            recv_queue: VecDeque::new(),
            pending: Default::default(),
            send_buffer: MaxPackets::new_array([0; _]),
            recv_buffer: Default::default(),
            read_packet_head: 0,
            #[cfg(test)]
            received_packet_ids: std::collections::HashSet::new(),
        }
    }
}

impl<Config: MiniUdpConfig, MaxPackets: MaxConcurrentPackets> ReliableOrderedPacketHandler<Config>
    for ReliableOrdered<Config, MaxPackets>
{
    type MaxPackets = MaxPackets;

    fn write_packet<'a>(
        &'a mut self,
        resend_handler: &'a mut <Config::Context as MiniUdpContext>::ResendStrategy,
        priority: Priority,
    ) -> Result<PacketWriteGuard<'a, Self::MaxPackets, impl FnOnce(usize) + 'a>, Error> {
        if self.pending.push_will_override() {
            return Err(Error::PacketSendBufferFull);
        }
        let sequence_id = self.pending.get_next_index();
        let trace = PacketTrace::new(sequence_id, priority, ReliablePacketState::Constructed);
        let partial_ack = PartialPacketAck::new(self.get_ack());
        let pending_packets = &mut self.pending;
        Ok(PacketWriteGuard {
            sequence_id,
            partial_ack,
            buffer: MaxPackets::index_array_by_ringbuffer_index(&mut self.send_buffer, sequence_id),
            // After the ConnectionHandler has written the packet to the buffer, this closure will
            // be called with the amount of bytes written.
            f: move |byte_len| {
                let pending = PendingPacket::new(resend_handler, priority, byte_len, trace);
                pending_packets.push(pending);
            },
        })
    }

    fn read_packet(
        &mut self,
        sequence_id: u16,
        messages: Vec<<Config::Context as MiniUdpContext>::Recv>,
    ) -> Result<bool, Error> {
        let _span = trace_span!("ReliableOrdered::read_packet").entered();
        if self.recv_buffer.get(sequence_id).is_some() {
            #[cfg(test)]
            debug!("Received duplicate packet #{sequence_id}",);
            return Ok(true);
        }

        let newest_index = self.recv_buffer.get_newest_index();
        if wrapping_gt(newest_index.wrapping_sub(31), sequence_id, 64) {
            return Err(Error::PacketTooOld {
                sequence_id,
                newest_id: newest_index,
            });
        }

        trace!("Reading packet #{sequence_id} with {} msgs", messages.len());

        #[cfg(test)]
        assert!(self.received_packet_ids.insert(sequence_id));

        self.recv_buffer.insert(sequence_id, messages);
        for (id, messages) in self.recv_buffer.iter_mut() {
            if id == self.read_packet_head {
                self.read_packet_head = self.read_packet_head.wrapping_add(1);
                self.recv_queue.extend(messages.drain(..));
            } else if wrapping_gt(id, self.read_packet_head, 32) {
                break;
            }
        }

        Ok(false)
    }

    fn get_ack(&self) -> (u16, Self::MaxPackets) {
        let ordered_newest_received = self.recv_buffer.get_newest_index();
        let mut ordered_ack_bits = MaxPackets::UNSET;
        for i in self.recv_buffer.keys() {
            ordered_ack_bits.set_ack_flag(ordered_newest_received.wrapping_sub(i));
        }
        (ordered_newest_received, ordered_ack_bits)
    }

    fn acknowledge(&mut self, newest_received: u16, ack_bits: Self::MaxPackets) {
        let _span = trace_span!("ReliableOrdered::acknowledge").entered();
        for i in ack_bits.iter_acknowledged() {
            let index = newest_received.wrapping_sub(i);
            if let Some(PendingPacket { trace, .. }) = self.pending.take(index) {
                trace!("Received ack for packet #{index}");
                trace.update(|state| match state {
                    ReliablePacketState::Sending {
                        first_send,
                        times_send,
                    }
                    | ReliablePacketState::SendLimitReached {
                        first_send,
                        times_send,
                    } => {
                        *state = ReliablePacketState::Acknowledged {
                            first_send: *first_send,
                            times_send: *times_send,
                            ack_received: Instant::now(),
                        }
                    }
                    ReliablePacketState::Constructed => {
                        trace!("ERROR! Packet with state Constructed got acknowledged");
                    }
                    ReliablePacketState::Acknowledged { .. } => {
                        trace!(
                            "ERROR! Pending packet with state Acknowledged got acknowledged \
                            **and removed AGAIN**"
                        );
                    }
                });
            }
        }
    }

    fn flush_messages(
        &mut self,
        connection: &mut <Config::Context as MiniUdpContext>::ConnectionHandler,
        resend_handler: &mut <Config::Context as MiniUdpContext>::ResendStrategy,
        ack: PacketAck<
            <Config::ReliablePacketHandler as ReliablePacketHandler<Config::Context>>::MaxPackets,
            <Config::ReliableOrderedPacketHandler as ReliableOrderedPacketHandler<Config>>::MaxPackets,
        >,
        #[cfg(any(test, feature = "debug"))] socket: &UdpCommunicatorSocket<Config::Context>,
    ) -> Result<(), Error> {
        let _span = trace_span!("ReliableOrdered::flush_messages").entered();
        while !self.pending.push_will_override() && !self.send_queue.is_empty() {
            let mut available_bytes = MAX_PACKET_DATA_LEN;
            let mut included_msgs = 0;
            let mut priority = Priority::Low;
            for (msg, p, _) in self.send_queue.iter() {
                if msg.byte_len() <= available_bytes {
                    available_bytes -= msg.byte_len();
                    included_msgs += 1;
                    priority = priority.max(*p);
                } else {
                    break;
                }
            }
            if included_msgs == 0 {
                panic!(
                    "Msg {:#?} is too large (byte_len = {}), the max packet size is {}",
                    self.send_queue[0],
                    self.send_queue[0].0.byte_len(),
                    MAX_PACKET_DATA_LEN
                );
            }
            let index = self.pending.get_next_index();
            trace!(
                "Constructing packet #{index} with {included_msgs} msgs at priority {priority:?}"
            );
            let trace = PacketTrace::new(index, priority, ReliablePacketState::Constructed);
            let messages =
                self.send_queue
                    .drain(..included_msgs)
                    .fold(vec![], |mut msgs, (m, _p, t)| {
                        if let Some(t) = t {
                            trace!("Setting trace for {m:?}");
                            t.set(Some(trace.clone())).unwrap();
                        }
                        msgs.push(m);
                        msgs
                    });
            let packet = Packet::<
                <Config::Context as MiniUdpContext>::Send,
                Config,
                <Config::Context as MiniUdpContext>::ConnectionHandler,
            >::Data {
                sequence_id: index,
                ack,
                data: PacketData::ReliableOrdered { messages },
            };
            #[cfg(any(test, feature = "debug"))]
            if socket.debug_logs {
                debug!(
                    "Constructed new reliable ordered packet #{index} with {included_msgs} messages",
                );
            }
            let byte_len = connection.write_packet(
                MaxPackets::index_array_by_ringbuffer_index::<[u8; _]>(
                    &mut self.send_buffer,
                    index,
                ),
                packet,
            )?;

            self.pending.push(PendingPacket::new(
                resend_handler,
                priority,
                byte_len,
                trace,
            ));
        }
        Ok(())
    }

    fn send<Addr>(
        &mut self,
        socket: &mut UdpCommunicatorSocket<Config::Context>,
        addr: Addr,
        any_send: &mut bool,
    ) where
        Addr: SocketSendAddr,
    {
        let _span = trace_span!("ReliableOrdered::send").entered();
        self.pending.retain(
            |id,
             PendingPacket {
                 last_send,
                 byte_len,
                 context,
                 trace,
             }| {
                let keep_packet = match socket.resend_handler.resend(context, last_send) {
                    ResendAction::Resend => true,
                    ResendAction::ResendThenDrop => false,
                    ResendAction::DoNotResend => return true,
                };
                trace.update(|state| match state {
                    ReliablePacketState::Constructed if keep_packet => {
                        *state = ReliablePacketState::Sending {
                            first_send: Instant::now(),
                            times_send: 1,
                        }
                    }
                    ReliablePacketState::Constructed => {
                        *state = ReliablePacketState::SendLimitReached {
                            first_send: Instant::now(),
                            times_send: 1,
                        }
                    }
                    ReliablePacketState::Sending {
                        first_send,
                        times_send,
                    } if keep_packet => *times_send += 1,
                    ReliablePacketState::Sending {
                        first_send,
                        times_send,
                    } => {
                        *state = ReliablePacketState::SendLimitReached {
                            first_send: *first_send,
                            times_send: *times_send + 1,
                        }
                    }
                    ReliablePacketState::SendLimitReached { .. } => unreachable!(),
                    ReliablePacketState::Acknowledged { .. } => unreachable!(),
                });
                trace!("Sending packet #{id}");
                if let Err(error) = addr.send::<Config>(
                    socket,
                    &MaxPackets::index_array_by_ringbuffer_index(&mut self.send_buffer, id)
                        [..*byte_len],
                ) {
                    socket
                        .resend_handler
                        .handle_send_error::<<Config::Context as MiniUdpContext>::ErrorHandling>(
                            context,
                            error,
                            &mut socket.error_handler,
                        );
                    return keep_packet;
                }
                *any_send = true;
                keep_packet
            },
        );
    }
}

impl<Config: MiniUdpConfig, MaxPackets: MaxConcurrentPackets> MessageSink<Config>
    for ReliableOrdered<Config, MaxPackets>
{
    type MessageTrace = MessageTrace<ReliablePacketState>;
    fn queue_message(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
        priority: Priority,
    ) {
        self.send_queue.push_back((message, priority, None));
    }
    fn queue_message_with_trace(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
        priority: Priority,
    ) -> Self::MessageTrace {
        let (trace, handle) = MessageTrace::new();
        let handle = Some(handle);
        self.send_queue.push_back((message, priority, handle));
        trace
    }
}

impl<Config: MiniUdpConfig, MaxPackets: MaxConcurrentPackets> MessageSource<Config>
    for ReliableOrdered<Config, MaxPackets>
{
    fn next(&mut self) -> Option<<<Config as MiniUdpConfig>::Context as MiniUdpContext>::Recv> {
        self.recv_queue.pop_front()
    }
}

impl<Config: MiniUdpConfig, MaxPackets: MaxConcurrentPackets> MightHaveWork
    for ReliableOrdered<Config, MaxPackets>
{
    fn has_work(&self) -> bool {
        !(self.send_queue.is_empty() && self.pending.is_empty())
    }
}

#[derive(Debug)]
struct PendingPacket<Resend: ResendStrategy> {
    last_send: Instant,
    byte_len: usize,
    context: Resend::PacketContext,
    trace: PacketTrace<ReliablePacketState>,
}

impl<Resend: ResendStrategy> PendingPacket<Resend> {
    fn new(
        resend: &mut Resend,
        priority: Priority,
        byte_len: usize,
        trace: PacketTrace<ReliablePacketState>,
    ) -> Self {
        Self {
            last_send: Instant::now() - Duration::from_hours(1),
            byte_len,
            context: resend.new_packet(ReliablePacketKind::Ordered, priority),
            trace,
        }
    }
}
