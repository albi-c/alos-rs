mod physical;

use limine::request::{HhdmRequest, MemoryMapRequest};
use spin::RwLock;
use crate::memory::physical::buddy_allocator::BuddyAllocator;
use crate::memory::physical::MemoryManager;

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

static PMM: RwLock<MemoryManager<BuddyAllocator>> = RwLock::new(unsafe { core::mem::zeroed() });

pub fn init() {
    let memory_map_response = MEMORY_MAP_REQUEST.get_response().expect("No memory map");
    let hhdm_response = HHDM_REQUEST.get_response().expect("No HHDM");

    PMM.write().init(&memory_map_response, &hhdm_response);
}
