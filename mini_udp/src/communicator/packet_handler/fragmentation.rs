use std::fmt::Debug;

use crate::prelude::*;

pub trait UnreliableFragmentationHandler: Debug + Default {
    fn read_fragment(chunk_id: u16, fragment_id: u8, num_fragments: u8, data: &mut [u8; 1024]);
}

pub trait ReliableFragmentationHandler: Debug + Default {}

impl UnreliableFragmentationHandler for () {
    fn read_fragment(_chunk_id: u16, _fragment_id: u8, _num_fragments: u8, _data: &mut [u8; 1024]) {
    }
}

impl ReliableFragmentationHandler for () {}

#[derive(Debug, Default)]
pub struct UnreliableFragmentation<const MAX_FRAGMENTS: u8>;

impl<const MAX_FRAGMENTS: u8> UnreliableFragmentationHandler
    for UnreliableFragmentation<MAX_FRAGMENTS>
{
    fn read_fragment(chunk_id: u16, fragment_id: u8, num_fragments: u8, data: &mut [u8; 1024]) {
        todo!()
    }
}

#[derive(Debug, Default)]
pub struct ReliableFragmentation<const MAX_FRAGMENTS: u8>;

impl<const MAX_FRAGMENTS: u8> ReliableFragmentationHandler
    for ReliableFragmentation<MAX_FRAGMENTS>
{
}
