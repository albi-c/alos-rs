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

#[repr(C)]
struct HighGDT {
    base: u32,
    _res: u32,
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

    const fn new_double(base: u64, limit: u32, access: u8, flags: u8) -> [Self; 2] {
        let high: Self = unsafe { core::mem::transmute(HighGDT {
            base: (base >> 32) as u32,
            _res: 0,
        }) };
        let low = Self::new(base as u32, limit, access, flags);
        [low, high]
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
                "lgdt [{0}]",
                "mov {0}, rsp",
                "push {1}",
                "push {0}",
                "pushf",
                "or qword ptr [rsp], 0x200",
                "push {2}",
                "lea {0}, [2f]",
                "push {0}",
                "iretq",
                "2:",
                in(reg) self as *const Info,
                in(reg) ss,
                in(reg) cs,
            );
        }
    }
}

#[derive(Debug, Clone, Default)]
#[repr(packed)]
struct TSS {
    _res0: u32,
    rsp: [u64; 4],
    // _res1: u64,
    ist: [u64; 7],
    _res2: u64,
    _res3: u16,
    iopb: u16,
}

#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
struct StackAlignedByte(pub u8);

const GDT_SIZE: usize = 7;
static mut CORE_0_GDT: [GDT; GDT_SIZE] = unsafe { core::mem::zeroed() };
static mut CORE_0_TSS: TSS = unsafe { core::mem::zeroed() };
static mut CORE_0_EXC_STACK: [StackAlignedByte; 0x10000] = [StackAlignedByte(0); _];

pub fn init() {
    const {
        assert!(size_of::<TSS>() == 0x68);
        assert!(size_of::<GDT>() == size_of::<HighGDT>());
    }
    #[expect(static_mut_refs)]
    unsafe {
        let exc_stack = CORE_0_EXC_STACK.as_ptr().byte_add(CORE_0_EXC_STACK.len()) as u64;
        for i in 0..7 {
            CORE_0_TSS.ist[i] = exc_stack;
            if i < 4 {
                CORE_0_TSS.rsp[i] = exc_stack;
            }
        }
        let [tss_low, tss_high] = GDT::new_double(
            (&raw const CORE_0_TSS) as u64, (size_of::<TSS>() - 1) as u32, 0x89, 0x0);
        CORE_0_GDT = [
            GDT::default(),
            GDT::new(0, 0xffffffff, 0x9b, 0xa),
            GDT::new(0, 0xffffffff, 0x93, 0xa),
            GDT::new(0, 0xffffffff, 0xfb, 0xa),
            GDT::new(0, 0xffffffff, 0xf3, 0xa),
            tss_low,
            tss_high,
        ];
        let info = Info::new(&CORE_0_GDT);
        info.set_with_segments(0x10, 0x08);
        asm!("ltr {0:x}", in(reg) 0x28, options(nomem, nostack));
    };
}
