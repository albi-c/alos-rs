use core::iter::Step;
use core::ops::{Add, Mul, Shl, Shr, Sub};
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
        PhysAddrPageAligned(self.size())
    }
    #[inline(always)]
    pub fn as_virt_addr(self) -> VirtAddrPageAligned {
        VirtAddrPageAligned(self.size())
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

impl Step for PageCount {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        (end.0 - start.0, Some(end.0 - start.0))
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self(start.0.checked_add(count)?))
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self(start.0.checked_sub(count)?))
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

    pub fn hhdm_to_virt(self) -> VirtAddr {
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

    pub fn hhdm_to_virt(self) -> VirtAddrPageAligned {
        unsafe { VirtAddrPageAligned::new_unchecked(hhdm::add(self.0)) }
    }
}

impl Add<PageCount> for PhysAddrPageAligned {
    type Output = PhysAddrPageAligned;
    fn add(self, rhs: PageCount) -> Self::Output {
        Self(self.0 + rhs.size())
    }
}
impl Sub<PageCount> for PhysAddrPageAligned {
    type Output = PhysAddrPageAligned;
    fn sub(self, rhs: PageCount) -> Self::Output {
        Self(self.0 - rhs.size())
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);

impl VirtAddr {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    #[inline(always)]
    pub fn is_page_aligned(self) -> bool {
        is_page_aligned(self.0)
    }
    #[inline(always)]
    pub fn as_page_aligned(self) -> Option<VirtAddrPageAligned> {
        VirtAddrPageAligned::new(self.0)
    }
    #[inline(always)]
    pub fn page_align_up(self) -> VirtAddrPageAligned {
        VirtAddrPageAligned(page_align_up(self.0))
    }
    #[inline(always)]
    pub fn page_align_down(self) -> VirtAddrPageAligned {
        VirtAddrPageAligned(page_align_down(self.0))
    }
    #[inline(always)]
    pub fn page_count_up(self) -> PageCount {
        PageCount(page_count_up(self.0))
    }
    #[inline(always)]
    pub fn page_count_down(self) -> PageCount {
        PageCount(page_count_down(self.0))
    }

    pub fn hhdm_to_phys(self) -> PhysAddr {
        PhysAddr(hhdm::sub(self.0))
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    pub unsafe fn as_ref<T>(&self) -> &T {
        unsafe { &*self.as_ptr() }
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    pub unsafe fn as_mut<T>(&self) -> &mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }
}

impl From<VirtAddr> for usize {
    fn from(value: VirtAddr) -> Self {
        value.0
    }
}
impl From<VirtAddrPageAligned> for VirtAddr {
    fn from(value: VirtAddrPageAligned) -> Self {
        Self(value.0)
    }
}

impl<T: ?Sized> From<*const T> for VirtAddr {
    fn from(ptr: *const T) -> Self {
        Self::new(ptr.addr())
    }
}
impl<T: ?Sized> From<*mut T> for VirtAddr {
    fn from(ptr: *mut T) -> Self {
        Self::new(ptr.addr())
    }
}
impl<T: ?Sized> From<&T> for VirtAddr {
    fn from(value: &T) -> Self {
        Self::new((value as *const T).addr())
    }
}
impl<T: ?Sized> From<&mut T> for VirtAddr {
    fn from(value: &mut T) -> Self {
        Self::new((value as *mut T).addr())
    }
}
impl<T: ?Sized> From<NonNull<T>> for VirtAddr {
    fn from(ptr: NonNull<T>) -> Self {
        Self::new(ptr.as_ptr().addr())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddrPageAligned(pub usize);

impl VirtAddrPageAligned {
    pub fn new(addr: usize) -> Option<Self> {
        if is_page_aligned(addr) {
            Some(Self(addr))
        } else {
            None
        }
    }
    pub unsafe fn new_unchecked(addr: usize) -> Self {
        Self(addr)
    }

    #[inline(always)]
    pub fn page_count(self) -> PageCount {
        PageCount(self.0 >> PAGE_SHIFT)
    }

    pub fn hhdm_to_phys(self) -> PhysAddr {
        PhysAddr(hhdm::sub(self.0))
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    pub unsafe fn as_ref<'a, T>(&self) -> &'a T {
        unsafe { &*self.as_ptr() }
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    pub unsafe fn as_mut<'a, T>(&self) -> &'a mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }

    pub fn hhdm_offset() -> Self {
        Self(hhdm::get_offset())
    }
}

impl Add<PageCount> for VirtAddrPageAligned {
    type Output = VirtAddrPageAligned;
    fn add(self, rhs: PageCount) -> Self::Output {
        Self(self.0 + rhs.size())
    }
}
impl Sub<PageCount> for VirtAddrPageAligned {
    type Output = VirtAddrPageAligned;
    fn sub(self, rhs: PageCount) -> Self::Output {
        Self(self.0 - rhs.size())
    }
}

impl From<VirtAddrPageAligned> for usize {
    fn from(value: VirtAddrPageAligned) -> Self {
        value.0
    }
}

impl<T: ?Sized> TryFrom<*const T> for VirtAddrPageAligned {
    type Error = ();
    fn try_from(ptr: *const T) -> Result<Self, Self::Error> {
        Self::new(ptr.addr()).ok_or(())
    }
}
impl<T: ?Sized> TryFrom<*mut T> for VirtAddrPageAligned {
    type Error = ();
    fn try_from(ptr: *mut T) -> Result<Self, Self::Error> {
        Self::new(ptr.addr()).ok_or(())
    }
}
impl<T: ?Sized> TryFrom<NonNull<T>> for VirtAddrPageAligned {
    type Error = ();
    fn try_from(ptr: NonNull<T>) -> Result<Self, Self::Error> {
        Self::new(ptr.as_ptr().addr()).ok_or(())
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

