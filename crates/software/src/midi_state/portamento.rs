//! Provides a data structure for managing the MIDI Portamento controls of an instrument.

use wmidi::{ControlValue, Note};

/// A struct for managing the Portamento controls of an instrument.
#[derive(Clone)]
pub struct Portamento {
    /// MIDI CC 4: Portamento On/Off
    enabled: bool,
    /// MIDI CC 84: Portamento Control (glide from this note instead of the last one performed)
    origin_override: Option<Note>,
    /// MIDI CC 5: Portamento Time
    time: ControlValue,
    /// MIDI CC 37: Portamento Time (Least-Significant Bits)
    time_lsb: Option<ControlValue>,
}

impl Portamento {
    /// Sets the control value for CC 5: Portamento Time
    pub fn set_time(&mut self, time: ControlValue) {
        self.time = time;
    }
}

impl Default for Portamento {
    fn default() -> Self {
        Self {
            enabled: true,
            origin_override: Default::default(),
            time: Default::default(),
            time_lsb: Default::default(),
        }
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Portamento {
    fn format(&self, fmt: defmt::Formatter) {
        let Portamento {
            enabled,
            origin_override,
            time,
            time_lsb,
        } = *self;
        defmt::write!(
            fmt,
            "Portamento {{ enabled: {}, origin_override: {}, time: {}, time_lsb: {} }}",
            enabled,
            origin_override.map(|v| u8::from(v)),
            u8::from(time),
            time_lsb.map(|v| u8::from(v))
        );
    }
}
