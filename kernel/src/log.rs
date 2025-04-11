use crate::print;

pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

pub struct Logger<'a> {
    name: &'a str,
}

impl<'a> Logger<'a> {
    pub const fn new(name: &'a str) -> Self {
        Logger { name }
    }

    pub fn _header(&self, level: LogLevel) {
        match level {
            LogLevel::Debug => print!("[\x1b[34mD\x1b[0m] [{}] ", self.name),
            LogLevel::Info => print!("[\x1b[32mI\x1b[0m] [{}] ", self.name),
            LogLevel::Warning => print!("[\x1b[33m\x1b[1mW\x1b[0m] [{}] ", self.name),
            LogLevel::Error => print!("[\x1b[31m\x1b[1mE\x1b[0m] [{}] ", self.name),
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger._header($crate::log::LogLevel::Debug, );
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! info {
    ($logger:expr, $($arg:tt)*) => {
        $logger._header($crate::log::LogLevel::Info, );
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! warning {
    ($logger:expr, $($arg:tt)*) => {
        $logger._header($crate::log::LogLevel::Warning, );
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! error {
    ($logger:expr, $($arg:tt)*) => {
        $logger._header($crate::log::LogLevel::Error, );
        $crate::println!($($arg)*);
    };
}
