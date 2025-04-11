use core::fmt::Write;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::ports::Port;

lazy_static! {
    pub static ref SERIAL: Mutex<Serial> = Mutex::new(Serial::new());
}

pub struct Serial {
    port: Port,
}

impl Serial {
    fn new() -> Self {
        Serial {
            port: Port::alloc(0x3f8, 6).expect("Couldn't allocate port for serial"),
        }
    }

    fn write(&self, ch: u8) {
        self.port.out_b(0, ch);
        if ch == b'\n' {
            self.port.out_b(0, b'\r');
        }
    }
}

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for ch in s.bytes() {
            self.write(ch);
        }
        Ok(())
    }
}
