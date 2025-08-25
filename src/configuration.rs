/// Determines which note(s) sound(s) when more notes than the instrument can voice simultaneously are received.
///
/// When a note is released, it is replaced by the next note (if any) based on the selected algorithm.
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

pub enum EnvelopeTrigger {
    /// Envelope is triggered each time a break ends. That is, the envelope is triggered when the initial break ends
    /// (i.e., when the first note is played) as well as when any break between notes ends (i.e., at the start of each
    /// note when playing staccato). Notes played legato will be played within the same envelope contour.
    BreakEnd,
    /// The envelope is triggered each time the synthesizer changes notes, regardless of articulation.
    NoteChange,
}
