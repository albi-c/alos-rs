pub const PAGE_SHIFT: u8 = 12;
pub const PAGE_SIZE: u64 = 1 << PAGE_SHIFT;
pub const PAGE_MASK: u64 = PAGE_SIZE - 1;

#[inline]
pub fn page_align_up(addr: u64) -> u64 {
    (addr + PAGE_MASK) & !PAGE_MASK
}

#[inline]
pub fn page_align_down(addr: u64) -> u64 {
    addr & !PAGE_MASK
}
