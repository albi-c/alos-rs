use core::arch::asm;
use core::sync::atomic::{compiler_fence, Ordering};

#[inline(always)]
pub fn disable_interrupts() {
    unsafe {
        asm!(
            "cli",
            options(nomem, nostack)
        );
    }
    compiler_fence(Ordering::SeqCst);
}

#[inline(always)]
pub fn query_and_disable_interrupts() -> bool {
    let flags: usize;
    unsafe {
        asm!(
            "pushf",
            "pop {}",
            "cli",
            out(reg) flags,
            options(nomem)
        );
    }
    compiler_fence(Ordering::SeqCst);
    flags & 0x200 != 0
}

#[inline(always)]
pub fn enable_interrupts() {
    compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!(
            "sti",
            options(nomem, nostack)
        );
    }
}

pub fn hcf() -> ! {
    disable_interrupts();
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[inline(always)]
pub fn msr_read(msr: u32) -> u64 {
    let eax: u32;
    let edx: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") eax,
            out("edx") edx,
            options(nomem, nostack),
        )
    }
    ((eax as u64) << 32) | edx as u64
}

#[inline(always)]
pub fn msr_write(msr: u32, val: u64) {
    let eax: u32 = (val >> 32) as u32;
    let edx: u32 = val as u32;
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") eax,
            in("edx") edx,
            options(nomem, nostack),
        )
    }
}
