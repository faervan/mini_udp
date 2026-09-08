use crate::prelude2::*;

pub trait PacketProtection: Debug {
    type Header: ByteRepr;
}

#[derive(Debug)]
pub struct UnprotectedWithSalt<SALT> {
    pub salt: SALT,
}

impl<SALT: ByteRepr> PacketProtection for UnprotectedWithSalt<SALT> {
    type Header = SALT;
}
