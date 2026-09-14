#![no_std]
#![no_main]
#![feature(maybe_uninit_array_assume_init)]
#![feature(negative_impls)]
#![feature(unsafe_cell_access)]
#![feature(btree_cursors)]
#![feature(slice_from_ptr_range)]
#![feature(abi_x86_interrupt)]
#![feature(step_trait)]
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

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};
use drivers::time::pit;
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
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

logger!("Kernel");

fn exception_handler(ctx: interrupts::ExcContext) {
    if ctx.user {
        warning!("User exception: {:#x?}", ctx);
        panic!("User exception");
    } else {
        if ctx.exc == 0x8 {
            error!("Double fault - possible page fault at {:#x}", ctx.address);
        }
        panic!("Kernel exception: {:#x?}", ctx);
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

    // if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
    //     if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
    //         for i in 0..100_u64 {
    //             let pixel_offset = i * framebuffer.pitch() + i * 4;
    //
    //             unsafe {
    //                 framebuffer
    //                     .addr()
    //                     .add(pixel_offset as usize)
    //                     .cast::<u32>()
    //                     .write(0xFFFFFFFF)
    //             };
    //         }
    //     }
    // }
    //
    // info!("b");
    
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

    syscall::init();

    drivers::init();

    task::init(kernel_main_task, Box::new("hello, world!".to_owned()));
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

extern "C" fn kernel_main_task(msg: Box<String>) -> ! {
    debug!("Main task entered: {}", msg);

    let task = Task::new_kernel(
        Some((kernel_side_task, Box::new(()))), "side".to_owned());
    let id = task.add_to_tasks(true);
    println!("Side task id: {}", id);
    sched_yield();

    debug!("Main task continues");

    let space = MemorySpace::get().new_new_user();
    let task = Task::new_user(
        (user_start_task, Box::new(())), "user".to_owned(), space, 1 << 16);
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

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!("Kernel panic: {} [{}:{}]", info.message(), location.file(), location.line());
    } else {
        error!("Kernel panic: {}", info.message());
    }
    cpu::hcf();
}
