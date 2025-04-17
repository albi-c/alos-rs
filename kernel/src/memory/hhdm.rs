use core::sync::atomic::{AtomicUsize, Ordering};

static OFFSET: AtomicUsize = AtomicUsize::new(0);

#[inline(always)]
pub unsafe fn set_offset(offset: usize) {
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
pub fn from_ptr<T>(ptr: *const T) -> usize {
    sub(ptr as usize)
}

#[inline(always)]
pub unsafe fn as_ref<T>(addr: usize) -> &'static T {
    unsafe { &*as_ptr(addr) }
}

#[inline(always)]
pub unsafe fn as_mut_ref<T>(addr: usize) -> &'static mut T {
    unsafe { &mut *as_ptr(addr) }
}
