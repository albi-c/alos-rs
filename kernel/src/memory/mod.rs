mod physical;
pub mod address;
pub mod hhdm;
mod space;
mod virt;
mod slab;

use alloc::sync::Arc;
use core::arch::asm;
use core::ops::BitOr;
use limine::request::{ExecutableAddressRequest, HhdmRequest, MemoryMapRequest};
use crate::core_local;
use crate::lock::Lock;
use crate::memory::address::{PageCount, PhysAddrPageAligned, VirtAddr, VirtAddrPageAligned};
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

#[derive(Debug)]
pub struct MemorySpace {
    phys: PhysicalMemorySpace,
    virt_kernel: Arc<Lock<VirtualMemorySpace>>,
    virt_user: Option<Arc<Lock<VirtualMemorySpace>>>,
}

impl MemorySpace {
    pub fn new_same_user(&self) -> Arc<Self> {
        Arc::new(Self {
            phys: self.phys.new_same_user(),
            virt_kernel: self.virt_kernel.clone(),
            virt_user: self.virt_user.clone(),
        })
    }
    pub fn new_new_user(&self) -> Arc<Self> {
        self.new(Some(VirtualMemorySpace::new(VirtAddr::new(0x1000), 0x800000000000 - 0x1000)))
    }
    pub fn new(&self, virt_user: Option<VirtualMemorySpace>) -> Arc<Self> {
        Arc::new(Self {
            phys: self.phys.new(),
            virt_kernel: self.virt_kernel.clone(),
            virt_user: virt_user.map(|virt| Arc::new(Lock::new(virt))),
        })
    }

    pub fn make_current(self: Arc<Self>) {
        let addr = self.phys.map_addr();
        *MEMORY_SPACE.get_mut() = self;
        Self::set_cr3(addr as u64);
    }

    pub fn get() -> &'static Arc<Self> {
        MEMORY_SPACE.get()
    }
    pub fn with<T>(func: impl FnOnce(&Self) -> T) -> T {
        func(&Self::get())
    }

    pub fn phys_alloc(&self, count: PageCount) -> Option<PhysAddrPageAligned> {
        assert!(count > 0);
        if count == 1 {
            PMM.write().alloc_page()
        } else {
            PMM.write().alloc_pages(count)
        }
    }
    pub fn phys_dealloc(&self, addr: PhysAddrPageAligned, count: PageCount) {
        assert!(count > 0);
        if count == 1 {
            PMM.write().dealloc_page(addr)
        } else {
            PMM.write().dealloc_pages(addr, count)
        }
    }

    pub fn user_phys_alloc(&self, count: PageCount) -> Option<PhysAddrPageAligned> {
        let addr = self.phys_alloc(count)?;
        unsafe { hhdm::as_ptr::<u8>(addr.into()).write_bytes(0, count.size()); }
        Some(addr)
    }
    pub fn user_phys_dealloc(&self, addr: PhysAddrPageAligned, count: PageCount) {
        self.phys_dealloc(addr, count);
    }

    pub fn virt_alloc(&self, count: PageCount) -> VirtAddrPageAligned {
        self.virt_kernel.write().allocate(count).expect("out of virtual memory")
    }
    pub fn virt_dealloc(&self, addr: VirtAddrPageAligned, count: PageCount) {
        self.virt_kernel.write().deallocate(addr, count)
    }

    pub fn user_virt_alloc(&self, count: PageCount) -> Option<VirtAddrPageAligned> {
        self.virt_user.as_ref().expect("no user memory space").write().allocate(count)
    }
    pub fn user_virt_alloc_at(&self, addr: VirtAddrPageAligned, count: PageCount) -> Option<()> {
        self.virt_user.as_ref().expect("no user memory space").write().allocate_at(addr, count)
    }
    pub fn user_virt_dealloc(&self, addr: VirtAddrPageAligned, count: PageCount) {
        self.virt_user.as_ref().expect("no user memory space").write().deallocate(addr, count)
    }

    #[inline(always)]
    fn set_cr3(value: u64) {
        unsafe {
            asm!(
                "mov cr3, {0}",
                in(reg) value
            );
        }
    }

    #[inline(always)]
    fn reload_cr3() {
        unsafe {
            asm!(
                "mov {0}, cr3",
                "mov cr3, {0}",
                out(reg) _
            );
        }
    }

    fn check_selected_reload_cr3(&self) {
        if core::ptr::addr_eq(self, MEMORY_SPACE.get()) {
            Self::reload_cr3();
        }
        // IPI when unmapping or changing permissions
    }

    pub fn map(&self, phys: PhysAddrPageAligned, virt: VirtAddrPageAligned,
               count: PageCount, flags: MemoryFlags) {
        self.map_flag_func(phys, virt, count, |_| flags);
    }
    pub fn map_flag_func(&self, phys: PhysAddrPageAligned, virt: VirtAddrPageAligned,
                         count: PageCount, mut flags: impl FnMut(usize) -> MemoryFlags) {
        assert!(count > 0);
        let mut map_lock = self.phys.map();
        let mut it = map_lock.iterate(
            virt, || unsafe { hhdm::as_mut_ref(alloc_page().unwrap().into()) });
        for i in 0.into()..count {
            let me = it.next();
            *me = MapEntry::new_with_flags(usize::from(phys + i), flags(usize::from(i)).0).present();
        }
    }
    pub fn unmap(&mut self, virt: usize, count: usize) {
        assert!(address::is_page_aligned(virt));
        assert!(count > 0);
        todo!()
    }
}

core_local!(#late_init MEMORY_SPACE: Arc<MemorySpace>);

pub struct InitValues(PhysicalMemorySpace, VirtualMemorySpace);

pub fn init() -> InitValues {
    let memory_map_response = MEMORY_MAP_REQUEST.get_response().expect("No memory map");
    let hhdm_response = HHDM_REQUEST.get_response().expect("No HHDM");
    let exec_addr = EXEC_ADDR_REQUEST.get_response().expect("No executable address");

    let phys = PMM.write().init(memory_map_response, hhdm_response, exec_addr);
    let virt_kernel = VirtualMemorySpace::new(
        VirtAddr::new(0xffff_f000_0000_0000), 0xfff_8000_0000);

    InitValues(phys, virt_kernel)
}

pub fn init_core_local(InitValues(phys, virt_kernel): InitValues) {
    let space = MemorySpace {
        phys,
        virt_kernel: Arc::new(Lock::new(virt_kernel)),
        virt_user: None,
    };
    unsafe { MEMORY_SPACE.late_init(Arc::new(space)) };
}

pub fn alloc_page() -> Option<PhysAddrPageAligned> {
    PMM.write().alloc_page()
}
pub fn alloc_page_zeroed() -> Option<PhysAddrPageAligned> {
    if let Some(addr) = alloc_page() {
        unsafe { core::ptr::write_bytes(hhdm::as_ptr::<u8>(addr.into()), 0, address::PAGE_SIZE) };
        Some(addr)
    } else {
        None
    }
}
pub fn dealloc_page(addr: PhysAddrPageAligned) {
    PMM.write().dealloc_page(addr)
}

pub fn alloc_pages(count: PageCount) -> Option<PhysAddrPageAligned> {
    PMM.write().alloc_pages(count)
}
pub fn alloc_pages_zeroed(count: PageCount) -> Option<PhysAddrPageAligned> {
    if let Some(addr) = alloc_pages(count) {
        unsafe { core::ptr::write_bytes(hhdm::as_ptr::<u8>(addr.into()), 0, count.size()) };
        Some(addr)
    } else {
        None
    }
}
pub fn dealloc_pages(addr: PhysAddrPageAligned, count: PageCount) {
    PMM.write().dealloc_pages(addr, count)
}
