use std::marker::PhantomData;

use crate::{packet::ReliablePacketKind, prelude::*};

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
///         Self::ErrorHandling,
///         Self::ResendStrategy,
///     >;
///     
///     type ErrorHandling = mini_udp::context::error_handlers::ErrorCache;
///     type ResendStrategy = mini_udp::context::resend_strategies::FixedResend;
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
    /// The type of message to be send to the other side.
    type Send: ByteRepr;
    /// The type of message to be received from the other side.
    type Recv: ByteRepr;
    /// The current protocol version. When you change the `Send` or `Recv` types, you should also
    /// increment the protocol version. It is used to seed the CRC and thus prevent messages from an
    /// incompatible other side to be received.
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
            //
            ResendStrategy = Self::ResendStrategy,
        >;

    /// Type defining how errors should be handled.
    type ErrorHandling: ErrorHandlingStrategy;
    /// Type defining when reliable packets should be resend.
    type ResendStrategy: ResendStrategy;
}

/// See the trait docs for [`MiniUdpContext`].
pub struct UdpContext<
    SEND,
    RECV,
    const PROTOCOL_VERSION: u32,
    ErrorHandling = error_handlers::WarnOnError,
    ResendStrategy = resend_strategies::FixedResend,
> {
    _send: PhantomData<SEND>,
    _recv: PhantomData<RECV>,
    _error_handling: PhantomData<ErrorHandling>,
    _resend: PhantomData<ResendStrategy>,
}

impl<
    Send: ByteRepr,
    Recv: ByteRepr,
    const PROTOCOL_VERSION: u32,
    ErrorHandling: ErrorHandlingStrategy,
    Resend: ResendStrategy,
> MiniUdpContext for UdpContext<Send, Recv, PROTOCOL_VERSION, ErrorHandling, Resend>
{
    type Send = Send;
    type Recv = Recv;
    const PROTOCOL_VERSION: u32 = PROTOCOL_VERSION;

    type Reverse = UdpContext<Recv, Send, PROTOCOL_VERSION, ErrorHandling, Resend>;

    type ErrorHandling = ErrorHandling;
    type ResendStrategy = Resend;
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
            tracing::debug!("{error}");
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

pub trait ResendStrategy: Default {
    /// Data stored for each packet in the send buffer.
    type PacketContext;

    /// This method gets called whenever [`UdpCommunicator::send`] or [`MultiCommunicator::send`]
    /// are called.
    fn next_send(&mut self) {}
    /// This method is called whenever a new packet is constructed. As the packet is not send
    /// directly, the [`PacketContext`](Self::PacketContext) should be initialized such that the
    /// first call to [`resend`](Self::resend) will return [`ResendAction::Resend`].
    fn new_packet(&mut self, kind: ReliablePacketKind) -> Self::PacketContext;
    /// This method gets called for every reliable packet in the packet send buffer whenever
    /// [`UdpCommunicator::send`] or [`MultiCommunicator::send`] are called.
    /// It is used to determine if the packet should be resend at this moment, and if that resend
    /// should be the last resend.
    fn resend(&mut self, context: &mut Self::PacketContext) -> ResendAction;
    /// When an error occurs during sending of a reliable packet, this method is called.
    fn handle_send_error<ErrorHandler>(
        &mut self,
        context: &mut Self::PacketContext,
        error: Error,
        error_handler: &mut ErrorHandler::Handler,
    ) where
        ErrorHandler: ErrorHandlingStrategy;
}

#[derive(Debug)]
pub enum ResendAction {
    /// Resend the packet.
    Resend,
    /// Resend the packet for one final time, then drop it from the packet send buffer.
    ResendThenDrop,
    /// Do not resend the packet right now, maybe next time.
    DoNotResend,
}

pub mod resend_strategies {
    use crate::{packet::ReliablePacketKind, prelude::*};

    /// Resend reliable packets at a fixed interval, with a fixed retry limit.
    pub struct FixedResend {
        /// Maximum amount of retries for reliable ordered packets.
        pub max_ordered_retries: usize,
        /// Maximum amount of retries for reliable unordered packets.
        pub max_unordered_retries: usize,
        /// Maximum amount of retries for reliable packets that have been fragmented due to
        /// exceeding the [`MAX_PACKET_DATA_LEN`] - those packets are always ordered.
        pub max_fragmented_retries: usize,
        /// Fixed retry interval for reliable ordered packets.
        pub ordered_resend_interval: Duration,
        /// Fixed retry interval for reliable unordered packets.
        pub unordered_resend_interval: Duration,
        /// Fixed retry interval for reliable packets that have been fragmented due to
        /// exceeding the [`MAX_PACKET_DATA_LEN`] - those packets are always ordered.
        pub fragmented_resend_interval: Duration,
    }
    pub struct FixedResendPacketContext {
        remaining_retries: usize,
        last_send: Instant,
        resend_interval: Duration,
    }
    impl FixedResend {
        pub const DEFAULT: Self = Self {
            max_ordered_retries: 100,
            max_unordered_retries: 100,
            max_fragmented_retries: 100,
            ordered_resend_interval: Duration::from_millis(100),
            unordered_resend_interval: Duration::from_millis(100),
            fragmented_resend_interval: Duration::from_millis(100),
        };
        /// Set the maximum amount of retries for all reliable packets.
        pub fn set_max_retries(&mut self, max_retries: usize) {
            self.max_ordered_retries = max_retries;
            self.max_unordered_retries = max_retries;
            self.max_fragmented_retries = max_retries;
        }
        /// Set the retry interval for all reliable packets.
        pub fn set_resend_interval(&mut self, interval: Duration) {
            self.ordered_resend_interval = interval;
            self.unordered_resend_interval = interval;
            self.fragmented_resend_interval = interval;
        }
    }
    impl Default for FixedResend {
        fn default() -> Self {
            Self::DEFAULT
        }
    }
    impl ResendStrategy for FixedResend {
        type PacketContext = FixedResendPacketContext;
        fn new_packet(&mut self, kind: ReliablePacketKind) -> Self::PacketContext {
            let resend_interval = match kind {
                ReliablePacketKind::Ordered => self.ordered_resend_interval,
                ReliablePacketKind::Unordered => self.unordered_resend_interval,
                ReliablePacketKind::OrderedFragment => self.fragmented_resend_interval,
            };
            FixedResendPacketContext {
                remaining_retries: match kind {
                    ReliablePacketKind::Ordered => self.max_ordered_retries,
                    ReliablePacketKind::Unordered => self.max_unordered_retries,
                    ReliablePacketKind::OrderedFragment => self.max_fragmented_retries,
                },
                last_send: Instant::now() - resend_interval * 2,
                resend_interval,
            }
        }
        fn resend(&mut self, context: &mut Self::PacketContext) -> ResendAction {
            if context.last_send.elapsed() >= context.resend_interval {
                context.last_send = Instant::now();
                if context.remaining_retries > 1 {
                    context.remaining_retries -= 1;
                    ResendAction::Resend
                } else {
                    ResendAction::ResendThenDrop
                }
            } else {
                ResendAction::DoNotResend
            }
        }
        fn handle_send_error<ErrorHandler>(
            &mut self,
            _context: &mut Self::PacketContext,
            error: Error,
            error_handler: &mut ErrorHandler::Handler,
        ) where
            ErrorHandler: ErrorHandlingStrategy,
        {
            ErrorHandler::handle_error(error_handler, error);
        }
    }
}
