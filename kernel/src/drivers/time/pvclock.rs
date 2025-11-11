use core::arch::x86_64::{__cpuid, _rdtsc};
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use crate::cpu::msr_write;
use crate::{memory, println, volatile_struct};

const CPUID_LEAF: u32 = 0x4000001;
const MSR: u32 = 0x4b564d01;

volatile_struct! { TimeInfo;
    version: u32,
    _pad0: u32,
    tsc_timestamp: u64,
    system_time: u64,
    tsc_to_system_mul: u32,
    tsc_shift: i8,
    flags: u8,
    _pad1: u16,
}

static TIME_INFO: AtomicPtr<TimeInfo> = AtomicPtr::new(core::ptr::null_mut());
static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    println!("pvclock: cpuid({}) = 0x{:x}", CPUID_LEAF, unsafe { __cpuid(CPUID_LEAF) }.eax);
    if unsafe { __cpuid(CPUID_LEAF) }.eax & (1 << 3) == 0 {
        return;
    }
    if INITIALIZED.swap(true, Ordering::Relaxed) {
        let addr = memory::alloc_page().expect("failed to allocate memory for time info");
        msr_write(MSR, addr as u64);
        TIME_INFO.store(memory::hhdm::as_ptr(addr), Ordering::Relaxed);
    }
}

pub fn get_nanoseconds() -> Option<u64> {
    unsafe { TIME_INFO.load(Ordering::Relaxed).as_ref() }.map(|info| info.get_nanoseconds())
}

impl TimeInfo {
    pub fn get_nanoseconds(&self) -> u64 {
        loop {
            let version = self.version.load();
            if version & 1 != 0 {
                continue;
            }
            let time = unsafe { _rdtsc() } - self.tsc_timestamp.load();
            let shift = self.tsc_shift.load();
            let time = if shift >= 0 {
                time << shift
            } else {
                time >> -shift
            };
            let time = ((time * self.tsc_to_system_mul.load() as u64) >> 32) + self.system_time.load();
            if version == self.version.load() {
                return time;
            }
        }
    }
}
