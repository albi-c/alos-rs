use core::arch::x86_64::__cpuid;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};
use crate::cpu;
use crate::drivers::time::pit;
use crate::lock::InterruptGuard;
use crate::memory::address::PhysAddrPageAligned;
use crate::memory::mmio::MMIO;

const CPUID_LEAF: u32 = 0x1;
const MSR_APIC_BASE: u32 = 0x0000_001b;

const LVT_MASKED: u32 = 1 << 16;
const LVT_TIMER_PERIODIC: u32 = 1 << 17;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum Reg {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    ArbitrationPriority = 0x90,
    ProcessorPriority = 0xa0,
    EndOfInterrupt = 0xb0,
    RemoteRead = 0xc0,
    LogicalDestination = 0xd0,
    DestinationFormat = 0xe0,
    SpuriousInterruptVector = 0xf0,
    InService0 = 0x100,
    InService1 = 0x110,
    InService2 = 0x120,
    InService3 = 0x130,
    InService4 = 0x140,
    InService5 = 0x150,
    InService6 = 0x160,
    InService7 = 0x170,
    TriggerMode0 = 0x180,
    TriggerMode1 = 0x190,
    TriggerMode2 = 0x1a0,
    TriggerMode3 = 0x1b0,
    TriggerMode4 = 0x1c0,
    TriggerMode5 = 0x1d0,
    TriggerMode6 = 0x1e0,
    TriggerMode7 = 0x1f0,
    InterruptRequest0 = 0x200,
    InterruptRequest1 = 0x210,
    InterruptRequest2 = 0x220,
    InterruptRequest3 = 0x230,
    InterruptRequest4 = 0x240,
    InterruptRequest5 = 0x250,
    InterruptRequest6 = 0x260,
    InterruptRequest7 = 0x270,
    ErrorStatus = 0x280,
    LvtCorrectedMachineCheck = 0x2f0,
    InterruptCommand0 = 0x300,
    InterruptCommand1 = 0x310,
    LvtTimer = 0x320,
    LvtThermalSensor = 0x330,
    LvtPerfCounters = 0x340,
    LvtLint0 = 0x350,
    LvtLint1 = 0x360,
    LvtErrorStatus = 0x370,
    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivideConfiguration = 0x3e0,
}

static REGS: AtomicPtr<u32> = AtomicPtr::new(core::ptr::null_mut());

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
struct Registers(NonNull<u32>);

impl Registers {
    fn try_get() -> Option<Self> {
        NonNull::new(REGS.load(Ordering::Relaxed)).map(Self)
    }
    fn get() -> Self {
        Self::try_get().expect("lapic: attempted to get registers before initialization")
    }

    fn read(&self, reg: Reg) -> u32 {
        unsafe { self.0.byte_add(reg as usize).read_volatile() }
    }
    fn write(&self, reg: Reg, val: u32) {
        unsafe { self.0.byte_add(reg as usize).write_volatile(val) };
    }
}

fn pit_prepare_sleep(ticks: u64) -> u64 {
    let start_tick = pit::get_ticks() + 2;
    let end_tick = start_tick + ticks;
    while pit::get_ticks() < start_tick {}
    end_tick
}

fn pit_sleep(end: u64) {
    while pit::get_ticks() < end {}
}

fn measure_ticks_per_pit(regs: Registers, wait: u32) -> u32 {
    let end = pit_prepare_sleep(wait as u64);
    regs.write(Reg::TimerInitialCount, u32::MAX);
    pit_sleep(end);
    regs.write(Reg::LvtTimer, LVT_MASKED);

    (u32::MAX - regs.read(Reg::TimerCurrentCount)) / wait
}

pub fn init() {
    if __cpuid(CPUID_LEAF).edx & (1 << 9) == 0 {
        panic!("Local APIC is not supported");
    }

    let base = cpu::msr_read(MSR_APIC_BASE) & !0x1000_0000_0000_0fff;
    let mmio = unsafe { MMIO::new(
        PhysAddrPageAligned::new(base as usize).unwrap(),
        1.into(), true, false) };
    let (regs, _) = mmio.leak();
    REGS.store(regs.cast().as_ptr(), Ordering::SeqCst);

    let regs = Registers::get();
    regs.write(Reg::TimerDivideConfiguration, 0b1011);

    pit_sleep(pit_prepare_sleep(150));

    let mut measurements: [u32; 10] = core::array::from_fn(
        |_| measure_ticks_per_pit(regs, 5));
    measurements.sort_unstable();
    let ticks = measurements[4].midpoint(measurements[5]);

    InterruptGuard::with(|| {
        regs.write(Reg::TimerDivideConfiguration, 0b1011);
        regs.write(Reg::LvtTimer, 0x20 | LVT_TIMER_PERIODIC);
        regs.write(Reg::TimerInitialCount, ticks);
        pit::disable();
    });
}

pub fn eoi() {
    Registers::try_get().map(|regs| regs.write(Reg::EndOfInterrupt, 0));
}
