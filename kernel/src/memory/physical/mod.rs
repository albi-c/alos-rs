mod allocator;
pub mod buddy_allocator;
mod map;

use core::arch::asm;
use core::cell::Cell;
use core::cmp::{max, min};
use limine::memory_map::{Entry, EntryType};
use limine::response::{ExecutableAddressResponse, HhdmResponse, MemoryMapResponse};
use crate::{debug, logger};
use crate::memory::{address, hhdm};
use crate::memory::physical::allocator::MemoryAllocator;
use crate::memory::physical::map::MemoryMap;

logger!("PMM");

struct EarlyAllocator {
    memory: Cell<*mut u8>,
    start_length: usize,
    length: Cell<usize>,
}

impl EarlyAllocator {
    pub fn new(memory: *mut u8, length: usize) -> Self {
        EarlyAllocator {
            memory: Cell::new(memory),
            start_length: length,
            length: Cell::new(length),
        }
    }

    pub fn used(&self) -> usize {
        self.start_length - self.length.get()
    }

    pub fn allocate_bytes(&self, size: usize) -> Option<&'static mut [u8]> {
        let size = address::page_align_up(size);
        if size > self.length.get() {
            None
        } else {
            self.length.set(self.length.get() - size);
            let slice = unsafe { core::slice::from_raw_parts_mut(self.memory.get(), size) };
            self.memory.set(unsafe { self.memory.get().offset(size as isize) });
            Some(slice)
        }
    }

    pub fn allocate<T>(&self, data: T) -> Option<&'static mut T> {
        self.allocate_bytes(size_of::<T>()).map(|mem| {
            let mem = unsafe { (mem.as_mut_ptr() as *mut T).as_mut() }.unwrap();
            drop(core::mem::replace(mem, data));
            mem
        })
    }

    pub fn allocate_zeroed<T>(&self) -> Option<&'static mut T> {
        self.allocate_bytes(size_of::<T>()).map(|mem| {
            let mem = unsafe { (mem.as_mut_ptr() as *mut T).as_mut() }.unwrap();
            drop(core::mem::replace(mem, unsafe { core::mem::zeroed() }));
            mem
        })
    }

    pub fn allocate_map(&self) -> &'static mut MemoryMap {
        self.allocate_zeroed().expect("Not enough memory for memory map")
    }
}

fn get_current_map() -> &'static mut MemoryMap {
    let addr: usize;
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) addr,
        );
        hhdm::as_mut_ref(addr)
    }
}

fn map_kernel(map: &mut MemoryMap, alloc: &EarlyAllocator, source_addr: usize, page_count: usize) {
    let mut iterator = map.iterate(source_addr, || alloc.allocate_map());

    let curr_map = get_current_map();
    let mut curr_iterator = curr_map.iterate(source_addr, || alloc.allocate_map());

    for _ in 0..page_count {
        *iterator.next() = *curr_iterator.next();
    }
}

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
        unsafe { hhdm::set_offset(hhdm.offset() as usize) };

        Self::init_allocator(&mut self.alloc_32);
        Self::init_allocator(&mut self.alloc_main);

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

        let early_alloc = EarlyAllocator::new(
            hhdm::as_ptr(largest_entry.base as usize), largest_entry.length as usize);

        for alloc in [
            &mut self.alloc_32,
            &mut self.alloc_main
        ] {
            alloc.set_data(early_alloc.allocate_bytes(alloc.data_size())
                .expect("Not enough memory for allocator data"));
        }

        debug!("Allocator data: {} kB", early_alloc.used() >> 10);

        let kernel_pages = address::page_count_up(kernel_entry.length as usize);
        debug!("Kernel size: {} kB ({} pages)",
            (kernel_pages << address::PAGE_SHIFT) >> 10, kernel_pages);
        assert!(kernel_pages <= 512, "Kernel size exceeded 2048 kB (512 pages)");

        let map = early_alloc.allocate_map();

        map_kernel(map, &early_alloc, exec_addr.virtual_base() as usize, kernel_pages);

        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) hhdm::sub(map.addr()),
            );
        }
    }
}
