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
    /// Obtain a mutable reference to the configured [resend handler](MiniUdpContext::ResendStrategy).
    fn get_resend_handler_mut(&mut self) -> &mut CTX::ResendStrategy;
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
    pub error_handler: <CTX::ErrorHandling as ErrorHandlingStrategy>::Handler,
    pub resend_handler: CTX::ResendStrategy,
    #[cfg(any(test, feature = "debug"))]
    pub drop_probability: Option<f64>,
    #[cfg(any(test, feature = "debug"))]
    pub corruption_probability: Option<f64>,
    #[cfg(any(test, feature = "debug"))]
    pub fake_delay: std::ops::Range<u64>,
    #[cfg(any(test, feature = "debug"))]
    pub debug_logs: bool,
    #[cfg(any(test, feature = "debug"))]
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
            error_handler,
            resend_handler: CTX::ResendStrategy::default(),
            #[cfg(any(test, feature = "debug"))]
            drop_probability: None,
            #[cfg(any(test, feature = "debug"))]
            corruption_probability: None,
            #[cfg(any(test, feature = "debug"))]
            fake_delay: 0..0,
            #[cfg(any(test, feature = "debug"))]
            debug_logs: false,
            #[cfg(any(test, feature = "debug"))]
            fake_delayed_buffer: vec![],
        }
    }

    fn get_error_handler_mut(
        &mut self,
    ) -> &mut <<CTX as MiniUdpContext>::ErrorHandling as ErrorHandlingStrategy>::Handler {
        &mut self.error_handler
    }

    fn get_resend_handler_mut(&mut self) -> &mut <CTX as MiniUdpContext>::ResendStrategy {
        &mut self.resend_handler
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
