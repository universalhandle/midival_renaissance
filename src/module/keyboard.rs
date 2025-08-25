use wmidi::U7;

pub struct KeyboardSpec {
    low_key_note: U7,
    low_key_voltage: f32,
    key_count: U7,
    volts_per_octave: f32, // probably needs to be a float, actually
}

impl KeyboardSpec {
    pub fn new(
        low_key_note: U7,
        low_key_voltage: f32,
        key_count: U7,
        volts_per_octave: f32,
    ) -> Self {
        Self {
            low_key_note,
            low_key_voltage,
            key_count,
            volts_per_octave,
        }
    }

    pub fn low_key_note(&self) -> U7 {
        self.low_key_note
    }

    pub fn key_count(&self) -> U7 {
        self.key_count
    }

    /// Returns the highest note the keyboard can play.
    pub fn high_key_note(&self) -> U7 {
        U7::from_u8_lossy(u8::from(self.low_key_note) + u8::from(self.key_count) - 1)
    }

    pub fn volts_per_octave(&self) -> f32 {
        self.volts_per_octave
    }
}

pub trait Keyboard {
    type Spec;
}
