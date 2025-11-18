use core::sync::atomic::{AtomicU64, Ordering};
use crate::{const_port, interrupts, task};
use crate::interrupts::IrqContext;

// 1/x seconds
const I_REQ_TIME_STEP: u64 = 1000;
const FREQUENCY: u64 = 1193181;

const DIVISOR: u16 = (FREQUENCY / I_REQ_TIME_STEP) as u16;
const PER_SECOND: u64 = FREQUENCY / DIVISOR as u64;
const US_PER_TICK: u64 = 1000000 / I_REQ_TIME_STEP;

static TICKS: AtomicU64 = AtomicU64::new(0);

const_port!(PORT: 0x40, 4);

fn interrupt_handler(_ctx: IrqContext) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    task::time_tick();
}

pub fn init() {
    assert!(interrupts::attach_irq(32, interrupt_handler, true),
            "Failed to initialize PIT interrupts");
    
    PORT.out_b(3, 0b00110100);
    PORT.out_b(0, DIVISOR as u8);
    PORT.out_b(0, (DIVISOR >> 8) as u8);
    
    TICKS.store(0, Ordering::Relaxed);
}
