use core::ops::RangeInclusive;
use wmidi::Note;

pub struct KeyboardSpec {
    low_key_voltage: f32,
    playable_notes: RangeInclusive<Note>,
    volts_per_octave: f32, // probably needs to be a float, actually
}

impl KeyboardSpec {
    pub fn new(
        low_key_voltage: f32,
        playable_notes: RangeInclusive<Note>,
        volts_per_octave: f32,
    ) -> Self {
        if playable_notes.start() > playable_notes.end() {
            panic!("Invalid keyboard specification: range must contain at least one note.")
        }

        Self {
            low_key_voltage,
            playable_notes,
            volts_per_octave,
        }
    }

    pub fn playable_notes(&self) -> &RangeInclusive<Note> {
        &self.playable_notes
    }

    pub fn volts_per_octave(&self) -> f32 {
        self.volts_per_octave
    }
}

pub trait Keyboard {
    fn get_voltage(&self) -> f32;
    fn playable_notes(&self) -> &RangeInclusive<Note>;
    fn volts_per_octave(&self) -> f32;

    fn can_voice(&self, note: &Note) -> bool {
        self.playable_notes().contains(note)
    }
}
