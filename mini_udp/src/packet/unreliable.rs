use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::OnceLock;

use crate::com2::*;
use crate::prelude2::*;

pub trait UnreliablePacketHandler<Config: MiniUdpConfig>:
    MessageSink<Config> + MightHaveWork + Debug + Default
{
    fn read_packet(&mut self, messages: Vec<<Config::Context as MiniUdpContext>::Recv>);
}

type MaybeMsgTrace = Option<OnceLock<Option<Arc<PacketTrace<UnreliablePacketState>>>>>;

#[derive(Debug)]
pub struct Unreliable<Config: MiniUdpConfig> {
    low_send_queue: VecDeque<(<Config::Context as MiniUdpContext>::Send, MaybeMsgTrace)>,
    default_send_queue: VecDeque<(<Config::Context as MiniUdpContext>::Send, MaybeMsgTrace)>,
    high_send_queue: VecDeque<(<Config::Context as MiniUdpContext>::Send, MaybeMsgTrace)>,
}

impl<Config: MiniUdpConfig> Default for Unreliable<Config> {
    fn default() -> Self {
        Self {
            low_send_queue: VecDeque::new(),
            default_send_queue: VecDeque::new(),
            high_send_queue: VecDeque::new(),
        }
    }
}

impl<Config: MiniUdpConfig> UnreliablePacketHandler<Config> for Unreliable<Config> {
    fn read_packet(&mut self, messages: Vec<<Config::Context as MiniUdpContext>::Recv>) {
        todo!()
    }
}

impl<Config: MiniUdpConfig> MessageSink<Config> for Unreliable<Config> {
    type MessageTrace = MessageTrace<UnreliablePacketState>;
    fn queue_message(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
        priority: Priority,
    ) {
        match priority {
            Priority::Low => self.low_send_queue.push_back((message, None)),
            Priority::Default => self.default_send_queue.push_back((message, None)),
            Priority::High => self.high_send_queue.push_back((message, None)),
        }
    }
    fn queue_message_with_trace(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
        priority: Priority,
    ) -> Self::MessageTrace {
        let (trace, handle) = MessageTrace::new();
        let handle = Some(handle);
        match priority {
            Priority::Low => self.low_send_queue.push_back((message, handle)),
            Priority::Default => self.default_send_queue.push_back((message, handle)),
            Priority::High => self.high_send_queue.push_back((message, handle)),
        }
        trace
    }
}

impl<Config: MiniUdpConfig> MightHaveWork for Unreliable<Config> {
    fn has_work(&self) -> bool {
        !(self.low_send_queue.is_empty()
            && self.default_send_queue.is_empty()
            && self.high_send_queue.is_empty())
    }
}

#[derive(Debug, Clone)]
pub enum UnreliablePacketState {
    Constructed,
    Send { send_at: Instant },
}
