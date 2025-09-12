use enum_dispatch::enum_dispatch;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

/// A trait which allows infinite cycling of an enum's variants.
///
/// Expected to be useful for pushbutton user interfaces, wherein a button press advances from the current to the next variant,
/// cycling back to the beginning when all variants have been exhausted.
pub trait CycleConfig {
    fn cycle(self) -> Self
    where
        Self: FromPrimitive + ToPrimitive + Sized,
    {
        let index = self
            .to_u8()
            .expect("enum variants should be castable to u8");
        match <Self as FromPrimitive>::from_u8(index + 1) {
            Some(new_selection) => new_selection,
            None => FromPrimitive::from_u8(0).expect("enum should not be empty"),
        }
    }
}

pub struct InstrumentConfig {
    pub envelope_trigger: EnvelopeTrigger,
    pub input_mode: InputMode,
    pub note_priority: NotePriority,
}

/// A trait for reading from and writing to an instrument's configuration.
#[enum_dispatch(Instrument)]
pub trait Config {
    fn config(&self) -> &InstrumentConfig;
    fn config_mut(&mut self) -> &mut InstrumentConfig;
}

/// Determines which note(s) sound(s) when more notes than the instrument can voice simultaneously are received.
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
impl CycleConfig for NotePriority {}

#[derive(Debug, Copy, Clone, ToPrimitive, FromPrimitive)]
pub enum EnvelopeTrigger {
    /// Envelope is triggered each time a break ends. That is, the envelope is triggered when the initial break ends
    /// (i.e., when the first note is played) as well as when any break between notes ends (i.e., at the start of each
    /// note when playing staccato). Notes played legato will be played within the same envelope contour.
    BreakEnd,
    /// The envelope is triggered each time the synthesizer changes notes, regardless of articulation.
    NoteChange,
}
impl CycleConfig for EnvelopeTrigger {}

#[derive(Debug, Default, Copy, Clone, ToPrimitive, FromPrimitive)]
pub enum InputMode {
    /// Notes are played via the keyboard module, as though a performer were playing the instrument directly, respecting
    /// the synth's octave, frequency, doubling, and fine tune controls. The synth's glide setting is overridden, as this
    /// is part of the keyboard module. MIDI input signals which keys are struck, indirectly determining pitch (based on the
    /// aforementioned hardware setting) and filter cutoff. (The filter cutoff tracks the keyboard to various degrees depending
    /// on the filter mode setting.)
    #[default]
    Keyboard,
    /// TODO
    Oscillator,
}
impl CycleConfig for InputMode {}
