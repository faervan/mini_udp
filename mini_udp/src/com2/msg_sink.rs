use crate::prelude2::*;

pub trait MessageSink<Config: MiniUdpConfig>: Sized {
    type MessageTrace;
    fn queue_message(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
        priority: Priority,
    );
    fn queue_message_with_trace(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
        priority: Priority,
    ) -> Self::MessageTrace;

    fn new_message(
        &mut self,
        message: <Config::Context as MiniUdpContext>::Send,
    ) -> MessageRef<'_, Config, Self> {
        MessageRef {
            inner: Some(MessageRefInner {
                message,
                priority: Priority::Default,
                sink: self,
            }),
        }
    }
}

pub struct MessageRef<'a, Config: MiniUdpConfig, Sink: MessageSink<Config>> {
    inner: Option<MessageRefInner<'a, Config, Sink>>,
}

struct MessageRefInner<'a, Config: MiniUdpConfig, Sink: MessageSink<Config>> {
    message: <Config::Context as MiniUdpContext>::Send,
    priority: Priority,
    sink: &'a mut Sink,
}

impl<'a, Config: MiniUdpConfig, Sink: MessageSink<Config>> Drop for MessageRef<'a, Config, Sink> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.sink.queue_message(inner.message, inner.priority);
        }
    }
}

impl<'a, Config: MiniUdpConfig, Sink: MessageSink<Config>> MessageRef<'a, Config, Sink> {
    pub fn with_priority(mut self, priority: Priority) -> Self {
        if let Some(inner) = &mut self.inner {
            inner.priority = priority;
        }
        self
    }

    pub fn trace(mut self) -> Sink::MessageTrace {
        let Some(inner) = self.inner.take() else {
            unreachable!("inner is only set to None in this method, which can only be called once");
        };
        inner
            .sink
            .queue_message_with_trace(inner.message, inner.priority)
    }
}
