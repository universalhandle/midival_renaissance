use wmidi::{ControlFunction, MidiMessage};

mod activated_notes;
pub use activated_notes::*;

mod portamento;
pub use portamento::*;

/// A straightforward representation of the MIDI messages the device has received.
///
/// Related controllers are grouped together in structs of their own (see `Portamento` for example), as
/// determining appropriate behavior often requires considering several values in relation to one another.
///
/// Some data are represented in more convenient formats than those in which they were received. For example:
/// - When a note is activated, it is added to a list; when released, it is dropped from the list. As a result,
///   the state object does not explicitly persist data about NoteOff events.
/// - The values assigned to controllers for switched (i.e., on/off) functions (i.e., CC 64-69) are stored as
///   booleans rather than their original `U7` values.
///
/// This struct is expected to continue to grow as more features are added. State is persisted only as needed.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MidiState {
    /// Holds a representation of notes which are currently activated.
    pub activated_notes: ActivatedNotes,
    /// Contains a representation of MIDI controls related to the Portamento effect.
    pub portamento: Portamento,
}

impl Default for MidiState {
    fn default() -> Self {
        Self {
            activated_notes: ActivatedNotes::default(),
            portamento: Portamento::default(),
        }
    }
}

/// Given data, returns the MIDI messages contained therein, filtering out errors.
///
/// Data may contain one or more USB-MIDI Event Packets.
pub fn bytes_to_midi(data: &[u8]) -> impl Iterator<Item = MidiMessage<'_>> {
    data.chunks(4).filter_map(|potential_packet| {
        if potential_packet.len() != 4 {
            #[cfg(feature = "defmt")]
            defmt::error!("USB-MIDI Event Packets must always be 32 bits long");
            None
        } else {
            // the zeroth bit is intentionally ignored because the Packet Header is not of interest;
            // the remaining three bits contain the actual MIDI event
            MidiMessage::from_bytes(&potential_packet[1..]).ok()
        }
    })
}

impl MidiState {
    /// Updates the [`MidiState`] given a [`MidiMessage`].
    pub fn update(&mut self, msg: MidiMessage) -> () {
        match msg {
            MidiMessage::ControlChange(_channel, control_function, control_value) => {
                match control_function {
                    ControlFunction::PORTAMENTO_TIME => {
                        self.portamento.set_time(control_value);
                        #[cfg(feature = "defmt")]
                        defmt::info!(
                            "Received Portamento Time Control Change: channel {}, value: {}",
                            _channel.number(),
                            u8::from(control_value)
                        );
                    }
                    _ => {
                        #[cfg(feature = "defmt")]
                        defmt::info!(
                            "Received unsupported Control Change {} on channel {}",
                            u8::from(control_function),
                            _channel.number()
                        );
                    }
                }
            }
            MidiMessage::NoteOff(_channel, note, _velocity) => {
                self.activated_notes.remove(note);
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "Received NoteOff: channel {}, note {}, velocity: {}",
                    _channel.number(),
                    note.to_str(),
                    u8::from(_velocity)
                );
            }
            MidiMessage::NoteOn(_channel, note, _velocity) => {
                self.activated_notes.add(note);
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "Received NoteOn: channel {}, note {}, velocity: {}",
                    _channel.number(),
                    note.to_str(),
                    u8::from(_velocity)
                );
            }
            _ => {
                #[cfg(feature = "defmt")]
                {
                    let mut data = [0_u8; 3];
                    msg.copy_to_slice(&mut data).unwrap();
                    defmt::info!("Received unsupported MIDI message: {}", data);
                }
            }
        };
    }
}
