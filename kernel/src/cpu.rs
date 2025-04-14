use core::arch::asm;

#[inline]
pub fn disable_interrupts() {
    unsafe {
        asm!(
            "cli"
        );
    }
}

#[inline]
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
