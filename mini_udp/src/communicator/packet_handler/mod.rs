mod fragmentation;
pub use fragmentation::*;

pub trait PacketHandler {
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

pub struct DefaultPacketHandler;

impl PacketHandler for DefaultPacketHandler {
    type UnreliableFragmentationHandler = ();
    type ReliableFragmentationHandler = ();
}
