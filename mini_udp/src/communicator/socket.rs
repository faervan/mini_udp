use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::prelude::*;

pub trait CommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self;
    fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error>;
}

pub(crate) trait SocketSendAddr {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error>;
}

pub(crate) struct UdpCommunicatorSocket {
    pub socket: UdpSocket,
    pub data_buffer: [u8; MAX_PACKET_LEN],
    #[cfg(debug_assertions)]
    pub drop_probability: Option<f64>,
    #[cfg(debug_assertions)]
    pub corruption_probability: Option<f64>,
    #[cfg(debug_assertions)]
    fake_delay: std::ops::Range<u64>,
    #[cfg(debug_assertions)]
    pub debug_logs: bool,
    #[cfg(debug_assertions)]
    /// For each read from the socket, this stores the address from which the data was received,
    /// the copied data_buffer, the amount of bytes read and the instant at to which this packet
    /// is being delayed.
    fake_delayed_buffer: Vec<(
        Option<SocketAddr>,
        [u8; MAX_PACKET_LEN],
        usize,
        std::time::Instant,
    )>,
}

impl CommunicatorSocket for UdpCommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self {
        let socket = UdpSocket::bind(addr).expect("Failed to bind to udp socket");
        socket
            .set_nonblocking(true)
            .expect("Failed to set udp socket to nonblocking mode");
        Self {
            socket,
            data_buffer: [0; MAX_PACKET_LEN],
            #[cfg(debug_assertions)]
            drop_probability: None,
            #[cfg(debug_assertions)]
            corruption_probability: None,
            #[cfg(debug_assertions)]
            fake_delay: 0..0,
            #[cfg(debug_assertions)]
            debug_logs: false,
            #[cfg(debug_assertions)]
            fake_delayed_buffer: vec![],
        }
    }

    fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error> {
        self.socket.connect(addr)
    }
}

impl SocketSendAddr for () {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        socket.socket.send(&socket.data_buffer[slice_index])
    }
}

impl SocketSendAddr for SocketAddr {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error> {
        socket
            .socket
            .send_to(&socket.data_buffer[slice_index], self)
    }
}

impl UdpCommunicatorSocket {
    #[cfg(debug_assertions)]
    pub fn with_fake_drop(mut self, drop_probability: f64) -> Self {
        self.drop_probability = Some(drop_probability);
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_fake_corruption(mut self, corruption_probability: f64) -> Self {
        self.corruption_probability = Some(corruption_probability);
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_fake_delay(mut self, delay_ms: std::ops::Range<u64>) -> Self {
        self.fake_delay = delay_ms;
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_debug_logs(mut self) -> Self {
        self.debug_logs = true;
        self
    }

    #[cfg(debug_assertions)]
    pub fn delay_packet(&mut self, addr: Option<SocketAddr>, n: usize) -> bool {
        if !self.fake_delay.is_empty() {
            let delay_ms = rand::random_range(self.fake_delay.clone());
            if self.debug_logs {
                debug!("Received packet, delaying it by {delay_ms}ms");
            }
            self.fake_delayed_buffer.push((
                addr,
                self.data_buffer,
                n,
                Instant::now() + std::time::Duration::from_millis(delay_ms),
            ));
        }
        !self.fake_delay.is_empty()
    }

    #[cfg(debug_assertions)]
    pub fn read_delayed(&mut self) -> Option<(usize, Option<SocketAddr>)> {
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
