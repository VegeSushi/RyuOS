use hal::gpio::{DynPinId, FunctionSioOutput, Pin, PullNone};
use rp2040_hal as hal;

/// This type is the "erased" container. 
/// It holds the pin ID dynamically and is pre-configured as a SIO Output.
pub type GenericPin = Pin<DynPinId, FunctionSioOutput, PullNone>;

pub struct PinList {
    pub pins: [GenericPin; 30],
}

impl PinList {
    pub fn new(pins: hal::gpio::Pins) -> Self {
        // Macro to handle the conversion: 
        // 1. Into Output -> 2. Into PullNone -> 3. Into DynPin
        macro_rules! prep_pin {
            ($p:expr) => {
                $p.into_push_pull_output()
                  .into_pull_type::<PullNone>()
                  .into_dyn_pin()
            };
        }

        Self {
            pins: [
                prep_pin!(pins.gpio0),
                prep_pin!(pins.gpio1),
                prep_pin!(pins.gpio2),
                prep_pin!(pins.gpio3),
                prep_pin!(pins.gpio4),
                prep_pin!(pins.gpio5),
                prep_pin!(pins.gpio6),
                prep_pin!(pins.gpio7),
                prep_pin!(pins.gpio8),
                prep_pin!(pins.gpio9),
                prep_pin!(pins.gpio10),
                prep_pin!(pins.gpio11),
                prep_pin!(pins.gpio12),
                prep_pin!(pins.gpio13),
                prep_pin!(pins.gpio14),
                prep_pin!(pins.gpio15),
                prep_pin!(pins.gpio16),
                prep_pin!(pins.gpio17),
                prep_pin!(pins.gpio18),
                prep_pin!(pins.gpio19),
                prep_pin!(pins.gpio20),
                prep_pin!(pins.gpio21),
                prep_pin!(pins.gpio22),
                prep_pin!(pins.gpio23),
                prep_pin!(pins.gpio24),
                prep_pin!(pins.gpio25),
                prep_pin!(pins.gpio26),
                prep_pin!(pins.gpio27),
                prep_pin!(pins.gpio28),
                prep_pin!(pins.gpio29),
            ],
        }
    }
}