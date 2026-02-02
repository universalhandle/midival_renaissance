use num_derive::{FromPrimitive, ToPrimitive};

/// Determines when to trigger a new envelope for the attached synthesizer.
///
/// The Micromoog uses the same trigger to initiate both the loudness and filter envelopes.
#[derive(Debug, Copy, Clone, ToPrimitive, FromPrimitive)]
pub enum EnvelopeTrigger {
    /// Envelope is triggered each time a break ends. That is, the envelope is triggered when the initial break ends
    /// (i.e., when the first note is played) as well as when any break between notes ends (i.e., at the start of each
    /// note when playing staccato). Notes played legato will be played within the same envelope contour.
    BreakEnd,
    /// The envelope is triggered each time the synthesizer changes notes, regardless of articulation.
    NoteChange,
}
impl super::CycleConfig for EnvelopeTrigger {}
