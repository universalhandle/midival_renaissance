use embassy_time::Duration;
use num_derive::{FromPrimitive, ToPrimitive};

/// Determines how much delay to insert between MIDI input and electrical output to enable "chord cleanup" functionality,
/// expressed as divisions of a note.
///
/// The intended use case of this feature is liveplaying through a controller. When playing chords on a controller with
/// [`NotePriority`] set to low, it stands to reason that the performer expects the MIDIval Renaissance/Micromoog to
/// provide "bass lines for free." Inserting a delay enables "close enough" timing for all the keypresses associated
/// with the performance of a chord so that the Micromoog doesn't play the third or the fifth for a split second when
/// those notes land before the root.
///
/// As the chord cleanup feature batches and "swallows" notes by design, it should be disabled when driving the synth
/// from a sequencer or MIDI file.
#[derive(Debug, Clone, Copy, ToPrimitive, FromPrimitive, PartialEq)]
pub enum ChordCleanup {
    /// Effectively disables the "chord cleanup" feature.
    None,
    /// Introduces a margin of error of one 32nd note for the performer.
    ThirtySecondNote,
}

impl ChordCleanup {
    /// Return the duration of the batching period in a format compatible with Embassy's timekeeping API.
    ///
    /// In some future, this will be tied to BPM (beats per minute). For now, BPM is assumed to be 120.
    pub fn duration(&self) -> Duration {
        match self {
            Self::None => Duration::from_micros(0),
            Self::ThirtySecondNote => Duration::from_micros(62500),
        }
    }

    /// Returns true for any value other than [`ChordCleanup::None`].
    pub fn is_enabled(&self) -> bool {
        *self != Self::None
    }
}

impl super::CycleConfig for ChordCleanup {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_enabled() {
        assert!(
            ChordCleanup::ThirtySecondNote.is_enabled(),
            "Should be enabled"
        );
        assert!(!ChordCleanup::None.is_enabled(), "Should be disabled");
    }
}
