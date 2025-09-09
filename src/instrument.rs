use defmt::error;
use enum_dispatch::enum_dispatch;
use micromoog::Micromoog;
use wmidi::{Error, MidiMessage};

mod micromoog;

#[enum_dispatch]
pub enum Instrument {
    Micromoog(Micromoog),
}

impl Default for Instrument {
    fn default() -> Self {
        Self::Micromoog(Micromoog::default())
    }
}

/// Somewhat redundant with State; need to consolidate
pub struct Instructions {
    keyboard_voltage: u16,
    note_on: bool,
}

impl Instructions {
    pub fn keyboard_voltage(&self) -> u16 {
        self.keyboard_voltage
    }

    pub fn note_on(&self) -> bool {
        self.note_on
    }
}

/// A trait for processing MIDI messages.
///
/// Because not all MIDI messages have an obvious immediate expression (e.g., BPM) and because sometimes multiple messages are received at once
/// (e.g., when a chord is played), the processing of input and its expression are separate.
#[enum_dispatch(Instrument)]
pub trait Midi {
    /// Updates internal state given a single MIDI message.
    fn process_midi(&mut self, msg: MidiMessage) -> ();

    /// Updates internal state given one or more MIDI messages.
    fn process_usb_data(&mut self, data: &[u8]) {
        let mut bytes = Some(data);

        while let Some(data) = bytes {
            // unwrapping for now, but need to think about what to do in case the device receives unparseable MIDI;
            // wouldn't want to crash the device because some controller has a bug...
            let (msg, unprocessed_bytes) = parse_usb_midi_packets(data).unwrap();
            bytes = unprocessed_bytes;
            self.process_midi(msg);
        }
    }

    /// Expresses the current state of the implementor.
    ///
    /// This is mutable to allow for last-second processing. For example, when multiple keys are depressed, the note priority setting
    /// becomes important. If there is a glide setting as well, it is more appropriate to calculate the current note, the last note, etc.,
    /// as the notes are performed rather than as they are received. I'm feeling this out as I go; it may be feasible/desirable to
    /// separate these concerns later.
    fn voice(&mut self) -> Instructions;
}

/// Attempts to construct a MIDI message from data, four bytes at a time.
///
/// Returns the MidiMessage result as well as any unprocessed bytes. As incoming data sometimes contains multiple messages
// (i.e., when a chord is played), returning the unprocessed bytes allows using this function in a loop to make additional passes.
fn parse_usb_midi_packets(data: &[u8]) -> Result<(MidiMessage<'_>, Option<&[u8]>), Error> {
    if data.len() < 4 {
        error!("USB-MIDI Event Packets must always be 32 bits long");
        Err(Error::NotEnoughBytes)
    } else {
        // the zeroth bit is intentionally ignored because the Packet Header is not of interest; it is the remaining
        // three bits that contain the actual MIDI event
        MidiMessage::from_bytes(&data[1..4]).and_then(|msg| {
            let unprocessed_bytes = if data.len() > 4 {
                Some(&data[4..])
            } else {
                None
            };
            Ok((msg, unprocessed_bytes))
        })
    }
}
