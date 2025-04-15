use crate::memory::physical::allocator::MemoryAllocator;

pub struct BuddyAllocator {}

impl MemoryAllocator for BuddyAllocator {
    fn new() -> Self {
        BuddyAllocator {}
    }
}
