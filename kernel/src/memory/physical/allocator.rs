pub trait MemoryAllocator : Send + Sync {
    fn new() -> Self;

    fn init(&mut self, start: u64, end: u64);
    fn data_size(&self) -> u64;
    fn set_data(&mut self, data: *mut u8);
}
