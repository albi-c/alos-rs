use core::arch::global_asm;
use crate::cpu::{msr_read, msr_write};
use crate::println;

const MSR_EFER: u32 = 0xc0000080;
const MSR_STAR: u32 = 0xc0000081;
const MSR_LSTAR: u32 = 0xc0000082;
const MSR_CSTAR: u32 = 0xc0000083;
const MSR_SFMASK: u32 = 0xc0000084;

global_asm!(include_str!("asm/syscall.asm"));
unsafe extern "C" {
    #[allow(improper_ctypes)]
    fn _syscall_entry();
}

#[unsafe(no_mangle)]
extern "C" fn syscall_entry(call_number: u64, p1: u64, p2: u64, p3: u64, p4: u64, p5: u64) -> u64 {
    println!("system call [{:}] {:#x} {:#x} {:#x} {:#x} {:#x}", call_number, p1, p2, p3, p4, p5);
    0
}

pub fn init() {
    msr_write(MSR_STAR, ((0x18 | 0x3) << 48) | (0x8 << 32));
    msr_write(MSR_LSTAR, _syscall_entry as u64);
    msr_write(MSR_CSTAR, 0);
    msr_write(MSR_SFMASK, 0x200);

    msr_write(MSR_EFER, msr_read(MSR_EFER) | 0x1);
}
