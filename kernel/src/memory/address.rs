pub const PAGE_SHIFT: u8 = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

#[inline]
pub fn page_align_up(addr: usize) -> usize {
    (addr + PAGE_MASK) & !PAGE_MASK
}

#[inline]
pub fn page_align_down(addr: usize) -> usize {
    addr & !PAGE_MASK
}

#[inline]
pub fn page_count_up(addr: usize) -> usize {
    (addr + PAGE_MASK) >> PAGE_SHIFT
}

#[inline]
pub fn page_count_down(addr: usize) -> usize {
    addr >> PAGE_SHIFT
}
