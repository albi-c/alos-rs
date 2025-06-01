use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::rwlock::RwLock;
use crate::memory::physical::PhysicalMemorySpace;

#[derive(Debug)]
struct MemorySpaceDataLocked {
    pub phys: PhysicalMemorySpace,
}

#[derive(Debug)]
struct MemorySpaceData {
    data: RwLock<MemorySpaceDataLocked>,
    // TODO: use interrupt lock
    ref_count: AtomicU64,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct MemorySpace(NonNull<MemorySpaceData>);

impl MemorySpace {
}

impl Clone for MemorySpace {
    fn clone(&self) -> MemorySpace {
        unsafe { self.0.as_ref() }.ref_count.fetch_add(1, Ordering::Acquire);
        MemorySpace(self.0)
    }
}

impl Drop for MemorySpace {
    fn drop(&mut self) {
        if unsafe { self.0.as_ref() }.ref_count.fetch_sub(1, Ordering::Release) == 1 {
            // dealloc
        }
    }
}
