use crate::{configuration::NotePriority, midi_state::ActivatedNotes};
use core::ops::RangeInclusive;
use wmidi::Note;

/// Utility struct for selecting the appropriate [`Note`] to play based on configuration and instrument range.
pub struct NoteProvider<T> {
    config: T,
    playable_range: RangeInclusive<Note>,
}

impl<T> NoteProvider<T> {
    /// Constructs a [`NoteProvider`].
    pub fn new(config: T, playable_range: RangeInclusive<Note>) -> Self {
        Self {
            config,
            playable_range,
        }
    }
}

impl NoteProvider<NotePriority> {
    /// Selects the [`Note`] to play based on the [`NotePriority`] configuration.
    pub fn provide_note(&self, notes: &ActivatedNotes) -> Option<Note> {
        let mut filtered_notes = notes.iter().filter(|note| {
            note >= self.playable_range.start() && note <= self.playable_range.end()
        });

        match self.config {
            NotePriority::First => filtered_notes.next(),
            NotePriority::Last => filtered_notes.last(),
            NotePriority::Low => filtered_notes.min(),
            NotePriority::High => filtered_notes.max(),
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
            let np = NoteProvider {
                config: NotePriority::First,
                playable_range: Note::F3..=Note::C6,
            };
            assert_eq!(
                Some(Note::E4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }

        #[test]
        fn last() {
            let np = NoteProvider {
                config: NotePriority::Last,
                playable_range: Note::F3..=Note::C6,
            };
            assert_eq!(
                Some(Note::C4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }

        #[test]
        fn highest() {
            let np = NoteProvider {
                config: NotePriority::High,
                playable_range: Note::F3..=Note::C6,
            };
            assert_eq!(
                Some(Note::B4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }

        #[test]
        fn lowest() {
            let np = NoteProvider {
                config: NotePriority::Low,
                playable_range: Note::F3..=Note::C6,
            };
            assert_eq!(
                Some(Note::C4),
                np.provide_note(&chord()),
                "Expected left but right"
            );
        }
    }
}
