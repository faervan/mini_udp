use std::net::SocketAddr;

use crate::prelude2::*;

#[derive(Default, Debug, PartialEq)]
pub enum ConnectionState<ConHandler: ConnectionHandler> {
    #[default]
    Disconnected,
    Aborted {
        addr: SocketAddr,
        /// Point in time at which the last connection has been aborted / the last connection
        /// establishment has failed.
        aborted_since: Instant,
        /// IO, connection refused, response timeout
        reason: ConHandler::ConnectionFailureReason,
    },
    OutgoingConnectRequest {
        addr: SocketAddr,
        /// Point in time at which the first request has been sent or the last time a valid message
        /// has been received from the remote.
        instant: Instant,
        state: ConHandler::OutgoingRequestState,
    },
    IncomingConnectRequest {
        addr: SocketAddr,
        /// The last time a valid message has been received from the remote.
        instant: Instant,
        state: ConHandler::IncomingRequestState,
    },
    Connected {
        addr: SocketAddr,
        /// Point in time at which the connection has been established.
        connected_since: Instant,
        info: ConHandler::Protection,
    },
}

impl<ConHandler: ConnectionHandler> ConnectionState<ConHandler> {
    pub fn connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn connecting(&self) -> bool {
        matches!(self, Self::OutgoingConnectRequest { .. } | Self::IncomingConnectRequest { .. })
    }

    pub fn idle(&self) -> bool {
        matches!(self, Self::Disconnected { .. } | Self::Aborted { .. })
    }
}

pub trait ConnectionHandler: Debug + Default + Sized {
    type OutgoingRequestState;
    type IncomingRequestState;

    #[cfg(not(test))]
    type ConnectionEstablishmentData: ByteRepr;
    #[cfg(test)]
    type ConnectionEstablishmentData: ByteRepr + PartialEq;
    /// Input a client has to provide when it wants to establish a connection (e.g. a known public
    /// key of the remote).
    type ConnectionInput;
    type ConnectionFailureReason;

    type Protection: PacketProtection;

    fn state(&self) -> &ConnectionState<Self>;

    fn send_addr(&self) -> Option<&SocketAddr> {
        match self.state() {
            ConnectionState::Disconnected | ConnectionState::Aborted { .. } => None,
            ConnectionState::Connected { addr, .. }
            | ConnectionState::OutgoingConnectRequest { addr,.. }
            | ConnectionState::IncomingConnectRequest { addr,.. } => Some(addr)
        }
    }

    fn connect<Config: MiniUdpConfig>(
        &mut self,
        resend_handler: &mut <Config::Context as MiniUdpContext>::ResendStrategy,
        addr: SocketAddr,
        input: Self::ConnectionInput,
        ordered: &mut Config::ReliableOrderedPacketHandler,
    ) -> Result<(), Error>
    where
        Config::Context: MiniUdpContext<ConnectionHandler = Self>;

    /// Returns `Ok(true)` if a connection has been established successfully
    fn incoming_connect<F: FnOnce(SocketAddr) -> bool, Config: MiniUdpConfig>(
        &mut self,
        resend_handler: &mut <Config::Context as MiniUdpContext>::ResendStrategy,
        remote_addr: SocketAddr,
        data: Self::ConnectionEstablishmentData,
        connect_if: F,
        ordered: &mut Config::ReliableOrderedPacketHandler,
    ) -> Result<bool, Error>
         where Config::Context: MiniUdpContext<ConnectionHandler = Self>;

    /// On success this returns the amount of bytes written.
    fn write_packet<M: ByteRepr, Config: MiniUdpConfig>(
        &mut self,
        buffer: &mut [u8],
        packet: Packet<M, Config, Self>,
    ) -> Result<usize, Error>;

    fn read_packet<Config: MiniUdpConfig>(
        &mut self,
        remote_addr: SocketAddr,
        buffer: &mut [u8],
    ) -> Result<Packet<<Config::Context as MiniUdpContext>::Recv, Config, Self>, Error>;
}

#[derive(Default)]
pub struct InsecureConnection {
    state: ConnectionState<Self>,
}

impl Debug for InsecureConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InsecureConnection {{ state: {:?} }}", self.state)
    }
}

// #[cfg(test)]
// impl PartialEq for InsecureConnection {
//     fn eq(&self, other: &Self) -> bool {
//         self.crc.algorithm.init == other.crc.algorithm.init && self.state == other.state
//     }
// }

impl ConnectionHandler for InsecureConnection {
    /// Local salt if false, XOR`ed local and server salt if true
    /// The bool corresponds to "Received challenge"
    type OutgoingRequestState = (u32, bool);
    /// Remote salt
    type IncomingRequestState = u32;

    type ConnectionEstablishmentData = InsecureConnectionEstablishment;
    type ConnectionInput = ();
    type ConnectionFailureReason = ();

    /// XOR of local and remote salt
    type Protection = UnprotectedWithSalt<u32>;

    fn state(&self) -> &ConnectionState<Self> {
        &self.state
    }

    fn connect<Config: MiniUdpConfig>(
        &mut self,
        resend_handler: &mut <Config::Context as MiniUdpContext>::ResendStrategy,
        addr: SocketAddr,
        (): Self::ConnectionInput,
        ordered: &mut Config::ReliableOrderedPacketHandler,
    ) -> Result<(), Error>
    where
        Config::Context: MiniUdpContext<ConnectionHandler = Self>,
    {
        let _span = trace_span!("InsecureConnection::connect").entered();
        let local_salt = rand::random();
        self.state = ConnectionState::OutgoingConnectRequest {
            addr,
            instant: Instant::now(),
            state: (local_salt, false),
        };
        // Send connect
        let guard = ordered.write_packet(resend_handler, Priority::High)?;
        guard.write_to_connection::<Config, <Config::Context as MiniUdpContext>::Send, _>(
            self,
            |sequence_id, partial_ack| Packet::Connection {
                sequence_id,
                ack: PacketAck::from_reliable_ordered_ack(partial_ack),
                con_data: InsecureConnectionEstablishment::Connect {
                    local_salt,
                    ddos_padding: [0; _],
                },
            },
        )?;
        Ok(())
    }

    fn incoming_connect<F: FnOnce(SocketAddr) -> bool, Config: MiniUdpConfig>(
        &mut self,
        resend_handler: &mut <Config::Context as MiniUdpContext>::ResendStrategy,
        remote_addr: SocketAddr,
        data: Self::ConnectionEstablishmentData,
        connect_if: F,
        ordered: &mut Config::ReliableOrderedPacketHandler,
    ) -> Result<bool, Error>
        where Config::Context: MiniUdpContext<ConnectionHandler = Self>
    {
        let _span = trace_span!("InsecureConnection::incoming_connect").entered();
        match &mut self.state {
            ConnectionState::Disconnected | ConnectionState::Aborted { .. } => {
                match data {
                    InsecureConnectionEstablishment::Connect {
                        local_salt: remote_salt,
                        ..
                    } => {
                        if connect_if(remote_addr) {
                            let local_salt = rand::random::<u32>();
                            let salt = remote_salt ^ local_salt;
                            self.state = ConnectionState::IncomingConnectRequest {
                                addr: remote_addr,
                                instant: Instant::now(),
                                state: salt,
                            };
                            // Send challenge
                            let guard =
                                ordered.write_packet(resend_handler, Priority::High)?;
                            guard.write_to_connection::<Config, <Config::Context as MiniUdpContext>::Send, _>(
                                self,
                                |sequence_id, partial_ack| {
                                    Packet::Connection { 
                                        sequence_id,
                                        ack: PacketAck::from_reliable_ordered_ack(partial_ack),
                                        con_data: InsecureConnectionEstablishment::Challenge {
                                            local_salt: remote_salt,
                                            remote_salt: local_salt,
                                        },
                                    }
                                }
                            )?;
                        } else {
                            // Connection refused, do nothing
                        }
                    }
                    _ => {
                        trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                        return Err(Error::UnexpectedMessage);
                    },
                }
            }
            ConnectionState::OutgoingConnectRequest {
                addr,
                state: (salt, received_challenge),
                ..
            } => match data {
                InsecureConnectionEstablishment::Connect { .. } => {
                    // Incoming connection request while we are trying to connect.
                    // TODO! We might want to let the user decide what to do here.
                }
                InsecureConnectionEstablishment::Challenge {
                    local_salt,
                    remote_salt,
                } => {
                    if *addr == remote_addr && !*received_challenge && *salt == local_salt {
                        *received_challenge = true;
                        *salt = local_salt ^ remote_salt;
                        // Send challenge response
                        let guard = ordered.write_packet(resend_handler, Priority::High)?;
                        let salt = *salt;
                        guard.write_to_connection::<Config, <Config::Context as MiniUdpContext>::Send, _>(
                            self,
                            |sequence_id, partial_ack| {
                                Packet::Connection {
                                    sequence_id,
                                    ack: PacketAck::from_reliable_ordered_ack(partial_ack),
                                    con_data: InsecureConnectionEstablishment::ChallengeResponse { salt },
                                }
                            }
                        )?;
                    } else {
                        trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                        return Err(Error::UnexpectedMessage);
                    }
                }
                InsecureConnectionEstablishment::ChallengeResponse { .. } => {
                    trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                    return Err(Error::UnexpectedMessage);
                }
                InsecureConnectionEstablishment::ConnectionAccepted {
                    salt: accepted_salt,
                } => {
                    if *addr == remote_addr && *received_challenge && *salt == accepted_salt {
                        self.state = ConnectionState::Connected {
                            addr: remote_addr,
                            connected_since: Instant::now(),
                            info: UnprotectedWithSalt {
                                salt: accepted_salt,
                            },
                        };
                        return Ok(true);
                    } else {
                        trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                        return Err(Error::UnexpectedMessage);
                    }
                }
            },
            ConnectionState::IncomingConnectRequest {
                addr,
                state: our_salt,
                ..
            } => {
                match data {
                    InsecureConnectionEstablishment::Connect { .. } => {
                        // Incoming connection request while we accepted another request already.
                        // TODO! We might want to let the user decide what to do here.
                    }
                    InsecureConnectionEstablishment::Challenge { .. } => {
                        trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                        return Err(Error::UnexpectedMessage);
                    }
                    InsecureConnectionEstablishment::ChallengeResponse { salt } => {
                        if *addr == remote_addr && salt == *our_salt {
                            self.state = ConnectionState::Connected {
                                addr: remote_addr,
                                connected_since: Instant::now(),
                                info: UnprotectedWithSalt { salt },
                            };
                            // Send connection accepted
                            let guard =
                                ordered.write_packet(resend_handler, Priority::High)?;
                            guard.write_to_connection::<Config, <Config::Context as MiniUdpContext>::Send, _>(
                                self,
                                |sequence_id, partial_ack| {
                                    Packet::Connection { 
                                        sequence_id,
                                        ack: PacketAck::from_reliable_ordered_ack(partial_ack),
                                        con_data: InsecureConnectionEstablishment::ConnectionAccepted { salt },
                                    }
                                }
                            )?;
                            return Ok(true);
                        } else {
                            trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                            return Err(Error::UnexpectedMessage);
                        }
                    }
                    InsecureConnectionEstablishment::ConnectionAccepted { .. } => {
                        trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                        return Err(Error::UnexpectedMessage);
                    }
                }
            }
            ConnectionState::Connected { .. } => {
                return Err(match data {
                    InsecureConnectionEstablishment::Connect { .. } => Error::AlreadyConnected,
                    _ => {
                        trace!("Received {data:#?} from {remote_addr} while in state {:#?}", self.state);
                        Error::UnexpectedMessage
                    }
                });
            }
        }
        Ok(false)
    }

    fn write_packet<M: ByteRepr, Config: MiniUdpConfig>(
        &mut self,
        buffer: &mut [u8],
        packet: Packet<M, Config, Self>,
    ) -> Result<usize, Error> {
        let _span = trace_span!("InsecureConnection::write_packet").entered();
        let mut written = 0;
        match packet {
            Packet::Connection {
                sequence_id,
                ack,
                con_data,
            } => {
                let header = PacketHeader::<Config, Self>::Unprotected {
                    sequence_id,
                    ack,
                    con_data,
                };
                header.write_to_bytes(&mut buffer[4..])?;
                written += header.byte_len();
                let crc = Config::CRC.checksum(&buffer[4..4 + written]);
                buffer[..4].copy_from_slice(&crc.to_le_bytes());
                written += 4;
            }
            Packet::Data {
                sequence_id,
                ack,
                data,
            } => {
                let ConnectionState::Connected {
                    info: UnprotectedWithSalt { salt },
                    ..
                } = self.state
                else {
                    panic!("Attempted to write packet while not connected");
                };
                let header = PacketHeader::<Config, Self>::Protected {
                    sequence_id,
                    ack,
                    header: salt,
                };
                header.write_to_bytes(&mut buffer[4..])?;
                written += header.byte_len();
                data.write_to_bytes(&mut buffer[4 + written..])?;
                written += data.byte_len();
                let crc = Config::CRC.checksum(&buffer[4..4 + written]);
                buffer[..4].copy_from_slice(&crc.to_le_bytes());
                written += 4;
            }
        }
        Ok(written)
    }

    fn read_packet<Config: MiniUdpConfig>(
        &mut self,
        remote_addr: SocketAddr,
        buffer: &mut [u8],
    ) -> Result<Packet<<Config::Context as MiniUdpContext>::Recv, Config, Self>, Error> {
        let _span = trace_span!("InsecureConnection::read_packet").entered();
        let Ok(crc_bytes) = buffer[..4].try_into() else {
            return Err(Error::PacketLengthLessThanCrcBytes);
        };
        let crc = u32::from_le_bytes(crc_bytes);
        if Config::CRC.checksum(&buffer[4..]) != crc {
            trace!("CRC check failed");
            return Err(Error::CrcFailed);
        }
        let header = PacketHeader::<Config, Self>::from_bytes(&buffer[4..])?;
        match header {
            PacketHeader::Unprotected {
                sequence_id,
                ack,
                con_data,
            } => Ok(Packet::Connection {
                sequence_id,
                ack,
                con_data,
            }),
            PacketHeader::Protected {
                sequence_id,
                ack,
                header: packet_salt,
            } => {
                let buffer = &buffer[4+header.byte_len()..];
                match &self.state {
                    ConnectionState::Connected {
                        addr,
                        info: UnprotectedWithSalt { salt },
                        ..
                    } if *addr == remote_addr => {
                        if *salt == packet_salt {
                            let data = PacketData::from_bytes(buffer)?;
                            Ok(Packet::Data {
                                sequence_id,
                                ack,
                                data,
                            })
                        } else {
                            Err(Error::PacketSessionMismatch)
                        }
                    }
                    ConnectionState::OutgoingConnectRequest {
                        addr,
                        state: (salt, received_challenge),
                        ..
                    } if *addr == remote_addr && *received_challenge => {
                        if *salt == packet_salt {
                            let data = PacketData::from_bytes(buffer)?;
                            // We have not received the connection accepted message, but it was
                            // probably send already.
                            // TODO! Make sure this makes sense
                            Ok(Packet::Data {
                                sequence_id,
                                ack,
                                data,
                            })
                        } else {
                            Err(Error::PacketSessionMismatch)
                        }
                    }
                    _ => {
                        trace!("received PacketHeader::Protected in state: {:?}", self.state);
                        Err(Error::UnexpectedMessage)
                    }
                }
            }
        }
    }
}

#[derive(ByteRepr, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum InsecureConnectionEstablishment {
    Connect {
        local_salt: u32,
        ddos_padding: [u8; 200],
    },
    Challenge {
        local_salt: u32,
        remote_salt: u32,
    },
    ChallengeResponse {
        salt: u32,
    },
    ConnectionAccepted {
        salt: u32,
    },
}
