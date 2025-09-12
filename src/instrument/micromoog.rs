use core::ops::RangeInclusive;

use defmt::*;
use wmidi::{MidiMessage, Note};

use crate::{
    activated_notes::ActivatedNotes,
    configuration::{
        Config, EnvelopeTrigger, InputMode, InstrumentConfig, NotePriority,
    },
    io::{
        control_voltage::ControlVoltage,
        gate::{Gate, GateState},
        midi::Midi,
    },
};

struct State {
    activated_notes: ActivatedNotes,
    current_note: Note,
}

impl Default for State {
    fn default() -> Self {
        Self {
            activated_notes: ActivatedNotes::default(),
            current_note: Note::F3,
        }
    }
}

// maybe the essential configs are type parameters, so that we impl TraitX
// differently for Micromoog<InputMode=Keyboard> vs Micromoog<InputMode=Oscillator>?
pub struct Micromoog {
    config: InstrumentConfig,
    state: State,
}

impl Micromoog {
    fn new(config: InstrumentConfig) -> Self {
        Self {
            config,
            state: State::default(),
        }
    }
}

impl Default for Micromoog {
    fn default() -> Self {
        Self::new(InstrumentConfig {
            envelope_trigger: EnvelopeTrigger::BreakEnd,
            input_mode: InputMode::default(),
            note_priority: NotePriority::Low,
        })
    }
}

impl Config for Micromoog {
    fn config(&self) -> &InstrumentConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut InstrumentConfig {
        &mut self.config
    }
}

impl Gate for Micromoog {
    fn gate_state(&self) -> GateState {
        if self.state.activated_notes.is_empty() {
            GateState::Low
        } else {
            GateState::High
        }
    }
}

impl ControlVoltage for Micromoog {
    fn current_note_to_voltage(&self) -> f32 {
        let nth_key = self.state.current_note as u8 - *self.playable_notes().start() as u8;
        nth_key as f32 * self.volts_per_octave() / 12.0
    }

    fn playable_notes(&self) -> RangeInclusive<Note> {
        Note::F3..=Note::C6
    }

    fn volts_per_octave(&self) -> f32 {
        1.0
    }
}

impl Midi for Micromoog {
    fn compute_state(&mut self) {
        self.state.current_note = match self.config.note_priority {
            NotePriority::First => self.state.activated_notes.first(),
            NotePriority::Last => self.state.activated_notes.last(),
            NotePriority::High => self.state.activated_notes.highest(),
            NotePriority::Low => self.state.activated_notes.lowest(),
        }
        .unwrap_or(self.state.current_note);
    }

    fn receive_midi(&mut self, msg: MidiMessage) -> () {
        match msg {
            MidiMessage::NoteOff(channel, note, velocity) => {
                if self.can_voice(&note) {
                    self.state.activated_notes.remove(note);
                    info!(
                        "Micromoog received a NoteOff event: channel {}, note {}, velocity: {}",
                        channel.number(),
                        note.to_str(),
                        u8::from(velocity)
                    );
                } else {
                    info!(
                        "Ignoring out-of-range Note Off event: channel {}, note {}, velocity: {}",
                        channel.number(),
                        note.to_str(),
                        u8::from(velocity)
                    );
                }
            }
            MidiMessage::NoteOn(channel, note, velocity) => {
                if self.can_voice(&note) {
                    self.state.activated_notes.add(note);
                    info!(
                        "Micromoog received a NoteOn event: channel {}, note {}, velocity: {}",
                        channel.number(),
                        note.to_str(),
                        u8::from(velocity)
                    );
                } else {
                    info!(
                        "Ignoring out-of-range Note On event: channel {}, note {}, velocity: {}",
                        channel.number(),
                        note.to_str(),
                        u8::from(velocity)
                    );
                }
            }
            _ => {
                let mut data = [0_u8; 3];
                msg.copy_to_slice(&mut data).unwrap();
                info!(
                    "Micromoog does not implement the following valid MIDI message: {}",
                    data
                );
            }
        };
    }
}
