#![no_std]
#![no_main]
#![feature(naked_functions)]

mod drivers;
mod ports;
mod print;
mod log;
mod gdt;
mod interrupts;
mod cpu;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};

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
        error!("Kernel exception: {:#?}", ctx);
        panic!("Kernel exception: {:#?}", ctx);
    }
}
fn irq_handler(ctx: interrupts::IrqContext) {
    debug!("IRQ: {:#?}", ctx);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    cpu::disable_interrupts();

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

    unsafe { core::ptr::read_volatile(16 as *const u64) };

    cpu::hcf();
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    error!("Kernel panic: {}", info.message());
    cpu::hcf();
}
