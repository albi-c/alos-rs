use crate::{linker_set_declare, linker_set_slice};

pub mod serial;
pub mod pit;

#[derive(Debug)]
pub struct DriverInitializer {
    name: &'static str,
    init: fn(),
    requirements: &'static mut [(&'static str, Option<&'static DriverInitializer>)],
    next: Option<&'static DriverInitializer>,
    _padding: [u64; 2],
}

impl DriverInitializer {
    pub const fn new(name: &'static str, init: fn(), requirements: &'static mut [(&'static str, Option<&'static DriverInitializer>)]) -> Self {
        const {
            assert!(size_of::<Self>() == 64);
        }
        Self {
            name,
            init,
            requirements,
            next: None,
            _padding: [0; 2],
        }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn requirements(&self) -> &[(&'static str, Option<&'static DriverInitializer>)] {
        self.requirements
    }
    pub unsafe fn requirements_mut(&self) -> &mut [(&'static str, Option<&'static DriverInitializer>)] {
        unsafe { core::mem::transmute(self.requirements) }
    }

    pub fn requirement(&self, name: &str) -> Option<&'static DriverInitializer> {
        // TODO: compile time lookup
        let res = self.requirements.iter().find(|(req_name, _)| *req_name == name);
        match res {
            Some((_, init)) => *init,
            None => panic!("DriverInitializer::requirement: requirement '{}' not specified", name),
        }
    }
}

#[macro_export]
macro_rules! count_tts {
    () => {0usize};
    ($_head:tt) => {1usize};
    ($_head:tt, $($tail:tt),*) => {1usize + crate::count_tts!($($tail),*)};
}

#[macro_export]
macro_rules! driver_initializer {
    ($name:literal, $init:expr) => {
        crate::driver_initializer!($name, $init, []);
    };
    ($name:literal, $init:expr, [$($requirements:literal),*]) => {
        crate::driver_initializer!(DRIVER, $name, $init, [$($requirements),*]);
    };
    ($var:ident, $name:literal, $init:expr) => {
        crate::driver_initializer!($var, $name, $init, []);
    };
    ($var:ident, $name:literal, $init:expr, [$($requirements:literal),*]) => {
        paste::paste! {
            static mut [<_DRIVER_REQS_ $var>]: [(&'static str, Option<&'static crate::drivers::DriverInitializer>); crate::count_tts!($($requirements),*)] = [$(($requirements, None)),*];
            crate::linker_set_item!(driver_initializers, $var: crate::drivers::DriverInitializer = crate::drivers::DriverInitializer::new(
                $name,
                $init,
                #[expect(static_mut_refs)]
                unsafe { &mut [<_DRIVER_REQS_ $var>] },
            ));
        }
    };
}

linker_set_declare!(driver_initializers, DriverInitializer);

pub fn init() {
    for init in linker_set_slice!(driver_initializers) {
        (init.init)();
    }
}
