use crate::memory::address;
use crate::memory::address::PAGE_SHIFT;
use crate::memory::physical::allocator::MemoryAllocator;

type PageData = u8;
const PAGE_DATA_SHIFT: u8 = 3;
const PAGE_DATA_FULL: PageData = (1 << PAGE_DATA_SHIFT) - 1;

pub struct BuddyAllocator<const N: usize> {
    data: *mut u8,
    buddies: [&'static mut [PageData]; N],
    page_data_size: u64,
    num_page_data_elements: u64,
    start_page: u64,
    end_page: u64,
    enabled: bool,
}

unsafe impl<const N: usize> Send for BuddyAllocator<N> {}
unsafe impl<const N: usize> Sync for BuddyAllocator<N> {}

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

    fn init(&mut self, start: u64, end: u64) {
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
        self.num_page_data_elements = (num_pages >> (3 + PAGE_DATA_SHIFT));
    }
    fn data_size(&self) -> u64 {
        self.page_data_size
    }
    fn set_data(&mut self, data: *mut u8) {
        self.data = data;
        let mut data = unsafe { core::slice::from_raw_parts_mut(data, self.page_data_size as usize) };
        data.fill(0xff);

        for i in 0..N {
            let (buddy, rest) = data.split_at_mut(self.page_data_size as usize >> (i + 1));
            data = rest;
            self.buddies[i] = buddy;
        }
    }
}
