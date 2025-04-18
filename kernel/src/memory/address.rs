pub const PAGE_SHIFT: u8 = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

pub const LARGE_PAGE_SHIFT: u8 = 21;
pub const LARGE_PAGE_SIZE: usize = 1 << LARGE_PAGE_SHIFT;
pub const LARGE_PAGE_MASK: usize = LARGE_PAGE_SIZE - 1;

#[inline(always)]
pub fn is_page_aligned(addr: usize) -> bool {
    addr & PAGE_MASK == 0
}

#[inline(always)]
pub fn page_align_up(addr: usize) -> usize {
    (addr + PAGE_MASK) & !PAGE_MASK
}

#[inline(always)]
pub fn page_align_down(addr: usize) -> usize {
    addr & !PAGE_MASK
}

#[inline(always)]
pub fn page_count_up(addr: usize) -> usize {
    (addr + PAGE_MASK) >> PAGE_SHIFT
}

#[inline(always)]
pub fn page_count_down(addr: usize) -> usize {
    addr >> PAGE_SHIFT
}

#[inline(always)]
pub fn is_large_page_aligned(addr: usize) -> bool {
    addr & LARGE_PAGE_MASK == 0
}

#[inline(always)]
pub fn large_page_align_up(addr: usize) -> usize {
    (addr + LARGE_PAGE_MASK) & !LARGE_PAGE_MASK
}

#[inline(always)]
pub fn large_page_align_down(addr: usize) -> usize {
    addr & !LARGE_PAGE_MASK
}

#[inline(always)]
pub fn large_page_count_up(addr: usize) -> usize {
    (addr + LARGE_PAGE_MASK) >> LARGE_PAGE_SHIFT
}

#[inline(always)]
pub fn large_page_count_down(addr: usize) -> usize {
    addr >> LARGE_PAGE_SHIFT
}

