#![no_std]
#![no_main]
#![feature(maybe_uninit_array_assume_init)]
#![feature(negative_impls)]
#![feature(unsafe_cell_access)]
#![feature(slice_as_array)]
#![feature(btree_cursors)]
#![feature(slice_from_ptr_range)]
#![feature(abi_x86_interrupt)]
#![feature(thread_local)]
#![feature(transmutability)]
#![feature(push_mut)]
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

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};
use drivers::time::pit;
use crate::drivers::serial;

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
        todo!()
    } else {
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
    memory::init_core_local(memory_values);

    drivers::init();

    task::init(kernel_main_task, Box::new("hello, world!".to_owned()));
}

#[expect(unconditional_recursion)]
fn overflow() -> ! {
    overflow();
}

extern "C" fn kernel_main_task(msg: Box<String>) -> ! {
    debug!("Main task entered: {}", msg);

    // overflow();

    loop {
        if let Some(ch) = serial::read() {
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
