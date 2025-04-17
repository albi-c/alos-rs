use core::arch::asm;

#[inline(always)]
pub fn disable_interrupts() {
    unsafe {
        asm!(
            "cli"
        );
    }
}

#[inline(always)]
pub fn enable_interrupts() {
    unsafe {
        asm!(
            "sti"
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
