use std::{fmt::Debug, marker::PhantomData};

#[derive(Debug)]
/// A ring buffer with a capacity of `NUM_ITEMS` items.
pub struct RingBuffer<T, const NUM_ITEMS: usize = 32> {
    newest: u16,
    items: [Option<T>; NUM_ITEMS],
}

impl<T, const NUM_ITEMS: usize> Default for RingBuffer<T, NUM_ITEMS> {
    /// `NUM_ITEMS` has to be a power of 2
    fn default() -> Self {
        assert!(NUM_ITEMS < u16::MAX as usize);
        assert_eq!(u16::MAX % NUM_ITEMS as u16, NUM_ITEMS as u16 - 1);
        Self {
            newest: u16::MAX,
            items: std::array::from_fn(|_| None),
        }
    }
}

impl<T, const NUM_ITEMS: usize> RingBuffer<T, NUM_ITEMS> {
    /// `NUM_ITEMS` has to be a power of 2
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, index: u16) -> Option<&T> {
        let i = self.newest.wrapping_sub(index) as usize;
        if i < NUM_ITEMS {
            self.items[index as usize % NUM_ITEMS].as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: u16) -> Option<&mut T> {
        let i = self.newest.wrapping_sub(index) as usize;
        if i < NUM_ITEMS {
            self.items[index as usize % NUM_ITEMS].as_mut()
        } else {
            None
        }
    }

    pub fn push(&mut self, item: T) -> u16 {
        self.newest = self.newest.wrapping_add(1);
        let i = self.newest;
        self.items[i as usize % NUM_ITEMS] = Some(item);
        i
    }

    pub fn insert(&mut self, index: u16, item: T) {
        let prev_oldest = self.newest.wrapping_sub(NUM_ITEMS as u16 - 1);
        if wrapping_gt(prev_oldest, index, NUM_ITEMS as u16 * 2)
            || wrapping_gt(
                index,
                self.newest.wrapping_add(NUM_ITEMS as u16 - 1),
                NUM_ITEMS as u16 * 2,
            )
        {
            // The index is either older than the previous oldest posibble entry or it is at least
            // [`NUM_ITEMS`] greater than the previous newest entry
            // *Nuke the whole buffer*
            for item in &mut self.items {
                item.take();
            }
            self.newest = index;
            self.items[index as usize % NUM_ITEMS] = Some(item);
        } else if wrapping_gt(self.newest.wrapping_add(1), index, NUM_ITEMS as u16)
            && wrapping_gt(
                index,
                self.newest.wrapping_sub(NUM_ITEMS as u16),
                NUM_ITEMS as u16,
            )
        {
            // The index is in range of the previous buffer
            // *Only insert the new value, nothing more*
            self.items[index as usize % NUM_ITEMS] = Some(item);
        } else {
            // The index is greater than the previous newest item, but not by too much.
            // *Purge all values between the previous oldest and `index - NUM_ITEMS`*
            for i in wrapping_range(
                self.newest.wrapping_sub(NUM_ITEMS as u16 - 1),
                index.wrapping_sub(NUM_ITEMS as u16 - 1),
            ) {
                self.take(i);
            }
            self.newest = index;
            self.items[index as usize % NUM_ITEMS] = Some(item);
        }
    }

    pub fn take(&mut self, index: u16) -> Option<T> {
        let i = self.newest.wrapping_sub(index) as usize;
        if i < NUM_ITEMS {
            self.items[index as usize % NUM_ITEMS].take()
        } else {
            None
        }
    }

    /// Iterate over all existing items in chronological order (oldest first).
    pub fn iter(&self) -> Iter<'_, T, NUM_ITEMS> {
        Iter {
            i: self.newest.wrapping_sub(NUM_ITEMS as u16 - 1),
            ring: self,
        }
    }

    /// Iterate over all existing items in chronological order (oldest first) mutably.
    pub fn iter_mut(&mut self) -> IterMut<'_, T, NUM_ITEMS> {
        IterMut {
            i: self.newest.wrapping_sub(NUM_ITEMS as u16 - 1),
            ring: self,
            _marker: PhantomData,
        }
    }

    /// Iterate over all existing values in chronological order (oldest first).
    pub fn values(&self) -> IterValues<'_, T, NUM_ITEMS> {
        IterValues {
            i: self.newest.wrapping_sub(NUM_ITEMS as u16 - 1),
            ring: self,
        }
    }

    /// Iterate over all existing values in chronological order (oldest first) mutably.
    pub fn values_mut(&mut self) -> IterValuesMut<'_, T, NUM_ITEMS> {
        IterValuesMut {
            i: self.newest.wrapping_sub(NUM_ITEMS as u16 - 1),
            ring: self,
            _marker: PhantomData,
        }
    }

    /// Iterate over the indices of all existing items in chronological order (oldest first).
    /// The indices returned will increase, but wrap around at `u16::MAX`.
    pub fn keys(&self) -> IterKeys<'_, T, NUM_ITEMS> {
        IterKeys {
            i: self.newest.wrapping_sub(NUM_ITEMS as u16 - 1),
            ring: self,
        }
    }

    #[inline]
    pub fn push_will_override(&self) -> bool {
        let i = self.newest.wrapping_add(1);
        self.items[i as usize % NUM_ITEMS].is_some()
    }

    #[inline(always)]
    pub fn get_newest_index(&self) -> u16 {
        self.newest
    }

    #[inline(always)]
    pub fn get_next_index(&self) -> u16 {
        self.newest.wrapping_add(1)
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.items.iter().filter(|i| i.is_some()).count()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct Iter<'a, T, const NUM_ITEMS: usize> {
    i: u16,
    ring: &'a RingBuffer<T, NUM_ITEMS>,
}

impl<'a, T, const NUM_ITEMS: usize> Iterator for Iter<'a, T, NUM_ITEMS> {
    type Item = (u16, &'a T);
    fn next(&mut self) -> Option<Self::Item> {
        for i in wrapping_range(self.i, self.ring.newest.wrapping_add(1)) {
            self.i = self.i.wrapping_add(1);
            if let Some(item) = self.ring.get(i) {
                return Some((i, item));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.ring.len();
        if len == 0 || wrapping_gt(self.i.wrapping_sub(1), self.ring.newest, NUM_ITEMS as u16) {
            (0, Some(0))
        } else {
            let unvisited = self.ring.newest.wrapping_sub(self.i).wrapping_add(1) as usize;
            (
                len.saturating_sub(NUM_ITEMS.saturating_sub(unvisited)),
                Some(unvisited.min(len)),
            )
        }
    }
}

pub struct IterMut<'a, T, const NUM_ITEMS: usize> {
    i: u16,
    ring: *mut RingBuffer<T, NUM_ITEMS>,
    _marker: PhantomData<&'a mut RingBuffer<T, NUM_ITEMS>>,
}

impl<'a, T, const NUM_ITEMS: usize> Iterator for IterMut<'a, T, NUM_ITEMS> {
    type Item = (u16, &'a mut T);
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ring = &mut *self.ring;
            for i in wrapping_range(self.i, ring.newest.wrapping_add(1)) {
                self.i = self.i.wrapping_add(1);
                if let Some(item) = ring.get_mut(i) {
                    let item_ptr = item as *mut T;
                    return Some((i, &mut *item_ptr));
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        unsafe {
            let ring = &mut *self.ring;
            let len = ring.len();
            if len == 0 || wrapping_gt(self.i.wrapping_sub(1), ring.newest, NUM_ITEMS as u16) {
                (0, Some(0))
            } else {
                let unvisited = ring.newest.wrapping_sub(self.i).wrapping_add(1) as usize;
                (
                    len.saturating_sub(NUM_ITEMS.saturating_sub(unvisited)),
                    Some(unvisited.min(len)),
                )
            }
        }
    }
}

pub struct IterValues<'a, T, const NUM_ITEMS: usize> {
    i: u16,
    ring: &'a RingBuffer<T, NUM_ITEMS>,
}

impl<'a, T, const NUM_ITEMS: usize> Iterator for IterValues<'a, T, NUM_ITEMS> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        for i in wrapping_range(self.i, self.ring.newest.wrapping_add(1)) {
            self.i = self.i.wrapping_add(1);
            if let Some(item) = self.ring.get(i) {
                return Some(item);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.ring.len();
        if len == 0 || wrapping_gt(self.i.wrapping_sub(1), self.ring.newest, NUM_ITEMS as u16) {
            (0, Some(0))
        } else {
            let unvisited = self.ring.newest.wrapping_sub(self.i).wrapping_add(1) as usize;
            (
                len.saturating_sub(NUM_ITEMS.saturating_sub(unvisited)),
                Some(unvisited.min(len)),
            )
        }
    }
}

pub struct IterValuesMut<'a, T, const NUM_ITEMS: usize> {
    i: u16,
    ring: *mut RingBuffer<T, NUM_ITEMS>,
    _marker: PhantomData<&'a mut RingBuffer<T, NUM_ITEMS>>,
}

impl<'a, T, const NUM_ITEMS: usize> Iterator for IterValuesMut<'a, T, NUM_ITEMS> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ring = &mut *self.ring;
            for i in wrapping_range(self.i, ring.newest.wrapping_add(1)) {
                self.i = self.i.wrapping_add(1);
                if let Some(item) = ring.get_mut(i) {
                    let item_ptr = item as *mut T;
                    return Some(&mut *item_ptr);
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        unsafe {
            let ring = &mut *self.ring;
            let len = ring.len();
            if len == 0 || wrapping_gt(self.i.wrapping_sub(1), ring.newest, NUM_ITEMS as u16) {
                (0, Some(0))
            } else {
                let unvisited = ring.newest.wrapping_sub(self.i).wrapping_add(1) as usize;
                (
                    len.saturating_sub(NUM_ITEMS.saturating_sub(unvisited)),
                    Some(unvisited.min(len)),
                )
            }
        }
    }
}

pub struct IterKeys<'a, T, const NUM_ITEMS: usize> {
    i: u16,
    ring: &'a RingBuffer<T, NUM_ITEMS>,
}

impl<'a, T, const NUM_ITEMS: usize> Iterator for IterKeys<'a, T, NUM_ITEMS> {
    type Item = u16;
    fn next(&mut self) -> Option<Self::Item> {
        for i in wrapping_range(self.i, self.ring.newest.wrapping_add(1)) {
            self.i = self.i.wrapping_add(1);
            if self.ring.get(i).is_some() {
                return Some(i);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.ring.len();
        if len == 0 || wrapping_gt(self.i.wrapping_sub(1), self.ring.newest, NUM_ITEMS as u16) {
            (0, Some(0))
        } else {
            let unvisited = self.ring.newest.wrapping_sub(self.i).wrapping_add(1) as usize;
            (
                len.saturating_sub(NUM_ITEMS.saturating_sub(unvisited)),
                Some(unvisited.min(len)),
            )
        }
    }
}

fn wrapping_range(start: u16, end: u16) -> impl Iterator<Item = u16> {
    let len = end.wrapping_sub(start);
    (0..len).map(move |i| start.wrapping_add(i))
}

pub fn wrapping_gt(x: u16, y: u16, max_dif: u16) -> bool {
    if u16::MAX - x < max_dif && y <= x && u16::MAX - x + y < max_dif {
        return false;
    } else if u16::MAX - y < max_dif && x <= y && u16::MAX - y + x < max_dif {
        return true;
    }
    x > y
}

#[cfg(test)]
mod test {
    use crate::ring_buffer::{self, RingBuffer, wrapping_range};

    #[test]
    fn test_ring_buffer() {
        let mut i = 0;
        let mut ring = super::RingBuffer::<usize>::new();
        while i < 35 {
            let index = ring.push(i);
            assert_eq!(i, index as usize);
            assert_eq!(Some(&i), ring.get(index));
            if i > 30 {
                println!("i: {i}, i - 31: {}", i - 31);
                assert_eq!(Some(&(i - 31)), ring.get(i as u16 - 31));
            }
            i += 1;
        }
    }

    #[test]
    fn unlimited_push() {
        let mut i = 0;
        let mut wrap = true;
        let mut ring = super::RingBuffer::<u16>::new();
        loop {
            let index = ring.push(i);
            // indices increment sequentially
            assert_eq!(i, index);
            // Values can be accessed by their index
            assert_eq!(Some(&i), ring.get(index));
            {
                let oldest = i.wrapping_sub(31);
                let expected = (i > 30 || !wrap).then_some(&oldest);
                // We can access other values in the buffer if they exist
                assert_eq!(expected, ring.get(oldest));
            }
            // Iterate from i = 0 to i = u16::MAX exactly twice
            if i == u16::MAX {
                if wrap {
                    i = 0;
                    wrap = false;
                } else {
                    break;
                }
            } else {
                i += 1;
            }
        }
    }

    #[test]
    fn insert() {
        let mut ring = super::RingBuffer::<()>::new();
        ring.insert(1, ());
        assert!(ring.get(0).is_none());
        assert!(ring.get(1).is_some());
        assert!(ring.get(2).is_none());
        assert_eq!(ring.push(()), 2);
        assert_eq!(ring.push(()), 3);
        ring.insert(34, ());
        for i in 0..64 {
            if i == 34 || i == 3 {
                assert!(ring.get(i).is_some());
            } else {
                assert!(ring.get(i).is_none());
            }
        }
        for i in u16::MAX - 50..=u16::MAX {
            ring.insert(i, ());
        }
        for i in u16::MAX - 50..=u16::MAX {
            if (u16::MAX - 31..=u16::MAX).contains(&i) {
                assert!(ring.get(i).is_some());
            } else {
                assert!(ring.get(i).is_none());
            }
        }
        assert!(ring.get(0).is_none());
        assert!(ring.get(1).is_none());
    }

    #[test]
    fn insert2() {
        let mut ring = super::RingBuffer::<(), 4>::new();
        check_item_range(&ring, (0, 8), Option::is_none);
        ring.insert(2, ());
        check_item_range(&ring, (0, 2), Option::is_none);
        check_items(&ring, [(2, true)]);
        check_item_range(&ring, (3, 8), Option::is_none);
        //
        ring.insert(4, ());
        check_item_range(&ring, (0, 2), Option::is_none);
        check_items(&ring, [(2, true), (3, false), (4, true)]);
        check_item_range(&ring, (5, 8), Option::is_none);
        //
        ring.insert(1, ());
        check_items(
            &ring,
            [(0, false), (1, true), (2, true), (3, false), (4, true)],
        );
        check_item_range(&ring, (5, 8), Option::is_none);
        //
        ring.insert(0, ());
        check_items(&ring, [(0, true)]);
        check_item_range(&ring, (1, 8), Option::is_none);
        //
        ring.insert(5, ());
        check_item_range(&ring, (0, 5), Option::is_none);
        check_items(&ring, [(5, true)]);
        check_item_range(&ring, (6, 8), Option::is_none);
        //
        ring.insert(7, ());
        check_item_range(&ring, (0, 5), Option::is_none);
        check_items(&ring, [(5, true), (6, false), (7, true), (8, false)]);
        //
        ring.insert(3, ());
        check_item_range(&ring, (0, 3), Option::is_none);
        check_items(&ring, [(3, true)]);
        check_item_range(&ring, (4, 8), Option::is_none);
        //
        ring.insert(0, ());
        check_items(&ring, [(0, true), (1, false), (2, false), (3, true)]);
        check_item_range(&ring, (4, 8), Option::is_none);
    }

    fn check_items<const N: usize>(
        ring: &RingBuffer<(), N>,
        indices: impl IntoIterator<Item = (u16, bool)>,
    ) {
        for (i, is_some) in indices {
            if ring.get(i).is_some() != is_some {
                dbg!(i);
                dbg!(ring.get(i).is_some());
                dbg!(is_some);
            }
            assert_eq!(ring.get(i).is_some(), is_some);
        }
    }

    fn check_item_range<const N: usize>(
        ring: &RingBuffer<(), N>,
        bounds: (u16, u16),
        check: impl Fn(&Option<()>) -> bool,
    ) {
        for i in wrapping_range(bounds.0, bounds.1) {
            assert!(check(&ring.get(i).map(|_| ())));
        }
    }

    #[test]
    fn iterate() {
        let mut ring = super::RingBuffer::<u16>::new();
        assert_eq!(ring.push(0), 0);
        assert_eq!(ring.push(1), 1);
        assert_eq!(ring.len(), 2);
        let mut iter = ring.iter();
        assert_eq!(iter.size_hint(), (2, Some(2)));
        assert_eq!(iter.next(), Some((0, &0)));
        assert_eq!(iter.size_hint(), (0, Some(1)));
        assert_eq!(iter.next(), Some((1, &1)));
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_size_hint() {
        let mut ring = super::RingBuffer::<u16>::new();
        for i in 0..33 {
            assert_eq!(ring.push(i), i);
        }
        assert_eq!(ring.len(), 32);
        let mut iter = ring.iter();
        for i in 0..32 {
            assert_eq!(iter.size_hint(), (32 - i, Some(32 - i)));
            assert_eq!(iter.next(), Some((i as u16 + 1, &(i as u16 + 1))));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_size_hint2() {
        let mut ring = super::RingBuffer::<u16>::new();
        for i in 5..33 {
            ring.insert(i, i);
        }
        assert_eq!(ring.len(), 28);
        let mut iter = ring.iter();
        for i in 0..28 {
            assert_eq!(iter.next(), Some((i as u16 + 5, &(i as u16 + 5))));
            assert_eq!(iter.size_hint(), (23_usize.saturating_sub(i), Some(27 - i)));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_mut_size_hint() {
        let mut ring = super::RingBuffer::<u16>::new();
        for i in 0..33 {
            assert_eq!(ring.push(i), i);
        }
        assert_eq!(ring.len(), 32);
        let mut iter = ring.iter_mut();
        for i in 0..32 {
            assert_eq!(iter.size_hint(), (32 - i, Some(32 - i)));
            assert_eq!(iter.next(), Some((i as u16 + 1, &mut (i as u16 + 1))));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_mut_size_hint2() {
        let mut ring = super::RingBuffer::<u16>::new();
        for i in 5..33 {
            ring.insert(i, i);
        }
        assert_eq!(ring.len(), 28);
        let mut iter = ring.iter_mut();
        for i in 0..28 {
            assert_eq!(iter.next(), Some((i as u16 + 5, &mut (i as u16 + 5))));
            assert_eq!(iter.size_hint(), (23_usize.saturating_sub(i), Some(27 - i)));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iterate_many() {
        let mut ring = super::RingBuffer::<u16>::new();
        let range = 0..33;
        for i in range {
            ring.push(i);
        }
        // 33 elements were inserted, so all 32 slots in the buffer should be filled.
        // The oldest element (0) was replaced by the last element (32), so the first element now is
        // 1.
        let mut i = 1;
        for item in ring.values() {
            assert_eq!(*item, i);
            i += 1;
        }
    }

    #[test]
    fn iterate_values() {
        let mut ring = super::RingBuffer::<u16>::new();
        ring.push(0);
        ring.push(1);
        let mut values = ring.values();
        assert_eq!(values.next(), Some(&0));
        assert_eq!(values.next(), Some(&1));
        assert_eq!(values.next(), None);
    }

    #[test]
    fn iterate_values_mut() {
        let mut ring = super::RingBuffer::<u16>::new();
        ring.push(0);
        ring.push(1);
        let mut values = ring.values_mut();
        assert_eq!(values.next(), Some(&mut 0));
        assert_eq!(values.next(), Some(&mut 1));
        assert_eq!(values.next(), None);
    }

    #[test]
    fn iterate_keys() {
        let mut ring = super::RingBuffer::<()>::new();
        ring.push(());
        ring.push(());
        let mut iter = ring.keys();
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), None);
        ring.insert(u16::MAX, ());
        ring.push(());
        let mut iter = ring.keys();
        assert_eq!(iter.next(), Some(u16::MAX));
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn insert_will_override() {
        let mut ring = super::RingBuffer::<()>::new();
        for _ in 0..32 {
            assert!(!ring.push_will_override());
            ring.push(());
        }
        assert!(ring.push_will_override());
    }

    #[test]
    fn len() {
        let mut ring = super::RingBuffer::<()>::new();
        assert!(ring.is_empty());
        for i in 0..32 {
            assert_eq!(ring.len(), i);
            ring.push(());
        }
        assert_eq!(ring.len(), 32);
        ring.push(());
        assert_eq!(ring.len(), 32);
        // Doesn't do anything, the first element was overriden by the last push
        ring.take(0);
        assert_eq!(ring.len(), 32);
        ring.take(1);
        assert_eq!(ring.len(), 31);
    }

    #[test]
    fn wrapping_gt() {
        assert!(ring_buffer::wrapping_gt(10, 2, 32));
        assert!(!ring_buffer::wrapping_gt(10102, 12042, 32));
        assert!(ring_buffer::wrapping_gt(12042, 10102, 32));
        assert!(!ring_buffer::wrapping_gt(u16::MAX, 2, 32));
        assert!(ring_buffer::wrapping_gt(30, u16::MAX, 32));
        assert!(ring_buffer::wrapping_gt(31, u16::MAX, 32));
        assert!(!ring_buffer::wrapping_gt(32, u16::MAX, 32));
        assert!(!ring_buffer::wrapping_gt(65535_u16.wrapping_sub(31), 0, 32));
    }
}
