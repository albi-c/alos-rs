pub trait MemoryAllocator : Send + Sync {
    fn new() -> Self;

    fn init(&mut self, start: usize, end: usize);
    fn data_size(&self) -> usize;
    fn set_data(&mut self, data: &mut [u8]);
    fn include(&mut self, start: usize, end: usize);
    
    fn alloc_page(&mut self) -> Option<usize>;
    fn dealloc_page(&mut self, addr: usize) -> bool;
    
    fn alloc_pages(&mut self, count: usize) -> Option<usize>;
    fn dealloc_pages(&mut self, addr: usize, count: usize) -> bool;
}
