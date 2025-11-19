use core::marker::PhantomData;
use core::ops::{Add, Mul, Shl, Shr};
use core::ptr::NonNull;
use crate::memory::hhdm;

pub const PAGE_SHIFT: u8 = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

pub const LARGE_PAGE_SHIFT: u8 = 21;
pub const LARGE_PAGE_SIZE: usize = 1 << LARGE_PAGE_SHIFT;
pub const LARGE_PAGE_MASK: usize = LARGE_PAGE_SIZE - 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PageCount(pub usize);

impl PageCount {
    #[inline(always)]
    pub fn new(count: usize) -> Self {
        PageCount(count)
    }

    #[inline(always)]
    pub fn as_phys_addr(self) -> PhysAddrPageAligned {
        PhysAddrPageAligned(self.0 << PAGE_SHIFT)
    }

    #[inline(always)]
    pub fn size(self) -> usize {
        self.0 << PAGE_SHIFT
    }

    #[inline(always)]
    pub fn pages_up(addr: usize) -> PageCount {
        PageCount(page_count_up(addr))
    }
    #[inline(always)]
    pub fn pages_down(addr: usize) -> PageCount {
        PageCount(page_count_down(addr))
    }
}

impl Add<PageCount> for PageCount {
    type Output = PageCount;
    fn add(self, rhs: PageCount) -> Self::Output {
        PageCount(self.0 + rhs.0)
    }
}
impl Add<usize> for PageCount {
    type Output = PageCount;
    fn add(self, rhs: usize) -> Self::Output {
        PageCount(self.0 + rhs)
    }
}
impl Mul<usize> for PageCount {
    type Output = PageCount;
    fn mul(self, rhs: usize) -> Self::Output {
        PageCount(self.0 * rhs)
    }
}
impl Shl<usize> for PageCount {
    type Output = PageCount;
    fn shl(self, rhs: usize) -> Self::Output {
        PageCount(self.0 << rhs)
    }
}
impl Shr<usize> for PageCount {
    type Output = PageCount;
    fn shr(self, rhs: usize) -> Self::Output {
        PageCount(self.0 >> rhs)
    }
}
impl From<usize> for PageCount {
    fn from(count: usize) -> Self {
        Self(count)
    }
}
impl From<PageCount> for usize {
    fn from(count: PageCount) -> Self {
        count.0
    }
}
impl PartialEq<usize> for PageCount {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}
impl PartialOrd<usize> for PageCount {
    fn partial_cmp(&self, other: &usize) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(pub usize);

impl PhysAddr {
    #[inline(always)]
    pub fn new(addr: usize) -> Self {
        PhysAddr(addr)
    }

    #[inline(always)]
    pub fn is_page_aligned(self) -> bool {
        is_page_aligned(self.0)
    }
    #[inline(always)]
    pub fn as_page_aligned(self) -> Option<PhysAddrPageAligned> {
        PhysAddrPageAligned::new(self.0)
    }
    #[inline(always)]
    pub fn page_align_up(self) -> PhysAddrPageAligned {
        PhysAddrPageAligned(page_align_up(self.0))
    }
    #[inline(always)]
    pub fn page_align_down(self) -> PhysAddrPageAligned {
        PhysAddrPageAligned(page_align_down(self.0))
    }
    #[inline(always)]
    pub fn page_count_up(self) -> PageCount {
        PageCount(page_count_up(self.0))
    }
    #[inline(always)]
    pub fn page_count_down(self) -> PageCount {
        PageCount(page_count_down(self.0))
    }

    pub fn hhdm_to_virt(self) -> VirtAddrW {
        VirtAddr::new(hhdm::add(self.0))
    }
}

impl Add<usize> for PhysAddr {
    type Output = PhysAddr;
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}
impl Add<PageCount> for PhysAddr {
    type Output = PhysAddr;
    fn add(self, rhs: PageCount) -> Self::Output {
        Self(self.0 + (rhs.0 << PAGE_SHIFT))
    }
}
impl From<usize> for PhysAddr {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<PhysAddr> for usize {
    fn from(value: PhysAddr) -> Self {
        value.0
    }
}
impl From<PhysAddrPageAligned> for PhysAddr {
    fn from(value: PhysAddrPageAligned) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddrPageAligned(usize);

impl PhysAddrPageAligned {
    #[inline(always)]
    pub fn new(addr: usize) -> Option<Self> {
        if is_page_aligned(addr) {
            Some(Self(addr))
        } else {
            None
        }
    }
    #[inline(always)]
    pub unsafe fn new_unchecked(addr: usize) -> Self {
        Self(addr)
    }

    #[inline(always)]
    pub fn page_count(self) -> PageCount {
        PageCount(self.0 >> PAGE_SHIFT)
    }

    pub fn hhdm_to_virt(self) -> VirtAddrPageAligned<perms::PWrite> {
        unsafe { VirtAddrPageAligned::new_unchecked(hhdm::add(self.0)) }
    }
}

impl Add<PageCount> for PhysAddrPageAligned {
    type Output = PhysAddrPageAligned;
    fn add(self, rhs: PageCount) -> Self::Output {
        Self(self.0 + (rhs.0 << PAGE_SHIFT))
    }
}
impl From<PhysAddrPageAligned> for usize {
    fn from(value: PhysAddrPageAligned) -> Self {
        value.0
    }
}
impl TryFrom<PhysAddr> for PhysAddrPageAligned {
    type Error = ();
    fn try_from(value: PhysAddr) -> Result<Self, Self::Error> {
        value.as_page_aligned().ok_or(())
    }
}
impl TryFrom<usize> for PhysAddrPageAligned {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(())
    }
}

mod perms {
    pub trait Base {}

    pub trait Read : Base {}
    pub trait Write : Read {}
    pub trait Execute : Read {}

    pub struct PNone {}
    impl Base for PNone {}

    pub struct PRead {}
    impl Base for PRead {}
    impl Read for PRead {}

    pub struct PWrite {}
    impl Base for PWrite {}
    impl Read for PWrite {}
    impl Write for PWrite {}

    pub struct PExecute {}
    impl Base for PExecute {}
    impl Read for PExecute {}
    impl Execute for PExecute {}

    pub struct PWriteExecute {}
    impl Base for PWriteExecute {}
    impl Read for PWriteExecute {}
    impl Write for PWriteExecute {}
    impl Execute for PWriteExecute {}
}

pub type VirtAddrN = VirtAddr<perms::PNone>;
pub type VirtAddrR = VirtAddr<perms::PRead>;
pub type VirtAddrW = VirtAddr<perms::PWrite>;
pub type VirtAddrX = VirtAddr<perms::PExecute>;
pub type VirtAddrWX = VirtAddr<perms::PWriteExecute>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr<P: perms::Base>(pub usize, PhantomData<P>);

impl<P: perms::Base> VirtAddr<P> {
    pub fn new(addr: usize) -> Self {
        Self(addr, PhantomData)
    }

    pub fn hhdm_to_phys(self) -> PhysAddr {
        PhysAddr(hhdm::sub(self.0))
    }
}

impl<P: perms::Read> VirtAddr<P> {
    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    pub unsafe fn as_ref<T>(&self) -> &T {
        unsafe { &*self.as_ptr() }
    }
}
impl<P: perms::Write> VirtAddr<P> {
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    pub unsafe fn as_mut<T>(&self) -> &mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }
}

impl<P: perms::Base, T> From<*const T> for VirtAddr<P> {
    fn from(ptr: *const T) -> Self {
        Self::new(ptr as usize)
    }
}
impl<P: perms::Base, T> From<*mut T> for VirtAddr<P> {
    fn from(ptr: *mut T) -> Self {
        Self::new(ptr as usize)
    }
}
impl<P: perms::Base, T> From<NonNull<T>> for VirtAddr<P> {
    fn from(ptr: NonNull<T>) -> Self {
        Self::new(ptr.as_ptr() as usize)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddrPageAligned<P: perms::Base>(pub usize, PhantomData<P>);

impl<P: perms::Base> VirtAddrPageAligned<P> {
    pub fn new(addr: usize) -> Option<Self> {
        if is_page_aligned(addr) {
            Some(Self(addr, PhantomData))
        } else {
            None
        }
    }
    pub unsafe fn new_unchecked(addr: usize) -> Self {
        Self(addr, PhantomData)
    }

    pub fn hhdm_to_phys(self) -> PhysAddr {
        PhysAddr(hhdm::sub(self.0))
    }
}

impl<P: perms::Read> VirtAddrPageAligned<P> {
    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    pub unsafe fn as_ref<'a, T>(&self) -> &'a T {
        unsafe { &*self.as_ptr() }
    }
}
impl<P: perms::Write> VirtAddrPageAligned<P> {
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    pub unsafe fn as_mut<'a, T>(&self) -> &'a mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }
}

impl<P: perms::Base, T> TryFrom<*const T> for VirtAddrPageAligned<P> {
    type Error = ();
    fn try_from(ptr: *const T) -> Result<Self, Self::Error> {
        Self::new(ptr as usize).ok_or(())
    }
}
impl<P: perms::Base, T> TryFrom<*mut T> for VirtAddrPageAligned<P> {
    type Error = ();
    fn try_from(ptr: *mut T) -> Result<Self, Self::Error> {
        Self::new(ptr as usize).ok_or(())
    }
}
impl<P: perms::Base, T> TryFrom<NonNull<T>> for VirtAddrPageAligned<P> {
    type Error = ();
    fn try_from(ptr: NonNull<T>) -> Result<Self, Self::Error> {
        Self::new(ptr.as_ptr() as usize).ok_or(())
    }
}

#[inline(always)]
pub fn is_page_aligned(addr: usize) -> bool {
    addr & PAGE_MASK == 0
}

#[inline(always)]
pub fn page_align_up(addr: usize) -> usize {
    (addr + PAGE_MASK) & !PAGE_MASK
}

#[inline(always)]
pub fn page_align_down(addr: usize) -> usize {
    addr & !PAGE_MASK
}

#[inline(always)]
pub fn page_count_up(addr: usize) -> usize {
    (addr + PAGE_MASK) >> PAGE_SHIFT
}

#[inline(always)]
pub fn page_count_down(addr: usize) -> usize {
    addr >> PAGE_SHIFT
}

#[inline(always)]
pub fn is_large_page_aligned(addr: usize) -> bool {
    addr & LARGE_PAGE_MASK == 0
}

#[inline(always)]
pub fn large_page_align_up(addr: usize) -> usize {
    (addr + LARGE_PAGE_MASK) & !LARGE_PAGE_MASK
}

#[inline(always)]
pub fn large_page_align_down(addr: usize) -> usize {
    addr & !LARGE_PAGE_MASK
}

#[inline(always)]
pub fn large_page_count_up(addr: usize) -> usize {
    (addr + LARGE_PAGE_MASK) >> LARGE_PAGE_SHIFT
}

#[inline(always)]
pub fn large_page_count_down(addr: usize) -> usize {
    addr >> LARGE_PAGE_SHIFT
}

