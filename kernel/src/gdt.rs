use core::arch::asm;

#[derive(Debug, Clone)]
#[allow(unused)]
#[repr(C, align(8))]
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

#[derive(Debug, Clone)]
#[allow(unused)]
#[repr(packed)]
struct Info {
    size: u16,
    gdt: *const GDT,
}

impl Info {
    fn new(gdt: &[GDT]) -> Self {
        Info {
            size: (gdt.len() * size_of::<GDT>()) as u16 - 1,
            gdt: gdt.as_ptr(),
        }
    }

    unsafe fn set_with_segments(&self, ss: usize, cs: usize) {
        unsafe {
            asm!(
                "cli",
                "lgdt [rdi]",
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
                inout("rdi") self as *const Info => _,
                in("rsi") ss,
                in("rdx") cs,
            );
        }
    }
}

const GDT_SIZE: usize = 5;
static mut CORE_0_GDT: [GDT; GDT_SIZE] = unsafe { core::mem::zeroed() };

pub fn init() {
    unsafe {
        CORE_0_GDT = [
            GDT::default(),
            GDT::new(0, 0xffffffff, 0x9b, 0xa),
            GDT::new(0, 0xffffffff, 0x93, 0xa),
            GDT::new(0, 0xffffffff, 0xfb, 0xa),
            GDT::new(0, 0xffffffff, 0xf3, 0xa),
        ];
        #[allow(static_mut_refs)]
        let info = Info::new(&CORE_0_GDT);
        info.set_with_segments(0x10, 0x08);
    };
}
