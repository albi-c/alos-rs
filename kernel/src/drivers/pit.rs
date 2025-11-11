use core::sync::atomic::{AtomicU64, Ordering};
use crate::{driver, interrupts, println};
use crate::interrupts::IrqContext;
use crate::ports::Port;

// 1/x seconds
const I_REQ_TIME_STEP: u64 = 1000;
const FREQUENCY: u64 = 1193181;

const DIVISOR: u16 = (FREQUENCY / I_REQ_TIME_STEP) as u16;
const PER_SECOND: u64 = FREQUENCY / DIVISOR as u64;
const US_PER_TICK: u64 = 1000000 / I_REQ_TIME_STEP;

static TICKS: AtomicU64 = AtomicU64::new(0);

fn interrupt_handler(_ctx: IrqContext) {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn init() {
    assert!(interrupts::attach_irq(32, interrupt_handler, true),
            "Failed to initialize PIT interrupts");
    let port = Port::alloc(0x40, 4)
        .expect("Failed to allocate ports for PIT");
    
    port.out_b(3, 0b00110100);
    port.out_b(0, DIVISOR as u8);
    port.out_b(0, (DIVISOR >> 8) as u8);
    
    TICKS.store(0, Ordering::Relaxed);
}

driver!("PIT", || println!("PIT initialized"), ["serial", "PIT"]);
