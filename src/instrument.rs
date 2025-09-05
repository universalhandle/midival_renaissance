use defmt::info;
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

#[enum_dispatch(Instrument)]
pub trait Midi {
    fn handle_midi(&mut self, msg: &[u8]) -> Instructions;
}

// This is a fairly janky parser. On "success" it returns (1) an optional MidiMessage as well as (2) the number
// of bytes that have been processed. The optionality of 1 is due to Embassy [for whatever
// reason](https://github.com/embassy-rs/embassy/issues/4537) duplicating part of the message; used in
// a loop, this function can filter out said noise. This provides a partial explanation for 2; the loop can simply
// skip over bits determined to be errata. The other reason for 2 is that sometimes `data` contains multiple messages
// (i.e., when a chord is played); knowing how many bits have been processed allows the loop to make additional passes.
pub fn parse_midi(data: &[u8]) -> Result<(Option<MidiMessage<'_>>, usize), wmidi::FromBytesError> {
    match MidiMessage::from_bytes(data) {
        Ok(msg) => {
            let processed_bytes = msg.bytes_size();
            Ok((Some(msg), processed_bytes))
        }
        Err(e) => match e {
            Error::UnexpectedDataByte => {
                info!("Discarding malformed status bit: {:x}", data[0]);
                Ok((None, 1))
            }
            _ => {
                info!("Unknown parsing problem");
                Err(e)
            }
        },
    }
}
