mod physical;
pub mod address;
pub mod hhdm;
mod space;
mod virt;
mod slab;

use limine::request::{ExecutableAddressRequest, HhdmRequest, MemoryMapRequest};
use crate::lock::Lock;
use crate::memory::physical::buddy_allocator::BuddyAllocator;
use crate::memory::physical::MemoryManager;
use crate::memory::virt::VirtualMemorySpace;

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static EXEC_ADDR_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

static PMM: Lock<MemoryManager<BuddyAllocator<9>>> = Lock::new(MemoryManager::default(
    BuddyAllocator::default(),
    BuddyAllocator::default(),
));

pub fn init() {
    let memory_map_response = MEMORY_MAP_REQUEST.get_response().expect("No memory map");
    let hhdm_response = HHDM_REQUEST.get_response().expect("No HHDM");
    let exec_addr = EXEC_ADDR_REQUEST.get_response().expect("No executable address");

    let phys = PMM.write().init(memory_map_response, hhdm_response, exec_addr);
    let virt_kernel = VirtualMemorySpace::new(
        0xffff_f000_0000_0000, 0xfff_8000_0000);
}

pub fn alloc_page() -> Option<usize> {
    PMM.write().alloc_page()
}
pub fn dealloc_page(addr: usize) {
    PMM.write().dealloc_page(addr)
}
pub fn alloc_pages(count: usize) -> Option<usize> {
    PMM.write().alloc_pages(count)
}
pub fn dealloc_pages(addr: usize, count: usize) {
    PMM.write().dealloc_pages(addr, count)
}
