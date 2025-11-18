use alloc::vec;
use core::alloc::Layout;
use core::arch::asm;
use core::cell::Cell;
use core::marker::PhantomData;
use core::mem::offset_of;
use core::ptr::NonNull;
use core::sync::atomic::AtomicBool;
use crate::{linker_set_declare, linker_set_slice};
use crate::cpu::msr_write;

const MSR_GS_BASE: u32 = 0xc0000101;
const MSR_KERNEL_GS_BASE: u32 = 0xc0000102;

#[derive(Debug)]
#[repr(C)]
pub struct CoreInfo {
    pub core_local_data: NonNull<u8>,
    pub id: usize,
    pub syscall_kernel_stack: Cell<NonNull<u8>>,
    pub syscall_buffer: usize,
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    const {
        assert!(offset_of!(CoreInfo, syscall_kernel_stack) == 16);
        assert!(offset_of!(CoreInfo, syscall_buffer) == 24);
    }

    let mut offset = size_of::<CoreInfo>();
    for (init_offset, _) in linker_set_slice!(core_locals) {
        init_offset(&mut offset);
    }
    let mut mem = vec![0u64; (offset + 7) >> 3].into_boxed_slice();
    let p = mem.as_mut_ptr();
    msr_write(MSR_GS_BASE, p as u64);
    msr_write(MSR_KERNEL_GS_BASE, 0);
    unsafe { (p as *mut CoreInfo).write(CoreInfo {
        core_local_data: NonNull::new(p as *mut u8).unwrap(),
        id: 0,
        syscall_kernel_stack: Cell::new(NonNull::dangling()),
        syscall_buffer: 0,
    }); }
    for (_, init_value) in linker_set_slice!(core_locals) {
        init_value();
    }
    core::mem::forget(mem);
    INITIALIZED.store(true, core::sync::atomic::Ordering::Release);
}

#[inline(always)]
pub fn core_local_block_pointer() -> CoreLocal<NonNull<u8>> {
    CoreLocal(Cell::new(offset_of!(CoreInfo, core_local_data)), PhantomData)
}
#[inline(always)]
fn core_info_local() -> CoreLocal<CoreInfo> {
    CoreLocal(Cell::new(0), PhantomData)
}

#[inline(always)]
pub fn core_info() -> &'static CoreInfo {
    core_info_local().get()
}

#[repr(C)]
pub struct CoreLocal<T>(Cell<usize>, PhantomData<T>);

impl<T> CoreLocal<T> {
    pub const unsafe fn new() -> Self {
        Self(Cell::new(0), PhantomData)
    }
    pub unsafe fn init_offset(&self, offset: &mut usize) {
        let align = Layout::new::<T>().align();
        *offset = (*offset + align - 1) & !(align - 1);
        self.0.set(*offset);
        *offset += size_of::<T>();
    }
    pub unsafe fn late_init(&self, value: T) {
        unsafe { self.get_ptr().write(value) };
    }

    #[inline(always)]
    pub fn initialized(&self) -> bool {
        INITIALIZED.load(core::sync::atomic::Ordering::Acquire)
    }

    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.0.get()
    }

    #[inline(always)]
    pub fn get_ptr(&self) -> NonNull<T> {
        unsafe { core_local_block_pointer().read().byte_add(self.offset()).cast() }
    }

    #[inline(always)]
    pub fn get(&self) -> &'static T {
        unsafe { self.get_ptr().as_ref() }
    }
    #[inline(always)]
    pub fn get_mut(&self) -> &'static mut T {
        unsafe { self.get_ptr().as_mut() }
    }
}

impl<T: CoreLocalItem> CoreLocal<T> {
    #[inline(always)]
    pub fn read(&self) -> T {
        T::core_local_read(self)
    }
    #[inline(always)]
    pub fn write(&self, value: T) {
        T::core_local_write(value, self)
    }
}

unsafe impl<T> Sync for CoreLocal<T> {}

trait CoreLocalItem : Sized {
    fn core_local_read(variable: &CoreLocal<Self>) -> Self;
    fn core_local_write(self, variable: &CoreLocal<Self>);
}

linker_set_declare!(core_locals, (fn(&mut usize), fn()));

#[macro_export]
macro_rules! _core_local_ls_item {
    ($name: ident, $ty:ty) => {
        paste::paste! {
            crate::linker_set_item!(core_locals, [<_CL_INIT_ $name>]: (fn(&mut usize), fn()) = (|offset| {
                unsafe { $name.init_offset(offset); }
            }, || {}));
        }
    };
    ($name: ident, $ty:ty, $value:expr) => {
        paste::paste! {
            crate::linker_set_item!(core_locals, [<_CL_INIT_ $name>]: (fn(&mut usize), fn()) = (|offset| {
                unsafe { $name.init_offset(offset); }
            }, || {
                unsafe { $name.get_ptr().write($value); }
            }));
        }
    };
}

#[macro_export]
macro_rules! core_local {
    ($vis:vis $name:ident: $ty:ty = $value:expr) => {
        $vis static $name: crate::core_local::CoreLocal<$ty> = unsafe { crate::core_local::CoreLocal::new() };
        crate::_core_local_ls_item!($name, $ty, $value);
    };
    (#no_mangle $vis:vis $name:ident: $ty:ty = $value:expr) => {
        #[unsafe(no_mangle)]
        $vis static $name: crate::core_local::CoreLocal<$ty> = unsafe { crate::core_local::CoreLocal::new() };
        crate::_core_local_ls_item!($name, $ty, $value);
    };
    (#late_init $vis:vis $name:ident: $ty:ty) => {
        $vis static $name: crate::core_local::CoreLocal<$ty> = unsafe { crate::core_local::CoreLocal::new() };
        crate::_core_local_ls_item!($name, $ty);
    };
}

macro_rules! core_local_item {
    (b $ty:ty) => {
        impl CoreLocalItem for $ty {
            #[inline(always)]
            fn core_local_read(variable: &CoreLocal<Self>) -> Self {
                let result: Self;
                unsafe { asm!("mov {0}, byte ptr gs:[{1}]", out(reg_byte) result, in(reg) variable.offset()); }
                result
            }
            #[inline(always)]
            fn core_local_write(self, variable: &CoreLocal<Self>) {
                unsafe { asm!("mov byte ptr gs:[{0}], {1}", in(reg) variable.offset(), in(reg_byte) self); }
            }
        }
    };
    (w $ty:ty) => {
        impl CoreLocalItem for $ty {
            #[inline(always)]
            fn core_local_read(variable: &CoreLocal<Self>) -> Self {
                let result: Self;
                unsafe { asm!("mov {0:x}, word ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
                result
            }
            #[inline(always)]
            fn core_local_write(self, variable: &CoreLocal<Self>) {
                unsafe { asm!("mov word ptr gs:[{0}], {1:x}", in(reg) variable.offset(), in(reg) self); }
            }
        }
    };
    (d $ty:ty) => {
        impl CoreLocalItem for $ty {
            #[inline(always)]
            fn core_local_read(variable: &CoreLocal<Self>) -> Self {
                let result: Self;
                unsafe { asm!("mov {0:e}, dword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
                result
            }
            #[inline(always)]
            fn core_local_write(self, variable: &CoreLocal<Self>) {
                unsafe { asm!("mov dword ptr gs:[{0}], {1:e}", in(reg) variable.offset(), in(reg) self); }
            }
        }
    };
    (q $ty:ty) => {
        impl CoreLocalItem for $ty {
            #[inline(always)]
            fn core_local_read(variable: &CoreLocal<Self>) -> Self {
                let result: Self;
                unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
                result
            }
            #[inline(always)]
            fn core_local_write(self, variable: &CoreLocal<Self>) {
                unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self); }
            }
        }
    };
}

core_local_item!(b u8);
core_local_item!(w u16);
core_local_item!(d u32);
core_local_item!(q u64);
core_local_item!(q usize);

core_local_item!(b i8);
core_local_item!(w i16);
core_local_item!(d i32);
core_local_item!(q i64);
core_local_item!(q isize);

impl<T: Sized> CoreLocalItem for *const T {
    #[inline(always)]
    fn core_local_read(variable: &CoreLocal<Self>) -> Self {
        let result: Self;
        unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
        result
    }
    #[inline(always)]
    fn core_local_write(self, variable: &CoreLocal<Self>) {
        unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self); }
    }
}

impl<T: Sized> CoreLocalItem for NonNull<T> {
    #[inline(always)]
    fn core_local_read(variable: &CoreLocal<Self>) -> Self {
        let result: *mut T;
        unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
        NonNull::new(result).unwrap()
    }
    #[inline(always)]
    fn core_local_write(self, variable: &CoreLocal<Self>) {
        unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self.as_ptr()); }
    }
}

impl<T: Sized> CoreLocalItem for &'static T {
    #[inline(always)]
    fn core_local_read(variable: &CoreLocal<Self>) -> Self {
        let result: *const T;
        unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
        unsafe { result.as_ref() }.unwrap()
    }
    #[inline(always)]
    fn core_local_write(self, variable: &CoreLocal<Self>) {
        unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self as *const T); }
    }
}

impl<T: Sized> CoreLocalItem for Option<&'static T> {
    #[inline(always)]
    fn core_local_read(variable: &CoreLocal<Self>) -> Self {
        let result: *const T;
        unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
        unsafe { result.as_ref() }
    }
    #[inline(always)]
    fn core_local_write(self, variable: &CoreLocal<Self>) {
        unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self.map_or(core::ptr::null(), |r| r as *const T)); }
    }
}

impl<T: Sized> CoreLocalItem for &'static mut T {
    #[inline(always)]
    fn core_local_read(variable: &CoreLocal<Self>) -> Self {
        let result: *mut T;
        unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
        unsafe { result.as_mut() }.unwrap()
    }
    #[inline(always)]
    fn core_local_write(self, variable: &CoreLocal<Self>) {
        unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self as *mut T); }
    }
}

impl<T: Sized> CoreLocalItem for Option<&'static mut T> {
    #[inline(always)]
    fn core_local_read(variable: &CoreLocal<Self>) -> Self {
        let result: *mut T;
        unsafe { asm!("mov {0:r}, qword ptr gs:[{1}]", out(reg) result, in(reg) variable.offset()); }
        unsafe { result.as_mut() }
    }
    #[inline(always)]
    fn core_local_write(self, variable: &CoreLocal<Self>) {
        unsafe { asm!("mov qword ptr gs:[{0}], {1:r}", in(reg) variable.offset(), in(reg) self.map_or(core::ptr::null(), |r| r as *mut T)); }
    }
}
