//! USB Serial communication

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::{cell::RefCell, fmt::Write};
use critical_section::Mutex;
use lazy_static::lazy_static;
use log::{error, info, warn};
use teensy4_bsp::{hal::ral::usb::USB1, interrupt, usb};

use crate::events;

pub const END: u8 = 0x00;
pub const READY: u8 = 0x01;
pub const FUNCTION_HEADER: u8 = 0x02;
pub const RETURN_HEADER: u8 = 0x03;

trait BlockingReader {
    fn read_n(&mut self, num_bytes: usize) -> Result<Vec<u8>, usb::Error>;
    fn read_n_blocking(&mut self, num_bytes: usize) -> Result<Vec<u8>, usb::Error>;
}

impl BlockingReader for usb::Reader {
    fn read_n(&mut self, num_bytes: usize) -> Result<Vec<u8>, usb::Error> {
        if num_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut data = vec![0u8; num_bytes];
        self.read(&mut data)?;
        Ok(data)
    }
    fn read_n_blocking(&mut self, num_bytes: usize) -> Result<Vec<u8>, usb::Error> {
        if num_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut data = vec![0u8; num_bytes];
        while self.read(&mut data)? == 0 {}
        Ok(data)
    }
}

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
    pub fn get() -> Result<Self, usb::Error> {
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
            let mut rx = rx.borrow_ref_mut(cs);
            match rx.read_n(1).map(|e| e[0]) {
                Ok(READY) => {
                    // if we receive ready event, we should respond back with ready
                    self.ready();
                }
                Ok(FUNCTION_HEADER) => {
                    // we've received a request to call a function.
                    // we need to dispatch it and return a RETURN event.
                    let function_len = rx.read_n_blocking(1).unwrap()[0];
                    info!("got function length of {function_len}");

                    let function =
                        String::from_utf8(rx.read_n_blocking(function_len as usize).unwrap())
                            .unwrap();
                    info!("got function {function}");

                    let data_len = rx.read_n_blocking(2).unwrap();
                    let data_len = u16::from_le_bytes([data_len[0], data_len[1]]);
                    info!("got data length of {data_len}");

                    let data = rx.read_n_blocking(data_len as usize).unwrap();

                    // read end
                    rx.read_n_blocking(1).unwrap();

                    let result = match function.as_str() {
                        "set_led" => events::set_led(data),
                        "reset" => events::reset(data),
                        _ => {
                            warn!("Function {function} does not exist");
                            vec![]
                        }
                    };
                    self.return_event(result);
                }
                Ok(END) => {}
                Ok(b) => {
                    // if we haven't matched, then the even had an invalid format (no event type)
                    warn!("Received invalid event {b}");
                    // flush buffer
                    //TODO proper flush
                    let buffer = [0u8; 1];
                    while rx.read(buffer).unwrap() > 0 {}
                }
                Err(error) => {
                    error!("Error reading from serial: {:?}", error);
                }
            }
        });
    }
    fn return_event<B: AsRef<[u8]>>(&self, data: B) {
        critical_section::with(|cs| {
            let tx = self.tx.clone();
            let mut tx = tx.borrow(cs).borrow_mut();
            tx.write([RETURN_HEADER]).unwrap();
            tx.write(data).unwrap();
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
