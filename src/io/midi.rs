use crate::instrument::Instrument;
use defmt::error;
use enum_dispatch::enum_dispatch;
use wmidi::MidiMessage;

/// A trait for processing MIDI messages.
///
/// Because not all MIDI messages have an obvious immediate expression (e.g., BPM) and because sometimes multiple messages are received at once
/// (e.g., when a chord is played), the processing of input and its expression are separate.
#[enum_dispatch(Instrument)]
pub trait Midi {
    /// Sets the _calculated_ properties of state (based on both MIDI input and configuration).
    ///
    /// Whereas [`receive_midi()`](`Self::receive_midi()`) straightforwardly incorporates data from received messages into state, `compute_state`
    /// updates those properties which are a little less immediate or which must be calculated. Notably, this allows the
    /// instrument to receive a chord in its entirety, and moreover to consult configurations such as note priorty, before setting
    /// the note to be played.
    fn compute_state(&mut self);

    /// Updates internal state given a single MIDI message.
    fn receive_midi(&mut self, msg: MidiMessage) -> ();
}

/// Construct MIDI messages from data assumed to be USB-MIDI Event Packets.
///
/// Given bytes, returns an iterator over the MIDI messages therein.
pub fn bytes_to_midi_message_iterator(data: &[u8]) -> impl Iterator<Item = MidiMessage> {
    data.chunks(4).filter_map(|potential_packet| {
        if potential_packet.len() != 4 {
            error!("USB-MIDI Event Packets must always be 32 bits long");
            None
        } else {
            // the zeroth bit is intentionally ignored because the Packet Header is not of interest; the remaining
            // three bits contain the actual MIDI event
            MidiMessage::from_bytes(&potential_packet[1..]).ok()
        }
    })
}

pub fn is_note_event(msg: &MidiMessage) -> bool {
    match msg {
        MidiMessage::NoteOff(..) | MidiMessage::NoteOn(..) => true,
        _ => false,
    }
}
