use crate::instrument::Instrument;
use core::ops::RangeInclusive;
use enum_dispatch::enum_dispatch;
use wmidi::Note;

/// A trait for sending note data to a synthesizer via control voltage.
#[enum_dispatch(Instrument)]
pub trait ControlVoltage {
    /// Expresses the note that should be played as a voltage.
    fn current_note_to_voltage(&self) -> f32;

    /// Returns the musical range of the instrument.
    ///
    /// Note: the order of the bookend notes in the range should match the order in which they'd appear on a keyboard,
    /// e.g., C4..=C5, not C5..=C4.
    ///
    /// (This would be a const except that trait consts aren't compatible with [`enum_dispatch`].)
    fn playable_notes(&self) -> RangeInclusive<Note>;

    /// Returns the voltage required to change the pitch by an octave.
    ///
    /// (This would be a const except that trait consts aren't compatible with [`enum_dispatch`].)
    fn volts_per_octave(&self) -> f32;

    /// Returns true if the note is within the instrument's musical range.
    fn can_voice(&self, note: &Note) -> bool {
        Self::playable_notes(self).contains(note)
    }
}
