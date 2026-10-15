#![no_std]
#![no_main]
#![feature(maybe_uninit_array_assume_init)]
#![feature(negative_impls)]
#![feature(btree_cursors)]
#![feature(slice_from_ptr_range)]
#![feature(abi_x86_interrupt)]
#![feature(step_trait)]
#![feature(linked_list_cursors)]

extern crate core;
extern crate alloc;

mod drivers;
mod ports;
mod print;
mod log;
mod gdt;
mod interrupts;
mod cpu;
mod memory;
mod lock;
mod linker_set;
mod volatile;
mod acpi;
mod task;
mod core_local;
mod syscall;
mod elf_loader;
mod pcie;
mod backtrace;
mod device;

use alloc::boxed::Box;
use compact_str::CompactString;
use limine::BaseRevision;
use limine::request::{ExecutableFileRequest, FramebufferRequest, RequestsEndMarker, RequestsStartMarker};
use drivers::time::pit;
use crate::device::{DeviceTransfer, BLOCK_DEVICE_REGISTRY};
use crate::drivers::serial;
use crate::elf_loader::ElfError;
use crate::memory::MemorySpace;
use crate::task::{sched_yield, Task};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

logger!("Kernel");

fn backtrace_addr_print(ip: usize) {
    error!("  0x{:016x}", ip);
}

fn exception_handler(ctx: interrupts::ExcContext) {
    if ctx.user {
        warning!("User exception: {:#x?}", ctx);
        panic!("User exception");
    } else {
        error!("Kernel exception backtrace:");
        backtrace::print_from(backtrace_addr_print, ctx.bp, ctx.sp, ctx.instruction);
        if ctx.exc == 0x8 {
            error!("Double fault - possible page fault at instruction {:#x}", ctx.address);
            error!("Double fault backtrace:");
            backtrace::print_from(backtrace_addr_print, ctx.bp, ctx.sp, ctx.address);
        }
        error!("Kernel exception: {:#x?}", ctx);
        panic_no_message();
    }
}
fn irq_handler(ctx: interrupts::IrqContext) {
    debug!("IRQ: {:#x?}", ctx);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    cpu::disable_interrupts();

    serial::init();
    ports::init();

    assert!(BASE_REVISION.is_supported());
    
    gdt::init();

    interrupts::init(0x0000);
    for i in 0..256 {
        if i < 32 {
            interrupts::attach_exc(i, exception_handler, true);
        } else{
            interrupts::attach_irq(i, irq_handler, true);
        }
    }

    pit::init();
    let memory_values = memory::init();
    core_local::init();

    // call on all cores when SMP is added
    memory::init_core_local(memory_values);
    gdt::init_core_local();

    if let Some(file) = EXECUTABLE_FILE_REQUEST.get_response().map(|res| res.file()) {
        backtrace::set_executable(unsafe { core::slice::from_raw_parts(file.addr(), file.size() as usize) });
    } else {
        warning!("no executable file response - addr2line will be unavailable");
    }

    let acpi_tables = acpi::init();
    pcie::init(&acpi_tables);

    syscall::init();

    drivers::init();

    task::init(kernel_main_task, Box::new(CompactString::new("hello, world!")));
}

extern "C" fn user_start_task(_: Box<()>) -> ! {
    debug!("User start task entered");

    let program_data = include_bytes!("../../test_program.elf");
    let func_addr = match elf_loader::load_elf(program_data) {
        Ok(addr) => addr,
        Err(err) => match err {
            ElfError::Lib(err) => panic!("ELF error: {}", err),
            ElfError::Err(msg) => panic!("ELF error: {}", msg),
        }
    };

    task::switch_to_ring_3(func_addr as u64)
}

extern "C" fn kernel_side_task(_: Box<()>) -> ! {
    debug!("Side task entered");

    loop {
        sched_yield();
    }
}

extern "C" fn kernel_main_task(msg: Box<CompactString>) -> ! {
    debug!("Main task entered: {}", msg);

    let task = Task::new_kernel(
        Some((kernel_side_task, Box::new(()))), CompactString::new("side"));
    let id = task.add_to_tasks(true);
    println!("Side task id: {}", id);
    sched_yield();

    debug!("Opening file");
    let device = BLOCK_DEVICE_REGISTRY.find(|_| true)
        .expect("kernel: no block device");
    assert!(device.block_count() > 0, "kernel: block device is empty");
    assert_eq!(device.block_size(), 512, "kernel: invalid device block size");
    let transfer = DeviceTransfer::new(512);
    let (transfer, count) = device.read(0, transfer)
        .wait()
        .expect("kernel: failed to read from block device");
    assert_eq!(count, transfer.size(), "kernel: invalid block device read count");

    debug!("Main task continues");

    let space = MemorySpace::get().new_new_user();
    let task = Task::new_user(
        (user_start_task, Box::new(())), CompactString::new("user"), space, 1 << 16);
    let id = task.add_to_tasks(true);
    println!("User task id: {}", id);
    sched_yield();

    loop {
        while let Some(ch) = serial::read() {
            match ch {
                13 => println!(),
                27 => if serial::read() == Some(91) {
                    if let Some(ch) = serial::read() {
                        match ch {
                            65 => println!("Up"),
                            66 => println!("Down"),
                            67 => println!("Right"),
                            68 => println!("Left"),
                            _ => println!("Unknown escape sequence: {}", ch),
                        }
                    }
                }
                127 => print!("\x08 \x08"),
                _ => print!("{}", ch as char),
            }
        }
        sched_yield();
    }
}

unsafe extern "C" {
    static __eh_frame_hdr_start: u8;
    static __eh_frame_hdr_end: u8;
    static __eh_frame_start: u8;
    static __eh_frame_end: u8;
}

fn panic_no_message() -> ! {
    cpu::hcf()
}

fn handle_panic(info: &core::panic::PanicInfo) -> ! {
    error!("Panic backtrace:");
    backtrace::print(backtrace_addr_print);

    if let Some(location) = info.location() {
        error!("Panic: {} [{}:{}]", info.message(), location.file(), location.line());
    } else {
        error!("Panic: {}", info.message());
    }

    panic_no_message()
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    handle_panic(info)
}
