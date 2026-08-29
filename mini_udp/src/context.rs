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
///     type Send = Message;
///     type Recv = Message;
///     const PROTOCOL_VERSION: u32 = 0;
///
///     type Reverse = UdpContext<Self::Recv, Self::Send, { Self::PROTOCOL_VERSION }>;
///     
///     type ErrorHandling = mini_udp::context::error_handlers::WarnOnError;
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
    type Send: ByteRepr;
    type Recv: ByteRepr;
    const PROTOCOL_VERSION: u32;

    /// Reverse the `Send` and `Recv` parameters, to represent the context of the connected
    /// communicator instead (which should receive what this sends, and send what this receives).
    type Reverse: MiniUdpContext<Send = Self::Recv, Recv = Self::Send, ErrorHandling = Self::ErrorHandling>;

    type ErrorHandling: ErrorHandlingStrategy;
}

/// See the trait docs for [`MiniUdpContext`].
pub struct UdpContext<
    SEND,
    RECV,
    const PROTOCOL_VERSION: u32,
    ErrorHandling = error_handlers::WarnOnError,
> {
    _send: PhantomData<SEND>,
    _recv: PhantomData<RECV>,
    _error_handling: PhantomData<ErrorHandling>,
}

impl<
    Send: ByteRepr,
    Recv: ByteRepr,
    const PROTOCOL_VERSION: u32,
    ErrorHandling: ErrorHandlingStrategy,
> MiniUdpContext for UdpContext<Send, Recv, PROTOCOL_VERSION, ErrorHandling>
{
    type Send = Send;
    type Recv = Recv;
    const PROTOCOL_VERSION: u32 = PROTOCOL_VERSION;

    type Reverse = UdpContext<Recv, Send, PROTOCOL_VERSION, ErrorHandling>;

    type ErrorHandling = ErrorHandling;
}

pub trait ErrorHandlingStrategy {
    type Handler;
    fn handle_error(handler: &mut Self::Handler, error: Error);
}

pub mod error_handlers {
    use crate::prelude::*;

    pub struct WarnOnError;
    impl ErrorHandlingStrategy for WarnOnError {
        type Handler = ();
        fn handle_error(_handler: &mut Self::Handler, error: Error) {
            warn!("{error}");
        }
    }

    pub struct PanicOnError;
    impl ErrorHandlingStrategy for PanicOnError {
        type Handler = ();
        fn handle_error(_handler: &mut Self::Handler, error: Error) {
            panic!("{error}");
        }
    }
}
