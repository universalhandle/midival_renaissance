use num_derive::{FromPrimitive, ToPrimitive};

/// Determines which note sounds when more notes than the instrument can voice simultaneously are received.
///
/// When a note is released, it is replaced by the next note (if any) based on the selected algorithm.
#[derive(Debug, Copy, Clone, ToPrimitive, FromPrimitive)]
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
