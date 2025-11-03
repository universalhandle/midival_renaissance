//! This module provides traits for managing the MIDIval Renaissance's inputs (MIDI) and outputs (CV/Gate).
//!
//! Because not all MIDI messages have an obvious immediate expression (e.g., BPM) and because sometimes multiple messages are received at once
//! (e.g., when a chord is played), the processing of input and its expression are separate.

pub mod control_voltage;
pub mod gate;
pub mod midi;
