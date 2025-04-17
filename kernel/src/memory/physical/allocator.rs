pub trait MemoryAllocator : Send + Sync {
    fn new() -> Self;

    fn init(&mut self, start: usize, end: usize);
    fn data_size(&self) -> usize;
    fn set_data(&mut self, data: &mut [u8]);
    fn include(&mut self, start: usize, end: usize);
}
