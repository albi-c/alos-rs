use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use macros::slabs;
use crate::lock::Lock;
use crate::memory;
use crate::memory::{address, hhdm};

struct SlabNode {
    next: *mut SlabNode,
}

unsafe impl Sync for SlabNode {}
unsafe impl Send for SlabNode {}

#[derive(Debug)]
struct SlabAllocator<const N: usize> {
    node: Cell<*mut SlabNode>,
}

impl<const N: usize> Default for SlabAllocator<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> SlabAllocator<N> {
    pub const fn new() -> Self {
        const { assert!(N >= size_of::<SlabNode>()) };
        SlabAllocator { node: Cell::new(core::ptr::null_mut()) }
    }

    pub fn allocate(&mut self) -> &'static mut [u8; N] {
        let node = self.node.get();
        if let Some(node) = unsafe { node.as_mut() } {
            self.node.set(node.next);
            unsafe { core::mem::transmute(node) }
        } else {
            let addr = hhdm::as_ptr::<u8>(memory::alloc_page().expect("Out of memory"));
            let data = unsafe { core::slice::from_raw_parts_mut(addr, address::PAGE_SIZE) };
            let (chunks, rest) = data.as_chunks_mut::<N>();
            assert_eq!(rest.len(), 0);
            for chunk in chunks {
                self.deallocate(chunk);
            }
            let node = unsafe { self.node.get().as_mut() }.expect("No memory was allocated");
            self.node.set(node.next);
            unsafe { core::mem::transmute(node) }
        }
    }

    pub fn deallocate(&mut self, data: &'static mut [u8; N]) {
        let node: &'static mut SlabNode = unsafe { core::mem::transmute(data) };
        node.next = self.node.get();
        self.node.set(node as *mut _);
    }
}

// 2^3 (8) ..= 2^11 (2048)
slabs!(SlabContainer: 3..11);

struct Slabs {
    container: Lock<SlabContainer>,
}

#[global_allocator]
static SLABS: Slabs = Slabs::new();

impl Slabs {
    const fn new() -> Self {
        Slabs { container: Lock::new(SlabContainer::new()) }
    }

    fn allocate(&self, size: usize) -> &'static mut [u8] {
        if size > 2048 {
            let addr = memory::alloc_pages(address::page_count_up(size)).expect("Out of memory");
            unsafe { core::slice::from_raw_parts_mut(hhdm::as_ptr(addr), size) }
        } else {
            let bit_len = usize::BITS - (size - 1).leading_zeros();
            self.container.write().allocate(bit_len as usize)
        }
    }

    fn deallocate(&self, size: usize, memory: &'static mut [u8]) {
        if size > 2048 {
            memory::dealloc_pages(hhdm::from_ptr(memory.as_ptr()), address::page_count_up(size));
        } else {
            let bit_len = usize::BITS - (size - 1).leading_zeros();
            self.container.write().deallocate(bit_len as usize, unsafe {
                core::slice::from_raw_parts_mut(memory.as_mut_ptr(), 1 << bit_len) });
        }
    }
}

unsafe impl GlobalAlloc for Slabs {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout.size()).as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        self.deallocate(size, unsafe { core::slice::from_raw_parts_mut(ptr, size) })
    }
}
