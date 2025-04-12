use core::arch::asm;

#[derive(Copy, Clone)]
#[allow(unused)]
#[repr(packed)]
struct GDT {
    limit1: u16,
    base1: u16,
    base2: u8,
    access: u8,
    limit2_flags: u8,
    base3: u8,
}

impl GDT {
    const fn new(base: u32, limit: u32, access: u8, flags: u8) -> Self {
        GDT {
            limit1: limit as u16,
            base1: base as u16,
            base2: (base >> 16) as u8,
            access,
            limit2_flags: ((limit >> 16) as u8 & 0xf) | flags << 4,
            base3: (base >> 24) as u8,
        }
    }

    const fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

#[derive(Copy, Clone)]
#[allow(unused)]
#[repr(packed)]
struct Info {
    size: u16,
    gdt: *const GDT,
}

impl Info {
    const fn default() -> Self {
        Info {
            size: 0,
            gdt: 0 as *const GDT,
        }
    }

    fn new(gdt: &[GDT]) -> Self {
        Info {
            size: gdt.len() as u16 - 1,
            gdt: gdt.as_ptr(),
        }
    }

    fn get() -> Self {
        let mut info = Info::default();
        unsafe {
            asm!(
                "sgdt [{0}]",
                in(reg) &raw mut info,
            )
        }
        info
    }

    unsafe fn set_with_segments(&self, ss: usize, cs: usize) {
        unsafe {
            asm!(
                "lgdt [rdi]",
                "cli",
                "mov rdi, rsp",
                "push rsi",
                "push rdi",
                "pushf",
                "or qword ptr [rsp], 0x200",
                "push rdx",
                "lea rdi, [2f]",
                "push rdi",
                "iretq",
                "2:",
                in("rdi") self as *const Info,
                in("rsi") ss,
                in("rdx") cs,
            )
        }
    }

    unsafe fn as_slice(&self) -> &[GDT] {
        unsafe { core::slice::from_raw_parts(self.gdt, self.size as usize + 1) }
    }
}

static mut CORE_0_GDT: [GDT; 7] = [GDT::default(); 7];

pub fn init() {
    let info = Info::get();
    unsafe {
        info.set_with_segments(0x30, 0x28);
    };

    unsafe {
        CORE_0_GDT[1] = GDT::new(0, 0xfffff, 0x9b, 0xa);
        CORE_0_GDT[2] = GDT::new(0, 0xfffff, 0x93, 0xc);
        CORE_0_GDT[3] = GDT::new(0, 0xfffff, 0xfb, 0xa);
        CORE_0_GDT[4] = GDT::new(0, 0xfffff, 0xf3, 0xc);
        #[allow(static_mut_refs)]
        let info = Info::new(&CORE_0_GDT);
        info.set_with_segments(0x10, 0x8);
    };
}
