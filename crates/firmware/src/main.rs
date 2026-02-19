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
mod keyboard;
mod note_provider;

use crate::{
    chord_cleanup::{CHORD_CLEANUP_SYNC, ChordCleanupSpy, DEFERRED_MIDI_MSG, chord_cleanup_config},
    keyboard::KBD,
    note_provider::{
        NOTE_PROVIDER_SYNC, NoteProviderReceiver, display_note_provider, select_note_provider,
    },
};
use defmt::{panic, *};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_stm32::{
    Config, bind_interrupts,
    dac::Dac,
    exti::{self, ExtiInput},
    gpio::{Level, Output, Pull, Speed},
    interrupt,
    peripherals::{self},
    time::Hertz,
    usb,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    signal::Signal,
    watch::{Receiver, Sender, Watch},
};
use embassy_time::Instant;
use embassy_usb::{Builder, UsbDevice, class::midi::MidiClass, driver::EndpointError};
use midival_renaissance_lib::{
    configuration::{Keyboard, NotePriority},
    midi_state::{MidiState, bytes_to_midi},
    portamento::Portamento,
    voltage::Voltage,
};
use static_cell::StaticCell;
use wmidi::{MidiMessage, Note, U7};

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(
    #[doc(hidden)]
    struct Irqs {
        EXTI1 => exti::InterruptHandler<interrupt::typelevel::EXTI1>;
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
        OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
    }
);

type UsbDriver = usb::Driver<'static, peripherals::USB_OTG_FS>;

const MIDI_STATE_RECEIVER_CNT: usize = 1;
type MidiStateSync = Watch<CriticalSectionRawMutex, MidiState, MIDI_STATE_RECEIVER_CNT>;
type MidiStateSender<'a> = Sender<'a, CriticalSectionRawMutex, MidiState, MIDI_STATE_RECEIVER_CNT>;
type MidiStateReceiver<'a> =
    Receiver<'a, CriticalSectionRawMutex, MidiState, MIDI_STATE_RECEIVER_CNT>;

/// Synchronizes MIDI state.
static MIDI_STATE_SYNC: MidiStateSync = Watch::new();

enum Trigger {
    On,
    Off,
}

static TRIGGER: Signal<CriticalSectionRawMutex, Trigger> = Signal::new();

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

    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::None, Irqs);
    let note_provider_sender = NOTE_PROVIDER_SYNC.sender();
    unwrap!(spawner.spawn(select_note_provider(button, note_provider_sender)));

    let red_led = Output::new(p.PB14, Level::Low, Speed::Low);
    let note_provider_receiver = NOTE_PROVIDER_SYNC
        .receiver()
        .expect("Note provider synchronizer should have a receiver available");
    unwrap!(spawner.spawn(display_note_provider(red_led, note_provider_receiver)));

    let toggle = ExtiInput::new(p.PD1, p.EXTI1, Pull::Up, Irqs);
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

    let (dac_ch1, _dac_ch2) =
        Dac::new(p.DAC1, dac_ch1_dma, dac_ch2_dma, dac_ch1_out, dac_ch2_out).split();

    unwrap!(spawner.spawn(usb_task(usb)));

    let chord_cleanup = CHORD_CLEANUP_SYNC.anon_receiver();
    let midi_state_sender = MIDI_STATE_SYNC.sender();
    // initialize state before any dependent tasks so that they can always assume Some(state)
    midi_state_sender.send(MidiState::default());
    unwrap!(spawner.spawn(midi_task(class, chord_cleanup, midi_state_sender)));

    let note_provider = NOTE_PROVIDER_SYNC
        .receiver()
        .expect("Note provider synchronizer should have a receiver available");
    unwrap!(
        spawner.spawn(update_voicing(
            MIDI_STATE_SYNC
                .receiver()
                .expect("MIDI State synchronizer should have a receiver available"),
            note_provider,
        ))
    );

    unwrap!(spawner.spawn(keyboard::keyboard(dac_ch1)));

    unwrap!(spawner.spawn(chord_cleanup::handle_deferred_midi_msg(
        MIDI_STATE_SYNC.sender()
    )));

    let switch_trigger = Output::new(p.PG0, Level::Low, Speed::Low);
    unwrap!(spawner.spawn(trigger(switch_trigger)));
}

/// Task responsible for kicking off voicing tasks, accounting for changes in MIDI state as well as configuration.
#[embassy_executor::task]
async fn update_voicing(
    mut midi_state: MidiStateReceiver<'static>,
    mut note_provider: NoteProviderReceiver<'static>,
) {
    // TODO: if/when support for additional instruments is added, these values should change based on the instrument
    // selection rather than be hardcoded here
    let default_note = Note::F3;
    let playable_notes = Note::F3..=Note::C6;
    let voltage_per_octave = Voltage::from_volts(1.0);

    // TODO: hardcoding `NotePriority` is no good for when we want to add an arpeggiator; factor this out later
    let mut keyboard = Keyboard::new(
        NotePriority::Low,
        playable_notes.clone(),
        voltage_per_octave,
    );

    let mut portamento =
        Portamento::new(default_note, default_note, U7::from_u8_lossy(0), keyboard);

    loop {
        let (midi_state, note_provider) =
            match select(midi_state.changed(), note_provider.changed()).await {
                Either::First(state) => (state, note_provider.get().await),
                Either::Second(np) => (midi_state.get().await, np),
            };

        keyboard = Keyboard::new(note_provider, playable_notes.clone(), voltage_per_octave);

        let note = keyboard.provide_note(&midi_state.activated_notes);

        // TODO: account for changes to Portamento config as well
        if let Some(n) = note
            && portamento.destination() != n
        {
            portamento = portamento.new_destination(n)
        }

        KBD.signal(portamento.clone().glide());

        TRIGGER.signal(if midi_state.activated_notes.is_empty() {
            Trigger::Off
        } else {
            Trigger::On
        });
    }
}

/// Task responsible for communicating with the Micromoog's S-TRIG input.
#[embassy_executor::task]
async fn trigger(mut switch_trigger: Output<'static>) -> ! {
    loop {
        match TRIGGER.wait().await {
            Trigger::On => {
                #[cfg(feature = "defmt")]
                info!("Note is on");
                switch_trigger.set_high();
            }
            Trigger::Off => {
                #[cfg(feature = "defmt")]
                info!("Note is off");
                switch_trigger.set_low();
            }
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
        let bytes = &buf[..n];

        let chord_cleanup = chord_cleanup
            .try_get()
            .expect("Chord cleanup state should never be uninitialized");

        let mut state = *(midi_state
            .try_get()
            .as_mut()
            .expect("MIDI state should never be uninitialized"));

        let mut is_immediate_state_update = true;
        bytes_to_midi(bytes).for_each(|msg| match (chord_cleanup.is_enabled(), &msg) {
            (false, _) => {
                state.update(msg);
            }
            (true, MidiMessage::NoteOn(_, _, _) | MidiMessage::NoteOff(_, _, _)) => {
                is_immediate_state_update = false;
                let now = Instant::now();

                let expiry;
                match chord_cleanup_start {
                    None => {
                        chord_cleanup_start = Some(now);
                        expiry = now + chord_cleanup.duration();
                    }
                    Some(start) => {
                        let x = start + chord_cleanup.duration();
                        if now > x {
                            // in this branch, the note event arrived outside the previous cleanup period, starting a new period
                            chord_cleanup_start = Some(now);
                            expiry = now + chord_cleanup.duration();
                        } else {
                            // otherwise, the previous expiry is valid for this event
                            expiry = x;
                        }
                    }
                };

                DEFERRED_MIDI_MSG.signal((expiry, msg.to_owned()));
            }
            (true, _) => {
                state.update(msg);
            }
        });

        if is_immediate_state_update {
            midi_state.send(state);
        }
    }
}
