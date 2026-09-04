use std::marker::PhantomData;

use crate::{
    context2::{error_handlers, resend_strategies},
    prelude2::*,
};

pub trait MiniUdpConfig: Debug + Sized {
    type Context: MiniUdpContext;

    /// Type defining how unreliable packets should be handled.
    type UnreliablePacketHandler: UnreliablePacketHandler<Self>;

    /// Type defining how unreliable packets should be handled.
    type ReliablePacketHandler: ReliablePacketHandler<Self::Context>;

    /// Type defining how unreliable packets should be handled.
    type ReliableOrderedPacketHandler: ReliableOrderedPacketHandler<Self::Context>;

    /// Type defining how unreliable fragmentation should be handled.
    /// If you do not need to send unreliable packets larger than [`MAX_PACKET_DATA_LEN`], you can
    /// just use `()` to not do unreliable fragmentation.
    /// Else, you can use
    /// [`UnreliableFragmentation`](fragmentation_handlers::UnreliableFragmentation), which lets
    /// you also define the maximum allowed number of fragments.
    type UnreliableFragmentationHandler: UnreliableFragmentationHandler;

    /// Type defining how reliable fragmentation should be handled.
    /// If you do not need to send reliable packets larger than [`MAX_PACKET_DATA_LEN`], you can
    /// just use `()` to not do reliable fragmentation.
    /// Else, you can use
    /// [`ReliableFragmentation`](fragmentation_handlers::ReliableFragmentation), which lets you
    /// also define the maximum allowed number of fragments.
    type ReliableFragmentationHandler: ReliableFragmentationHandler;
}

/// See the trait docs for [`MiniUdpConfig`].
pub struct UdpConfig<
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
> Debug for UdpConfig<Send, Recv, PROTOCOL_VERSION, ErrorHandling, Resend>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UdpConfig")
    }
}

impl<
    Send: ByteRepr,
    Recv: ByteRepr,
    const PROTOCOL_VERSION: u32,
    ErrorHandling: ErrorHandlingStrategy,
    Resend: ResendStrategy,
> MiniUdpConfig for UdpConfig<Send, Recv, PROTOCOL_VERSION, ErrorHandling, Resend>
{
    type Context = UdpContext<Send, Recv, PROTOCOL_VERSION, ErrorHandling, Resend>;

    type UnreliablePacketHandler = Unreliable<Self>;
    type ReliablePacketHandler = Reliable<32>;
    type ReliableOrderedPacketHandler = ReliableOrdered<32>;
    type UnreliableFragmentationHandler = UnreliableFragmentation<16>;
    type ReliableFragmentationHandler = ReliableFragmentation<256>;
}
