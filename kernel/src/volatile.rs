#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Volatile<T>(T);

impl<T> Volatile<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub fn set(&mut self, value: T) {
        self.0 = value;
    }
    #[inline(always)]
    pub fn get_ref(&self) -> &T {
        &self.0
    }
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Copy> Volatile<T> {
    #[inline(always)]
    pub fn get(&self) -> T {
        self.0
    }

    #[inline(always)]
    pub fn load(&self) -> T {
        unsafe { core::ptr::read_volatile(&raw const self.0) }
    }
    #[inline(always)]
    pub fn store(&mut self, value: T) {
        unsafe { core::ptr::write_volatile(&raw mut self.0, value) };
    }
}

#[macro_export]
macro_rules! volatile_struct {
    ($name:ident; $($ident:ident: $ty:ty,)*) => {
        #[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
        #[repr(C)]
        pub struct $name {
            $(pub $ident: crate::volatile::Volatile<$ty>,)*
        }
    }
}
