use core::arch::asm;
use crate::memory::hhdm;
use crate::{print, println};
use crate::memory::address::{PageCount, VirtAddrPageAligned};

#[derive(Debug, Copy, Clone)]
pub struct MapEntry(pub u64);

impl MapEntry {
    pub const MASK_ADDR: u64 = 0x000f_ffff_ffff_f000;
    pub const MASK_FLAGS: u64 = !Self::MASK_ADDR;

    pub const SHIFT_AVL: u8 = 9;
    pub const MASK_AVL: u64 = 0xe00;

    pub const FLAG_PRESENT: u64 = 1 << 0;
    pub const FLAG_WRITE: u64 = 1 << 1;
    pub const FLAG_USER: u64 = 1 << 2;
    pub const FLAG_WRITE_THROUGH: u64 = 1 << 3;
    pub const FLAG_NO_CACHE: u64 = 1 << 4;
    pub const FLAG_ACCESSED: u64 = 1 << 5;
    pub const FLAG_DIRTY: u64 = 1 << 6;
    pub const FLAG_LARGE: u64 = 1 << 7;
    pub const FLAG_NO_EXEC: u64 = 1 << 63;

    pub const FLAGS_DEFAULT: u64 = Self::FLAG_PRESENT | Self::FLAG_NO_EXEC;

    #[inline(always)]
    pub const fn zero() -> Self {
        MapEntry(0)
    }

    #[inline(always)]
    pub fn new(addr: usize) -> Self {
        MapEntry((addr as u64 & Self::MASK_ADDR) | Self::FLAGS_DEFAULT)
    }
    #[inline(always)]
    pub fn new_with_flags(addr: usize, flags: u64) -> Self {
        MapEntry((addr as u64 & Self::MASK_ADDR) | (flags & Self::MASK_FLAGS))
    }

    #[inline(always)]
    pub fn from_map(map: &mut MemoryMap) -> Self {
        Self::new(map.addr())
    }
    #[inline(always)]
    pub fn from_map_with_flags(map: &mut MemoryMap, flags: u64) -> Self {
        Self::new_with_flags(map.addr(), flags)
    }

    #[inline(always)]
    pub fn with_flags(self, flags: u64) -> Self {
        MapEntry((self.0 & Self::MASK_ADDR) | (flags & Self::MASK_FLAGS))
    }

    #[inline(always)]
    pub fn addr(self) -> usize {
        (self.0 & Self::MASK_ADDR) as usize
    }
    #[inline(always)]
    pub fn flags(self) -> u64 {
        self.0 & Self::MASK_FLAGS
    }

    #[inline(always)]
    pub fn as_map_ptr(self) -> *mut MemoryMap {
        hhdm::as_ptr(self.addr())
    }

    #[inline(always)]
    pub fn as_map(self) -> Option<&'static mut MemoryMap> {
        match self.addr() {
            0 => None,
            addr => Some(unsafe { hhdm::as_mut_ref(addr) }),
        }
    }

    #[inline(always)]
    pub fn present(self) -> Self {
        MapEntry(self.0 | Self::FLAG_PRESENT)
    }
    #[inline(always)]
    pub fn is_present(self) -> bool {
        self.0 & Self::FLAG_PRESENT != 0
    }

    #[inline(always)]
    pub fn write(self) -> Self {
        MapEntry(self.0 | Self::FLAG_WRITE)
    }
    #[inline(always)]
    pub fn is_write(self) -> bool {
        self.0 & Self::FLAG_WRITE != 0
    }

    #[inline(always)]
    pub fn user(self) -> Self {
        MapEntry(self.0 | Self::FLAG_USER)
    }
    #[inline(always)]
    pub fn is_user(self) -> bool {
        self.0 & Self::FLAG_USER != 0
    }

    #[inline(always)]
    pub fn write_through(self) -> Self {
        MapEntry(self.0 | Self::FLAG_WRITE_THROUGH)
    }
    #[inline(always)]
    pub fn is_write_through(self) -> bool {
        self.0 & Self::FLAG_WRITE_THROUGH != 0
    }

    #[inline(always)]
    pub fn no_cache(self) -> Self {
        MapEntry(self.0 | Self::FLAG_NO_CACHE)
    }
    #[inline(always)]
    pub fn is_no_cache(self) -> bool {
        self.0 & Self::FLAG_NO_CACHE != 0
    }

    #[inline(always)]
    pub fn no_accessed(self) -> Self {
        MapEntry(self.0 & !Self::FLAG_ACCESSED)
    }
    #[inline(always)]
    pub fn is_accessed(self) -> bool {
        self.0 & Self::FLAG_ACCESSED != 0
    }

    #[inline(always)]
    pub fn no_dirty(self) -> Self {
        MapEntry(self.0 & !Self::FLAG_DIRTY)
    }
    #[inline(always)]
    pub fn is_dirty(self) -> bool {
        self.0 & Self::FLAG_DIRTY != 0
    }

    #[inline(always)]
    pub fn large(self) -> Self {
        MapEntry(self.0 | Self::FLAG_LARGE)
    }
    #[inline(always)]
    pub fn is_large(self) -> bool {
        self.0 & Self::FLAG_LARGE != 0
    }

    #[inline(always)]
    pub fn no_exec(self) -> Self {
        MapEntry(self.0 | Self::FLAG_NO_EXEC)
    }
    #[inline(always)]
    pub fn is_no_exec(self) -> bool {
        self.0 & Self::FLAG_NO_EXEC != 0
    }

    #[inline(always)]
    pub fn with_avl(self, avl: u8) -> Self {
        MapEntry(self.0 | (((avl as u64) << Self::SHIFT_AVL) & Self::MASK_AVL))
    }
    #[inline(always)]
    pub fn get_avl(self) -> u8 {
        ((self.0 & Self::MASK_AVL) >> Self::SHIFT_AVL) as u8
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct MemoryMap {
    data: [MapEntry; 512],
}

impl MemoryMap {
    pub const fn new() -> Self {
        MemoryMap {
            data: [MapEntry::zero(); 512],
        }
    }

    pub fn get_current() -> &'static mut Self {
        let addr: usize;
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) addr,
            );
            hhdm::as_mut_ref(addr)
        }
    }

    pub unsafe fn set_current(&mut self) {
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) self.addr(),
            );
        }
    }

    #[inline]
    pub fn get_index(addr: VirtAddrPageAligned, level: usize) -> usize {
        (addr.addr() >> (12 + 9 * (level - 1))) & 0x1ff
    }

    #[inline(always)]
    pub fn get_index_c<const L: usize>(addr: VirtAddrPageAligned) -> usize {
        (addr.addr() >> (12 + 9 * (L - 1))) & 0x1ff
    }

    pub fn get_indices(addr: VirtAddrPageAligned) -> [usize; 4] {
        [
            Self::get_index_c::<4>(addr), Self::get_index_c::<3>(addr),
            Self::get_index_c::<2>(addr), Self::get_index_c::<1>(addr),
        ]
    }

    #[inline(always)]
    pub fn index_to_page_count(index: usize, level: usize) -> PageCount {
        PageCount::new(index << (9 * (level - 1)))
    }

    #[inline(always)]
    pub fn addr(&mut self) -> usize {
        hhdm::from_ref(self)
    }

    #[inline(always)]
    pub fn at(&mut self, index: usize) -> &mut MapEntry {
        &mut self.data[index]
    }

    pub fn at_addr(&mut self, addr: VirtAddrPageAligned) -> Option<&mut MapEntry> {
        let indices = Self::get_indices(addr);
        Some(self
            .at(indices[0]).as_map()?
            .at(indices[1]).as_map()?
            .at(indices[2]).as_map()?
            .at(indices[3]))
    }

    fn iter_present_in_range_start(&mut self, base: VirtAddrPageAligned, start: VirtAddrPageAligned,
                                   end: VirtAddrPageAligned, level: usize)  -> impl Iterator<Item = (&mut MapEntry, VirtAddrPageAligned)> {
        // TODO: fix
        // TODO: optimize skip_while and take_while
        self.data
            .iter_mut()
            .enumerate()
            .map(move |(i, entry)| (i, entry, base + Self::index_to_page_count(i, level)))
            // .inspect(move |(i, _, addr)| println!("{i} {addr:x?} {start:x?} {end:x?}"))
            .skip_while(move |&(_, _, addr)| addr + Self::index_to_page_count(1, level) < start)
            // .inspect(move |(i, _, addr)| println!("ns {i} {addr:x?} {start:x?} {end:x?}"))
            .take_while(move |&(_, _, addr)| addr < end + Self::index_to_page_count(1, level))
            // .inspect(move |(i, _, addr)| println!("tk {i} {addr:x?} {start:x?} {end:x?}"))
            .filter_map(move |(_, entry, addr)| entry
                .is_present()
                .then_some((entry, addr)))
            // .inspect(|(entry, addr)| println!("ipirs {:x?} {:x?}", entry, addr))
    }

    fn iter_present_in_range_level(&mut self, base: VirtAddrPageAligned, start: VirtAddrPageAligned,
                                   end: VirtAddrPageAligned, level: usize, func: &mut impl FnMut(&mut MapEntry, VirtAddrPageAligned)) {
        if level == 1 {
            Self::iter_present_in_range_start(self, base, start, end, level)
                .for_each(move |(entry, addr)| func(entry, addr))
        } else {
            Self::iter_present_in_range_start(self, base, start, end, level)
                .for_each(move |(entry, addr)| entry
                    .as_map().unwrap()
                    .iter_present_in_range_level(addr, start, end, level - 1, func))
        }
    }

    pub fn iter_present_in_range(&mut self, start: VirtAddrPageAligned,
                                 end: VirtAddrPageAligned, mut func: impl FnMut(&mut MapEntry, VirtAddrPageAligned)) {
        // self.iter_present_in_range_level(
        //     VirtAddrPageAligned::new(0).unwrap(), start, end, 4, &mut func)
        for addr in start.page_count()..end.page_count() {
            let addr = addr.as_virt_addr();
            if let Some(entry) = self.at_addr(addr) {
                func(entry, addr)
            }
        }
    }

    pub fn map_or_insert(&mut self, index: usize,
                         alloc: impl FnOnce() -> &'static mut MemoryMap) -> &'static mut MemoryMap {
        let flags = if index < 256 {
            MapEntry::FLAG_PRESENT | MapEntry::FLAG_WRITE | MapEntry::FLAG_USER
        } else {
            MapEntry::FLAG_PRESENT | MapEntry::FLAG_WRITE
        };
        self.map_or_insert_with_flags(index, flags, alloc)
    }

    pub fn map_or_insert_with_flags(&mut self, index: usize, flags: u64,
                                    alloc: impl FnOnce() -> &'static mut MemoryMap) -> &'static mut MemoryMap {
        let entry = self.data[index];
        if let Some(map) = entry.as_map() {
            map
        } else {
            let map = alloc();
            self.data[index] = MapEntry::from_map_with_flags(map, flags);
            map
        }
    }

    pub fn iterate<A: FnMut() -> &'static mut MemoryMap>(&mut self, addr: VirtAddrPageAligned,
                                                         mut alloc: A) -> MemoryMapIterator<'_, 4, A> {
        let indices = Self::get_indices(addr);

        let m1 = self.map_or_insert(indices[0], || alloc());
        let m2 = m1.map_or_insert(indices[1], || alloc());
        let m3 = m2.map_or_insert(indices[2], || alloc());

        MemoryMapIterator {
            indices,
            maps: [self, m1, m2, m3],
            alloc,
            has: true,
        }
    }

    pub fn iterate_3<A: FnMut() -> &'static mut MemoryMap>(&mut self, addr: VirtAddrPageAligned,
                                                            mut alloc: A) -> MemoryMapIterator<'_, 3, A> {
        let [indices @ .., _] = Self::get_indices(addr);

        let m1 = self.map_or_insert(indices[0], || alloc());
        let m2 = m1.map_or_insert(indices[1], || alloc());

        MemoryMapIterator {
            indices,
            maps: [self, m1, m2],
            alloc,
            has: true,
        }
    }

    fn print_ident(n: usize) {
        for _ in 0..n {
            print!("  ");
        }
    }

    fn sign_extend_addr(a: usize) -> usize {
        if a >= 0x8000_0000_0000 {
            a | 0xffff_8000_0000_0000
        } else {
            a
        }
    }

    pub fn dump(&self, level: usize, virt_address: usize, skip: usize) {
        let addr_shift = 12 + 9 * (3 - level);
        let mut prev = None;
        for (i, &entry) in self.data.iter().enumerate().skip(skip) {
            if !entry.is_present() {
                continue;
            }
            let addr = virt_address | (i << addr_shift);
            if level == 3 || entry.is_large() {
                let sx = Self::sign_extend_addr(addr);
                let ad = entry.addr();
                Self::print_ident(level);
                if i != 511 && let Some((psx, pad)) = prev {
                    if sx == (psx + (1 << addr_shift)) && ad == (pad + (1 << addr_shift)) {
                        println!("...");
                    } else {
                        println!("{:x} -> {:x}", sx, ad);
                    }
                } else {
                    println!("{:x} -> {:x}", sx, ad);
                }
                prev = Some((sx, ad));
            } else if let Some(child) = entry.as_map() {
                if child.data.iter().any(|e| e.is_present()) {
                    Self::print_ident(level);
                    println!("{:x} -> ... [{:p}]", Self::sign_extend_addr(addr), child);
                    child.dump(level + 1, addr, 0);
                }
            } else if entry.0 != 0 {
                println!("? {:x} [{:x}]", entry.0, entry.addr());
            }
        }
    }
}

pub struct MemoryMapIterator<'a, const N: usize, A: FnMut() -> &'static mut MemoryMap> {
    indices: [usize; N],
    maps: [&'a mut MemoryMap; N],
    alloc: A,
    has: bool,
}

impl<'a, const N: usize, A: FnMut() -> &'static mut MemoryMap> MemoryMapIterator<'a, N, A> {
    fn increment(&mut self, idx: usize) -> bool {
        self.indices[idx] += 1;
        if self.indices[idx] >= 512 {
            self.indices[idx] = 0;

            if let Some(lower) = idx.checked_sub(1) {
                if !self.increment(lower) {
                    false
                } else {
                    self.maps[idx] = self.maps[lower].map_or_insert(self.indices[lower],
                                                                    || (self.alloc)());
                    true
                }
            } else {
                false
            }
        } else {
            true
        }
    }

    pub fn try_next(&mut self) -> Option<&mut MapEntry> {
        if !self.has {
             None
        } else {
            let idx = self.indices[N - 1];
            let map = self.maps[N - 1] as *mut MemoryMap;
            self.has = self.increment(N - 1);
            Some(unsafe { map.as_mut().unwrap() }.at(idx))
        }
    }

    pub fn next(&mut self) -> &mut MapEntry {
        self.try_next().expect("Memory map L4 index overflow")
    }
}
