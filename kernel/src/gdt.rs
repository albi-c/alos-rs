use core::arch::asm;

#[repr(packed)]
#[derive(Copy, Clone)]
#[allow(unused)]
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

#[repr(packed)]
#[derive(Copy, Clone)]
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

    unsafe fn set_with_segments(&self) {
        unsafe {
            asm!(
                "lgdt [rax]",
                "cli",
                "mov rax, rsp",
                "push 0x30",
                "push rax",
                "pushf",
                "or qword ptr [rsp], 0x200",
                "push 0x28",
                "push 2f",
                "iretq",
                "2:",
                in("rax") self as *const Info,
            )
        }
    }

    unsafe fn as_slice(&self) -> &[GDT] {
        unsafe { core::slice::from_raw_parts(self.gdt, self.size as usize + 1) }
    }
}

static mut CORE_0_GDT: [GDT; 11] = [GDT::default(); 11];

pub fn init() {
    let old_gdt = Info::get();

    unsafe {
        CORE_0_GDT[0..7].copy_from_slice(&old_gdt.as_slice()[0..7]);
        CORE_0_GDT[7..9].copy_from_slice(&old_gdt.as_slice()[5..7]);
        CORE_0_GDT[7].access |= 3 << 5;
        CORE_0_GDT[8].access |= 3 << 5;
        core::mem::swap(&mut CORE_0_GDT[7], &mut CORE_0_GDT[8]);
        #[allow(static_mut_refs)]
        let info = Info::new(&CORE_0_GDT);
        info.set_with_segments();
    };
}
