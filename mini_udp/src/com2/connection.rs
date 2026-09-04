use std::net::SocketAddr;

use crate::prelude2::*;

enum XConnectionState {
    Disconnected,
    ConnectionFailed {
        /// IO, connection refused, response timeout
        reason: Error,
    },
    SendingConnectionRequest {
        addr: SocketAddr,
        local_salt: u32,
    },
    SendingChallengeResponse {
        addr: SocketAddr,
        /// XOR of local salt and remote salt
        salt: u32,
    },
    IncomingConnectionRequest {
        addr: SocketAddr,
        remote_salt: u32,
    },
    Connected {
        addr: SocketAddr,
    },
}

pub enum ConnectionState<ConHandler: ConnectionHandler> {
    Disconnected,
    ConnectionFailed {
        addr: SocketAddr,
        /// IO, connection refused, response timeout
        reason: ConHandler::ConnectionFailureReason,
    },
    OutgoingConnectRequest {
        addr: SocketAddr,
        state: ConHandler::OutgoingRequestState,
    },
    IncomingConnectRequest {
        addr: SocketAddr,
        state: ConHandler::IncomingRequestState,
    },
    Connected {
        addr: SocketAddr,
        info: ConHandler::ConnectionInfo,
    },
}

pub trait ConnectionHandler {
    type OutgoingRequestState;
    type IncomingRequestState;

    type ConnectionFailureReason;

    type ConnectionInfo;
    type Protection: PacketProtection;
}
