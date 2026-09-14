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
