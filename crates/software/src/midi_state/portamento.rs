//! Provides a data structure for managing the MIDI Portamento controls of an instrument.

use wmidi::{ControlValue, Note};

/// A struct for managing the Portamento controls of an instrument.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Returns the control value for CC 5: Portamento Time.
    pub fn time(&self) -> ControlValue {
        self.time
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use wmidi::U7;

    #[test]
    fn get_time() {
        let p = Portamento {
            enabled: true,
            origin_override: None,
            time: U7::from_u8_lossy(100),
            time_lsb: None,
        };
        assert_eq!(
            U7::from_u8_lossy(100),
            p.time(),
            "Expected left but got right"
        );
    }

    #[test]
    fn set_time() {
        let mut p = Portamento::default();
        p.set_time(U7::from_u8_lossy(111));
        assert_eq!(
            Portamento {
                enabled: true,
                origin_override: None,
                time: U7::from_u8_lossy(111),
                time_lsb: None,
            },
            p,
            "Expected left but got right"
        );
    }
}
