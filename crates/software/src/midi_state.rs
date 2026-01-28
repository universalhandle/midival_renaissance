use bitmask_enum::bitmask;
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
                    #[cfg(feature = "defmt")]
                    defmt::error!("USB-MIDI Event Packets must always be 32 bits long");
                    None
                } else {
                    // the zeroth bit is intentionally ignored because the Packet Header is not of interest;
                    // the remaining three bits contain the actual MIDI event
                    MidiMessage::from_bytes(&potential_packet[1..]).ok()
                }
            })
            .for_each(|msg| match msg {
                MidiMessage::ControlChange(_channel, control_function, control_value) => {
                    match control_function {
                        ControlFunction::PORTAMENTO_TIME => {
                            operation |= Operation::PortamentoChange;
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
                    operation |= Operation::NoteChange;
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
                    operation |= Operation::NoteChange;
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
            });
        operation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmidi::{Channel, Note, U7};

    #[test]
    fn set_portamento() {
        let mut bytes = [0_u8; 3];
        let _ = MidiMessage::ControlChange(
            Channel::Ch1,
            ControlFunction::PORTAMENTO_TIME,
            U7::from_u8_lossy(111),
        )
        .copy_to_slice(&mut bytes);
        let packet = [0, bytes[0], bytes[1], bytes[2]];

        let mut state = MidiState::default();
        let op = state.update(&packet);
        assert_eq!(
            op,
            Operation::PortamentoChange,
            "Expected left but got right"
        );
    }

    #[test]
    fn note_change() {
        let mut bytes = [0_u8; 3];
        let _ = MidiMessage::NoteOn(Channel::Ch1, Note::C4, U7::from_u8_lossy(111))
            .copy_to_slice(&mut bytes);
        let packet = [0, bytes[0], bytes[1], bytes[2]];

        let mut state = MidiState::default();
        let op = state.update(&packet);
        assert_eq!(op, Operation::NoteChange, "Expected left but got right");
    }

    #[test]
    fn note_and_portamento_change() {
        let mut note_bytes = [0_u8; 3];
        let _ = MidiMessage::NoteOn(Channel::Ch1, Note::C4, U7::from_u8_lossy(111))
            .copy_to_slice(&mut note_bytes);
        let mut portamento_bytes = [0_u8; 3];
        let _ = MidiMessage::ControlChange(
            Channel::Ch1,
            ControlFunction::PORTAMENTO_TIME,
            U7::from_u8_lossy(111),
        )
        .copy_to_slice(&mut portamento_bytes);
        let packet = [
            0,
            note_bytes[0],
            note_bytes[1],
            note_bytes[2],
            0,
            portamento_bytes[0],
            portamento_bytes[1],
            portamento_bytes[2],
        ];

        let mut state = MidiState::default();
        let op = state.update(&packet);
        assert_eq!(
            op,
            Operation::NoteChange | Operation::PortamentoChange,
            "Expected left but got right"
        );
    }

    #[test]
    fn noop() {
        let mut bytes = [0_u8; 3];
        let _ = MidiMessage::ControlChange(
            Channel::Ch1,
            // a control function unlikely ever to be supported by this device
            ControlFunction::BANK_SELECT,
            U7::from_u8_lossy(111),
        )
        .copy_to_slice(&mut bytes);
        let packet = [0, bytes[0], bytes[1], bytes[2]];

        let mut state = MidiState::default();
        let op = state.update(&packet);
        assert_eq!(op, Operation::none(), "Expected left but got right");
    }
}
