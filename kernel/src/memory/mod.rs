mod physical;
pub mod address;
pub mod hhdm;
mod space;
mod virt;
mod slab;

use core::arch::asm;
use core::ops::BitOr;
use limine::request::{ExecutableAddressRequest, HhdmRequest, MemoryMapRequest};
use crate::{core_local, print, println};
use crate::lock::Lock;
use crate::memory::physical::buddy_allocator::BuddyAllocator;
use crate::memory::physical::{MemoryManager, PhysicalMemorySpace};
use crate::memory::physical::map::MapEntry;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryFlags(u64);

impl MemoryFlags {
    pub const WRITE: Self = Self(1 << 1);
    pub const USER: Self = Self(1 << 2);
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    pub const NO_CACHE: Self = Self(1 << 4);
    pub const NO_EXEC: Self = Self(1 << 63);

    pub const EMPTY: Self = Self(0);
    pub const DEFAULT_RO: Self = Self(Self::NO_EXEC.0);
    pub const DEFAULT_RW: Self = Self(Self::NO_EXEC.0 | Self::WRITE.0);
    pub const DEFAULT_EXEC: Self = Self(0);
}

impl BitOr for MemoryFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub struct MemorySpace {
    phys: PhysicalMemorySpace,
    virt_kernel: VirtualMemorySpace,
}

impl MemorySpace {
    pub unsafe fn get() -> &'static mut Self {
        MEMORY_SPACE.get_mut()
    }
    pub fn with<T>(func: impl FnOnce(&mut Self) -> T) -> T {
        func(unsafe { Self::get() })
    }

    pub fn phys_alloc(&self, count: usize) -> Option<usize> {
        assert!(count > 0);
        if count == 1 {
            PMM.write().alloc_page()
        } else {
            PMM.write().alloc_pages(count)
        }
    }
    pub fn phys_dealloc(&self, addr: usize, count: usize) {
        assert!(count > 0);
        if count == 1 {
            PMM.write().dealloc_page(addr)
        } else {
            PMM.write().dealloc_pages(addr, count)
        }
    }

    pub fn virt_alloc(&mut self, count: usize) -> usize {
        self.virt_kernel.allocate(count * address::PAGE_SIZE).expect("out of virtual memory")
    }
    pub fn virt_dealloc(&mut self, addr: usize, count: usize) -> Option<usize> {
        self.virt_kernel.deallocate(addr, count * address::PAGE_SIZE)
    }

    pub fn map(&mut self, phys: usize, virt: usize, count: usize, flags: MemoryFlags) {
        self.map_flag_func(phys, virt, count, |_| flags);
        // assert!(address::is_page_aligned(phys));
        // assert!(address::is_page_aligned(virt));
        // assert!(count > 0);
        // let mut it = self.phys.map.iterate(
        //     phys, || unsafe { hhdm::as_mut_ref(alloc_page().unwrap()) });
        // for i in 0..count {
        //     let me = it.next();
        //     *me = MapEntry::new_with_flags(virt + i * address::PAGE_SIZE, flags.0).present();
        // }
    }
    pub fn map_flag_func(&mut self, phys: usize, virt: usize, count: usize,
                         mut flags: impl FnMut(usize) -> MemoryFlags) {
        assert!(address::is_page_aligned(phys));
        assert!(address::is_page_aligned(virt));
        assert!(count > 0);
        let mut it = self.phys.map.iterate(
            virt, || unsafe { hhdm::as_mut_ref(alloc_page().unwrap()) });
        for i in 0..count {
            let me = it.next();
            *me = MapEntry::new_with_flags(phys + i * address::PAGE_SIZE, flags(i).0).present();
        }

        // unsafe {
        //     asm!(
        //         "mov {0}, cr3",
        //         "mov cr3, {0}",
        //         out(reg) _
        //     );
        // }
    }
    pub fn unmap(&mut self, virt: usize, count: usize) {
        assert!(address::is_page_aligned(virt));
        assert!(count > 0);
        todo!()
    }
}

core_local!(#late_init MEMORY_SPACE: MemorySpace);

pub struct InitValues(PhysicalMemorySpace, VirtualMemorySpace);

pub fn init() -> InitValues {
    let memory_map_response = MEMORY_MAP_REQUEST.get_response().expect("No memory map");
    let hhdm_response = HHDM_REQUEST.get_response().expect("No HHDM");
    let exec_addr = EXEC_ADDR_REQUEST.get_response().expect("No executable address");

    let phys = PMM.write().init(memory_map_response, hhdm_response, exec_addr);
    let virt_kernel = VirtualMemorySpace::new(
        0xffff_f000_0000_0000, 0xfff_8000_0000);

    InitValues(phys, virt_kernel)
}

pub fn init_core_local(InitValues(phys, virt_kernel): InitValues) {
    let space = MemorySpace { phys, virt_kernel };
    unsafe { MEMORY_SPACE.late_init(space) };
}

pub fn alloc_page() -> Option<usize> {
    PMM.write().alloc_page()
}
pub fn alloc_page_zeroed() -> Option<usize> {
    if let Some(addr) = alloc_page() {
        unsafe { core::ptr::write_bytes(hhdm::as_ptr::<u8>(addr), 0, address::PAGE_SIZE) };
        Some(addr)
    } else {
        None
    }
}
pub fn dealloc_page(addr: usize) {
    PMM.write().dealloc_page(addr)
}

pub fn alloc_pages(count: usize) -> Option<usize> {
    PMM.write().alloc_pages(count)
}
pub fn alloc_pages_zeroed(count: usize) -> Option<usize> {
    if let Some(addr) = alloc_pages(count) {
        unsafe { core::ptr::write_bytes(hhdm::as_ptr::<u8>(addr), 0, address::PAGE_SIZE * count) };
        Some(addr)
    } else {
        None
    }
}
pub fn dealloc_pages(addr: usize, count: usize) {
    PMM.write().dealloc_pages(addr, count)
}
