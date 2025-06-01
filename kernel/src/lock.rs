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

// #[cfg(not(feature = "smp"))]
mod no_smp {
    use core::cell::UnsafeCell;
    use core::ops::{Deref, DerefMut};
    use crate::lock::InterruptGuard;

    struct InterruptLock<T: ?Sized> {
        data: UnsafeCell<T>,
    }

    unsafe impl<T: ?Sized + Send> Send for InterruptLock<T> {}
    unsafe impl<T: ?Sized + Send + Sync> Sync for InterruptLock<T> {}

    struct InterruptLockGuard<'a, T: ?Sized> {
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

    impl<T> InterruptLock<T> {
        fn new(data: T) -> Self {
            InterruptLock { data: UnsafeCell::new(data) }
        }

        fn lock(&self) -> InterruptLockGuard<T> {
            let guard = InterruptGuard::new();
            InterruptLockGuard {
                guard,
                data: unsafe { self.data.as_mut_unchecked() }
            }
        }
    }
}

struct SpinMutex<T: ?Sized> {
    guard: InterruptGuard,
    lock: spin::Mutex<T>,
}

struct SpinRwLock<T: ?Sized> {
    guard: InterruptGuard,
    lock: spin::RwLock<T>,
}
