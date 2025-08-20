use defmt::*;
use wmidi::{MidiMessage, Note, U7};

use crate::{
    active_keys::ActiveKeys,
    configuration::{EnvelopeTrigger, NotePriority},
    instrument::{Instructions, Midi, parse_midi},
    module::keyboard::KeyboardSpec,
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

#[derive(Default)]
struct State {
    active_keys: ActiveKeys,
    current_note: U7,
}

// maybe the essential configs are type parameters, so that we impl TraitX
// differently for Micromoog<InputMode=Keyboard> vs Micromoog<InputMode=Oscillator>?
pub struct Micromoog {
    settings: Settings,
    modules: Modules,
    state: State,
}

impl Micromoog {
    pub fn new(settings: Settings) -> Self {
        // this feels weird here, maybe should be a associated constant somewhere?
        let low_key = U7::from_u8_lossy(Note::F3 as u8);
        Self {
            settings,
            modules: Modules {
                keyboard: KeyboardSpec::new(low_key, 0.0, U7::new(32).unwrap(), 1.0),
            },
            state: State {
                current_note: low_key.into(),
                ..State::default()
            },
        }
    }

    /// this belongs in some trait TBD -- and some of these checks really ought to happen earlier (e.g., when the note is received, or when
    /// the active note is selected); this is much to late to deal with questions of whether or not the note is in range of the attached
    /// instrument, for example.... furthermore, this isn't a concern of the Micromoog per se but really of any instrument which has a keyboard
    fn keyboard_voltage(&self) -> u16 {
        let mut note = u8::from(self.state.current_note);
        let key_count = u8::from(self.modules.keyboard.key_count());
        let floor = u8::from(self.modules.keyboard.low_key_note());
        let ceiling = floor + key_count - 1;

        note = if note >= floor && note <= ceiling {
            note
        } else {
            info!(
                "Played note ({}) out of range; forcing lowest note as an arbitrary default to protect the synth.",
                note
            );
            floor
        };

        // counting from zero, of course
        let nth_key = note - floor;

        let voltage = nth_key as f32 * self.modules.keyboard.volts_per_octave() / 12.0;

        // scale and round off () the voltage
        (voltage
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
                            self.state.active_keys.remove(U7::from_u8_lossy(note as u8));
                            info!(
                                "Micromoog received a NoteOff event: channel {}, note {}, velocity: {}",
                                channel.number(),
                                note.to_str(),
                                u8::from(velocity)
                            );
                        }
                        MidiMessage::NoteOn(channel, note, velocity) => {
                            self.state.active_keys.add(U7::from_u8_lossy(note as u8));
                            info!(
                                "Micromoog received a NoteOn event: channel {}, note {}, velocity: {}",
                                channel.number(),
                                note.to_str(),
                                u8::from(velocity)
                            );
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
        let active = self.state.active_keys.get();
        self.state.current_note = *match self.settings.note_priority {
            // TBD: how and when should the vec be sorted? e.g., should note priority be taken into account when values go into
            // state or only when they come out? do we expect note priority to be something that a performer can change? is sorting
            // a slice a destructive/mutable act in rust by default; does a copy need to be made if we wish to preserve the order?
            NotePriority::First | NotePriority::Last => self::todo!(),
            NotePriority::High => active.last(),
            NotePriority::Low => active.first(),
        }
        .unwrap_or(&self.state.current_note);

        Instructions {
            keyboard_voltage: self.keyboard_voltage(),
            note_on: !self.state.active_keys.is_empty(),
        }
    }
}
