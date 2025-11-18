use alloc::collections::BTreeMap;
use core::ops::Bound;
use crate::memory::address;

#[derive(Debug, Copy, Clone)]
struct MemoryAllocation {
    allocated: bool,
    length: usize,
}

impl MemoryAllocation {
    pub const fn free(length: usize) -> Self {
        Self { allocated: false, length }
    }
    pub const fn allocated(length: usize) -> Self {
        Self { allocated: true, length }
    }
}

#[derive(Debug)]
pub struct VirtualMemorySpace {
    data: BTreeMap<usize, MemoryAllocation>,
}

impl VirtualMemorySpace {
    pub fn new(start: usize, length: usize) -> Self {
        VirtualMemorySpace {
            data: BTreeMap::from([(
                address::page_align_up(start),
                MemoryAllocation::free(address::page_align_down(length)),
            )]),
        }
    }

    pub fn allocate(&mut self, size: usize) -> Option<usize> {
        assert!(address::is_page_aligned(size));
        let (base, rem) = self.data.iter_mut().filter_map(
            |(&base, entry)| if !entry.allocated && entry.length >= size {
                let rem = entry.length - size;
                entry.allocated = true;
                entry.length = size;
                Some((base, rem))
            } else {
                None
            }
        ).next()?;
        if rem == 0 {}
        self.data.insert(base + size, MemoryAllocation::free(rem));
        Some(base)
    }

    pub fn allocate_at(&mut self, addr: usize, size: usize) -> Option<()> {
        assert!(address::is_page_aligned(addr));
        assert!(address::is_page_aligned(size));
        let mut cur = self.data.upper_bound_mut(Bound::Included(&addr));
        let (&base, entry) = cur.prev()?;
        let diff = addr - base;
        if diff == 0 {
            if entry.length >= size {
                let rem = entry.length - size;
                entry.length = size;
                entry.allocated = true;
                if rem > 0 {
                    self.data.insert(base + size, MemoryAllocation::free(rem));
                }
                Some(())
            } else {
                None
            }
        } else {
            if entry.length - diff >= size {
                let rest = entry.length - diff;
                entry.length = diff;
                let rem = rest - size;
                self.data.insert(addr, MemoryAllocation::allocated(size));
                if rem > 0 {
                    self.data.insert(addr + size, MemoryAllocation::free(rem));
                }
                Some(())
            } else {
                None
            }
        }
    }
    
    pub fn deallocate(&mut self, base: usize, size: usize) {
        // TODO: callback for physical memory deallocation
        assert!(address::is_page_aligned(base));
        assert!(address::is_page_aligned(size));
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
