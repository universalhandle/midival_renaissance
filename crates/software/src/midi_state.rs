use bitmask_enum::bitmask;
use defmt::*;
use wmidi::{ControlFunction, MidiMessage};

mod activated_notes;
pub use activated_notes::*;

mod portamento;
pub use portamento::*;

/// Operations that may be performed during a state update.
#[bitmask(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Operation {
    /// Indicates a note was added or removed during the last state update.
    NoteChange,
    /// Indicates a [`Portamento`] parameter changed during the last state update.
    PortamentoChange,
}

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

impl MidiState {
    /// Updates the `MidiState` given a slice of data. Returns the type of [`Operation`] performed.
    ///
    /// Data may contain one or more one or more USB-MIDI Event Packets.
    pub fn update(&mut self, data: &[u8]) -> Operation {
        let mut operation = Operation::none();
        data.chunks(4)
            .filter_map(|potential_packet| {
                if potential_packet.len() != 4 {
                    error!("USB-MIDI Event Packets must always be 32 bits long");
                    None
                } else {
                    // the zeroth bit is intentionally ignored because the Packet Header is not of interest;
                    // the remaining three bits contain the actual MIDI event
                    MidiMessage::from_bytes(&potential_packet[1..]).ok()
                }
            })
            .for_each(|msg| match msg {
                MidiMessage::ControlChange(channel, control_function, control_value) => {
                    match control_function {
                        ControlFunction::PORTAMENTO_TIME => {
                            operation |= Operation::PortamentoChange;
                            self.portamento.set_time(control_value);
                            info!(
                                "Received Portamento Time Control Change: channel {}, value: {}",
                                channel.number(),
                                u8::from(control_value)
                            );
                        }
                        _ => {
                            info!(
                                "Received unsupported Control Change {} on channel {}",
                                u8::from(control_function),
                                channel.number()
                            );
                        }
                    }
                }
                MidiMessage::NoteOff(channel, note, velocity) => {
                    operation |= Operation::NoteChange;
                    self.activated_notes.remove(note);
                    info!(
                        "Received NoteOff: channel {}, note {}, velocity: {}",
                        channel.number(),
                        note.to_str(),
                        u8::from(velocity)
                    );
                }
                MidiMessage::NoteOn(channel, note, velocity) => {
                    operation |= Operation::NoteChange;
                    self.activated_notes.add(note);
                    info!(
                        "Received NoteOn: channel {}, note {}, velocity: {}",
                        channel.number(),
                        note.to_str(),
                        u8::from(velocity)
                    );
                }
                _ => {
                    let mut data = [0_u8; 3];
                    msg.copy_to_slice(&mut data).unwrap();
                    info!("Received unsupported MIDI message: {}", data);
                }
            });
        operation
    }
}
