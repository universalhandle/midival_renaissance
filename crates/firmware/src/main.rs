//! MIDIval Renaissance is [Embassy](https://embassy.dev)-based firmware for a MIDI adapter targeting the
//! [Micromoog Model 2090](https://en.wikipedia.org/wiki/Micromoog), a monophonic analog synthesizer produced
//! by Moog Music from 1975 to 1979. The firmware runs on the [Nucleo-F767ZI development
//! board](https://www.st.com/en/evaluation-tools/nucleo-f767zi.html), which is powered by an F7-series
//! STM32 microcontroller.
//!
//! It works by translating MIDI messages into electrical signals compatible with the Moog Open System, which
//! is essentially a flavor of [CV/gate](https://en.wikipedia.org/wiki/CV/gate). In addition to enabling the Micromoog
//! to be controlled externally, the firmware seeks to extend the capabilities of the synthesizer by allowing the
//! [`NotePriority`][`configuration::NotePriority`] to be configured, adding arpeggiation, providing BPM (beats per minute)
//! context, and supporting keyboard expression such as aftertouch that the original hardware isn't equipped to handle.
//! (Note: not all of these features are implemented yet.)
//!
//! For details about the hardware or how to use the device, see the `README`.

#![no_std]
#![no_main]

mod chord_cleanup;
mod configuration;
mod note_provider;

use crate::{
    chord_cleanup::{CHORD_CLEANUP_SYNC, ChordCleanupSpy, chord_cleanup_config},
    note_provider::{
        NOTE_PROVIDER_SYNC, NoteProviderReceiver, display_note_provider, select_note_provider,
    },
};
use defmt::{panic, *};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_stm32::{
    Config, bind_interrupts,
    dac::{Dac, DacCh1, DacCh2, Value},
    exti::ExtiInput,
    gpio::{Level, Output, Pull, Speed},
    mode::Async,
    peripherals::{self, DAC1},
    time::Hertz,
    usb,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    signal::Signal,
    watch::{AnonReceiver, Receiver, Sender, Watch},
};
use embassy_time::{Instant, Timer};
use embassy_usb::{Builder, UsbDevice, class::midi::MidiClass, driver::EndpointError};
use midival_renaissance_lib::midi_state::{MidiState, Operation};
use static_cell::StaticCell;
use wmidi::Note;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(
    #[doc(hidden)]
    struct Irqs {
        OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
    }
);

type UsbDriver = usb::Driver<'static, peripherals::USB_OTG_FS>;

const MIDI_STATE_RECEIVER_CNT: usize = 0;
type MidiStateSync = Watch<CriticalSectionRawMutex, MidiState, MIDI_STATE_RECEIVER_CNT>;
type MidiStateSender<'a> = Sender<'a, CriticalSectionRawMutex, MidiState, MIDI_STATE_RECEIVER_CNT>;
type MidiStateSpy<'a> =
    AnonReceiver<'a, CriticalSectionRawMutex, MidiState, MIDI_STATE_RECEIVER_CNT>;

/// Synchronizes MIDI state.
static MIDI_STATE_SYNC: MidiStateSync = Watch::new();

/// Notifies the [`Instant`] at which voicing may be updated, essentially communicating the end any
/// chord cleanup period.
static VOICE_SCHEDULE: Signal<CriticalSectionRawMutex, Instant> = Signal::new();

const UPDATE_VOICING_RECEIVER_CNT: usize = 2;
type UpdateVoicingSync = Watch<CriticalSectionRawMutex, (), UPDATE_VOICING_RECEIVER_CNT>;
type UpdateVoicingSender<'a> = Sender<'a, CriticalSectionRawMutex, (), UPDATE_VOICING_RECEIVER_CNT>;
type UpdateVoicingReceiver<'a> =
    Receiver<'a, CriticalSectionRawMutex, (), UPDATE_VOICING_RECEIVER_CNT>;

/// Indicates that something has changed which may affect how (or whether) the synthesizer sounds.
static UPDATE_VOICING: UpdateVoicingSync = Watch::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Initializing MIDIval Renaissance");

    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        // hse: high-speed external clock
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Bypass,
        });

        // pll: phase-locked loop, crucial for dividing clock
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL216,
            divp: Some(PllPDiv::DIV2), // 8mhz / 4 * 216 / 2 = 216Mhz
            // per section 5.2 of RM0410: most peripheral clocks are derived from their bus clock, but the 48MHz clock used for USB OTG FS
            // is derived from main PLL VCO (PLLQ clock) or PLLSAI VCO (PLLSAI clock)
            divq: Some(PllQDiv::DIV9), // 8mhz / 4 * 216 / 9 = 48Mhz
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
    }
    let p = embassy_stm32::init(config);

    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::None);
    let note_provider_sender = NOTE_PROVIDER_SYNC.sender();
    unwrap!(spawner.spawn(select_note_provider(button, note_provider_sender)));

    let red_led = Output::new(p.PB14, Level::Low, Speed::Low);
    let note_provider_receiver = NOTE_PROVIDER_SYNC
        .receiver()
        .expect("Note provider synchronizer should have a receiver available");
    unwrap!(spawner.spawn(display_note_provider(red_led, note_provider_receiver)));

    let toggle = ExtiInput::new(p.PD1, p.EXTI1, Pull::Up);
    let blue_led = Output::new(p.PB7, Level::Low, Speed::Low);
    let chord_cleanup = CHORD_CLEANUP_SYNC.sender();
    unwrap!(spawner.spawn(chord_cleanup_config(toggle, blue_led, chord_cleanup)));

    // Create the driver, from the HAL.
    static ENDPOINT_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    let mut config = embassy_stm32::usb::Config::default();

    // USB devices which are self-powered (i.e., that can stay powered on if unplugged from the host)
    // need to enable vbus_detection to comply with the USB spec. Per section 6.10 of the Nucleo board
    // manual (UM1974), CN13 (the USB port) cannot power the board; external power is necessary.
    // See docs on `vbus_detection` for details.
    config.vbus_detection = true;

    let driver = usb::Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        ENDPOINT_OUT_BUFFER.init([0; 256]),
        config,
    );

    // per https://pid.codes, FOSS projects can apply to be listed under the vendor ID owned by InterBiometrics
    let vendor_id = 0x1209;
    // product ID honors the Micromoog (Moog Model 2090) that inspired this project
    let product_id = 0x2090;

    let mut config = embassy_usb::Config::new(vendor_id, product_id);
    config.manufacturer = Some("Pawpaw Works");
    config.product = Some("MIDIval Renaissance");
    config.self_powered = true;
    config.max_power = 0;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [], // no msos descriptors
        CONTROL_BUFFER.init([0; 64]),
    );

    // Create classes on the builder.
    let class = MidiClass::new(&mut builder, 0, 1, 64);

    // Build the builder.
    let usb = builder.build();

    // set up the DAC to output voltage to the synth
    // per RM0410 (the reference manual for the chip), DAC channel 1 outputs on port A, pin 4
    let dac_ch1_out = p.PA4;
    // DMA: direct memory access controller
    let dac_ch1_dma = p.DMA1_CH5;

    // the second DAC channel will provide as-yet unimplemented input to the Micromoog (perhaps to OSC)
    let dac_ch2_out = p.PA5;
    let dac_ch2_dma = p.DMA1_CH6;

    let (dac_ch1, dac_ch2) =
        Dac::new(p.DAC1, dac_ch1_dma, dac_ch2_dma, dac_ch1_out, dac_ch2_out).split();

    unwrap!(spawner.spawn(usb_task(usb)));

    let chord_cleanup = CHORD_CLEANUP_SYNC.anon_receiver();
    let midi_state_sender = MIDI_STATE_SYNC.sender();
    midi_state_sender.send(MidiState::default());
    unwrap!(spawner.spawn(midi_task(class, chord_cleanup, midi_state_sender)));

    let sender = UPDATE_VOICING.sender();
    unwrap!(spawner.spawn(update_voicing(sender)));

    let note_provider_receiver = NOTE_PROVIDER_SYNC
        .receiver()
        .expect("Note provider synchronizer should have a receiver available");
    let update_voicing = UPDATE_VOICING
        .receiver()
        .expect("Update voicing synchronizer should have a receiver available");
    let midi_state_receiver = MIDI_STATE_SYNC.anon_receiver();
    unwrap!(spawner.spawn(keyboard(
        dac_ch1,
        note_provider_receiver,
        update_voicing,
        midi_state_receiver
    )));

    let switch_trigger = Output::new(p.PG0, Level::Low, Speed::Low);
    let update_voicing = UPDATE_VOICING
        .receiver()
        .expect("Update voicing synchronizer should have a receiver available");
    let midi_state_receiver = MIDI_STATE_SYNC.anon_receiver();
    unwrap!(spawner.spawn(trigger(switch_trigger, update_voicing, midi_state_receiver)));

    unwrap!(spawner.spawn(tbd_task(dac_ch2)));
}

/// Task responsible for kicking off voicing tasks, delaying per the chord cleanup configuration as needed.
///
/// Waiting inside this intermediary task prevents blocking the MIDI processing task as well as the peripherals
/// that drive the attached synthesizer.
#[embassy_executor::task]
async fn update_voicing(sender: UpdateVoicingSender<'static>) {
    loop {
        let expiry = { VOICE_SCHEDULE.wait().await };
        Timer::at(expiry).await;
        sender.send(());
    }
}

/// Task responsible for communicating with the Micromoog's KBD input.
#[embassy_executor::task]
async fn keyboard(
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
            configuration::NotePriority::First => state.activated_notes.first(),
            configuration::NotePriority::Last => state.activated_notes.last(),
            configuration::NotePriority::Low => state.activated_notes.lowest(),
            configuration::NotePriority::High => state.activated_notes.highest(),
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

/// Task responsible for communicating with the Micromoog's S-TRIG input.
#[embassy_executor::task]
async fn trigger(
    mut switch_trigger: Output<'static>,
    mut update_voicing: UpdateVoicingReceiver<'static>,
    mut midi_state: MidiStateSpy<'static>,
) -> ! {
    loop {
        let _ = { update_voicing.changed().await };
        let state = midi_state
            .try_get()
            .expect("MIDI state should never be uninitialized");

        if state.activated_notes.is_empty() {
            info!("Note is off");
            switch_trigger.set_low();
        } else {
            info!("Note is on");
            switch_trigger.set_high();
        }
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, UsbDriver>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn midi_task(
    mut class: MidiClass<'static, UsbDriver>,
    mut chord_cleanup: ChordCleanupSpy<'static>,
    mut midi_state: MidiStateSender<'static>,
) -> ! {
    loop {
        class.wait_connection().await;
        info!("USB connected");
        let _ = process_midi(&mut class, &mut chord_cleanup, &mut midi_state).await;
        info!("USB disconnected");
    }
}

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

#[doc(hidden)]
struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

/// Helper function which interprets data received over USB.
///
/// Extracts MIDI from bytes, updates state, and schedules voicing update if appropriate.
async fn process_midi<'d, T: usb::Instance + 'd>(
    class: &mut MidiClass<'d, usb::Driver<'d, T>>,
    chord_cleanup: &mut ChordCleanupSpy<'static>,
    midi_state: &mut MidiStateSender<'static>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    let mut chord_cleanup_start: Option<Instant> = None;
    loop {
        let n = class.read_packet(&mut buf).await?;
        let mut state = *(midi_state
            .try_get()
            .as_mut()
            .expect("MIDI state should never be uninitialized"));
        let operation = state.update(&buf[..n]);

        midi_state.send(state);

        // most changes in MIDI state should be acted upon immediately; however, the voicing of notes must be scheduled
        // according to the chord cleanup configuration
        if operation.contains(Operation::NoteChange) {
            let now = Instant::now();
            let cc = chord_cleanup
                .try_get()
                .expect("Chord cleanup state should never be uninitialized");

            match (cc.is_enabled(), chord_cleanup_start) {
                // chord cleanup is enabled but hasn't started
                (true, None) => {
                    chord_cleanup_start = Some(now);
                    VOICE_SCHEDULE.signal(now + cc.duration());
                }
                // chord cleanup is enabled...
                (true, Some(start)) => {
                    let expiry = start + cc.duration();

                    // ...and this note event lands outside the previous cleanup period, marking the beginning of a new period
                    if now > expiry {
                        chord_cleanup_start = Some(now);
                        VOICE_SCHEDULE.signal(now + cc.duration());
                    } else {
                        info!(
                            "Note event received during chord cleanup period, will be considered in batch"
                        );
                    }
                }
                (false, _) => {
                    chord_cleanup_start = None;
                    VOICE_SCHEDULE.signal(now);
                }
            }
        }
    }
}

/// Placeholder task to ensure both DAC channels are used, preventing the DAC itself from being disabled;
/// see <https://github.com/embassy-rs/embassy/issues/4577>.
#[embassy_executor::task]
async fn tbd_task(dac: DacCh2<'static, DAC1, Async>) -> ! {
    loop {
        Timer::after_secs(60).await;
        info!("TBD task placeholder DAC reading: {}", dac.read());
    }
}
