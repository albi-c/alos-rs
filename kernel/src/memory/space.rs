use alloc::boxed::Box;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::lock::Lock;
use crate::memory::physical::PhysicalMemorySpace;
use crate::memory::virt::VirtualMemorySpace;

#[derive(Debug)]
pub struct MemorySpaceData {
    pub phys: Lock<PhysicalMemorySpace>,
    pub virt_kernel: Lock<VirtualMemorySpace>,
    ref_count: AtomicU64,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct MemorySpace(NonNull<MemorySpaceData>);

impl MemorySpace {
    pub fn new(phys: PhysicalMemorySpace, virt_kernel: VirtualMemorySpace) -> Self {
        MemorySpace(Box::leak(Box::new(MemorySpaceData {
            phys: Lock::new(phys),
            virt_kernel: Lock::new(virt_kernel),
            ref_count: AtomicU64::new(0),
        })).into())
    }
}

impl Deref for MemorySpace {
    type Target = MemorySpaceData;
    
    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
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
