use alloc::collections::btree_map::CursorMut;
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

fn insert_entry(cursor: &mut CursorMut<VirtAddrPageAligned, MemoryAllocation>,
                mut addr: VirtAddrPageAligned, mut alloc: MemoryAllocation) {
    assert!(alloc.length > 0, "length of an entry must be greater than zero");

    if let Some((&prev_addr, &mut prev_alloc)) = cursor.peek_prev()
        && prev_alloc.allocated == alloc.allocated && prev_addr + prev_alloc.length == addr {
        cursor.remove_prev().unwrap();
        addr = prev_addr;
        alloc.length += prev_alloc.length;
    }
    if let Some((&next_addr, &mut next_alloc)) = cursor.peek_next()
        && next_alloc.allocated == alloc.allocated && addr + alloc.length == next_addr {
        cursor.remove_next().unwrap();
        alloc.length += next_alloc.length;
    }

    cursor.insert_before(addr, alloc).expect("failed to insert entry");
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
        let selection = BaseWithLength(start, size);
        println!("deallocating {:x?} +{:x?}", start, size);
        let end = selection.end();
        let mut cur = self.data.lower_bound_mut(Bound::Unbounded);
        while let Some((&base, &mut alloc)) = cur.next() {
            if !alloc.allocated {
                continue;
            }
            let entry_end = base + alloc.length;
            if entry_end <= start {
                continue;
            }
            if base >= end {
                break;
            }
            let entry = BaseWithLength(base, alloc.length);
            println!("deallocate {:x?} +{:x?} {:x?}", base, alloc.length, entry.intersect(selection));
            match entry.intersect(selection) {
                AddrIntersection::None => {},
                AddrIntersection::All => {
                    cur.remove_prev().unwrap();
                    insert_entry(&mut cur, entry.start(), MemoryAllocation::free(entry.length()));
                },
                AddrIntersection::PartialMid(sel) => {
                    cur.remove_prev().unwrap();
                    insert_entry(&mut cur, entry.start(), MemoryAllocation::allocated(sel.start() - entry.start()));
                    insert_entry(&mut cur, sel.start(), MemoryAllocation::free(sel.length()));
                    insert_entry(&mut cur, sel.end(), MemoryAllocation::allocated(entry.end() - sel.end()));
                },
                AddrIntersection::PartialUp(sel_start) => {
                    cur.remove_prev().unwrap();
                    insert_entry(&mut cur, entry.start(), MemoryAllocation::allocated(sel_start - entry.start()));
                    insert_entry(&mut cur, sel_start, MemoryAllocation::free(entry.end() - sel_start));
                },
                AddrIntersection::PartialDown(sel_end) => {
                    cur.remove_prev().unwrap();
                    insert_entry(&mut cur, entry.start(), MemoryAllocation::free(sel_end - entry.start()));
                    insert_entry(&mut cur, sel_end, MemoryAllocation::allocated(entry.end() - sel_end));
                },
            }
        }

        println!("{:#x?}", self.data);
    }
}
