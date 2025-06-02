use alloc::collections::BTreeMap;
use crate::memory::address;

#[derive(Debug)]
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
}
