//! Tasks and types related the [chord cleanup](`ChordCleanup`) feature.

use crate::MidiStateSender;
use embassy_futures::select::{Either, select};
use embassy_stm32::{exti::ExtiInput, gpio::Output};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    signal::Signal,
    watch::{AnonReceiver, Sender, Watch},
};
use embassy_time::{Instant, Timer};
use midival_renaissance_lib::{
    configuration::{ChordCleanup, CycleConfig},
    midi_state::ActivatedNotes,
};
use wmidi::MidiMessage;

const CHORD_CLEANUP_RECEIVER_CNT: usize = 0;
/// Syncs [chord cleanup](`ChordCleanup`) config across tasks.
pub static CHORD_CLEANUP_SYNC: Watch<
    CriticalSectionRawMutex,
    ChordCleanup,
    CHORD_CLEANUP_RECEIVER_CNT,
> = Watch::new_with(ChordCleanup::None);
pub type ChordCleanupSender<'a> =
    Sender<'a, CriticalSectionRawMutex, ChordCleanup, CHORD_CLEANUP_RECEIVER_CNT>;
pub type ChordCleanupSpy<'a> =
    AnonReceiver<'a, CriticalSectionRawMutex, ChordCleanup, CHORD_CLEANUP_RECEIVER_CNT>;

/// Provisional input and status indicator for the "chord cleanup" feature.
///
/// Presently this has two states: off (no LED) and 32nd note (solid blue LED). These represent the batching delay period for
/// the "chord cleanup" feature. The input and display are provisional because I only have pushbutton inputs at present.
/// Should it turn out that more states are necessary, a selector switch seems more appropriate. If not, a toggle or slider
/// switch seems preferable to a pushbutton because they obviate the need for an indicator LED.
#[embassy_executor::task]
pub async fn chord_cleanup_config(
    mut button: ExtiInput<'static>,
    mut led: Output<'static>,
    chord_cleanup: ChordCleanupSender<'static>,
) -> ! {
    loop {
        button.wait_for_falling_edge().await;

        let new_state = chord_cleanup
            .try_get()
            .as_mut()
            .expect("Chord cleanup state should never be uninitialized")
            .cycle();
        chord_cleanup.send(new_state);

        match new_state {
            ChordCleanup::None => {
                led.set_low();
            }
            ChordCleanup::ThirtySecondNote => {
                led.set_high();
            }
        }
    }
}

type DeferredMidiSync<'a> = Signal<CriticalSectionRawMutex, (Instant, MidiMessage<'a>)>;
pub static DEFERRED_MIDI_MSG: DeferredMidiSync = Signal::new();

/// Temporarily caches note events that comprise the performance (or release) of a chord, atomically applying them
/// upon expiry of the chord cleanup batching period.
#[embassy_executor::task]
pub async fn handle_deferred_midi_msg(midi_state: MidiStateSender<'static>) -> ! {
    let mut deferred_notes = ActivatedNotes::new();
    let mut expiry: Option<Instant> = None;

    loop {
        // if a chord cleanup period is active…
        if let Some(x) = expiry {
            // …this task wakes on either receipt of new MIDI or end of the period…
            match select(Timer::at(x), DEFERRED_MIDI_MSG.wait()).await {
                Either::First(_) => {
                    #[cfg(feature = "defmt")]
                    defmt::info!("Chord cleanup period over; updating state");
                    expiry = None;

                    let mut state = midi_state
                        .try_get()
                        .expect("MIDI state should never be uninitialized");
                    state.activated_notes = deferred_notes;
                    midi_state.send(state);
                }
                Either::Second((_, msg)) => {
                    store_note_event(msg, &mut deferred_notes);
                }
            }
        // …otherwise, the task wakes on new MIDI, initiating a new chord cleanup period
        } else {
            let (x, msg) = DEFERRED_MIDI_MSG.wait().await;
            #[cfg(feature = "defmt")]
            defmt::info!("Initiating chord cleanup period");
            expiry = Some(x);
            // Take a snapshot of the current state of activated notes to use as the basis for the atomic
            // update at the end of the cleanup period.
            deferred_notes = midi_state
                .try_get()
                .expect("MIDI state should never be uninitialized")
                .activated_notes;
            store_note_event(msg, &mut deferred_notes);
        }
    }

    fn store_note_event(msg: MidiMessage, store: &mut ActivatedNotes) {
        match msg {
            MidiMessage::NoteOff(_channel, note, _velocity) => {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "Batching NoteOff: channel {}, note {}, velocity: {}",
                    _channel.number(),
                    note.to_str(),
                    u8::from(_velocity)
                );
                store.remove(note);
            }
            MidiMessage::NoteOn(_channel, note, _velocity) => {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "Batching NoteOn: channel {}, note {}, velocity: {}",
                    _channel.number(),
                    note.to_str(),
                    u8::from(_velocity)
                );
                store.add(note);
            }
            _ => {
                panic!("Only NoteOff and NoteOn events may be deferred");
            }
        }
    }
}
