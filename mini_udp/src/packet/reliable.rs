use crate::prelude2::*;

pub trait ReliablePacketHandler<Context: MiniUdpContext>: Debug + Default {
    fn read_packet(&mut self, messages: Vec<Context::Recv>);
}

#[derive(Debug, Default)]
/// `MAX_CONCURRENT_PACKETS` is the maximum amount of packets that can be send in one direction
/// concurrently.
/// If `MAX_CONCURRENT_PACKETS` is 32, then the sender can send 32 packets to the other side, but
/// will wait with sending the 33th packet until it receives an acknowledgement that the first
/// packet it send was actually received.
///
/// **Panics**
/// If `MAX_CONCURRENT_PACKETS` is not a power of 2.
pub struct Reliable<const MAX_CONCURRENT_PACKETS: usize>;

impl<Context: MiniUdpContext, const MAX_CONCURRENT_PACKETS: usize> ReliablePacketHandler<Context>
    for Reliable<MAX_CONCURRENT_PACKETS>
{
    fn read_packet(&mut self, messages: Vec<Context::Recv>) {
        todo!()
    }
}
