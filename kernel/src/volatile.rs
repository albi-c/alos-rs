use core::fmt::{Debug, Formatter};
use core::ptr::NonNull;
use bytemuck::{Pod, Zeroable};

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
pub struct Volatile<T: Copy + Debug>(T);

impl<T: Copy + Debug> Volatile<T> {
    pub fn from_ref(reference: &mut T) -> &mut Self {
        unsafe { NonNull::from(reference).cast().as_mut() }
    }

    pub fn from_ptr(ptr: NonNull<T>) -> &'static mut Self {
        unsafe { ptr.cast().as_mut() }
    }

    #[inline(always)]
    pub fn load(&self) -> T {
        unsafe { core::ptr::read_volatile(&raw const self.0) }
    }
    #[inline(always)]
    pub fn store(&mut self, value: T) {
        unsafe { core::ptr::write_volatile(&raw mut self.0, value) };
    }

    pub fn modify(&mut self, func: impl FnOnce(T) -> T) {
        self.store(func(self.load()))
    }

    pub fn as_ptr(&self) -> *const T {
        &self.0 as *const _
    }
    pub fn as_mut_ptr(&mut self) -> *mut T {
        &mut self.0 as *mut _
    }
}

impl<T: Copy + Debug> Debug for Volatile<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.load().fmt(f)
    }
}

#[macro_export]
macro_rules! volatile_struct {
    ($vi:vis $name:ident; $($ident:ident: $ty:ty,)*) => {
        #[derive(Debug, Copy, Clone)]
        #[repr(C)]
        $vi struct $name {
            $(pub $ident: crate::volatile::Volatile<$ty>,)*
        }

        unsafe impl bytemuck::Zeroable for $name {}
        unsafe impl bytemuck::Pod for $name {}
    }
}
