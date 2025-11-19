use alloc::collections::BTreeMap;
use core::ops::Bound;
use crate::memory::address::{PageCount, VirtAddr, VirtAddrPageAligned};

#[derive(Debug, Copy, Clone)]
struct MemoryAllocation {
    allocated: bool,
    length: VirtAddrPageAligned,
}

impl MemoryAllocation {
    pub const fn free(length: VirtAddrPageAligned) -> Self {
        Self { allocated: false, length }
    }
    pub const fn allocated(length: VirtAddrPageAligned) -> Self {
        Self { allocated: true, length }
    }
}

#[derive(Debug)]
pub struct VirtualMemorySpace {
    data: BTreeMap<VirtAddrPageAligned, MemoryAllocation>,
}

impl VirtualMemorySpace {
    pub fn new(start: VirtAddr, length: usize) -> Self {
        VirtualMemorySpace {
            data: BTreeMap::from([(
                start.page_align_up(),
                MemoryAllocation::free(VirtAddr::new(length).page_align_down()),
            )]),
        }
    }

    pub fn allocate(&mut self, size: PageCount) -> Option<VirtAddrPageAligned> {
        let size = size.as_virt_addr();
        let (base, rem) = self.data.iter_mut().filter_map(
            |(&base, entry)| if !entry.allocated && entry.length >= size {
                let rem = entry.length - size.page_count();
                entry.allocated = true;
                entry.length = size;
                Some((base, rem))
            } else {
                None
            }
        ).next()?;
        if usize::from(rem) != 0 {
            self.data.insert(base + size.page_count(), MemoryAllocation::free(rem));
        }
        Some(base)
    }

    pub fn allocate_at(&mut self, addr: VirtAddrPageAligned, size: PageCount) -> Option<()> {
        let size = size.as_virt_addr();
        let mut cur = self.data.upper_bound_mut(Bound::Included(&addr));
        let (&base, entry) = cur.prev()?;
        let diff = addr - base.page_count();
        if usize::from(diff) == 0 {
            if entry.length >= size {
                let rem = entry.length - size.page_count();
                entry.length = size;
                entry.allocated = true;
                if usize::from(rem) > 0 {
                    self.data.insert(base + size.page_count(), MemoryAllocation::free(rem));
                }
                Some(())
            } else {
                None
            }
        } else {
            if entry.length - diff.page_count() >= size {
                let rest = entry.length - diff.page_count();
                entry.length = diff;
                let rem = rest - size.page_count();
                self.data.insert(addr, MemoryAllocation::allocated(size));
                if usize::from(rem) > 0 {
                    self.data.insert(addr + size.page_count(), MemoryAllocation::free(rem));
                }
                Some(())
            } else {
                None
            }
        }
    }
    
    pub fn deallocate(&mut self, base: VirtAddrPageAligned, size: PageCount) {
        // TODO: callback for physical memory deallocation
        let end = base + size;
        let mut cur = self.data.lower_bound_mut(Bound::Included(&base));
        while let Some((&base, entry)) = cur.next() {
            if base >= end {
                break;
            }
            todo!()
        }
    }
}
