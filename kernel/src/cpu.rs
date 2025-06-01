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
