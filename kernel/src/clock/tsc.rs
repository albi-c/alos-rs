use core::arch::x86_64::{__cpuid, _rdtsc};
use core::hint::spin_loop;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::clock::hpet::HPET;
use crate::{debug, error, logger};
use crate::lock::InterruptGuard;

logger!("TSC");

static FREQUENCY: AtomicU64 = AtomicU64::new(0);
static FS_PER_TICK: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
pub fn get_ticks() -> u64 {
    unsafe { _rdtsc() }
}

#[inline(always)]
pub fn ticks_to_ns(ticks: u64) -> u64 {
    (ticks as u128 * get_femtoseconds_per_tick() as u128 / 1_000_000) as u64
}

#[inline(always)]
pub fn get_ns() -> u64 {
    ticks_to_ns(get_ticks())
}

#[inline(always)]
pub fn get_frequency() -> u64 {
    FREQUENCY.load(Ordering::Acquire)
}

#[inline(always)]
pub fn get_femtoseconds_per_tick() -> u64 {
    FS_PER_TICK.load(Ordering::Acquire)
}

#[inline]
pub fn wait_ns(ns: u64) {
    let start = get_ticks();
    let ticks = ns * 1_000_000 / get_femtoseconds_per_tick();
    while get_ticks() < start + ticks {
        spin_loop();
    }
}

pub fn init(hpet: &HPET) {
    if __cpuid(0x80000007).edx & (1 << 8) == 0 {
        error!("invariant tsc not supported");
        return;
    }

    // 1 ms
    let hpet_ticks_wait = 1_000_000_000_000 / hpet.per_tick_femtoseconds() as u64;
    debug!("waiting {hpet_ticks_wait} HPET ticks for calibration");

    let (diff, diff_hpet) = InterruptGuard::with(|| {
        let start = get_ticks();
        let start_hpet = hpet.get_ticks();

        while hpet.get_ticks() < start_hpet + hpet_ticks_wait {
            spin_loop();
        }

        let end = get_ticks();
        let end_hpet = hpet.get_ticks();

        (end - start, end_hpet - start_hpet)
    });

    assert_ne!(diff, 0, "tsc: no tsc ticks during calibration");
    assert_ne!(diff_hpet, 0, "tsc: no hpet ticks during calibration");

    let hpet_fs = diff_hpet * hpet.per_tick_femtoseconds() as u64;
    let fs_per_tsc_tick = hpet_fs / diff;
    assert_ne!(fs_per_tsc_tick, 0, "tsc: calculated per tick time is zero");
    let frequency = 1_000_000_000_000_000 / fs_per_tsc_tick;

    debug!("per tick: {fs_per_tsc_tick} fs = {} ps", fs_per_tsc_tick / 1000);
    debug!("frequency: {} kHz = {} MHz", frequency / 1000, frequency / 1_000_000);

    FREQUENCY.store(frequency, Ordering::Release);
    FS_PER_TICK.store(fs_per_tsc_tick, Ordering::Release);
}
