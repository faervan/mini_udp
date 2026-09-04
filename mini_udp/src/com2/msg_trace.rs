use std::sync::{Arc, OnceLock, RwLock};

use crate::prelude2::*;

pub struct MessageTrace<PacketState> {
    inner: InnerMessageTrace<PacketState>,
}

enum InnerMessageTrace<PacketState> {
    Queued {
        update: OnceLock<Option<Arc<PacketTrace<PacketState>>>>,
    },
    Cancelled,
    Packeted {
        handle: Arc<PacketTrace<PacketState>>,
    },
}

#[derive(Debug)]
pub enum MessageState<PacketState> {
    Queued,
    Cancelled,
    Packeted {
        packet_id: u16,
        packet_priority: Priority,
        state: PacketState,
    },
}

#[derive(Debug)]
pub(crate) struct PacketTrace<PacketState> {
    id: u16,
    priority: Priority,
    state: RwLock<PacketState>,
}

enum ReliablePacketState {
    Constructed,
    Sending {
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
    pub(crate) fn new() -> (Self, OnceLock<Option<Arc<PacketTrace<PacketState>>>>) {
        let update = OnceLock::new();
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
                        packet_id: handle.id,
                        packet_priority: handle.priority,
                        state: handle.state.read().unwrap().clone(),
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
                packet_id: handle.id,
                packet_priority: handle.priority,
                state: handle.state.read().unwrap().clone(),
            },
        }
    }
}
