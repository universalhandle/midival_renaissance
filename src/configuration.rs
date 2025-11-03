//! This module contains both user-configurable settings (implemented as enums) and traits to make them easier to work with in code.

use embassy_time::Duration;
use enum_dispatch::enum_dispatch;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

/// A trait which allows infinite cycling of an enum's variants.
///
/// Useful for pushbutton user interfaces, allowing presses to advance from the current to the next variant,
/// cycling back to the beginning when all variants have been exhausted.
pub trait CycleConfig {
    /// Return the next variant, cycling back to the beginning as needed.
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

/// Configurations that should be available to any instrument. Generally, these provide opportunities to make an instrument
/// behave in ways that are impossible when directly played.
///
/// `InstrumentConfig` may be a bit of a misnomer as, strictly speaking, this struct configures how the MIDIval Renaissance
/// itself interprets certain MIDI events. For example, using [`note_priority`][Self::note_priority] the MIDIval
/// Renaissance can be configured to send control voltages that voice the leftmost key when multiple keys are pressed, matching
/// the Micromoog's behavior, or it can override that behavior by sending voltages for the high note instead.
pub struct InstrumentConfig {
    /// Not yet implemented.
    pub envelope_trigger: EnvelopeTrigger,
    /// Not yet implemented.
    pub input_mode: InputMode,
    /// This config can be thought of enabling a "chord cleanup" feature. It can be used to insert a slight delay between MIDI
    /// input and eletrical output to account for human imprecision.
    ///
    /// For example, it stands to reason that when playing chords on a controller with the [note_priority][`Self::note_priority`]
    /// set to low, the MIDIval Renaissance/Micromoog would be expected to provide "bass lines for free." Inserting a delay enables
    /// "close enough" timing for all the keypresses associated with the performance of a chord so that the Micromoog doesn't play
    /// the third or the fifth for a split second on occasions when those notes land before the root.
    ///
    /// As the chord cleanup feature batches and "swallows" notes by design, users will likely want to disable it if they intend to
    /// drive the attached synthesizer from a sequencer or MIDI file. Its intended use case is for live-playing through a controller.
    pub note_embargo: NoteEmbargo,
    /// Determines which note sounds when more notes than the instrument can voice simultaneously are received.
    pub note_priority: NotePriority,
}

/// A trait for reading from and writing to an instrument's configuration.
#[enum_dispatch(Instrument)]
pub trait Config {
    fn config(&self) -> &InstrumentConfig;
    fn config_mut(&mut self) -> &mut InstrumentConfig;
}

/// Determines how much delay to insert between MIDI input and electrical output to enable "chord cleanup" functionality,
/// expressed as divisions of a note.
///
/// Messages received within this interval are effectively batched rather than processed one at a time. See [`InstrumentConfig::note_embargo`].
#[derive(Debug, Clone, Copy, ToPrimitive, FromPrimitive, PartialEq)]
pub enum NoteEmbargo {
    /// Effectively disables the "chord cleanup" feature.
    None,
    /// Introduces a margin of error of one 32nd note for the performer.
    ThirtySecondNote,
}

impl NoteEmbargo {
    /// Return the duration of the note embargo in a format compatible with Embassy's timekeeping API.
    ///
    /// In some future, this will be tied to BPM (beats per minute). For now, BPM is assumed to be 120.
    pub fn duration(&self) -> Duration {
        match self {
            Self::None => Duration::from_micros(0),
            Self::ThirtySecondNote => Duration::from_micros(62500),
        }
    }

    pub fn is_enabled(&self) -> bool {
        *self != Self::None
    }
}

impl CycleConfig for NoteEmbargo {}

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
impl CycleConfig for NotePriority {}

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
impl CycleConfig for EnvelopeTrigger {}

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
impl CycleConfig for InputMode {}
