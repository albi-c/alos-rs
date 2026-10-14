use crate::drivers::serial::Serial;

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    Serial.write_fmt(args).unwrap();
}

#[doc(hidden)]
pub fn _print_char_n(value: char, n: usize) {
    Serial.write_char_n(value, n);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print_char_n {
    ($ch:expr, $n:expr) => ($crate::print::_print_char_n($ch, $n));
}

#[derive(Debug, Copy, Clone)]
pub struct ByteSize(pub usize);

impl core::fmt::Display for ByteSize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", ByteSize64(self.0 as u64))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ByteSize64(pub u64);

impl core::fmt::Display for ByteSize64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let ByteSize64(size) = *self;
        if size < 3 << 10 {
            write!(f, "{} B", size)
        } else if size < 3 << 20 {
            write!(f, "{} kB", size >> 10)
        } else if size < 3 << 30 {
            write!(f, "{} MB", size >> 20)
        } else if size < 3 << 40 {
            write!(f, "{} GB", size >> 30)
        } else {
            write!(f, "{} TB", size >> 40)
        }
    }
}
