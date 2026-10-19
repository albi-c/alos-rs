use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::cmp::max;
use core::ptr::NonNull;
use bytemuck::Zeroable;
use macros::slabs;
use crate::lock::Lock;
use crate::memory;
use crate::memory::address;
use crate::memory::address::{PageCount, PhysAddr, VirtAddr};

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
        const {
            assert!(N >= size_of::<SlabNode>());
            assert!(N <= address::PAGE_SIZE);
            assert!(address::PAGE_SIZE % N == 0);
        }
        SlabAllocator { node: Cell::new(core::ptr::null_mut()) }
    }

    pub fn allocate(&mut self) -> PhysAddr {
        let ptr = if let Some(node) = unsafe { self.node.get().as_mut() } {
            self.node.set(node.next);
            NonNull::from(node)
        } else {
            let addr = memory::alloc_page()
                .expect("SlabAllocator::allocate: out of memory")
                .hhdm_to_virt()
                .as_mut_ptr::<u8>();
            let data = unsafe { core::slice::from_raw_parts_mut(addr, address::PAGE_SIZE) };
            let (chunks, rest) = data.as_chunks_mut::<N>();
            assert_eq!(rest.len(), 0);
            assert!(chunks.len() > 0);
            for chunk in chunks {
                self.deallocate(VirtAddr::new(NonNull::from(chunk).as_ptr() as usize).hhdm_to_phys());
            }
            let node = unsafe { self.node.get().as_mut() }
                .expect("SlabAllocator::allocate: deallocate did not put chunk into linked list");
            self.node.set(node.next);
            NonNull::from(node)
        };
        VirtAddr::new(ptr.as_ptr() as usize).hhdm_to_phys()
    }

    pub fn deallocate(&mut self, data: PhysAddr) {
        let mut data = data.hhdm_to_virt().as_non_null::<SlabNode>()
            .expect("SlabAllocator::deallocate: tried to deallocate null");
        let node: &'static mut SlabNode = unsafe { data.as_mut() };
        node.next = self.node.get();
        self.node.set(node as *mut _);
    }
}

// 2^3 (8) ..= 2^11 (2048)
slabs!(SlabContainer: 3..11);

pub struct Slabs {
    container: Lock<SlabContainer>,
}

#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(not(test), global_allocator)]
pub static SLABS: Slabs = Slabs::new();

impl Slabs {
    const fn new() -> Self {
        Slabs { container: Lock::new(SlabContainer::new()) }
    }

    pub fn allocate_physical(&self, size: usize) -> PhysAddr {
        if size > 2048 {
            memory::alloc_pages(PageCount::pages_up(size))
                .expect("Slabs::allocate: out of memory")
                .into()
        } else {
            let bit_len = max(usize::BITS - (size - 1).leading_zeros(), 3);
            self.container.write().allocate(bit_len as usize)
        }
    }

    pub fn deallocate_physical(&self, size: usize, addr: PhysAddr) {
        if size > 2048 {
            memory::dealloc_pages(addr.as_page_aligned()
                                      .expect("Slabs::deallocate_physical: not page aligned"),
                                  PageCount::pages_up(size));
        } else {
            let bit_len = max(usize::BITS - (size - 1).leading_zeros(), 3);
            self.container.write().deallocate(bit_len as usize, addr);
        }
    }

    pub fn deallocate_physical_one<T: Zeroable>(&self, addr: PhysAddr) {
        self.deallocate_physical(size_of::<T>(), addr)
    }

    pub fn deallocate_physical_slice<T: Zeroable>(&self, count: usize, addr: PhysAddr) {
        self.deallocate_physical(size_of::<T>() * count, addr)
    }

    pub fn allocate(&self, size: usize) -> (NonNull<u8>, PhysAddr) {
        let phys = self.allocate_physical(size);
        let ptr = phys.hhdm_to_virt().as_non_null()
            .expect("Slabs::allocate: allocation returned null");
        (ptr, phys)
    }

    pub fn allocate_one<T: Zeroable>(&self) -> (&mut T, PhysAddr) {
        let (ptr, phys) = self.allocate(size_of::<T>());
        let mut ptr = ptr.cast::<T>();
        unsafe { ptr.write(T::zeroed()) };
        (unsafe { ptr.as_mut() }, phys)
    }

    pub fn allocate_slice<T: Zeroable>(&self, count: usize) -> (&mut [T], PhysAddr) {
        let (ptr, phys) = self.allocate(size_of::<T>() * count);
        let ptr = ptr.cast::<T>();
        unsafe { ptr.write_bytes(0, count) };
        (unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), count) }, phys)
    }

    pub fn deallocate(&self, size: usize, memory: NonNull<u8>) {
        self.deallocate_physical(size, VirtAddr::new(memory.as_ptr() as usize).hhdm_to_phys())
    }

    pub fn deallocate_one<T: Zeroable>(&self, mem: &mut T) {
        self.deallocate(size_of::<T>(), NonNull::from(mem).cast())
    }

    pub fn deallocate_slice<T: Zeroable>(&self, count: usize, mem: &mut [T]) {
        self.deallocate(size_of::<T>() * count, NonNull::from(mem).cast())
    }
}

unsafe impl GlobalAlloc for Slabs {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert_ne!(layout.size(), 0, "GlobalAlloc::alloc: layout size is 0");
        self.allocate(layout.size()).0.as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        assert_ne!(size, 0, "GlobalAlloc::dealloc: layout size is 0");
        self.deallocate(size, NonNull::new(ptr).expect("GlobalAlloc::dealloc: null pointer"));
    }
}
