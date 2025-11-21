use alloc::collections::BTreeMap;
use core::ops::Bound;
use crate::memory::address::{PageCount, VirtAddr, VirtAddrPageAligned};
use crate::println;

#[derive(Debug, Copy, Clone)]
struct MemoryAllocation {
    allocated: bool,
    length: PageCount,
}

#[derive(Debug, Copy, Clone)]
enum AddrIntersection {
    None,
    All,
    PartialDown(VirtAddrPageAligned),
    PartialUp(VirtAddrPageAligned),
    PartialMid(BaseWithLength),
}

#[derive(Debug, Copy, Clone)]
struct BaseWithLength(VirtAddrPageAligned, PageCount);

impl BaseWithLength {
    pub fn start(self) -> VirtAddrPageAligned {
        self.0
    }

    pub fn length(self) -> PageCount {
        self.1
    }

    pub fn end(self) -> VirtAddrPageAligned {
        self.0 + self.1
    }

    pub fn intersect(self, other: BaseWithLength) -> AddrIntersection {
        if self.start() > other.end() || self.end() <= other.start() {
            AddrIntersection::None
        } else if self.start() >= other.start() && self.end() <= other.end() {
            AddrIntersection::All
        } else if self.start() < other.start() && self.end() > other.end() {
            AddrIntersection::PartialMid(other)
        } else if self.start() < other.start() {
            AddrIntersection::PartialUp(other.start())
        } else if self.end() > other.end() {
            AddrIntersection::PartialDown(other.end())
        } else {
            unreachable!()
        }
    }
}

impl MemoryAllocation {
    pub const fn free(length: PageCount) -> Self {
        Self { allocated: false, length }
    }
    pub const fn allocated(length: PageCount) -> Self {
        Self { allocated: true, length }
    }
}

#[derive(Debug)]
pub struct VirtualMemorySpace {
    data: BTreeMap<VirtAddrPageAligned, MemoryAllocation>,
}

impl VirtualMemorySpace {
    pub fn new(start: VirtAddr, length: PageCount) -> Self {
        VirtualMemorySpace {
            data: BTreeMap::from([(
                start.page_align_up(),
                MemoryAllocation::free(length),
            )]),
        }
    }

    pub fn allocate(&mut self, size: PageCount) -> Option<VirtAddrPageAligned> {
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
        if rem != 0 {
            self.data.insert(base + size, MemoryAllocation::free(rem));
        }
        Some(base)
    }

    pub fn allocate_at(&mut self, addr: VirtAddrPageAligned, size: PageCount) -> Option<()> {
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
    
    pub fn deallocate(&mut self, start: VirtAddrPageAligned, size: PageCount) {
        // TODO: callback for physical memory deallocation
        let base_with_length = BaseWithLength(start, size);
        println!("deallocating {:x?} +{:x?}", start, size);
        let end = base_with_length.end();
        let mut cur = self.data.lower_bound_mut(Bound::Unbounded);
        while let Some((&base, entry)) = cur.next() {
            if !entry.allocated {
                continue;
            }
            let entry_end = base + entry.length;
            if entry_end <= start {
                continue;
            }
            if base >= end {
                break;
            }
            println!("deallocate {:x?} +{:x?} {:x?}", base, entry.length, BaseWithLength(base, entry.length).intersect(base_with_length));
            match BaseWithLength(base, entry.length).intersect(base_with_length) {
                AddrIntersection::None => {},
                AddrIntersection::All => {
                    entry.allocated = false;
                    let entry = *entry;
                    cur.prev().unwrap();
                    if let Some(prev) = cur.peek_prev() {
                        if !prev.1.allocated {
                            prev.1.length = prev.1.length + entry.length;
                            cur.remove_next().unwrap();
                        } else {
                            cur.next().unwrap();
                        }
                    }
                    if let Some(next) = cur.peek_next() {
                        if !next.1.allocated {
                            let length = next.1.length;
                            cur.remove_next().unwrap();
                            let prev = cur.peek_prev().unwrap();
                            prev.1.length = prev.1.length + length;
                        }
                    }
                },
                AddrIntersection::PartialMid(bwl) => {
                    todo!()
                },
                AddrIntersection::PartialUp(up) => {
                    todo!()
                },
                AddrIntersection::PartialDown(down) => {
                    todo!()
                },
            }
        }

        println!("{:#x?}", self.data);
    }
}
