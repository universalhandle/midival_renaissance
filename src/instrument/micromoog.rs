use defmt::*;
use wmidi::{MidiMessage, Note};

use crate::{
    activated_notes::ActivatedNotes,
    configuration::{EnvelopeTrigger, NotePriority},
    instrument::{Instructions, Midi, parse_midi},
    module::keyboard::{Keyboard, KeyboardSpec},
};

#[derive(Debug)]
enum InputMode {
    /// Notes are played via the keyboard module, as though a performer were playing the instrument directly, respecting
    /// the synth's octave, frequency, doubling, and fine tune controls. The synth's glide setting is overridden, as this
    /// is part of the keyboard module. MIDI input signals which keys are struck, indirectly determining pitch (based on the
    /// aforementioned hardware setting) and filter cutoff. (The filter cutoff tracks the keyboard to various degrees depending
    /// on the filter mode setting.)
    Keyboard,
    /// TODO
    Oscillator,
}

impl Default for InputMode {
    fn default() -> Self {
        Self::Keyboard
    }
}

pub struct Settings {
    envelope_trigger: EnvelopeTrigger,
    input_mode: InputMode,
    note_priority: NotePriority,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            envelope_trigger: EnvelopeTrigger::BreakEnd,
            input_mode: InputMode::default(),
            note_priority: NotePriority::Low,
        }
    }
}

/// Immutable stuff about the hardware, like number of keys, voltage/octave values, etc.
struct Modules {
    keyboard: KeyboardSpec,
}

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
    settings: Settings,
    modules: Modules,
    state: State,
}

impl Micromoog {
    fn new(settings: Settings) -> Self {
        Self {
            settings,
            modules: Modules {
                keyboard: KeyboardSpec::new(Note::F3..=Note::C6, 1.0),
            },
            state: State::default(),
        }
    }

    /// this belongs in some trait TBD; this isn't a concern of the Micromoog per se but really of any instrument which has a keyboard
    fn keyboard_voltage(&self) -> u16 {
        (self.get_voltage()
            // This is the reference voltage 3.333333; TODO: this should not be hardcoded, as reference voltages may vary
            / (10.0 / 3.0)
            // The calculation above gives the percentage of the reference voltage; below we scale it to 12 bits; this
            // also shouldn't be hardcoded, as it's specific to this particular DAC (other hardware might have different
            // resolutions)
            * 4095.0)
            // Casting to u16 serves as a quick and dirty rounding. The DAC resolution is high enough I don't think this will
            // matter.
            as u16
    }
}

impl Default for Micromoog {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}

impl Keyboard for Micromoog {
    fn get_voltage(&self) -> f32 {
        let nth_key = self.state.current_note as u8 - *self.playable_notes().start() as u8;
        nth_key as f32 * self.volts_per_octave() / 12.0
    }

    fn playable_notes(&self) -> &core::ops::RangeInclusive<Note> {
        self.modules.keyboard.playable_notes()
    }

    fn volts_per_octave(&self) -> f32 {
        self.modules.keyboard.volts_per_octave()
    }
}

/// Gah this is messy. We probably want to factor state out of this, yadda yadda. And did the Micromoog receive MIDI, or did
/// the MIDIval Renaissance?
impl Midi for Micromoog {
    fn handle_midi(&mut self, bytes: &[u8]) -> Instructions {
        let mut i = 0;
        while i < bytes.len() {
            let data = &bytes[i..];

            if let Ok((potential_msg, bytes_processed)) = parse_midi(data) {
                if let Some(msg) = potential_msg {
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
                            info!("Ignoring valid MIDI message: {}", data);
                        }
                    };
                }
                i += bytes_processed;
            }
        }
        // by this point our state re depressed keys is updated; we can calculate a new note and return the new state
        self.state.current_note = match self.settings.note_priority {
            NotePriority::First => self.state.activated_notes.first(),
            NotePriority::Last => self.state.activated_notes.last(),
            NotePriority::High => self.state.activated_notes.highest(),
            NotePriority::Low => self.state.activated_notes.lowest(),
        }
        .unwrap_or(self.state.current_note);

        Instructions {
            keyboard_voltage: self.keyboard_voltage(),
            note_on: !self.state.activated_notes.is_empty(),
        }
    }
}
