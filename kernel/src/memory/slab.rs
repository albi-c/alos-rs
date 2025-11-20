use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::cmp::max;
use core::ptr::NonNull;
use macros::slabs;
use crate::lock::Lock;
use crate::memory;
use crate::memory::address;
use crate::memory::address::{PageCount, VirtAddr};

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

    pub fn allocate(&mut self) -> NonNull<u8> {
        if let Some(node) = unsafe { self.node.get().as_mut() } {
            self.node.set(node.next);
            NonNull::from(node).cast()
        } else {
            let addr = memory::alloc_page()
                .expect("SlabAllocator::allocate: out of memory")
                .hhdm_to_virt()
                .as_mut_ptr::<u8>();
            let data = unsafe { core::slice::from_raw_parts_mut(addr, address::PAGE_SIZE) };
            let (chunks, rest) = data.as_chunks_mut::<N>();
            assert_eq!(rest.len(), 0);
            for chunk in chunks {
                self.deallocate(NonNull::from(chunk).cast());
            }
            let node = unsafe { self.node.get().as_mut() }
                .expect("SlabAllocator::allocate: No memory was allocated");
            self.node.set(node.next);
            NonNull::from(node).cast()
        }
    }

    pub fn deallocate(&mut self, data: NonNull<u8>) {
        let node: &'static mut SlabNode = unsafe { data.cast().as_mut() };
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

    fn allocate(&self, size: usize) -> NonNull<u8> {
        if size > 2048 {
            memory::alloc_pages(PageCount::pages_up(size))
                .expect("Slabs::allocate: out of memory")
                .hhdm_to_virt()
                .as_non_null()
                .expect("Slabs::allocate: allocation failed")
        } else {
            let bit_len = max(usize::BITS - (size - 1).leading_zeros(), 3);
            self.container.write().allocate(bit_len as usize)
        }
    }

    fn deallocate(&self, size: usize, memory: NonNull<u8>) {
        if size > 2048 {
            memory::dealloc_pages(VirtAddr::from(memory)
                                      .hhdm_to_phys()
                                      .as_page_aligned()
                                      .expect("Slabs::deallocate: not page aligned"),
                                  PageCount::pages_up(size));
        } else {
            let bit_len = max(usize::BITS - (size - 1).leading_zeros(), 3);
            self.container.write().deallocate(bit_len as usize, memory);
        }
    }
}

unsafe impl GlobalAlloc for Slabs {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert_ne!(layout.size(), 0, "GlobalAlloc::alloc: layout size is 0");
        self.allocate(layout.size()).as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        assert_ne!(size, 0, "GlobalAlloc::dealloc: layout size is 0");
        self.deallocate(size, NonNull::new(ptr).expect("GlobalAlloc::dealloc: null pointer"));
    }
}
