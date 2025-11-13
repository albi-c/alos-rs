use core::ops::{Deref, DerefMut};

#[macro_export]
macro_rules! linker_set_declare {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            unsafe extern "C" {
                #[allow(improper_ctypes)]
                static mut [<__start_ $name>]: $ty;
                #[allow(improper_ctypes)]
                static mut [<__stop_ $name>]: $ty;
            }
        }
    };
}

#[macro_export]
macro_rules! linker_set_slice {
    ($name:ident) => {
        paste::paste! {
            unsafe { core::slice::from_ptr_range(
                &raw const [<__start_ $name>]..&raw const [<__stop_ $name>]) }
        }
    }
}

#[macro_export]
macro_rules! linker_set_slice_mut {
    ($name:ident) => {
        paste::paste! {
            unsafe { core::slice::from_mut_ptr_range(
                &raw mut [<__start_ $name>]..&raw mut [<__stop_ $name>]) }
        }
    }
}

pub struct LinkerSetItem<T: 'static>(&'static mut T);

impl<T> LinkerSetItem<T> {
    pub const fn new(ptr: &'static mut T) -> Self {
        Self(ptr)
    }
}

impl<T> Deref for LinkerSetItem<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl<T> DerefMut for LinkerSetItem<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

#[macro_export]
macro_rules! linker_set_item {
    ($set:ident, $name:ident: $ty:ty = $expr:expr) => {
        paste::paste! {
            #[unsafe(link_section = stringify!($set))]
            #[used]
            static mut [<_LINKER_SET_VAL_ $name>]: $ty = $expr;
            #[expect(static_mut_refs)]
            #[used]
            static $name: crate::linker_set::LinkerSetItem<$ty> = crate::linker_set::LinkerSetItem::new(unsafe { &mut [<_LINKER_SET_VAL_ $name>] });
        }
    };
    ($set:ident, mut $name:ident: $ty:ty = $expr:expr) => {
        paste::paste! {
            #[unsafe(link_section = stringify!($set))]
            #[used]
            static mut $name: $ty = $expr;
        }
    };
}
