use std::sync::RwLock;

use crate::prelude2::*;

pub struct MessageTrace<PacketState> {
    inner: InnerMessageTrace<PacketState>,
}

pub(crate) type MessageTracePacketUpdate<PacketState> =
    Arc<OnceLock<Option<PacketTrace<PacketState>>>>;

enum InnerMessageTrace<PacketState> {
    Queued {
        update: MessageTracePacketUpdate<PacketState>,
    },
    Cancelled,
    Packeted {
        handle: PacketTrace<PacketState>,
    },
}

#[derive(Debug, PartialEq)]
pub enum MessageState<PacketState> {
    Queued,
    Cancelled,
    Packeted {
        packet_id: u16,
        packet_priority: Priority,
        state: PacketState,
    },
}

#[derive(Debug, Clone)]
pub struct PacketTrace<PacketState> {
    inner: Arc<InnerPacketTrace<PacketState>>,
}

#[derive(Debug)]
struct InnerPacketTrace<PacketState> {
    id: u16,
    priority: Priority,
    state: RwLock<PacketState>,
}

impl<PacketState> PacketTrace<PacketState> {
    pub fn new(id: u16, priority: Priority, state: PacketState) -> Self {
        Self {
            inner: Arc::new(InnerPacketTrace {
                id,
                priority,
                state: RwLock::new(state),
            }),
        }
    }

    pub fn update<F: FnOnce(&mut PacketState)>(&self, f: F) {
        let mut state = self.inner.state.write().unwrap();
        f(&mut *state);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReliablePacketState {
    Constructed,
    Sending {
        first_send: Instant,
        times_send: usize,
    },
    /// The resend handler decided to not continue resending this packet.
    SendLimitReached {
        first_send: Instant,
        times_send: usize,
    },
    Acknowledged {
        first_send: Instant,
        times_send: usize,
        ack_received: Instant,
    },
}

impl<PacketState: Clone> MessageTrace<PacketState> {
    pub(crate) fn new() -> (Self, MessageTracePacketUpdate<PacketState>) {
        let update = Arc::new(OnceLock::new());
        (
            Self {
                inner: InnerMessageTrace::Queued {
                    update: update.clone(),
                },
            },
            update,
        )
    }

    pub fn state(&mut self) -> MessageState<PacketState> {
        match &self.inner {
            InnerMessageTrace::Queued { update } => match update.get() {
                Some(Some(handle)) => {
                    let s = MessageState::Packeted {
                        packet_id: handle.inner.id,
                        packet_priority: handle.inner.priority,
                        state: handle.inner.state.read().unwrap().clone(),
                    };
                    self.inner = InnerMessageTrace::Packeted {
                        handle: handle.clone(),
                    };
                    s
                }
                Some(None) => {
                    self.inner = InnerMessageTrace::Cancelled;
                    MessageState::Cancelled
                }
                None => MessageState::Queued,
            },
            InnerMessageTrace::Cancelled => MessageState::Cancelled,
            InnerMessageTrace::Packeted { handle } => MessageState::Packeted {
                packet_id: handle.inner.id,
                packet_priority: handle.inner.priority,
                state: handle.inner.state.read().unwrap().clone(),
            },
        }
    }
}
