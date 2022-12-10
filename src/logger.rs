use crate::serial::SerialComm;
use alloc::string::ToString;
use alloc::vec;
use log::LevelFilter;

pub struct SerialLogger {
    serial_comm: Option<SerialComm>,
}

static mut SERIAL_LOGGER: SerialLogger = SerialLogger { serial_comm: None };

impl SerialLogger {
    // This should only be called once
    pub fn init(serial_comm: SerialComm) {
        // Logger needs to have static lifetime to set - not owned by Log
        // TODO use lazy static
        unsafe {
            SERIAL_LOGGER = SerialLogger {
                serial_comm: Some(serial_comm),
            };
            log::set_logger(&SERIAL_LOGGER).unwrap();
        }
        // levels should be configured on kotlin side
        log::set_max_level(LevelFilter::max());
    }
}

impl log::Log for SerialLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        self.serial_comm.is_some() // make sure we've initialized logger
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let level = record.level().to_string();
            let content = record.args().to_string();

            let mut data = vec![level.len() as u8];
            data.append(&mut level.into_bytes());

            data.extend_from_slice(&(content.len() as u16).to_le_bytes());
            data.append(&mut content.into_bytes());

            self.serial_comm.as_ref().unwrap().call("log", data);
        }
    }

    // currently no flush implementation
    fn flush(&self) {}
}
