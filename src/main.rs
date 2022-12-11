//! BallBot teensy component

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use crate::logger::SerialLogger;
use crate::serial::SerialComm;
use alloc::format;
use core::alloc::Layout;
use core::panic::PanicInfo;
use cortex_m_rt as rt;
use embedded_alloc::Heap;

mod events;
mod hardware;
mod logger;
mod serial;

#[rt::entry]
fn main() -> ! {
    // Initialize the allocator BEFORE you use it.
    init_heap();

    // See the `logging` module docs for more info.
    let serial = SerialComm::get().unwrap();
    SerialLogger::init(serial.clone());

    loop {
        serial.read();
    }
}

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[alloc_error_handler]
fn oom(_: Layout) -> ! {
    #[allow(clippy::empty_loop)]
    loop {}
}

fn init_heap() {
    use core::mem::MaybeUninit;
    const HEAP_SIZE: usize = 1024;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    SerialComm::get().unwrap().call("panic", format!("{info}"));
    #[allow(clippy::empty_loop)]
    loop {}
}
