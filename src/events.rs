use crate::hardware::Hardware;
use alloc::vec::Vec;
use log::{debug, trace};

pub fn set_led(data: Vec<u8>) -> Vec<u8> {
    critical_section::with(|cs| {
        let hardware = Hardware::get();
        let mut hardware = hardware.borrow_ref_mut(cs);
        if !data.is_empty() {
            if data[0] == 0 {
                trace!("called set_led with argument: false");
                // turn off led
                hardware.led.clear();
            } else {
                trace!("called set_led with argument: true");
                // turn on led
                hardware.led.set();
            }
        } else {
            trace!("called set_led with no argument, defaulting to toggling");
            // toggle led
            hardware.led.toggle();
        }
    });
    // void return
    Vec::new()
}

pub fn reset(_: Vec<u8>) -> ! {
    debug!("called reset");
    critical_section::with(|cs| {
        let hw = Hardware::get();
        let mut hw = hw.borrow_ref_mut(cs);
        for _ in 1..12 {
            hw.led.toggle();
            hw.systick.delay_ms(84);
        }
    });
    cortex_m::peripheral::SCB::sys_reset();
}
