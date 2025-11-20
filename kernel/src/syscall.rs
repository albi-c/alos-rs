use core::arch::global_asm;
use crate::cpu::{msr_read, msr_write};
use crate::{print, println};
use crate::memory::address::UserVirtAddr;
use crate::task::sched_exit;

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
static mut SYSCALL_TABLE: [u64; 3] = [0; _];
#[unsafe(no_mangle)]
#[expect(static_mut_refs)]
static SYSCALL_TABLE_LENGTH: usize = unsafe { SYSCALL_TABLE.len() };

extern "C" fn syscall_debug(p1: u64, p2: u64, p3: u64, p4: u64, p5: u64, p6: u64) -> u64 {
    println!("[syscall: debug] {:#x} {:#x} {:#x} {:#x} {:#x} {:#x}", p1, p2, p3, p4, p5, p6);
    0
}

extern "C" fn syscall_write(file: i32, buf: UserVirtAddr, count: usize) -> i64 {
    if count > isize::MAX as usize {
        println!("[syscall: write] count too large");
        -4i64
    } else if file != 1 && file != 2 {
        println!("[syscall: write] invalid file");
        -1i64
    } else {
        if count == 0 {
            return 0;
        }
        let buf = if let Some(buf) = buf.check_read(count) {
            buf.as_mut_ptr()
        } else {
            println!("[syscall: write] invalid buffer");
            return -3i64
        };
        let buf = unsafe { core::slice::from_raw_parts(buf, count) };
        if let Ok(string) = core::str::from_utf8(buf) {
            print!("{}", string);
            count as i64
        } else {
            println!("[syscall: write] invalid utf8");
            -2i64
        }
    }
}

extern "C" fn syscall_exit(code: i32) -> ! {
    println!("[syscall: exit] code {}", code);
    sched_exit()
}

macro_rules! syscall {
    ($n:literal, $f:ident) => {
        unsafe { SYSCALL_TABLE[$n] = $f as u64 };
    };
}

pub fn init() {
    syscall!(0, syscall_debug);
    syscall!(1, syscall_write);
    syscall!(2, syscall_exit);

    msr_write(MSR_STAR, ((0x10 | 0x3) << 48) | (0x8 << 32));
    msr_write(MSR_LSTAR, _syscall_entry as u64);
    msr_write(MSR_CSTAR, 0);
    msr_write(MSR_SFMASK, 0x200);

    msr_write(MSR_EFER, msr_read(MSR_EFER) | 0x1);
}
