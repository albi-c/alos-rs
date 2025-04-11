use core::arch::asm;
use spin::Mutex;

const NUM_PORTS: usize = 1 << 16;
static PORT_MAP: Mutex<[u8; NUM_PORTS / 8]> = Mutex::new([0; NUM_PORTS / 8]);

#[derive(Debug)]
pub struct Port {
    start: u16,
    length: u16,
}

impl Drop for Port {
    fn drop(&mut self) {
        self.dealloc();
    }
}

#[allow(unused)]
impl Port {
    const fn new(start: u16, length: u16) -> Self {
        Port { start, length }
    }

    pub const fn empty() -> Self {
        Self::new(0, 0)
    }

    pub fn alloc(start: u16, length: u16) -> Option<Self> {
        let mut map = PORT_MAP.lock();
        for i in start..(start + length) {
            if map[(i >> 3) as usize] & (i as u8 & 0x7) != 0 {
                return None;
            }
        }
        for i in start..(start + length) {
            map[(i >> 3) as usize] |= i as u8 & 0x7;
        }
        Some(Self::new(start, length))
    }

    pub fn dealloc(&mut self) {
        let mut map = PORT_MAP.lock();
        for i in self.start..(self.start + self.length) {
            map[(i >> 3) as usize] &= !(i as u8 & 0x7);
        }
        self.start = 0;
        self.length = 0;
    }

    pub fn out_b(&self, offset: u16, value: u8) {
        assert!(offset < self.length);
        unsafe {
            asm!(
                "out dx, al",
                in("dx") self.start + offset,
                in("al") value,
            );
        }
    }
    pub fn in_b(&self, offset: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!(
                "in al, dx",
                in("dx") self.start + offset,
                out("al") value,
            )
        }
        value
    }

    pub fn out_w(&self, offset: u16, value: u16) {
        unsafe {
            asm!(
                "out dx, ax",
                in("dx") self.start + offset,
                in("ax") value,
            );
        }
    }
    pub fn in_w(&self, offset: u16) -> u16 {
        let value: u16;
        unsafe {
            asm!(
                "in ax, dx",
                in("dx") self.start + offset,
                out("ax") value,
            )
        }
        value
    }

    pub fn out_d(&self, offset: u16, value: u32) {
        unsafe {
            asm!(
                "out dx, eax",
                in("dx") self.start + offset,
                in("eax") value,
            );
        }
    }
    pub fn in_d(&self, offset: u16) -> u32 {
        let value: u32;
        unsafe {
            asm!(
                "in eax, dx",
                in("dx") self.start + offset,
                out("eax") value,
            )
        }
        value
    }

    pub fn out_q(&self, offset: u16, value: u64) {
        unsafe {
            asm!(
                "out dx, rax",
                in("dx") self.start + offset,
                in("rax") value,
            );
        }
    }
    pub fn in_q(&self, offset: u16) -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "in rax, dx",
                in("dx") self.start + offset,
                out("rax") value,
            )
        }
        value
    }
}
