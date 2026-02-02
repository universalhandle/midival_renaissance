//! Tasks and types related the [chord cleanup](`ChordCleanup`) feature.

use embassy_stm32::{exti::ExtiInput, gpio::Output};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    watch::{AnonReceiver, Sender, Watch},
};
use midival_renaissance_lib::configuration::{ChordCleanup, CycleConfig};

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
