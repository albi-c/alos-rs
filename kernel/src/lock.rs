use crate::cpu;

struct InterruptGuard(bool);

impl !Send for InterruptGuard {}

impl InterruptGuard {
    pub fn new() -> Self {
        InterruptGuard(cpu::query_and_disable_interrupts())
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.0 {
            cpu::enable_interrupts();
        }
    }
}
