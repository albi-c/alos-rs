use core::fmt::{Debug, Formatter};

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Volatile<T: Copy + Debug>(T);

impl<T: Copy + Debug> Volatile<T> {
    #[inline(always)]
    pub fn load(&self) -> T {
        unsafe { core::ptr::read_volatile(&raw const self.0) }
    }
    #[inline(always)]
    pub fn store(&mut self, value: T) {
        unsafe { core::ptr::write_volatile(&raw mut self.0, value) };
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
    ($name:ident; $($ident:ident: $ty:ty,)*) => {
        #[derive(Debug, Copy, Clone)]
        #[repr(C)]
        pub struct $name {
            $(pub $ident: crate::volatile::Volatile<$ty>,)*
        }

        unsafe impl bytemuck::Zeroable for $name {}
        unsafe impl bytemuck::Pod for $name {}
    }
}
