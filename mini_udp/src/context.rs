use std::marker::PhantomData;

use crate::prelude::*;

/// A helper trait used to keep the number of generic parameters for `mini_udp` types in check.
/// You can implement it yourself or use a type alias to [`UdpContext`].
///
/// ### Example with manual implementation
/// ```rust
/// use mini_udp::prelude::*;
///
/// struct UdpCtx;
///
/// type Message = String;
///
/// impl MiniUdpContext for UdpCtx {
///     type SEND = Message;
///     type RECV = Message;
///     const PROTOCOL_VERSION: u32 = 0;
///
///     type REVERSE = UdpContext<Self::RECV, Self::SEND, { Self::PROTOCOL_VERSION }>;
/// }
///
/// let _com = UdpCommunicator::<UdpCtx>::default();
/// ```
///
/// ### Example using [`UdpContext`]
/// ```rust
/// use mini_udp::prelude::*;
///
/// type Message = String;
///
/// type UdpCtx = UdpContext<Message, Message, 0>;
///
/// let _com = UdpCommunicator::<UdpCtx>::default();
/// ```
pub trait MiniUdpContext {
    type SEND: ByteRepr;
    type RECV: ByteRepr;
    const PROTOCOL_VERSION: u32;

    type REVERSE: MiniUdpContext;
}

/// See the trait docs for [`MiniUdpContext`].
pub struct UdpContext<SEND, RECV, const PROTOCOL_VERSION: u32> {
    _send: PhantomData<SEND>,
    _recv: PhantomData<RECV>,
}

impl<SEND: ByteRepr, RECV: ByteRepr, const PROTOCOL_VERSION: u32> MiniUdpContext
    for UdpContext<SEND, RECV, PROTOCOL_VERSION>
{
    type SEND = SEND;
    type RECV = RECV;
    const PROTOCOL_VERSION: u32 = PROTOCOL_VERSION;

    type REVERSE = UdpContext<RECV, SEND, PROTOCOL_VERSION>;
}
