//! USB logging support
//!
//! If you don't want USB logging, remove
//!
//! - this module
//! - the `log` dependency in Cargo.toml

use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::borrow::BorrowMut;
use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use core::fmt::Write;
use bsp::hal::ral::usb::USB1;
use bsp::interrupt;
use teensy4_bsp as bsp;
use bsp::usb;
use log::LevelFilter;
use teensy4_bsp::hal::dma::Buffer;

pub const READY: u8 = 0x01;
pub const FUNCTION_HEADER: u8 = 0x02;
pub const RETURN_HEADER: u8 = 0x03;

#[derive(Clone)]
pub struct SerialComm {
    rx: Arc<Mutex<RefCell<usb::Reader>>>,
    tx: Arc<Mutex<RefCell<usb::Writer>>>,
}

impl SerialComm {
    /// Initializes and returns the SerialComm.
    ///
    /// When `init` returns, the USB interrupt will be enabled,
    /// and the host may begin to interface the device.
    /// You should only call this once.
    ///
    /// # Panics
    ///
    /// Panics if the USB1 instance is already taken.
    pub fn init() -> Result<SerialComm, usb::Error> {
        usb::split(USB1::take().unwrap())
            .map(|(poller, rx, tx)| {
                setup(poller);
                let rx = Arc::new(Mutex::new(RefCell::new(rx)));
                let tx = Arc::new(Mutex::new(RefCell::new(tx)));
                SerialComm{rx, tx}
            })
    }
    pub fn ready(&self) {
        cortex_m::interrupt::free(|cs| {
            let tx = self.tx.clone();
            tx.borrow(cs).borrow_mut().write([READY]).unwrap();
        });
    }
    pub fn send_string(&self, message: &str) {
        cortex_m::interrupt::free(|cs| {
            let tx = self.tx.clone();
            writeln!(tx.borrow(cs).borrow_mut(), "{}", message).unwrap();
        });
    }
    pub fn call<B: AsRef<[u8]>>(&self, function: &str, data: B) {
        cortex_m::interrupt::free(|cs| {
            let tx = self.tx.clone();
            let mut tx = tx.borrow(cs).borrow_mut();
            tx.write([FUNCTION_HEADER, function.len().try_into().expect("Function name must be less than 255 characters")]).unwrap();
            write!(tx, "{}", function).unwrap();
            let len: u16 = data.as_ref().len() as u16;
            tx.write(len.to_le_bytes()).unwrap();
            tx.write(data).unwrap();
        });
    }
    // fn tests(&self) {
    //     self.rx.
    // }
}

/// Setup the USB ISR with the USB poller
fn setup(poller: usb::Poller) {
    static POLLER: Mutex<RefCell<Option<usb::Poller>>> = Mutex::new(RefCell::new(None));

    #[cortex_m_rt::interrupt]
    fn USB_OTG1() {
        cortex_m::interrupt::free(|cs| {
            POLLER
                .borrow(cs)
                .borrow_mut()
                .as_mut()
                .map(|poller| poller.poll());
        });
    }

    cortex_m::interrupt::free(|cs| {
        *POLLER.borrow(cs).borrow_mut() = Some(poller);
        // Safety: invoked in a critical section that also prepares the ISR
        // shared memory. ISR memory is ready by the time the ISR runs.
        unsafe { cortex_m::peripheral::NVIC::unmask(interrupt::USB_OTG1) };
    });
}
