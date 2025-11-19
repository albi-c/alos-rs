use crate::memory::address::{PageCount, PhysAddrPageAligned};

pub trait MemoryAllocator : Send + Sync {
    fn new() -> Self;

    fn init(&mut self, start: usize, end: usize);
    fn data_size(&self) -> usize;
    fn set_data(&mut self, data: &mut [u8]);
    fn include(&mut self, start: usize, end: usize);
    
    fn alloc_page(&mut self) -> Option<PhysAddrPageAligned>;
    fn dealloc_page(&mut self, addr: PhysAddrPageAligned) -> bool;
    
    fn alloc_pages(&mut self, count: PageCount) -> Option<PhysAddrPageAligned>;
    fn dealloc_pages(&mut self, addr: PhysAddrPageAligned, count: PageCount) -> bool;
}
