use crate::prelude2::*;

pub(crate) struct InnerCommunicator<Config: MiniUdpConfig> {
    pub unreliable: Config::UnreliablePacketHandler,
    pub reliable: Config::ReliablePacketHandler,
    pub reliable_ordered: Config::ReliableOrderedPacketHandler,
    pub unreliable_fragmented: Config::UnreliableFragmentationHandler,
    pub reliable_fragmented: Config::ReliableFragmentationHandler,
    pub last_seen: Instant,
    pub last_send: Instant,
}

impl<Config: MiniUdpConfig> InnerCommunicator<Config> {
    pub fn has_work(&self) -> bool {
        self.unreliable.has_work()
    }
}
