use crate::hardware::Hardware;
use alloc::vec::Vec;
use log::debug;

pub fn set_led(data: Vec<u8>) -> Vec<u8> {
    debug!("called set_led");
    critical_section::with(|cs| {
        let hardware = Hardware::get();
        let mut hardware = hardware.borrow_ref_mut(cs);
        if !data.is_empty() {
            if data[0] == 0 {
                // turn off led
                hardware.led.clear();
            } else {
                // turn on led
                hardware.led.set();
            }
        } else {
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
