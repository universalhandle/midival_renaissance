use core::ops::RangeInclusive;

use defmt::*;
use embassy_time::Instant;
use embedded_hal::digital::OutputPin;
use wmidi::{ControlFunction, ControlValue, MidiMessage, Note};

use crate::{
    activated_notes::ActivatedNotes,
    configuration::{
        Config, EnvelopeTrigger, InputMode, InstrumentConfig, NoteEmbargo, NotePriority,
    },
    io::{
        control_voltage::ControlVoltage,
        gate::Gate,
        midi::{Midi, is_note_event},
    },
};

struct State {
    activated_notes: ActivatedNotes,
    current_note: Note,
    /// The [`Instant`] of expiry of any embargo against acting on MIDI. (`None` means no embargo; MIDI may be acted on immediately.) See [`NoteEmbargo`].
    embargo_expiry: Option<Instant>,
    /// MIDI CC 5 value
    portamento_time: ControlValue,
}

impl Default for State {
    fn default() -> Self {
        Self {
            activated_notes: ActivatedNotes::default(),
            current_note: Note::F3,
            embargo_expiry: None,
            portamento_time: ControlValue::default(),
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
            note_embargo: NoteEmbargo::None,
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
    fn gate<T: OutputPin>(&self, switch_trigger: &mut T) {
        if self.state.activated_notes.is_empty() {
            info!("Note is off");
            switch_trigger
                .set_low()
                .expect("Toggling GPIO state should be infallible");
        } else {
            info!("Note is on");
            switch_trigger
                .set_high()
                .expect("Toggling GPIO state should be infallible");
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

    fn receive_midi(&mut self, msg: MidiMessage) -> Option<Instant> {
        match msg {
            MidiMessage::ControlChange(channel, control_function, control_value) => {
                match control_function {
                    ControlFunction::PORTAMENTO_TIME => {
                        self.state.portamento_time = control_value;
                        info!(
                            "Micromoog received a Portamento Time Control Change: channel {}, value: {}",
                            channel.number(),
                            u8::from(control_value)
                        );
                    }
                    _ => {
                        info!(
                            "Micromoog does not implement Control Change {}",
                            u8::from(control_function)
                        );
                    }
                }
            }
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

        // This match statement is responsible for setting the embargo expiry (if any) based on whether the "chord cleanup"
        // feature is enabled as well timing of this note event (if any) relative to previous ones.
        match (
            self.config.note_embargo.is_enabled(),
            is_note_event(&msg),
            self.state.activated_notes.updated_at(),
            self.state.embargo_expiry,
        ) {
            // If the config is turned off, ensure the state matches.
            (false, _, _, _) => {
                self.state.embargo_expiry = None;
            }
            // Set an embargo for the first time.
            (true, true, Some(updated_at), None) => {
                self.state.embargo_expiry = Some(updated_at + self.config.note_embargo.duration());
            }
            // Set a new embargo time, as the update in question occurred after expiry of the previous embargo.
            (true, true, Some(update_at), Some(embargo_expiry)) if update_at >= embargo_expiry => {
                self.state.embargo_expiry = Some(update_at + self.config.note_embargo.duration());
            }
            // This arm captures legitimate no-op cases (e.g., a second note arrived within the embargo period of an earlier
            // one) as well as cases which should be impossible (e.g., if the current MIDI event is a note event, then
            // `updated_at` can't be `None`).
            (_, _, _, _) => {}
        }

        self.state.embargo_expiry
    }
}
