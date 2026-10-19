use core::cmp::{max, min};
use core::mem::MaybeUninit;
use crate::memory::address::{PageCount, PhysAddrPageAligned};
use crate::memory::physical::allocator::MemoryAllocator;

type PageData = u64;
const PAGE_DATA_SHIFT: u8 = 6;
const PAGE_DATA_BITS_SHIFT: u8 = PAGE_DATA_SHIFT + 3;
const PAGE_DATA_FULL: PageData = !0;
const PAGE_DATA_MASK: usize = (1 << PAGE_DATA_SHIFT) - 1;

trait PageCountExt : Sized {
    fn buddy_index(self) -> usize;

    fn buddy_bit_index(self) -> usize;

    #[inline(always)]
    fn buddy_mask(self) -> PageData {
        1 << self.buddy_bit_index()
    }
}

impl PageCountExt for PageCount {
    #[inline(always)]
    fn buddy_index(self) -> usize {
        self.count() >> PAGE_DATA_BITS_SHIFT
    }

    #[inline(always)]
    fn buddy_bit_index(self) -> usize {
        self.count() & PAGE_DATA_MASK
    }
}

pub struct BuddyAllocator<const N: usize> {
    buddies: [&'static mut [PageData]; N],
    page_data_size: usize,
    num_page_data_elements: usize,
    start_page: PageCount,
    end_page: PageCount,
    enabled: bool,
    first_free: PageCount,
    last_free: PageCount,
}

unsafe impl<const N: usize> Send for BuddyAllocator<N> {}
unsafe impl<const N: usize> Sync for BuddyAllocator<N> {}

impl<const N: usize> BuddyAllocator<N> {
    const EMPTY_BUDDIES: [&'static mut [PageData]; N] = const {
        let mut buddies: [MaybeUninit<&'static mut [PageData]>; N] = [const { MaybeUninit::uninit() }; N];
        let mut i = 0;
        while i < N {
            buddies[i].write([].as_mut_slice());
            i += 1;
        }
        unsafe { MaybeUninit::array_assume_init(buddies) }
    };

    pub const fn default() -> Self {
        BuddyAllocator {
            buddies: Self::EMPTY_BUDDIES,
            page_data_size: 0,
            num_page_data_elements: 0,
            start_page: PageCount::zero(),
            end_page: PageCount::zero(),
            enabled: false,
            first_free: PageCount::zero(),
            last_free: PageCount::pages_down(usize::MAX),
        }
    }
}

impl<const N: usize> MemoryAllocator for BuddyAllocator<N> {
    fn new() -> Self {
        Self::default()
    }

    fn init(&mut self, start: PhysAddrPageAligned, end: PhysAddrPageAligned) {
        let start = start.page_count();
        let end = end.page_count();

        self.start_page = start;
        self.end_page = end;

        self.enabled = end > start;
        if !self.enabled {
            return;
        }

        self.first_free = end - start;
        self.last_free = start;

        let num_pages = end - start;
        self.page_data_size = (num_pages.count() >> 2) + 1 + (N << PAGE_DATA_SHIFT);
        self.num_page_data_elements = self.page_data_size >> PAGE_DATA_SHIFT;
    }
    fn data_size(&self) -> usize {
        self.page_data_size
    }
    fn set_data(&mut self, data: &'static mut [u8]) {
        if !self.enabled {
            return;
        }

        assert!(data.len() >= self.data_size());
        assert!(data.as_mut_ptr().is_aligned_to(align_of::<PageData>()));
        let mut data = bytemuck::cast_slice_mut(&mut data[..(self.num_page_data_elements << PAGE_DATA_SHIFT)]);
        data.fill(PAGE_DATA_FULL);

        for i in 0..N {
            let (buddy, rest) = data.split_at_mut(
                (self.num_page_data_elements >> (i + 1)) + 1);
            data = rest;
            self.buddies[i] = buddy;
        }
    }
    fn include(&mut self, start: PhysAddrPageAligned, end: PhysAddrPageAligned) {
        if !self.enabled {
            return;
        }

        let start = start.page_count();
        let end = end.page_count();

        if end <= start || end <= self.start_page || start > self.end_page {
            return;
        }

        let start = max(start, self.start_page);
        let end = min(end, self.end_page);

        if end <= start {
            return;
        }

        let start_offset = start - self.start_page;
        let end_offset = end - self.start_page;

        self.first_free = min(self.first_free, start_offset);
        self.last_free = max(self.last_free, end_offset - 1);

        for i in start_offset..end_offset {
            self.alloc_rec_unset(0, i);
        }

        self.alloc_rec_set(0, start_offset);
        self.alloc_rec_set(0, end_offset - 1);
    }

    fn alloc_page(&mut self) -> Option<PhysAddrPageAligned> {
        if !self.enabled {
            return None;
        }
        if self.first_free > self.last_free {
            return None;
        }
        if !self.alloc_get_set(0, self.first_free) {
            let addr = self.first_free.as_phys_addr();
            self.first_free += PageCount::new(1);
            Some(addr)
        } else {
            for i in self.first_free..=self.last_free {
                if !self.alloc_get_set(0, i) {
                    self.first_free = i + 1;
                    return Some(i.as_phys_addr());
                }
            }
            None
        }
    }
    fn dealloc_page(&mut self, addr: PhysAddrPageAligned) -> bool {
        if !self.enabled || addr < self.start_page.as_phys_addr() || addr > self.end_page.as_phys_addr() {
            return false;
        }
        let page = addr.page_count();
        self.alloc_set(0, page);
        self.first_free = min(self.first_free, page - self.start_page);
        self.last_free = max(self.last_free, page - self.start_page + 1);
        true
    }

    fn alloc_pages(&mut self, count: PageCount) -> Option<PhysAddrPageAligned> {
        if !self.enabled {
            return None;
        }
        // TODO: use buddies
        'outer: for i in self.first_free..=self.last_free.checked_sub(count)? {
            for j in PageCount::new(0)..count {
                if self.alloc_get(0, i + j) {
                    continue 'outer;
                }
            }
            self.alloc_set_n(0, i, count);
            if i == self.first_free {
                self.first_free = i + 1;
            }
            return Some(i.as_phys_addr());
        }
        None
    }
    fn dealloc_pages(&mut self, addr: PhysAddrPageAligned, count: PageCount) -> bool {
        if !self.enabled {
            return false;
        }
        if addr < self.start_page.as_phys_addr() || addr > self.end_page.as_phys_addr() {
            return false;
        }
        let page = addr.page_count();
        self.alloc_unset_n(0, page, count);
        self.first_free = min(self.first_free, page - self.start_page);
        self.last_free = max(self.last_free, page - self.start_page + count);
        true
    }
}

impl<const N: usize> BuddyAllocator<N> {
    #[inline(always)]
    fn alloc_get(&mut self, level: usize, i: PageCount) -> bool {
        self.buddies[level][i.buddy_index()] & i.buddy_mask() != 0
    }

    #[inline(always)]
    fn alloc_get_set(&mut self, level: usize, i: PageCount) -> bool {
        let bit = i.buddy_mask();
        let el = &mut self.buddies[level][i.buddy_index()];
        let set = (*el & bit) != 0;
        *el |= bit;
        set
    }

    #[inline]
    fn _alloc_modify_n<const OVERWRITE: PageData>(&mut self, level: usize,
                                                  mut i: PageCount, mut n: PageCount,
                                                  start_end: impl Fn(&mut Self, usize, PageCount) -> ()) {
        while i.buddy_bit_index() != 0 && n > 0 {
            start_end(self, level, i);
            i += PageCount::new(1);
            n -= PageCount::new(1);
        }

        let idx = i.buddy_index();
        self.buddies[level][idx..idx + n.buddy_index()].fill(OVERWRITE);
        let n1 = n;
        let mut n = PageCount::new(n.buddy_bit_index());
        i += n1 - n;

        while n > 0 {
            start_end(self, level, i);
            i += PageCount::new(1);
            n -= PageCount::new(1);
        }
    }

    #[inline(always)]
    fn alloc_set(&mut self, level: usize, i: PageCount) {
        self.buddies[level][i.buddy_index()] |= i.buddy_mask();
    }
    fn alloc_set_n(&mut self, level: usize, i: PageCount, n: PageCount) {
        self._alloc_modify_n::<PAGE_DATA_FULL>(level, i, n, Self::alloc_set);
    }

    fn alloc_rec_set(&mut self, level: usize, i: PageCount) {
        self.alloc_set(level, i);

        for o in 1..=level {
            self.alloc_set_n(level - o, i << o, PageCount::new(1) << o);
        }

        for o in 1..N-level {
            if self.alloc_get(o, i >> o) {
                break;
            }
            self.alloc_set(o, i >> o);
        }
    }

    #[inline(always)]
    fn alloc_unset(&mut self, level: usize, i: PageCount) {
        self.buddies[level][i.buddy_index()] &= !i.buddy_mask();
    }
    fn alloc_unset_n(&mut self, level: usize, i: PageCount, n: PageCount) {
        self._alloc_modify_n::<0>(level, i, n, Self::alloc_unset);
    }

    fn alloc_rec_unset(&mut self, level: usize, i: PageCount) {
        self.alloc_unset(level, i);

        for o in 1..=level {
            self.alloc_unset_n(level - o, i << o, PageCount::new(1) << o);
        }

        for o in 1..N-level {
            self.alloc_unset(level + o, i >> o);
            if self.alloc_get(level + o - 1, (i >> (o - 1)).xor(1)) {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::rust_2024::*;
    use crate::memory::address::{PageCount, PhysAddrPageAligned};
    use crate::memory::physical::allocator::MemoryAllocator;
    use crate::memory::physical::buddy_allocator::{BuddyAllocator, PageData};

    fn create_test_buddy() -> BuddyAllocator<8> {
        let mut buddy = BuddyAllocator::<8>::new();
        buddy.init(PhysAddrPageAligned::zero(), PageCount::new(0x400).as_phys_addr());
        let data_size = buddy.data_size();
        let data = Vec::leak(vec![0u8; data_size]);
        buddy.set_data(data);
        buddy
    }

    #[test]
    fn test_buddy_empty() {
        let mut buddy = BuddyAllocator::<8>::new();
        buddy.init(PhysAddrPageAligned::zero(), PhysAddrPageAligned::zero());
        assert_eq!(buddy.page_data_size, 0);
        assert_eq!(buddy.num_page_data_elements, 0);
        let data_size = buddy.data_size();
        assert_eq!(data_size, 0);
        buddy.set_data(bytemuck::cast_slice_mut::<PageData, _>(&mut []));

        assert_eq!(buddy.alloc_page(), None);
        assert_eq!(buddy.alloc_pages(1.into()), None);
    }

    #[test]
    fn test_buddy_alloc() {
        let mut buddy = create_test_buddy();
        assert_eq!(buddy.alloc_pages(buddy.end_page + PageCount::new(10)), None);
        assert_eq!(buddy.alloc_pages(1.into()), None);

        buddy.include(PageCount::new(0).as_phys_addr(), PageCount::new(10).as_phys_addr());

        assert_eq!(buddy.alloc_pages(9.into()), None);

        let large = buddy.alloc_pages(8.into());
        assert!(large.is_some());
        assert_eq!(buddy.alloc_page(), None);

        buddy.dealloc_pages(large.unwrap(), 8.into());

        let large_1 = buddy.alloc_pages(4.into());
        assert!(large_1.is_some());
        assert!(buddy.alloc_page().is_some());
        let large_2 = buddy.alloc_pages(3.into());
        assert!(large_2.is_some());
        assert_eq!(buddy.alloc_page(), None);

        buddy.dealloc_pages(large_1.unwrap(), 4.into());
        buddy.dealloc_pages(large_2.unwrap(), 3.into());

        assert_eq!(buddy.alloc_pages(5.into()), None);
        assert!(buddy.alloc_pages(4.into()).is_some());
        assert!(buddy.alloc_pages(3.into()).is_some());
        assert_eq!(buddy.alloc_page(), None);
    }
}
