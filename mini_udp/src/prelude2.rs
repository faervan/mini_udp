/// TODO!
pub const BUFFER_SIZE: usize = 1024;

pub use mini_udp_derive::ByteRepr;

pub use crate::byte_repr::{ByteRepr, ByteReprError, ByteReprExt, StaticByteRepr};
pub use crate::com2::*;
pub use crate::config::{MiniUdpConfig, UdpConfig};
pub use crate::context2::{MiniUdpContext, UdpContext};

pub(crate) use crate::error2::Error;
pub(crate) use crate::packet::*;
pub(crate) use crate::ring_buffer::{RingBuffer, RingBufferApi, wrapping_gt};

pub(crate) use crate::context2::ErrorHandlingStrategy;
pub(crate) use crate::context2::{ResendAction, ResendStrategy};

pub(crate) use std::collections::VecDeque;
pub(crate) use std::fmt::Debug;
pub(crate) use std::sync::{Arc, OnceLock};
pub(crate) use std::time::{Duration, Instant};

#[cfg(any(test, feature = "debug"))]
pub(crate) use tracing::debug;
pub(crate) use tracing::{error, trace, trace_span, warn};
