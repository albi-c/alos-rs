use crate::cpu;

struct InterruptGuard(bool);

impl !Send for InterruptGuard {}

impl InterruptGuard {
    pub fn new() -> Self {
        InterruptGuard(cpu::query_and_disable_interrupts())
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.0 {
            cpu::enable_interrupts();
        }
    }
}

#[cfg(not(feature = "smp"))]
mod inner {
    use core::cell::UnsafeCell;
    use core::fmt::Debug;
    use core::ops::{Deref, DerefMut};
    use crate::lock::InterruptGuard;

    pub struct Lock<T: ?Sized> {
        data: UnsafeCell<T>,
    }

    unsafe impl<T: ?Sized + Send> Send for Lock<T> {}
    unsafe impl<T: ?Sized> Sync for Lock<T> {}
    
    impl<T: Debug + ?Sized> Debug for Lock<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Lock {{ data: <locked> }}")
        }
    }

    pub struct InterruptLockGuard<'a, T: ?Sized> {
        guard: InterruptGuard,
        data: &'a mut T,
    }
    
    impl<'a, T: ?Sized> Deref for InterruptLockGuard<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.data
        }
    }
    impl<'a, T> DerefMut for InterruptLockGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.data
        }
    }

    impl<T> Lock<T> {
        pub const fn new(data: T) -> Self {
            Lock { data: UnsafeCell::new(data) }
        }
    }
    
    impl<T: ?Sized> Lock<T> {
        pub fn write(&self) -> InterruptLockGuard<T> {
            let guard = InterruptGuard::new();
            InterruptLockGuard {
                guard,
                data: unsafe { self.data.as_mut_unchecked() },
            }
        }
        pub fn read(&self) -> InterruptLockGuard<T> {
            self.write()
        }
    }
}

#[cfg(feature = "smp")]
mod inner {
    use core::fmt::Debug;
    use core::ops::{Deref, DerefMut};
    use crate::lock::InterruptGuard;

    pub struct Lock<T: ?Sized> {
        lock: spin::RwLock<T>,
    }

    unsafe impl<T: ?Sized + Send> Send for Lock<T> {}
    unsafe impl<T: ?Sized> Sync for Lock<T> {}

    impl<T: Debug + ?Sized> Debug for Lock<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Lock {{ lock: {:?} }}", &self.lock)
        }
    }
    
    pub struct RwLockGuard<'a, T: ?Sized> {
        guard: InterruptGuard,
        data: spin::RwLockReadGuard<'a, T>,
    }

    impl<T: ?Sized> Drop for RwLockGuard<'_, T> {
        fn drop(&mut self) {}
    }

    pub struct RwLockGuardMut<'a, T: ?Sized> {
        guard: InterruptGuard,
        data: spin::RwLockWriteGuard<'a, T>,
    }

    impl<T: ?Sized> Drop for RwLockGuardMut<'_, T> {
        fn drop(&mut self) {}
    }

    impl<'a, T: ?Sized> Deref for RwLockGuard<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.data.deref()
        }
    }

    impl<'a, T: ?Sized> Deref for RwLockGuardMut<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.data.deref()
        }
    }
    impl<'a, T> DerefMut for RwLockGuardMut<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.data.deref_mut()
        }
    }
    
    impl<T> Lock<T> {
        pub const fn new(data: T) -> Self {
            Lock { lock: spin::RwLock::new(data) }
        }
    }

    impl<T: ?Sized> Lock<T> {
        pub fn write(&self) -> RwLockGuardMut<'_, T> {
            let guard = InterruptGuard::new();
            RwLockGuardMut {
                guard,
                data: self.lock.write(),
            }
        }
        pub fn read(&self) -> RwLockGuard<'_, T> {
            let guard = InterruptGuard::new();
            RwLockGuard {
                guard,
                data: self.lock.read()
            }
        }

        pub unsafe fn force_write_unlock(&self) {
            unsafe { self.lock.force_write_unlock() }
        }
    }
}

pub use inner::*;
