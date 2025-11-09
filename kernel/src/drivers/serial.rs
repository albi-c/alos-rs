use core::fmt::Write;
use crate::ports::Port;

pub static mut SERIAL: Serial = unsafe { core::mem::zeroed() };

pub struct Serial {
    port: Port,
}

impl Serial {
    pub fn new() -> Self {
        let port = Port::alloc(0x3f8, 6).expect("Couldn't allocate port for serial");
        port.out_b(2, 0x01);
        Serial {
            port,
        }
    }

    pub fn write(&self, ch: u8) {
        self.port.out_b(0, ch);
        if ch == b'\n' {
            self.port.out_b(0, b'\r');
        }
    }

    pub fn read(&self) -> Option<u8> {
        (self.port.in_b(5) & 0x1 != 0).then(|| self.port.in_b(0))
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
