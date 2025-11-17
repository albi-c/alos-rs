use alloc::collections::BTreeMap;
use core::ops::Bound;
use crate::memory::address;
use crate::println;

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
