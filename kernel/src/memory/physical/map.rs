#[derive(Debug, Copy, Clone)]
pub struct MapEntry(pub u64);

impl MapEntry {
    pub const MASK_ADDR: u64 = 0x000f_ffff_ffff_f000;
    pub const MASK_FLAGS: u64 = !Self::MASK_ADDR;

    pub const FLAG_PRESENT: u64 = 1 << 0;
    pub const FLAG_WRITE: u64 = 1 << 1;
    pub const FLAG_USER: u64 = 1 << 2;
    pub const FLAG_WRITE_THROUGH: u64 = 1 << 3;
    pub const FLAG_NO_CACHE: u64 = 1 << 4;
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
        MapEntry(self.0 | (flags & Self::MASK_FLAGS))
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
        self.addr() as *mut MemoryMap
    }

    #[inline(always)]
    pub fn as_map(self) -> Option<&'static mut MemoryMap> {
        unsafe { self.as_map_ptr().as_mut() }
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

    #[inline]
    pub fn get_index(addr: usize, level: u8) -> usize {
        (addr >> (12 + 9 * (level - 1))) & 0x1ff
    }

    #[inline(always)]
    pub fn get_index_c<const L: u8>(addr: usize) -> usize {
        (addr >> (12 + 9 * (L - 1))) & 0x1ff
    }

    pub fn get_indices(addr: usize) -> [usize; 4] {
        [
            Self::get_index_c::<4>(addr), Self::get_index_c::<3>(addr),
            Self::get_index_c::<2>(addr), Self::get_index_c::<1>(addr),
        ]
    }

    #[inline(always)]
    pub fn addr(&mut self) -> usize {
        self as *mut MemoryMap as usize
    }

    #[inline(always)]
    pub fn at(&mut self, index: usize) -> &mut MapEntry {
        &mut self.data[index]
    }

    pub fn map_or_insert(&mut self, index: usize,
                         alloc: impl FnOnce() -> &'static mut MemoryMap) -> &'static mut MemoryMap {
        self.map_or_insert_with_flags(index, MapEntry::FLAGS_DEFAULT, alloc)
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

    pub fn iterate<A: FnMut() -> &'static mut MemoryMap>(&mut self, addr: usize,
                                                         mut alloc: A) -> MemoryMapIterator<A> {
        let indices = Self::get_indices(addr);

        let m1 = self.map_or_insert(indices[0], || alloc());
        let m2 = m1.map_or_insert(indices[1], || alloc());
        let m3 = m2.map_or_insert(indices[2], || alloc());

        MemoryMapIterator {
            indices,
            maps: [self, m1, m2, m3],
            alloc,
        }
    }
}

pub struct MemoryMapIterator<'a, A: FnMut() -> &'static mut MemoryMap> {
    indices: [usize; 4],
    maps: [&'a mut MemoryMap; 4],
    alloc: A,
    next_item: None,
}

impl<'a, A: FnMut() -> &'static mut MemoryMap> MemoryMapIterator<'a, A> {
    fn increment(&mut self) -> bool {
        self.indices[3] += 1;
        if self.indices[3] >= 512 {
            self.indices[3] = 0;

            self.indices[2] += 1;
            if self.indices[2] >= 512 {
                self.indices[2] = 0;

                self.indices[1] += 1;
                if self.indices[1] >= 512 {
                    self.indices[1] = 0;

                    self.indices[0] += 1;
                    if self.indices[0] >= 512 {
                        return false;
                    }

                    self.maps[1] = self.maps[0].map_or_insert(self.indices[0],
                                                              || (self.alloc)());
                }

                self.maps[2] = self.maps[1].map_or_insert(self.indices[1],
                                                          || (self.alloc)());
            }

            self.maps[3] = self.maps[2].map_or_insert(self.indices[2],
                                                      || (self.alloc)());
        }

        true
    }

    fn try_next(&mut self) -> Option<&mut MapEntry> {
        todo!("First fetch entry, then increment")
        if self.increment() {
            Some(self.maps[3].at(self.indices[3]))
        } else {
            None
        }
    }

    pub fn next(&mut self) -> &mut MapEntry {
        self.try_next().expect("Memory map L4 index overflow")
    }
}
