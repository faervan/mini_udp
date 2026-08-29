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
///     type Reverse = UdpContext<
///         Self::Recv,
///         Self::Send,
///         { Self::PROTOCOL_VERSION },
///         Self::ErrorHandling
///     >;
///     
///     type ErrorHandling = mini_udp::context::error_handlers::ErrorCache;
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
    type Reverse: MiniUdpContext<
            // Those comments are a workaround for `rustfmt` forcing it all to be on one line, then
            // complaining about it exceeding the character limit per line.
            Send = Self::Recv,
            //
            Recv = Self::Send,
            //
            ErrorHandling = Self::ErrorHandling,
        >;

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

/// Specify what to do with errors.
/// ### Example using the [`ErrorCache`](error_handlers::ErrorCache) error handler
/// ```rust
/// use mini_udp::prelude::*;
/// use mini_udp::context::error_handlers::ErrorCache;
///
/// /// For the sender, we send strings, receive nothing, use the first version of our protocol
/// /// and use the default error handler.
/// type SenderCtx = UdpContext<String, (), 1>;
///
/// /// For the receiver, we send nothing, receive strings, use the second version of our protocol
/// /// and cache all errors.
/// type ReceiverCtx = UdpContext<(), String, 2, ErrorCache>;
///
/// let mut sender = UdpCommunicator::<SenderCtx>::default().connect("0.0.0.0:7100").unwrap();
/// let mut receiver = UdpCommunicator::<ReceiverCtx>::bind("0.0.0.0:7100");
///
/// sender.write(String::from("hello"));
/// sender.send().unwrap();
///
/// receiver.recv();
/// /// Since the sender used protocol version 1, but the receiver has version 2, the message was
/// /// perceived as invalid by the receiver.
/// assert_eq!(receiver.read(), None);
///
/// let mut errors = receiver.get_error_handler_mut().drain(..);
/// /// Since the CRC algorithm is seeded by the protocol version, the check failed.
/// assert_eq!(errors.next(), Some(mini_udp::Error::CrcFailed));
/// assert_eq!(errors.next(), None);
/// ```
pub trait ErrorHandlingStrategy {
    type Handler;
    fn handle_error(handler: &mut Self::Handler, error: Error);
}

pub mod error_handlers {
    use crate::prelude::*;

    /// Emit an error log at [`Level::TRACE`](tracing::Level::TRACE) whenever an error occurs.
    pub struct TraceOnError;
    impl ErrorHandlingStrategy for TraceOnError {
        type Handler = ();
        fn handle_error(_handler: &mut Self::Handler, error: Error) {
            tracing::trace!("{error}");
        }
    }

    /// Emit an error log at [`Level::DEBUG`](tracing::Level::DEBUG) whenever an error occurs.
    pub struct DebugOnError;
    impl ErrorHandlingStrategy for DebugOnError {
        type Handler = ();
        fn handle_error(_handler: &mut Self::Handler, error: Error) {
            debug!("{error}");
        }
    }

    /// Emit an error log at [`Level::WARN`](tracing::Level::WARN) whenever an error occurs.
    pub struct WarnOnError;
    impl ErrorHandlingStrategy for WarnOnError {
        type Handler = ();
        fn handle_error(_handler: &mut Self::Handler, error: Error) {
            warn!("{error}");
        }
    }

    /// Emit an error log at [`Level::ERROR`](tracing::Level::ERROR) whenever an error occurs.
    pub struct ErrorOnError;
    impl ErrorHandlingStrategy for ErrorOnError {
        type Handler = ();
        fn handle_error(_handler: &mut Self::Handler, error: Error) {
            error!("{error}");
        }
    }

    /// Store errors in a [`Vec`] that can be [`drain'ed`](Vec::drain) to handle errors manually.
    ///
    /// When the cache is never cleared, this will panic eventually, see [`Vec::push`]. For a
    /// limited cache that will not panic, use [`LimitedErrorCache`].
    pub struct ErrorCache;
    impl ErrorHandlingStrategy for ErrorCache {
        type Handler = Vec<Error>;
        fn handle_error(handler: &mut Self::Handler, error: Error) {
            handler.push(error);
        }
    }

    /// Store errors in a [`Vec`] that can be [`drain'ed`](Vec::drain) to handle errors manually.
    ///
    /// However, instead of storing infinite numbers of errors and eventually crashing when the
    /// cache is never cleared like [`ErrorCache`] does, [`LimitedErrorCache`] will store only a
    /// maximum of `MAX_CACHED_ERRORS` errors, and emit warning logs if no more errors can be stored.
    pub struct LimitedErrorCache<const MAX_CACHED_ERRORS: usize = 50>;
    impl<const MAX_CACHED_ERRORS: usize> ErrorHandlingStrategy
        for LimitedErrorCache<MAX_CACHED_ERRORS>
    {
        type Handler = Vec<Error>;
        fn handle_error(handler: &mut Self::Handler, error: Error) {
            if handler.len() < MAX_CACHED_ERRORS {
                handler.push(error);
            } else {
                warn!("The error cache is full! Error: {error}");
            }
        }
    }
}
