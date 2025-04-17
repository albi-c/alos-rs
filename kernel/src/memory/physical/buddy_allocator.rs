use core::cmp::{max, min};
use core::mem::MaybeUninit;
use crate::memory::address;
use crate::memory::address::PAGE_SHIFT;
use crate::memory::physical::allocator::MemoryAllocator;

type PageData = u64;
const PAGE_DATA_SHIFT: u8 = 6;
const PAGE_DATA_FULL: PageData = !0;
const PAGE_DATA_SIZE: usize = 1 << PAGE_DATA_SHIFT;
const PAGE_DATA_MASK: usize = (1 << PAGE_DATA_SHIFT) - 1;

pub struct BuddyAllocator<const N: usize> {
    data: *mut u8,
    buddies: [&'static mut [PageData]; N],
    page_data_size: usize,
    num_page_data_elements: usize,
    start_page: usize,
    end_page: usize,
    enabled: bool,
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
            data: 0 as *mut u8,
            buddies: Self::EMPTY_BUDDIES,
            page_data_size: 0,
            num_page_data_elements: 0,
            start_page: 0,
            end_page: 0,
            enabled: false,
        }
    }
}

impl<const N: usize> MemoryAllocator for BuddyAllocator<N> {
    fn new() -> Self {
        BuddyAllocator {
            data: 0 as *mut u8,
            buddies: core::array::from_fn(|_| [].as_mut_slice()),
            page_data_size: 0,
            num_page_data_elements: 0,
            start_page: 0,
            end_page: 0,
            enabled: false,
        }
    }

    fn init(&mut self, start: usize, end: usize) {
        let start = address::page_align_up(start) >> PAGE_SHIFT;
        let end = address::page_align_down(end) >> PAGE_SHIFT;

        self.start_page = start;
        self.end_page = end;

        self.enabled = end > start;
        if !self.enabled {
            return;
        }

        let num_pages = end - start;
        self.page_data_size = (num_pages >> 2) + 1;
        self.num_page_data_elements = num_pages >> (3 + PAGE_DATA_SHIFT);
    }
    fn data_size(&self) -> usize {
        self.page_data_size
    }
    fn set_data(&mut self, data: &mut [u8]) {
        assert!(data.len() >= self.data_size());
        self.data = data.as_mut_ptr();
        let mut data = unsafe {
            core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut PageData,
                                            self.num_page_data_elements)
        };
        data.fill(0xff);

        for i in 0..N {
            let (buddy, rest) = data.split_at_mut(
                self.num_page_data_elements >> (i + 1));
            data = rest;
            self.buddies[i] = buddy;
        }
    }
    fn include(&mut self, start: usize, end: usize) {
        if !self.enabled {
            return;
        }

        let start = address::page_align_up(start) >> PAGE_SHIFT;
        let end = address::page_align_down(end) >> PAGE_SHIFT;

        if end <= start || end < self.start_page || start > self.end_page {
            return;
        }

        let start = max(start, self.end_page);
        let end = min(end, self.start_page);

        let start_offset = start - self.start_page;
        let end_offset = end - self.start_page;

        for i in start_offset..end_offset {
            self.alloc_rec_unset(0, i);
        }

        self.alloc_rec_set(0, start_offset);
        self.alloc_rec_set(0, end_offset);
    }
}

impl <const N: usize> BuddyAllocator<N> {
    #[inline(always)]
    fn alloc_get(&mut self, level: usize, i: usize) -> bool {
        self.buddies[level][i >> PAGE_DATA_SHIFT] & (1 << (i & PAGE_DATA_MASK)) != 0
    }

    #[inline]
    fn _alloc_modify_n(&mut self, level: usize, mut i: usize, mut n: usize,
                       start_end: impl Fn(&mut Self, usize, usize) -> (), overwrite: PageData) {
        while i & PAGE_DATA_MASK != 0 && n > 0 {
            start_end(self, level, i);
            i += 1;
            n -= 1;
        }

        // while n >= PAGE_DATA_SIZE {
        //     self.buddies[level][i >> PAGE_DATA_SHIFT] = overwrite;
        //     i += PAGE_DATA_SIZE;
        //     n -= PAGE_DATA_SIZE;
        // }

        let idx = i >> PAGE_DATA_SHIFT;
        self.buddies[level][idx..idx + (n >> PAGE_DATA_SHIFT)].fill(overwrite);
        let n1 = n;
        n &= PAGE_DATA_MASK;
        i += n1 - n;

        while n > 0 {
            start_end(self, level, i);
            i += 1;
            n -= 1;
        }
    }

    #[inline(always)]
    fn alloc_set(&mut self, level: usize, i: usize) {
        self.buddies[level][i >> PAGE_DATA_SHIFT] |= 1 << (i & PAGE_DATA_MASK);
    }
    fn alloc_set_n(&mut self, level: usize, i: usize, n: usize) {
        self._alloc_modify_n(level, i, n, Self::alloc_set, PAGE_DATA_FULL);
    }

    fn alloc_rec_set(&mut self, level: usize, i: usize) {
        self.alloc_set(level, i);

        for o in 1..=level {
            self.alloc_set_n(level - o, i << o, 1 << o);
        }

        for o in 1..N-level {
            if self.alloc_get(level - o, i >> o) {
                break;
            }
            self.alloc_set(level - o, i >> o);
        }
    }

    #[inline(always)]
    fn alloc_unset(&mut self, level: usize, i: usize) {
        self.buddies[level][i >> PAGE_DATA_SHIFT] &= !(1 << (i & PAGE_DATA_MASK));
    }
    fn alloc_unset_n(&mut self, level: usize, i: usize, n: usize) {
        self._alloc_modify_n(level, i, n, Self::alloc_unset, 0);
    }

    fn alloc_rec_unset(&mut self, level: usize, i: usize) {
        self.alloc_unset(level, i);

        for o in 1..=level {
            self.alloc_unset_n(level - o, i << o, 1 << o);
        }

        for o in 1..N-level {
            self.alloc_unset(level + o, i >> o);
            if self.alloc_get(level + o - 1, (i >> (o - 1)) ^ 1) {
                break;
            }
        }
    }
}
