//! Tasks and types related the configurations which determine which note will sound.

use crate::configuration::{CycleConfig, NotePriority};
use embassy_stm32::{exti::ExtiInput, gpio::Output};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    watch::{Receiver, Sender, Watch},
};
use embassy_time::Timer;

const NOTE_PROVIDER_RECEIVER_CNT: usize = 2;
/// Syncs note provider config across tasks.
pub static NOTE_PROVIDER_SYNC: Watch<
    CriticalSectionRawMutex,
    NotePriority,
    NOTE_PROVIDER_RECEIVER_CNT,
> = Watch::new_with(NotePriority::Low);
pub type NoteProviderSender<'a> =
    Sender<'a, CriticalSectionRawMutex, NotePriority, NOTE_PROVIDER_RECEIVER_CNT>;
pub type NoteProviderReceiver<'a> =
    Receiver<'a, CriticalSectionRawMutex, NotePriority, NOTE_PROVIDER_RECEIVER_CNT>;

/// Handles button presses, cycling through the [`NotePriority`] configurations.
#[embassy_executor::task]
pub async fn select_note_provider(
    mut button: ExtiInput<'static>,
    note_provider: NoteProviderSender<'static>,
) -> ! {
    loop {
        button.wait_for_rising_edge().await;

        let new_state = note_provider
            .try_get()
            .expect("Note provider state should never be uninitialized")
            .cycle();
        note_provider.send(new_state);
    }
}

/// Provides a quick and dirty status indicator for user-configurable [`NotePriority`][`configuration::NotePriority`].
///
/// Each cycle is divided in half. The LED remains dark for one half. For the other, the
/// LED lights up N times (where N is one more than the index of the selected item).
/// Of course this this won't scale well, but it suits our purposes for now.
#[embassy_executor::task]
pub async fn display_note_provider(
    mut led: Output<'static>,
    mut note_provider: NoteProviderReceiver<'static>,
) -> ! {
    const BLINK_SLEEP_MS: u64 = 1_000_000;

    loop {
        led.set_low();
        Timer::after_micros(BLINK_SLEEP_MS).await;

        // since the index starts with 0, 1 is added or else the LED wouldn't blink at all for the "first" (i.e., zeroth) configuration option
        let blink_cnt = { note_provider.get().await as u8 }.saturating_add(1);
        // mult by two to account for the "off" periods, sub 1 so the LED always starts and ends lit
        let animation_frames = blink_cnt * 2 - 1;
        let mut counter = animation_frames;
        while counter > 0 {
            led.toggle();
            Timer::after_micros(BLINK_SLEEP_MS / u64::from(animation_frames)).await;
            counter -= 1;
        }
    }
}
