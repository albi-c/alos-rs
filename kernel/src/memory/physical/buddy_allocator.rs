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
    first_free: usize,
    last_free: usize,
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
            first_free: 0,
            last_free: usize::MAX,
        }
    }
}

impl<const N: usize> MemoryAllocator for BuddyAllocator<N> {
    fn new() -> Self {
        Self::default()
    }

    fn init(&mut self, start: usize, end: usize) {
        let start = address::page_align_up(start) >> PAGE_SHIFT;
        let end = address::page_align_down(end) >> PAGE_SHIFT;

        self.start_page = start;
        self.end_page = end;

        self.first_free = end;
        self.last_free = start;

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
        self.alloc_rec_set(0, end_offset);
    }

    fn alloc_page(&mut self) -> Option<usize> {
        if !self.enabled {
            return None;
        }
        if !self.alloc_get_set(0, self.first_free) {
            let addr = self.first_free << PAGE_SHIFT;
            self.first_free += 1;
            Some(addr)
        } else {
            for i in self.first_free..=self.last_free {
                if !self.alloc_get_set(0, i) {
                    let addr = i << PAGE_SHIFT;
                    self.first_free = i + 1;
                    return Some(addr);
                }
            }
            None
        }
    }
    fn dealloc_page(&mut self, addr: usize) -> bool {
        if !self.enabled || addr < (self.start_page << PAGE_SHIFT) || addr > (self.end_page << PAGE_SHIFT) {
            return false;
        }
        assert!(address::is_page_aligned(addr));
        let page = addr >> PAGE_SHIFT;
        self.alloc_set(0, page);
        self.first_free = min(self.first_free, page - self.start_page);
        self.last_free = max(self.last_free, page - self.start_page + 1);
        true
    }

    fn alloc_pages(&mut self, count: usize) -> Option<usize> {
        if !self.enabled {
            return None;
        }
        // TODO: use buddies
        'outer: for i in self.first_free..=self.last_free.checked_sub(count)? {
            for j in 0..count {
                if self.alloc_get(0, i + j) {
                    continue 'outer;
                }
            }
            self.alloc_set_n(0, i, count);
            let addr = i << PAGE_SHIFT;
            if i == self.first_free {
                self.first_free = i + 1;
            }
            return Some(addr);
        }
        None
    }
    fn dealloc_pages(&mut self, addr: usize, count: usize) -> bool {
        if !self.enabled || addr < (self.start_page << PAGE_SHIFT) || addr > (self.end_page << PAGE_SHIFT) {
            return false;
        }
        assert!(address::is_page_aligned(addr));
        let page = addr >> PAGE_SHIFT;
        self.alloc_unset_n(0, page, count);
        self.first_free = min(self.first_free, page - self.start_page);
        self.last_free = max(self.last_free, page - self.start_page + count);
        true
    }
}

impl<const N: usize> BuddyAllocator<N> {
    #[inline(always)]
    fn alloc_get(&mut self, level: usize, i: usize) -> bool {
        self.buddies[level][i >> PAGE_DATA_SHIFT] & (1 << (i & PAGE_DATA_MASK)) != 0
    }

    #[inline(always)]
    fn alloc_get_set(&mut self, level: usize, i: usize) -> bool {
        let bit = 1 << (i & PAGE_DATA_MASK);
        let el = &mut self.buddies[level][i >> PAGE_DATA_SHIFT];
        let set = (*el & bit) != 0;
        *el |= bit;
        set
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
