use core::arch::asm;
use macros::{interrupt_handlers, interrupt_handlers_arr};
use crate::cpu;
use crate::lock::Lock;
use crate::ports::Port;

#[derive(Debug)]
#[allow(unused)]
#[repr(C, align(16))]
struct IdtEntry {
    offset_1: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_2: u16,
    offset_3: u32,
    zero: u32,
}

impl IdtEntry {
    fn new(offset: u64, trap: bool, i: usize) -> Self {
        IdtEntry {
            offset_1: offset as u16,
            selector: 0x08,
            // NMI, double fault, MCE have a special stack
            ist: if i == 2 || i == 8 || i == 18 { 0x1 } else { 0x0 },
            type_attr: if trap { 0x8f } else { 0x8e },
            offset_2: (offset >> 16) as u16,
            offset_3: (offset >> 32) as u32,
            zero: 0x0,
        }
    }
}

#[derive(Debug)]
#[allow(unused)]
#[repr(packed)]
struct Info {
    size: u16,
    idt: *const IdtEntry,
}

impl Info {
    fn new(idt: &[IdtEntry]) -> Self {
        Info {
            size: (idt.len() * size_of::<IdtEntry>()) as u16 - 1,
            idt: idt.as_ptr(),
        }
    }

    unsafe fn set(&self) {
        unsafe {
            asm!(
                "lidt [{0}]",
                in(reg) self as *const Info,
            );
        }
    }
}

type AsmHandler = unsafe extern "C" fn() -> ();

#[derive(Debug, Clone)]
pub struct IrqContext {
    pub irq: u16,
    pub flags: u64,
}

#[derive(Debug, Clone)]
pub struct ExcContext {
    pub exc: u16,
    pub error: u64,
    pub address: u64,
    pub instruction: u64,
    pub user: bool,
    pub flags: u64,
}

pub type IrqHandler = fn(IrqContext) -> ();
pub type ExcHandler = fn(ExcContext) -> ();

static mut IDT: [IdtEntry; 256] = unsafe { core::mem::zeroed() };
static mut HANDLERS: [u64; 256] = unsafe { core::mem::zeroed() };
static HANDLERS_REPLACEABLE: Lock<[bool; 256]> = Lock::new(unsafe { core::mem::zeroed() });

static mut ASM_IRQ_HANDLER_TABLE: [u64; 256] = unsafe { core::mem::zeroed() };
interrupt_handlers!();

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct InterruptStackFrame {
    pub ip: u64,
    pub cs: u16,
    _pad0: [u8; 6],
    pub flags: u64,
    pub sp: u64,
    pub ss: u16,
    _pad1: [u8; 6],
}

fn default_irq_handler(_: IrqContext) {}
fn default_exc_handler(_: ExcContext) {}

const USER_CS: u16 = 0x20 | 0x3;

#[unsafe(no_mangle)]
pub extern "C" fn irq_handler(frame: &InterruptStackFrame, irq: u16) {
    let user = frame.cs == USER_CS;
    if user {
        unsafe { asm!("swapgs", options(nostack, nomem, preserves_flags)); }
    }
    let ctx = IrqContext {
        irq,
        flags: frame.flags,
    };
    (unsafe { core::mem::transmute::<_, IrqHandler>(HANDLERS[irq as usize]) })(ctx);
    if user {
        unsafe { asm!("swapgs", options(nostack, nomem, preserves_flags)); }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn exc_handler(frame: &InterruptStackFrame, exc: u16, error_code: u64) {
    let cr2: u64;
    unsafe { asm!("mov {0}, cr2", "mov cr2, {1}", out(reg) cr2, in(reg) 0u64) };
    let user = frame.cs == USER_CS;
    if user {
        unsafe { asm!("swapgs", options(nostack, nomem, preserves_flags)); }
    }
    let ctx = ExcContext {
        exc,
        error: error_code,
        address: cr2,
        instruction: frame.ip,
        user,
        flags: frame.flags,
    };
    (unsafe { core::mem::transmute::<_, ExcHandler>(HANDLERS[exc as usize]) })(ctx);
    if user {
        unsafe { asm!("swapgs", options(nostack, nomem, preserves_flags)); }
    }
}

pub fn init(mask: u16) {
    unsafe {
        ASM_IRQ_HANDLER_TABLE = interrupt_handlers_arr!();
    }

    let port1 = Port::alloc(0x20, 2)
        .expect("Failed to allocate port 1 for interrupts");
    let port2 = Port::alloc(0xa0, 2)
        .expect("Failed to allocate port 2 for interrupts");

    for i in  0..256 {
        unsafe {
            HANDLERS[i] = if i < 32 {
                core::mem::transmute(default_exc_handler as ExcHandler)
            } else {
                core::mem::transmute(default_irq_handler as IrqHandler)
            };

            IDT[i] = IdtEntry::new(ASM_IRQ_HANDLER_TABLE[i], i < 32, i);
        };
    }

    HANDLERS_REPLACEABLE.write().fill(true);

    #[allow(static_mut_refs)]
    let info = unsafe { Info::new(&IDT) };

    cpu::disable_interrupts();

    unsafe { info.set() };

    port1.out_b(0, 0x11);
    port2.out_b(0, 0x11);
    port1.out_b(1, 0x20);
    port2.out_b(1, 0x28);
    port1.out_b(1, 0x04);
    port2.out_b(1, 0x02);
    port1.out_b(1, 0x01);
    port2.out_b(1, 0x01);
    port1.out_b(1, mask as u8);
    port2.out_b(1, (mask >> 8) as u8);

    cpu::enable_interrupts();
}

fn attach(irq: usize, handler: u64, replaceable: bool) -> bool {
    let mut handlers_replaceable = HANDLERS_REPLACEABLE.write();
    if !handlers_replaceable[irq] {
        false
    } else {
        handlers_replaceable[irq] = replaceable;
        unsafe {
            HANDLERS[irq] = handler;
        }
        true
    }
}

pub fn attach_irq(irq: u16, handler: IrqHandler, replaceable: bool) -> bool {
    assert!(irq >= 32, "Interrupts below ID 32 are exceptions");
    attach(irq as usize, unsafe { core::mem::transmute(handler) }, replaceable)
}
pub fn attach_exc(exc: u16, handler: ExcHandler, replaceable: bool) -> bool {
    assert!(exc < 32, "Interrupts from ID 32 are not exceptions");
    attach(exc as usize, unsafe { core::mem::transmute(handler) }, replaceable)
}
