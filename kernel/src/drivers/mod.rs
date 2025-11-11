use crate::{linker_set_declare, linker_set_slice_mut};

pub mod serial;
pub(crate) mod time;

#[derive(Debug)]
pub struct Driver {
    name: &'static str,
    init: fn(),
    requirements: &'static mut [(&'static str, Option<&'static Driver>)],
    next: Option<&'static Driver>,
    _padding: [u64; 2],
}

impl Driver {
    pub const fn new(name: &'static str, init: fn(), requirements: &'static mut [(&'static str, Option<&'static Driver>)]) -> Self {
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

    pub fn requirements(&self) -> &[(&'static str, Option<&'static Driver>)] {
        self.requirements
    }
    pub unsafe fn requirements_mut(&self) -> &mut [(&'static str, Option<&'static Driver>)] {
        unsafe { core::slice::from_raw_parts_mut(self.requirements.as_ptr() as *mut _, self.requirements.len()) }
    }

    pub fn requirement(&self, name: &str) -> Option<&'static Driver> {
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
macro_rules! driver {
    ($name:literal, $init:expr) => {
        crate::driver!($name, $init, []);
    };
    ($name:literal, $init:expr, [$($requirements:literal),*]) => {
        crate::driver!(DRIVER, $name, $init, [$($requirements),*]);
    };
    ($var:ident, $name:literal, $init:expr) => {
        crate::driver!($var, $name, $init, []);
    };
    ($var:ident, $name:literal, $init:expr, [$($requirements:literal),*]) => {
        paste::paste! {
            static mut [<_DRIVER_REQS_ $var>]: [(&'static str, Option<&'static crate::drivers::Driver>); crate::count_tts!($($requirements),*)] = [$(($requirements, None)),*];
            static [<_DRIVER_ $var>]: crate::drivers::Driver = crate::drivers::Driver::new(
                $name,
                $init,
                #[expect(static_mut_refs)]
                unsafe { &mut [<_DRIVER_REQS_ $var>] },
            );
            crate::linker_set_item!(driver_initializers, $var: &'static crate::drivers::Driver = &[<_DRIVER_ $var>]);
        }
    };
}

linker_set_declare!(driver_initializers, &'static Driver);
driver!(PLACEHOLDER, "__placeholder", || ());

pub fn init() {
    let drivers = linker_set_slice_mut!(driver_initializers);
    drivers.sort_unstable_by_key(|init| init.name);
    for driver in drivers {
        (driver.init)();
    }
}
