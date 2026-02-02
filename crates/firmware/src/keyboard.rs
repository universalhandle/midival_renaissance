//! Controls the device's communication with the KBD input.

use crate::{MidiStateSpy, UpdateVoicingReceiver, note_provider::NoteProviderReceiver};
use defmt::info;
use embassy_futures::select::{Either, select};
use embassy_stm32::{
    dac::{DacCh1, Value},
    mode::Async,
    peripherals::DAC1,
};
use midival_renaissance_lib::configuration::NotePriority;
use wmidi::Note;

/// Helper function to convert the voltage required for an instrument to play a specific note to a <abbr name="digital-to-analog converter">DAC</abbr> value.
///
/// There's an uncomfortable amount of hardcoding here. Ideally we could do without it, but, if not, this is the most appropriate place for it, as this is
/// where all the hardware-specific code goes.
fn voltage_to_dac_value(voltage: f32) -> Value {
    Value::Bit12Right(
        (voltage
            // This is the reference voltage 3.333333; TODO: this should not be hardcoded, as reference voltages may vary
            / (10.0 / 3.0)
            // The calculation above gives the percentage of the reference voltage; below we scale it to 12 bits; this
            // also shouldn't be hardcoded, as it's specific to this particular DAC (other hardware might have different
            // resolutions)
            * 4095.0)
            // Casting to u16 serves as a quick and dirty rounding. The DAC resolution is high enough I don't think this will
            // matter.
            as u16,
    )
}

/// Task responsible for communicating with the Micromoog's KBD input.
#[embassy_executor::task]
pub async fn keyboard(
    mut dac: DacCh1<'static, DAC1, Async>,
    mut note_provider: NoteProviderReceiver<'static>,
    mut update_voicing: UpdateVoicingReceiver<'static>,
    mut midi_state: MidiStateSpy<'static>,
) -> ! {
    // TODO: if/when support for additional instruments is added, these values should change based on the instrument
    // selection rather than be hardcoded here
    let playable_notes = Note::F3..=Note::C6;
    let volts_per_octave = 1.0_f32;
    let default_note = Note::F3;

    let mut voiced_note: Note = default_note;
    loop {
        let note_priority = match select(update_voicing.changed(), note_provider.changed()).await {
            Either::First(_) => note_provider.get().await,
            Either::Second(np) => np,
        };

        let state = midi_state
            .try_get()
            .expect("MIDI state should never be uninitialized");

        voiced_note = match note_priority {
            NotePriority::First => state.activated_notes.first(),
            NotePriority::Last => state.activated_notes.last(),
            NotePriority::Low => state.activated_notes.lowest(),
            NotePriority::High => state.activated_notes.highest(),
        }
        // when all keys have been released, the oscillator is meant to retain the frequency of the last played note
        .unwrap_or(voiced_note);

        let nth_key = voiced_note as u8 - *playable_notes.start() as u8;
        let voltage = nth_key as f32 * volts_per_octave / 12.0;

        let dac_value = voltage_to_dac_value(voltage);
        info!(
            "Sending {} to DAC to achieve a voltage of {}",
            dac_value, voltage
        );
        dac.set(dac_value);
    }
}
