use core::sync::atomic::{AtomicU64, Ordering};
use crate::{const_port, interrupts, lapic, task};
use crate::interrupts::IrqContext;

// 1/x seconds
const I_REQ_TIME_STEP: u64 = 1000;
const FREQUENCY: u64 = 1193181;

const DIVISOR: u16 = (FREQUENCY / I_REQ_TIME_STEP) as u16;
const PER_SECOND: u64 = FREQUENCY / DIVISOR as u64;
const US_PER_TICK: u64 = 1000000 / I_REQ_TIME_STEP;

static TICKS: AtomicU64 = AtomicU64::new(0);
static mut IRQ_HANDLER_DATA: [u8; 24] = [0; 24];

const_port!(PORT: 0x40, 4);

fn interrupt_handler(_ctx: IrqContext) {
    TICKS.fetch_add(1, Ordering::SeqCst);
    lapic::eoi();
    task::time_tick();
}

pub fn init() {
    #[expect(static_mut_refs)]
    interrupts::early_attach_irq(32, &interrupt_handler, unsafe { &mut IRQ_HANDLER_DATA });
    
    PORT.out_b(3, 0b00110100);
    PORT.out_b(0, DIVISOR as u8);
    PORT.out_b(0, (DIVISOR >> 8) as u8);
    
    TICKS.store(0, Ordering::SeqCst);
}

pub fn disable() {
    PORT.out_b(3, 0b00110100);
    PORT.out_b(0, 0);
    PORT.out_b(0, 0);
}

#[inline(always)]
pub fn get_ticks() -> u64 {
    TICKS.load(Ordering::SeqCst)
}

#[inline(always)]
pub fn get_ticks_per_second() -> u64 {
    PER_SECOND
}
#[inline(always)]
pub fn get_us_per_tick() -> u64 {
    US_PER_TICK
}
