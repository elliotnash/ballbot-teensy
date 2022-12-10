//! USB logging support
//!
//! If you don't want USB logging, remove
//!
//! - this module
//! - the `log` dependency in Cargo.toml

use alloc::sync::Arc;
use core::{cell::RefCell, fmt::Write};
use critical_section::Mutex;
use lazy_static::lazy_static;
use log::warn;
use teensy4_bsp::{hal::ral::usb::USB1, interrupt, usb};

pub const END: u8 = 0x00;
pub const READY: u8 = 0x01;
pub const DISCONNECT: u8 = 0x02;
pub const FUNCTION_HEADER: u8 = 0x03;
pub const RETURN_HEADER: u8 = 0x04;

#[derive(Clone)]
pub struct SerialComm {
    rx: Arc<Mutex<RefCell<usb::Reader>>>,
    tx: Arc<Mutex<RefCell<usb::Writer>>>,
    ready: Arc<Mutex<RefCell<bool>>>,
}

impl SerialComm {
    /// Gets the SerialComm instance and initializes it if it does not exist.
    ///
    /// When `get` returns, the USB interrupt will be enabled,
    /// and the host may begin to interface the device.
    /// You should only call this once.
    ///
    /// # Panics
    ///
    /// Panics if the USB1 instance is already taken.
    pub fn get() -> Result<SerialComm, usb::Error> {
        lazy_static! {
            static ref SERIAL: SerialComm = usb::split(USB1::take().unwrap())
                .map(|(poller, rx, tx)| {
                    setup(poller);
                    SerialComm {
                        rx: Arc::new(Mutex::new(RefCell::new(rx))),
                        tx: Arc::new(Mutex::new(RefCell::new(tx))),
                        ready: Arc::new(Mutex::new(RefCell::new(false))),
                    }
                })
                .unwrap();
        }
        Ok((*SERIAL).clone())
    }
    pub fn ready(&self) {
        critical_section::with(|cs| {
            let tx = self.tx.clone();
            let mut tx = tx.borrow(cs).borrow_mut();
            tx.write([READY])
                .expect("Failed to communicated with serial port");
            // tx.flush().unwrap();
            // we're ready to send communication now
            *self.ready.borrow_ref_mut(cs) = true;
        });
    }
    pub fn read(&self) {
        critical_section::with(|cs| {
            let rx = self.rx.clone();
            let mut rx = rx.borrow(cs).borrow_mut();
            let mut event = [0u8; 1];
            if let Ok(read) = rx.read(&mut event) {
                if read == 1 {
                    match event[0] {
                        READY => {
                            // if we receive ready event, we should respond back with ready
                            self.ready();
                        }
                        FUNCTION_HEADER => {
                            // we've received a request to call a function.
                            // we need to dispatch it and return a RETURN event.
                        }
                        b => {
                            // if we haven't matched, then the even had an invalid format (no event type)
                            warn!("Received invalid event {b}");
                            // flush buffer
                            //TODO proper flush
                            while rx.read(event).unwrap() > 0 {}
                        }
                    }
                }
            }
        });
    }
    pub fn call<B: AsRef<[u8]>>(&self, function: &str, data: B) {
        critical_section::with(|cs| {
            // make sure serial is ready to receive
            if *self.ready.borrow_ref(cs) {
                let tx = self.tx.clone();
                let mut tx = tx.borrow(cs).borrow_mut();
                tx.write([
                    FUNCTION_HEADER,
                    function
                        .len()
                        .try_into()
                        .expect("Function name must be less than 255 characters"),
                ])
                .unwrap();
                write!(tx, "{function}").unwrap();
                let len: u16 = data.as_ref().len() as u16;
                tx.write(len.to_le_bytes()).unwrap();
                tx.write(data).unwrap();
                tx.write([END]).unwrap();
                // tx.flush().unwrap();
            }
        });
    }
}

/// Setup the USB ISR with the USB poller
fn setup(poller: usb::Poller) {
    static POLLER: Mutex<RefCell<Option<usb::Poller>>> = Mutex::new(RefCell::new(None));

    #[cortex_m_rt::interrupt]
    fn USB_OTG1() {
        critical_section::with(|cs| {
            POLLER
                .borrow(cs)
                .borrow_mut()
                .as_mut()
                .map(|poller| poller.poll());
        });
    }

    critical_section::with(|cs| {
        *POLLER.borrow(cs).borrow_mut() = Some(poller);
        // Safety: invoked in a critical section that also prepares the ISR
        // shared memory. ISR memory is ready by the time the ISR runs.
        unsafe { cortex_m::peripheral::NVIC::unmask(interrupt::USB_OTG1) };
    });
}
