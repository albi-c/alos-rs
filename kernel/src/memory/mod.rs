mod physical;
pub mod address;
pub mod hhdm;
mod space;

use limine::request::{ExecutableAddressRequest, HhdmRequest, MemoryMapRequest};
use crate::lock::Lock;
use crate::memory::physical::buddy_allocator::BuddyAllocator;
use crate::memory::physical::MemoryManager;

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

    let phys_space = PMM.write().init(memory_map_response, hhdm_response, exec_addr);
}
