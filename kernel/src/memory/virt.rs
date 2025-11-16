use alloc::collections::BTreeMap;
use core::ops::Bound;
use crate::memory::address;

#[derive(Debug, Copy, Clone)]
enum MemoryAllocation {
    Free {
        length: usize,
    },
    Allocated {
        length: usize,
    },
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
                MemoryAllocation::Free {
                    length: address::page_align_down(length),
                }
            )]),
        }
    }

    pub fn allocate(&mut self, size: usize) -> Option<usize> {
        // TODO: broken? - maybe returns wrong address
        assert!(address::is_page_aligned(size));
        let (base, length) = self.data.iter_mut().filter_map(|(&base, entry)| match entry {
            MemoryAllocation::Free { length } if *length >= size => {
                *length -= size;
                Some((base, *length))
            },
            _ => None,
        }).next()?;
        if length == 0 {
            self.data.remove(&base);
        }
        self.data.insert(base + length, MemoryAllocation::Allocated { length: size });
        Some(base)
    }
    
    pub fn deallocate(&mut self, base: usize, size: usize) -> Option<usize> {
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
        None
    }
}
