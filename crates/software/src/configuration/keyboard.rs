use crate::midi_state::ActivatedNotes;
use core::ops::RangeInclusive;
use measurements::Voltage;
use num_derive::{FromPrimitive, ToPrimitive};
use wmidi::Note;

/// Configurations relating to the keyboard component of the attached synthesizer.
///
/// Stores performer selections which extend the native capabilities of the synth (e.g, note provider which enables
/// arpeggiation as well as changing note priority) in addition to the specification of the hardware synth (needed
/// for calculations converting notes to control voltage). It may not seem intuitive to treat fixed properties
/// like the playable range as configurations, but if this device comes to support more than one synthesizer, the
/// performer will have controls to select the attached instrument, which amounts to the same thing.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyboard<T> {
    note_provider: T,
    playable_range: RangeInclusive<Note>,
    voltage_per_octave: Voltage,
}

impl<T: ProvideNote> Keyboard<T> {
    /// Constructs a [`Keyboard`].
    pub fn new(
        note_provider: T,
        playable_range: RangeInclusive<Note>,
        voltage_per_octave: Voltage,
    ) -> Self {
        Self {
            note_provider,
            playable_range,
            voltage_per_octave,
        }
    }

    /// Selects the appropriate [`Note`] to play based on configuration and instrument range.
    pub fn provide_note(&self, notes: &ActivatedNotes) -> Option<Note> {
        let filtered_notes = notes.iter().filter(|note| {
            note >= self.playable_range.start() && note <= self.playable_range.end()
        });

        self.note_provider.provide_note(filtered_notes)
    }

    fn voltage_per_half_step(&self) -> Voltage {
        self.voltage_per_octave / 12.0
    }

    /// Returns the [`Voltage`] required for this particular [`Keyboard`] to play a given [`Note`].
    pub fn voltage(&self, note: Note) -> Voltage {
        let nth_key = u8::from(note).saturating_sub(*self.playable_range.start() as u8);
        nth_key as f64 * self.voltage_per_half_step()
    }
}

/// Trait for selecting which [`Note`] to play when many have been activated.
pub trait ProvideNote {
    /// Selects the appropriate [`Note`] to play based on configuration and instrument range.
    fn provide_note(&self, notes: impl Iterator<Item = Note>) -> Option<Note>;
}

/// A [`ProvideNote`] with variants for selecting a single activated [`Note`] from among many,
/// based on their relative order or position.
#[derive(Debug, Copy, Clone, ToPrimitive, FromPrimitive, PartialEq)]
pub enum NotePriority {
    /// Prioritizes notes based on the order in which they are received. Notes played earlier will be voiced over later ones.
    First,
    /// Prioritizes notes based on the order in which they are received. Notes played later will be voiced over earlier ones.
    Last,
    /// Prioritizes notes based on pitch. Lower notes (e.g., those on the left side of the keyboard) will be voiced over higher ones.
    Low,
    /// Prioritizes notes based on pitch. Higher notes (e.g., those on the right side of the keyboard) will be voiced over lower ones.
    High,
}
impl super::CycleConfig for NotePriority {}

impl ProvideNote for NotePriority {
    fn provide_note(&self, mut notes: impl Iterator<Item = Note>) -> Option<Note> {
        match self {
            NotePriority::First => notes.next(),
            NotePriority::Last => notes.last(),
            NotePriority::Low => notes.min(),
            NotePriority::High => notes.max(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord() -> ActivatedNotes {
        let mut notes = ActivatedNotes::new();
        notes.add(Note::E4);
        notes.add(Note::G4);
        notes.add(Note::B4);
        notes.add(Note::C4);

        notes
    }

    mod note_priority {
        use super::*;

        #[test]
        fn first() {
            let np = Keyboard {
                note_provider: NotePriority::First,
                playable_range: Note::F3..=Note::C6,
                voltage_per_octave: Voltage::from_volts(1.0),
            };
            assert_eq!(
                Some(Note::E4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }

        #[test]
        fn last() {
            let np = Keyboard {
                note_provider: NotePriority::Last,
                playable_range: Note::F3..=Note::C6,
                voltage_per_octave: Voltage::from_volts(1.0),
            };
            assert_eq!(
                Some(Note::C4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }

        #[test]
        fn highest() {
            let np = Keyboard {
                note_provider: NotePriority::High,
                playable_range: Note::F3..=Note::C6,
                voltage_per_octave: Voltage::from_volts(1.0),
            };
            assert_eq!(
                Some(Note::B4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }

        #[test]
        fn lowest() {
            let np = Keyboard {
                note_provider: NotePriority::Low,
                playable_range: Note::F3..=Note::C6,
                voltage_per_octave: Voltage::from_volts(1.0),
            };
            assert_eq!(
                Some(Note::C4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }
    }
}
