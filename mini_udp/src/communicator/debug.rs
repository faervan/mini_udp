use std::net::SocketAddr;

use crate::prelude::*;

pub trait MiniUdpDebugExt {
    /// Simulate fake UDP unreliability by randomly dropping packets according to the provided
    /// probability.
    ///
    /// A probability of `0.0` means never drop packets, `1.0` means drop every packet.
    fn with_fake_drop(self, drop_probability: f64) -> Self;

    /// Simulate fake UDP unreliability by randomly corrupting bits of packets according to the
    /// provided probability (the probability determines how likely it is for a packet to be
    /// corrupted, not how many bits will be flipped).
    ///
    /// A probability of `0.0` means never corrupt packets, `1.0` means corrupt every packet.
    fn with_fake_corruption(self, corruption_probability: f64) -> Self;

    /// Add an extra delay to packet receiving by a random amount of milliseconds in the range of
    /// the provided `delay_ms`.
    /// Only packet receiving is affected by this, not sending.
    fn with_fake_delay(self, delay_ms: std::ops::Range<u64>) -> Self;

    /// Enable debug logs like notifications when a packet has been artificially corrupted by
    /// [`Self::with_fake_corruption`].
    fn with_debug_logs(self) -> Self;
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> MiniUdpDebugExt
    for UdpCommunicator<CTX, PacketHandling>
{
    #[inline(always)]
    fn with_fake_drop(mut self, drop_probability: f64) -> Self {
        self.socket = self.socket.with_fake_drop(drop_probability);
        self
    }

    #[inline(always)]
    fn with_fake_corruption(mut self, corruption_probability: f64) -> Self {
        self.socket = self.socket.with_fake_corruption(corruption_probability);
        self
    }

    #[inline(always)]
    fn with_fake_delay(mut self, delay_ms: std::ops::Range<u64>) -> Self {
        self.socket = self.socket.with_fake_delay(delay_ms);
        self
    }

    #[inline(always)]
    fn with_debug_logs(mut self) -> Self {
        self.socket = self.socket.with_debug_logs();
        self
    }
}

impl<CTX: MiniUdpContext, PacketHandling: PacketHandler> MiniUdpDebugExt
    for MultiUdpCommunicator<CTX, PacketHandling>
{
    #[inline(always)]
    fn with_fake_drop(mut self, drop_probability: f64) -> Self {
        self.socket = self.socket.with_fake_drop(drop_probability);
        self
    }

    #[inline(always)]
    fn with_fake_corruption(mut self, corruption_probability: f64) -> Self {
        self.socket = self.socket.with_fake_corruption(corruption_probability);
        self
    }

    #[inline(always)]
    fn with_fake_delay(mut self, delay_ms: std::ops::Range<u64>) -> Self {
        self.socket = self.socket.with_fake_delay(delay_ms);
        self
    }

    #[inline(always)]
    fn with_debug_logs(mut self) -> Self {
        self.socket = self.socket.with_debug_logs();
        self
    }
}

impl<CTX: MiniUdpContext> MiniUdpDebugExt for UdpCommunicatorSocket<CTX> {
    fn with_fake_drop(mut self, drop_probability: f64) -> Self {
        self.drop_probability = Some(drop_probability);
        self
    }

    fn with_fake_corruption(mut self, corruption_probability: f64) -> Self {
        self.corruption_probability = Some(corruption_probability);
        self
    }

    fn with_fake_delay(mut self, delay_ms: std::ops::Range<u64>) -> Self {
        self.fake_delay = delay_ms;
        self
    }

    fn with_debug_logs(mut self) -> Self {
        self.debug_logs = true;
        self
    }
}

impl<CTX: MiniUdpContext> UdpCommunicatorSocket<CTX> {
    pub(super) fn delay_packet(&mut self, addr: Option<SocketAddr>, n: usize) -> bool {
        if !self.fake_delay.is_empty() {
            let delay_ms = rand::random_range(self.fake_delay.clone());
            if self.debug_logs {
                debug!("Received packet, delaying it by {delay_ms}ms");
            }
            self.fake_delayed_buffer.push((
                addr,
                self.data_buffer,
                n,
                Instant::now() + Duration::from_millis(delay_ms),
            ));
        }
        !self.fake_delay.is_empty()
    }

    pub(super) fn read_delayed(&mut self) -> Option<(usize, Option<SocketAddr>)> {
        let i = self
            .fake_delayed_buffer
            .iter()
            .position(|(_, _, _, instant)| {
                instant.saturating_duration_since(Instant::now()).is_zero()
            })?;
        let (addr, buf, n, _) = self.fake_delayed_buffer.swap_remove(i);
        self.data_buffer = buf;
        Some((n, addr))
    }
}
