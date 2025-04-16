mod allocator;
pub mod buddy_allocator;

use core::cmp::{max, min};
use limine::memory_map::{Entry, EntryType};
use limine::response::{ExecutableAddressResponse, HhdmResponse, MemoryMapResponse};
use crate::{debug, logger};
use crate::memory::address;
use crate::memory::physical::allocator::MemoryAllocator;

logger!("PMM");

pub struct MemoryManager<A: MemoryAllocator> {
    alloc_32: A,
    alloc_main: A,
}

impl<A: MemoryAllocator> MemoryManager<A> {
    pub const fn default(alloc_32: A, alloc_main: A) -> Self {
        MemoryManager {
            alloc_32,
            alloc_main,
        }
    }

    fn init_allocator(allocator: &mut A) {
        drop(core::mem::replace(allocator, A::new()));
    }

    pub fn init(&mut self, memory_map: &MemoryMapResponse, hhdm: &HhdmResponse,
                exec_addr: &ExecutableAddressResponse) {
        Self::init_allocator(&mut self.alloc_32);
        Self::init_allocator(&mut self.alloc_main);

        let hhdm_offset = hhdm.offset();

        let mut memory_end = 0;
        let mut largest_entry: Option<&Entry> = None;
        let mut kernel_entry: Option<&Entry> = None;
        for entry in memory_map.entries() {
            match entry.entry_type {
                EntryType::USABLE => {
                    debug!("Free memory block [{} | {} kB]", entry.base >> 10, entry.length >> 10);
                    memory_end = max(memory_end, (entry.base + entry.length) as usize);
                    if entry.length > largest_entry.map(|e| e.length).unwrap_or(0) {
                        largest_entry = Some(entry);
                    }
                },
                EntryType::EXECUTABLE_AND_MODULES => {
                    assert!(kernel_entry.replace(entry).is_none(), "Multiple kernel memory map entries");
                },
                _ => {},
            }
        }

        let largest_entry = largest_entry.expect("No free memory");
        let kernel_entry = kernel_entry.expect("No kernel memory map entry");

        debug!("Memory end: {} kB", memory_end >> 10);

        self.alloc_32.init(0, min(1 << 32, memory_end));
        self.alloc_main.init(1 << 32, memory_end);

        let allocator_data = (largest_entry.base + hhdm_offset) as *mut u8;
        let allocator_data_size = [
            &mut self.alloc_32,
            &mut self.alloc_main
        ].into_iter().fold(0, |off, alloc| {
            let size = address::page_align_up(alloc.data_size());
            alloc.set_data(unsafe { allocator_data.offset(off as isize) });
            off + size
        });

        debug!("Allocator data size: {} kB", allocator_data_size >> 10);
        assert!(allocator_data_size < largest_entry.length as usize,
                "Not enough memory for allocator data in the largest block");

        let kernel_pages = address::page_count_up(kernel_entry.length as usize);
        debug!("Kernel size: {} kB ({} pages)",
            (kernel_pages << address::PAGE_SHIFT) >> 10, kernel_pages);
        assert!(kernel_pages <= 512, "Kernel size exceeded 2048 kB");
    }
}
