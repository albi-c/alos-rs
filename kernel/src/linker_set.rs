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

#[macro_export]
macro_rules! linker_set_item {
    ($set:ident, $name:ident: $ty:ty = $expr:expr) => {
        #[unsafe(link_section = stringify!($set))]
        #[used]
        static $name: $ty = $expr;
    }
}
