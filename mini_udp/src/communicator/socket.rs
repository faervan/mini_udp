use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::prelude::*;

pub trait CommunicatorSocket<CTX: MiniUdpContext> {
    /// Create a new communicator, binding it to the provided `addr`.
    fn bind_with<A: ToSocketAddrs>(
        addr: A,
        error_handler: <CTX::ErrorHandling as ErrorHandlingStrategy>::Handler,
    ) -> Self;
    /// Obtain a mutable reference to the configured [error
    /// handler](ErrorHandlingStrategy::Handler).
    fn get_error_handler_mut(
        &mut self,
    ) -> &mut <CTX::ErrorHandling as ErrorHandlingStrategy>::Handler;
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
    fn send<CTX: MiniUdpContext>(
        &self,
        socket: &mut UdpCommunicatorSocket<CTX>,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, Error>;
}

pub(crate) struct UdpCommunicatorSocket<CTX: MiniUdpContext> {
    pub socket: UdpSocket,
    pub data_buffer: [u8; MAX_PACKET_LEN],
    /// The delay between resends of the packet.
    pub reliable_unordered_resend_interval: Duration,
    pub reliable_ordered_resend_interval: Duration,
    /// Maximum amount of times a packet will be resend after the initial send.
    pub max_reliable_unordered_retries: usize,
    pub max_reliable_ordered_retries: usize,
    pub error_handler: <CTX::ErrorHandling as ErrorHandlingStrategy>::Handler,
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

impl<CTX: MiniUdpContext> CommunicatorSocket<CTX> for UdpCommunicatorSocket<CTX> {
    fn bind_with<A: ToSocketAddrs>(
        addr: A,
        error_handler: <<CTX as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler,
    ) -> Self {
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
            //
            max_reliable_unordered_retries: 100,
            max_reliable_ordered_retries: 100,
            error_handler,
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

    fn get_error_handler_mut(
        &mut self,
    ) -> &mut <<CTX as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler {
        &mut self.error_handler
    }

    fn with_reliable_unordered_resend_interval(mut self, interval: Duration) -> Self {
        self.reliable_unordered_resend_interval = interval;
        self
    }

    fn with_reliable_ordered_resend_interval(mut self, interval: Duration) -> Self {
        self.reliable_ordered_resend_interval = interval;
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
    fn send<CTX: MiniUdpContext>(
        &self,
        socket: &mut UdpCommunicatorSocket<CTX>,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, Error> {
        Ok(socket.socket.send(&socket.data_buffer[slice_index])?)
    }
}

impl SocketSendAddr for SocketAddr {
    fn send<CTX: MiniUdpContext>(
        &self,
        socket: &mut UdpCommunicatorSocket<CTX>,
        slice_index: impl std::slice::SliceIndex<[u8], Output = [u8]>,
    ) -> Result<usize, Error> {
        Ok(socket
            .socket
            .send_to(&socket.data_buffer[slice_index], self)?)
    }
}
