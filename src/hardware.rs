use alloc::sync::Arc;
use core::cell::RefCell;
use cortex_m::delay::Delay;
use cortex_m::peripheral::syst::SystClkSource;
use critical_section::Mutex;
use lazy_static::lazy_static;
use teensy4_bsp::{pins, Led};

pub struct Hardware {
    pub led: Led,
    pub systick: Delay,
}

impl Hardware {
    pub fn get() -> Arc<Mutex<RefCell<Self>>> {
        lazy_static! {
            pub static ref HARDWARE: Arc<Mutex<RefCell<Hardware>>> =
                Arc::new(Mutex::new(RefCell::new(Hardware::setup())));
        }
        HARDWARE.clone()
    }
    pub fn setup() -> Self {
        let bsp_peripherals = teensy4_bsp::Peripherals::take().unwrap();
        let cortex_peripherals = cortex_m::Peripherals::take().unwrap();

        // // Reduce the number of pins to those specific
        // // to the Teensy 4.0.
        let pins = pins::t40::from_pads(bsp_peripherals.iomuxc);

        Self {
            led: teensy4_bsp::configure_led(pins.p13),
            systick: Delay::with_source(
                cortex_peripherals.SYST,
                teensy4_bsp::EXT_SYSTICK_HZ,
                SystClkSource::External,
            ),
        }
    }
}
