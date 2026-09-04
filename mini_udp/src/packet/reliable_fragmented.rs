use crate::prelude2::*;

pub trait ReliableFragmentationHandler: Debug + Default {}

impl ReliableFragmentationHandler for () {}

#[derive(Debug, Default)]
/// As [`ReliableFragmentation`] represents fragment ids as [`u8`], `MAX_FRAGMENTS` has to be in
/// range `1..=256`.
///
/// **Panics**
/// If `MAX_FRAGMENTS` is zero or greater than 256.
pub struct ReliableFragmentation<const MAX_FRAGMENTS: usize>;

impl<const MAX_FRAGMENTS: usize> ReliableFragmentationHandler
    for ReliableFragmentation<MAX_FRAGMENTS>
{
}
