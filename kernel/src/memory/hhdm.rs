use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::memory::address;

static OFFSET: AtomicUsize = AtomicUsize::new(0);

#[inline(always)]
pub unsafe fn set_offset(offset: usize) {
    assert!(address::is_page_aligned(offset));
    OFFSET.store(offset, Ordering::Relaxed);
}

#[inline(always)]
pub fn get_offset() -> usize {
    OFFSET.load(Ordering::Relaxed)
}

#[inline(always)]
pub fn add(addr: usize) -> usize {
    addr + get_offset()
}

#[inline(always)]
pub fn sub(addr: usize) -> usize {
    addr - get_offset()
}

#[inline(always)]
pub fn as_ptr<T>(addr: usize) -> *mut T {
    add(addr) as *mut T
}
#[inline(always)]
pub fn as_non_null<T>(addr: usize) -> NonNull<T> {
    NonNull::new(as_ptr(addr)).expect("invalid hhdm offset")
}

#[inline(always)]
pub fn from_ptr<T>(ptr: *const T) -> usize {
    sub(ptr as usize)
}

#[inline(always)]
pub fn from_ref<T>(val: &T) -> usize {
    from_ptr(val as *const T)
}

#[inline(always)]
pub unsafe fn as_ref<T>(addr: usize) -> &'static T {
    unsafe { &*as_ptr(addr) }
}

#[inline(always)]
pub unsafe fn as_mut_ref<T>(addr: usize) -> &'static mut T {
    unsafe { &mut *as_ptr(addr) }
}
