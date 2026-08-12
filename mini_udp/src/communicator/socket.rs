use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::prelude::*;

pub trait CommunicatorSocket {
    fn bind<A: ToSocketAddrs>(addr: A) -> Self;
    /// Set the interval at which reliable unordered packets are to be resend if no
    /// acknowledgement has been received.
    ///
    /// The default is 100 milliseconds.
    fn with_reliable_unordered_resend_interval(self, interval: Duration) -> Self;
    /// Set the interval at which reliable ordered packets are to be resend if no
    /// acknowledgement has been received.
    ///
    /// The default is 100 milliseconds.
    fn with_reliable_ordered_resend_interval(self, interval: Duration) -> Self;
    /// Set the delay after which reliable unordered packets are to be resend for the first time if
    /// no acknowledgement has been received.
    ///
    /// This can be viewed as an override of `with_reliable_unordered_resend_interval` for the first
    /// resend.
    ///
    /// The default is 500 milliseconds.
    fn with_initial_reliable_unordered_resend_delay(self, interval: Duration) -> Self;
    /// Set the delay after which reliable ordered packets are to be resend for the first time if no
    /// acknowledgement has been received.
    ///
    /// This can be viewed as an override of `with_reliable_ordered_resend_interval` for the first
    /// resend.
    ///
    /// The default is 500 milliseconds.
    fn with_initial_reliable_ordered_resend_delay(self, interval: Duration) -> Self;
    /// Set the maximum amount of times reliable unordered packets will be resend while no
    /// acknowledgement has been received.
    ///
    /// The default is 100.
    fn with_max_reliable_unordered_retries(self, retries: usize) -> Self;
    /// Set the maximum amount of times reliable ordered packets will be resend while no
    /// acknowledgement has been received.
    ///
    /// The default is 100.
    fn with_max_reliable_ordered_retries(self, retries: usize) -> Self;
}

pub(crate) trait SocketSendAddr: Copy {
    fn send(
        &self,
        socket: &mut UdpCommunicatorSocket,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, std::io::Error>;
}

pub(crate) struct UdpCommunicatorSocket {
    pub socket: UdpSocket,
    pub data_buffer: [u8; MAX_PACKET_LEN],
    /// The delay between resends of the packet.
    pub reliable_unordered_resend_interval: Duration,
    pub reliable_ordered_resend_interval: Duration,
    /// Send packet for the first time, then wait for this period, send again, then resend in the
    /// interval defined by `reliable_unordered_resend_interval`.
    pub initial_reliable_unordered_resend_delay: Duration,
    pub initial_reliable_ordered_resend_delay: Duration,
    /// Maximum amount of times a packet will be resend after the initial send.
    pub max_reliable_unordered_retries: usize,
    pub max_reliable_ordered_retries: usize,
    #[cfg(feature = "debug")]
    pub drop_probability: Option<f64>,
    #[cfg(feature = "debug")]
    pub corruption_probability: Option<f64>,
    #[cfg(feature = "debug")]
    pub fake_delay: std::ops::Range<u64>,
    #[cfg(feature = "debug")]
    pub debug_logs: bool,
    #[cfg(feature = "debug")]
    /// For each read from the socket, this stores the address from which the data was received,
    /// the copied data_buffer, the amount of bytes read and the instant at to which this packet
    /// is being delayed.
    pub fake_delayed_buffer: Vec<(
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
            //
            reliable_unordered_resend_interval: Duration::from_millis(100),
            reliable_ordered_resend_interval: Duration::from_millis(100),
            // TODO! Make the default smaller?
            initial_reliable_unordered_resend_delay: Duration::from_millis(500),
            initial_reliable_ordered_resend_delay: Duration::from_millis(500),
            //
            max_reliable_unordered_retries: 100,
            max_reliable_ordered_retries: 100,
            #[cfg(feature = "debug")]
            drop_probability: None,
            #[cfg(feature = "debug")]
            corruption_probability: None,
            #[cfg(feature = "debug")]
            fake_delay: 0..0,
            #[cfg(feature = "debug")]
            debug_logs: false,
            #[cfg(feature = "debug")]
            fake_delayed_buffer: vec![],
        }
    }

    fn with_reliable_unordered_resend_interval(mut self, interval: Duration) -> Self {
        self.reliable_unordered_resend_interval = interval;
        self
    }

    fn with_reliable_ordered_resend_interval(mut self, interval: Duration) -> Self {
        self.reliable_ordered_resend_interval = interval;
        self
    }

    fn with_initial_reliable_unordered_resend_delay(mut self, interval: Duration) -> Self {
        self.initial_reliable_unordered_resend_delay = interval;
        self
    }

    fn with_initial_reliable_ordered_resend_delay(mut self, interval: Duration) -> Self {
        self.initial_reliable_ordered_resend_delay = interval;
        self
    }

    fn with_max_reliable_unordered_retries(mut self, retries: usize) -> Self {
        self.max_reliable_unordered_retries = retries;
        self
    }

    fn with_max_reliable_ordered_retries(mut self, retries: usize) -> Self {
        self.max_reliable_ordered_retries = retries;
        self
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
