use core::iter::Step;
use core::ops::{Add, AddAssign, Mul, Shl, Shr, Sub, SubAssign};
use core::ptr::NonNull;
use bytemuck::Pod;
use crate::memory::{hhdm, MemorySpace};

pub const PAGE_SHIFT: u8 = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

pub const LARGE_PAGE_SHIFT: u8 = 21;
pub const LARGE_PAGE_SIZE: usize = 1 << LARGE_PAGE_SHIFT;
pub const LARGE_PAGE_MASK: usize = LARGE_PAGE_SIZE - 1;

macro_rules! simple_op {
    ($id:ident, $op:ident, $func:ident) => {
        impl $op<usize> for $id {
            type Output = $id;
            fn $func(self, rhs: usize) -> Self::Output {
                $id(self.0.$func(rhs))
            }
        }
    };
}

macro_rules! addr_add_sub_page_count {
    ($addr:ident) => {
        impl Add<PageCount> for $addr {
            type Output = $addr;
            fn add(self, rhs: PageCount) -> Self::Output {
                Self(self.0 + rhs.size())
            }
        }
        impl Sub<PageCount> for $addr {
            type Output = $addr;
            fn sub(self, rhs: PageCount) -> Self::Output {
                Self(self.0 - rhs.size())
            }
        }
    };
}
macro_rules! addr_add_sub_usize {
    ($addr:ident) => {
        impl Add<usize> for $addr {
            type Output = $addr;
            fn add(self, rhs: usize) -> Self::Output {
                Self(self.0 + rhs)
            }
        }
        impl Sub<usize> for $addr {
            type Output = $addr;
            fn sub(self, rhs: usize) -> Self::Output {
                Self(self.0 - rhs)
            }
        }
    };
}

macro_rules! addr_alignment {
    ($addr:ident, $aligned_addr:ident) => {
        impl $addr {
            #[inline(always)]
            pub fn is_page_aligned(self) -> bool {
                is_page_aligned(self.0)
            }
            #[inline(always)]
            pub fn as_page_aligned(self) -> Option<$aligned_addr> {
                $aligned_addr::new(self.0)
            }
            #[inline(always)]
            pub fn page_align_up(self) -> $aligned_addr {
                $aligned_addr(page_align_up(self.0))
            }
            #[inline(always)]
            pub fn page_align_down(self) -> $aligned_addr {
                $aligned_addr(page_align_down(self.0))
            }
            #[inline(always)]
            pub fn page_count_up(self) -> PageCount {
                PageCount(page_count_up(self.0))
            }
            #[inline(always)]
            pub fn page_count_down(self) -> PageCount {
                PageCount(page_count_down(self.0))
            }
        }
        impl $aligned_addr {
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
        }
        impl From<$aligned_addr> for $addr {
            fn from(value: $aligned_addr) -> Self {
                Self(value.0)
            }
        }
        impl TryFrom<$addr> for $aligned_addr {
            type Error = ();
            fn try_from(value: $addr) -> Result<Self, Self::Error> {
                value.as_page_aligned().ok_or(())
            }
        }
        impl From<$aligned_addr> for usize {
            fn from(value: $aligned_addr) -> Self {
                value.0
            }
        }
        impl TryFrom<usize> for $aligned_addr {
            type Error = ();
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                Self::new(value).ok_or(())
            }
        }
    };
}

macro_rules! addr_to_from_usize {
    ($addr:ident) => {
        impl $addr {
            #[inline(always)]
            pub fn new(addr: usize) -> Self {
                Self(addr)
            }
        }
        impl From<$addr> for usize {
            fn from(value: $addr) -> Self {
                value.0
            }
        }
        impl From<usize> for $addr {
            fn from(value: usize) -> Self {
                $addr(value)
            }
        }
    };
}

macro_rules! addr_usize_addr {
    ($addr:ident) => {
        impl $addr {
            #[inline(always)]
            pub fn addr(self) -> usize {
                self.0
            }
        }
    };
}

macro_rules! addr_cmp {
    ($addr:ident) => {
        impl PartialEq<usize> for $addr {
            fn eq(&self, other: &usize) -> bool {
                self.0 == *other
            }
        }
        impl PartialOrd<usize> for $addr {
            fn partial_cmp(&self, other: &usize) -> Option<core::cmp::Ordering> {
                self.0.partial_cmp(other)
            }
        }
        impl PartialEq<$addr> for usize {
            fn eq(&self, other: &$addr) -> bool {
                *self == other.0
            }
        }
        impl PartialOrd<$addr> for usize {
            fn partial_cmp(&self, other: &$addr) -> Option<core::cmp::Ordering> {
                self.partial_cmp(&other.0)
            }
        }
        impl $addr {
            pub const fn zero() -> Self {
                Self(0)
            }
        }
    };
}

macro_rules! addr_page_count_diff {
    ($addr:ident) => {
        impl Sub<$addr> for $addr {
            type Output = PageCount;
            fn sub(self, rhs: $addr) -> Self::Output {
                PageCount(page_count_down(self.0 - rhs.0))
            }
        }
    };
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PageCount(usize);

impl PageCount {
    #[inline(always)]
    pub fn as_phys_addr(self) -> PhysAddrPageAligned {
        PhysAddrPageAligned(self.size())
    }
    #[inline(always)]
    pub fn as_virt_addr(self) -> VirtAddrPageAligned {
        VirtAddrPageAligned(self.size())
    }

    #[inline(always)]
    pub fn count(self) -> usize {
        self.0
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

    fn forward_overflowing(start: Self, count: usize) -> (Self, bool) {
        let (page, overflow) = start.0.overflowing_add(count);
        (Self(page), overflow)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self(start.0.checked_sub(count)?))
    }

    fn backward_overflowing(start: Self, count: usize) -> (Self, bool) {
        let (page, overflow) = start.0.overflowing_sub(count);
        (Self(page), overflow)
    }
}

impl Add<PageCount> for PageCount {
    type Output = PageCount;
    fn add(self, rhs: PageCount) -> Self::Output {
        PageCount(self.0 + rhs.0)
    }
}
impl Sub<PageCount> for PageCount {
    type Output = PageCount;
    fn sub(self, rhs: PageCount) -> Self::Output {
        PageCount(self.0 - rhs.0)
    }
}
impl AddAssign<PageCount> for PageCount {
    fn add_assign(&mut self, rhs: PageCount) {
        self.0 += rhs.0;
    }
}
impl SubAssign<PageCount> for PageCount {
    fn sub_assign(&mut self, rhs: PageCount) {
        self.0 -= rhs.0;
    }
}
simple_op!(PageCount, Add, add);
simple_op!(PageCount, Sub, sub);
simple_op!(PageCount, Mul, mul);
simple_op!(PageCount, Shl, shl);
simple_op!(PageCount, Shr, shr);
addr_to_from_usize!(PageCount);
addr_cmp!(PageCount);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub fn hhdm_to_virt(self) -> VirtAddr {
        VirtAddr::new(hhdm::add(self.0))
    }
}

addr_add_sub_page_count!(PhysAddr);
addr_add_sub_usize!(PhysAddr);
addr_alignment!(PhysAddr, PhysAddrPageAligned);
addr_to_from_usize!(PhysAddr);
addr_usize_addr!(PhysAddr);
addr_cmp!(PhysAddr);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddrPageAligned(usize);

impl PhysAddrPageAligned {
    #[inline(always)]
    pub fn page_count(self) -> PageCount {
        PageCount(self.0 >> PAGE_SHIFT)
    }

    pub fn hhdm_to_virt(self) -> VirtAddrPageAligned {
        unsafe { VirtAddrPageAligned::new_unchecked(hhdm::add(self.0)) }
    }
}

addr_add_sub_page_count!(PhysAddrPageAligned);
addr_cmp!(PhysAddrPageAligned);
addr_page_count_diff!(PhysAddrPageAligned);
addr_usize_addr!(PhysAddrPageAligned);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub fn hhdm_to_phys(self) -> PhysAddr {
        PhysAddr(hhdm::sub(self.0))
    }

    pub fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }
    pub unsafe fn as_ref<'a, T>(self) -> &'a T {
        unsafe { &*self.as_ptr() }
    }

    pub fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
    pub unsafe fn as_mut<'a, T>(self) -> &'a mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }

    pub fn as_non_null<T>(self) -> Option<NonNull<T>> {
        NonNull::new(self.as_mut_ptr())
    }
}

addr_add_sub_page_count!(VirtAddr);
addr_add_sub_usize!(VirtAddr);
addr_alignment!(VirtAddr, VirtAddrPageAligned);
addr_to_from_usize!(VirtAddr);
addr_usize_addr!(VirtAddr);
addr_cmp!(VirtAddr);

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
pub struct VirtAddrPageAligned(usize);

impl VirtAddrPageAligned {
    #[inline(always)]
    pub fn page_count(self) -> PageCount {
        PageCount(self.0 >> PAGE_SHIFT)
    }

    pub fn hhdm_to_phys(self) -> PhysAddr {
        PhysAddr(hhdm::sub(self.0))
    }

    pub fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }
    pub unsafe fn as_ref<'a, T>(&self) -> &'a T {
        unsafe { &*self.as_ptr() }
    }

    pub fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
    pub unsafe fn as_mut<'a, T>(self) -> &'a mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }

    pub fn as_non_null<T>(self) -> Option<NonNull<T>> {
        NonNull::new(self.as_mut_ptr())
    }

    pub fn hhdm_offset() -> Self {
        Self(hhdm::get_offset())
    }
}

addr_add_sub_page_count!(VirtAddrPageAligned);
addr_cmp!(VirtAddrPageAligned);
addr_page_count_diff!(VirtAddrPageAligned);
addr_usize_addr!(VirtAddrPageAligned);

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct UserVirtAddr(usize);

impl UserVirtAddr {
    pub fn check_read(self, length: usize) -> Option<VirtAddr> {
        MemorySpace::get().user_check_read(self, length).then_some(VirtAddr(self.0))
    }
    pub fn check_write(self, length: usize) -> Option<VirtAddr> {
        MemorySpace::get().user_check_write(self, length).then_some(VirtAddr(self.0))
    }

    pub fn as_slice<'a, T: Pod>(self, length: usize) -> Option<&'a [T]> {
        self.check_read(length)
            .map(|addr| unsafe { core::slice::from_raw_parts(addr.as_ptr(), length) })
    }
    pub fn as_slice_mut<'a, T: Pod>(self, length: usize) -> Option<&'a mut [T]> {
        self.check_write(length)
            .map(|addr| unsafe { core::slice::from_raw_parts_mut(addr.as_mut_ptr(), length) })
    }
}

addr_add_sub_page_count!(UserVirtAddr);
addr_add_sub_usize!(UserVirtAddr);
addr_to_from_usize!(UserVirtAddr);
addr_usize_addr!(UserVirtAddr);
addr_cmp!(UserVirtAddr);

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

pub fn align_page_range(addr: usize, length: usize) -> (VirtAddrPageAligned, PageCount) {
    let virt = VirtAddr::new(addr).page_align_down();
    let count = PageCount::pages_up(length.saturating_add(addr - virt.addr()));
    (virt, count)
}
