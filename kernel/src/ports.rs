use core::arch::asm;
use crate::{linker_set_declare, linker_set_slice};
use crate::lock::Lock;

const NUM_PORTS: usize = 1 << 16;
type PortMap = [u8; NUM_PORTS / 8];
static PORT_MAP: Lock<PortMap> = Lock::new([0; NUM_PORTS / 8]);

#[macro_export]
macro_rules! const_port {
    ($name:ident: $start:literal, $end:literal) => {
        paste::paste! {
            crate::linker_set_item!(const_ports, [<_CONST_PORT_ $name>]: (u16, u16) = ($start, $end));
            static $name: crate::ports::Port = unsafe { crate::ports::Port::new($start, $end) };
        }
    };
}

linker_set_declare!(const_ports, (u16, u16));
const_port!(_PLACEHOLDER: 0, 0);

pub fn init() {
    let mut map = PORT_MAP.write();
    for &(start, length) in linker_set_slice!(const_ports) {
        assert!(Port::try_alloc(start, length, &mut map), "failed to allocate constant port 0x{:x}, {}", start, length);
    }
}

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
    pub const unsafe fn new(start: u16, length: u16) -> Self {
        Self { start, length }
    }

    pub const fn empty() -> Self {
        Self { start: 0, length: 0 }
    }

    fn try_alloc(start: u16, length: u16, map: &mut PortMap) -> bool {
        for i in start..(start + length) {
            if map[(i >> 3) as usize] & (1 << (i & 0x7)) != 0 {
                return false;
            }
        }
        for i in start..(start + length) {
            map[(i >> 3) as usize] |= 1 << (i & 0x7);
        }
        true
    }

    pub fn alloc(start: u16, length: u16) -> Option<Self> {
        let mut map = PORT_MAP.write();
        Self::try_alloc(start, length, &mut map).then_some(Self { start, length })
    }

    pub fn dealloc(&mut self) {
        let mut map = PORT_MAP.write();
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
        assert!(offset < self.length);
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
        assert!(offset + 1 < self.length);
        unsafe {
            asm!(
                "out dx, ax",
                in("dx") self.start + offset,
                in("ax") value,
            );
        }
    }
    pub fn in_w(&self, offset: u16) -> u16 {
        assert!(offset + 1 < self.length);
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
        assert!(offset + 3 < self.length);
        unsafe {
            asm!(
                "out dx, eax",
                in("dx") self.start + offset,
                in("eax") value,
            );
        }
    }
    pub fn in_d(&self, offset: u16) -> u32 {
        assert!(offset + 3 < self.length);
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
        assert!(offset + 7 < self.length);
        unsafe {
            asm!(
                "out dx, rax",
                in("dx") self.start + offset,
                in("rax") value,
            );
        }
    }
    pub fn in_q(&self, offset: u16) -> u64 {
        assert!(offset + 7 < self.length);
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
