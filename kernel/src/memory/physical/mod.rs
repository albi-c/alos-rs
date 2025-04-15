mod allocator;
pub mod buddy_allocator;

use limine::response::{HhdmResponse, MemoryMapResponse};
use crate::memory::physical::allocator::MemoryAllocator;

pub struct MemoryManager<A: MemoryAllocator> {
    allocator: A,
}

impl<A: MemoryAllocator> MemoryManager<A> {
    pub fn init(&mut self, memory_map: &MemoryMapResponse, hhdm: &HhdmResponse) {
        drop(core::mem::replace(&mut self.allocator, A::new()));
    }
}
