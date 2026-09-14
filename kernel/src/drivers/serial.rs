use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};
use crate::const_port;

const_port!(PORT: 0x3f8, 6);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    if !INITIALIZED.swap(true, Ordering::Relaxed) {
        PORT.out_b(2, 0x01);
    }
}

pub fn write(ch: u8) {
    PORT.out_b(0, ch);
    if ch == b'\n' {
        PORT.out_b(0, b'\r');
    }
}

pub fn read() -> Option<u8> {
    (PORT.in_b(5) & 0x1 != 0).then(|| PORT.in_b(0))
}

pub struct Serial;

impl Serial {
    pub fn write_char_n(&mut self, ch: char, count: usize) {
        let mut buf = [0u8; 4];
        let bytes = ch.encode_utf8(&mut buf).as_bytes();
        for _ in 0..count {
            for &b in bytes {
                write(b);
            }
        }
    }
}

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for ch in s.bytes() {
            write(ch);
        }
        Ok(())
    }
}
