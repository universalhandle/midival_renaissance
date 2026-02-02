//! This crate contains architecture-agnostic logic for the MIDIval Renaissance, a device which allows the [Micromoog Model
//! 2090](https://en.wikipedia.org/wiki/Micromoog) synthesizer to interface with modern music equipment by translating
//! [MIDI](https://midi.org/midi-1-0) messages into electrical signals compatible with a flavor of
//! [CV/gate](https://en.wikipedia.org/wiki/CV/gate) known as the Moog Open System.

#![deny(missing_docs)]
#![no_std]

/// Data structures for tracking MIDI messages the device has received.
pub mod midi_state;

pub mod configuration;
