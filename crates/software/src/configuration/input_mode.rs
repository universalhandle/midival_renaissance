use num_derive::{FromPrimitive, ToPrimitive};

/// Determines which of the synthesizer's modules will receive note input.
#[derive(Debug, Default, Copy, Clone, ToPrimitive, FromPrimitive)]
pub enum InputMode {
    /// Notes are played via the keyboard module, as though a performer were playing the instrument directly, respecting
    /// the synth's octave, frequency, doubling, and fine tune controls. The hardware control for glide is overridden, as this
    /// is part of the keyboard module. MIDI input signals which keys are struck, indirectly determining pitch (based on the
    /// aforementioned hardware setting) and filter cutoff. (The filter cutoff tracks the keyboard to various degrees depending
    /// on the filter mode setting.)
    #[default]
    Keyboard,
    /// TODO
    Oscillator,
}
impl super::CycleConfig for InputMode {}
