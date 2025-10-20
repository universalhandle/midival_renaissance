#![no_std]
#![no_main]

mod activated_notes;
mod configuration;
mod instrument;
mod io;

use core::fmt;

use crate::{
    configuration::{Config as _, CycleConfig},
    instrument::Instrument,
    io::{
        control_voltage::ControlVoltage,
        gate::Gate,
        midi::{Midi, bytes_to_midi_message_iterator},
    },
};
use defmt::{panic, *};
use embassy_executor::Spawner;
use embassy_stm32::{
    Config, bind_interrupts,
    dac::{Dac, DacCh1, DacCh2, Value},
    exti::ExtiInput,
    gpio::{Level, Output, Pull, Speed},
    mode::Async,
    peripherals::{self, DAC1, EXTI13},
    time::Hertz,
    usb,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex, signal::Signal};
use embassy_time::Timer;
use embassy_usb::{Builder, UsbDevice, class::midi::MidiClass, driver::EndpointError};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

type InstrumentAsyncMutex = mutex::Mutex<CriticalSectionRawMutex, Instrument>;
type UsbDriver = usb::Driver<'static, peripherals::USB_OTG_FS>;

static OUTPUT_UPDATE_REQUIRED: Signal<CriticalSectionRawMutex, bool> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

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

    static INSTRUMENT: StaticCell<InstrumentAsyncMutex> = StaticCell::new();
    let instrument = INSTRUMENT.init(mutex::Mutex::new(Instrument::default()));

    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::None);
    unwrap!(spawner.spawn(note_priority_input_task(button, instrument)));

    let red_led = Output::new(p.PB14, Level::Low, Speed::Low);
    unwrap!(spawner.spawn(note_priority_display_task(red_led, instrument)));

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

    // I set up the second DAC channel but didn't use it
    let dac_ch2_out = p.PA5;
    let dac_ch2_dma = p.DMA1_CH6;

    let (dac_ch1, dac_ch2) =
        Dac::new(p.DAC1, dac_ch1_dma, dac_ch2_dma, dac_ch1_out, dac_ch2_out).split();

    let switch_trigger = Output::new(p.PG0, Level::Low, Speed::Low);

    unwrap!(spawner.spawn(usb_task(usb)));
    unwrap!(spawner.spawn(midi_task(class, instrument)));
    unwrap!(spawner.spawn(voice_task(dac_ch1, switch_trigger, instrument)));
    unwrap!(spawner.spawn(tbd_task(dac_ch2)));
}

/// Task responsible for voicing, i.e., should the instrument play a note, and if so which?
#[embassy_executor::task]
async fn voice_task(
    mut dac: DacCh1<'static, DAC1, Async>,
    mut switch_trigger: Output<'static>,
    instrument: &'static InstrumentAsyncMutex,
) -> ! {
    loop {
        // The value doesn't really matter; we just need to know if "an event was raised." The actual state is read from elsewhere.
        OUTPUT_UPDATE_REQUIRED.wait().await;
        let mut instr = instrument.lock().await;

        // There's a bit of inconsistency in approach here. On the one hand, I'm hesitant to expose values (e.g., the note to play)
        // outside of the instrument, because I like the safety provided by knowing the instrument's note range/rejecting MIDI
        // messages outside that range. (Perhaps I'm overly sensitive to (imagined?) edge cases where the externalizing the note
        // results in the device sending harmful current in an attempt to play an out-of-range note.) On the other hand, I haven't
        // decided how much the library code, with its fairly music-focused logic, needs to know about the hardware (i.e., the
        // microprocessor and its peripherals). As a result, I end up gluing that all together here, perhaps awkwardly:
        //
        // - compute_state is just weird; if it must exist at all, it seems like it should be a private method; internally mutating
        //   state, taking no input, and returning nothing... code smell
        // - the aforementioned safety goes out the window the moment the note is converted to voltage; either I should bite the bullet and
        //   allow these values to be returned from the object, or I should pass in some reference to the hardware peripherals
        instr.compute_state();

        let voltage = instr.current_note_to_voltage();
        let dac_value = voltage_to_dac_value(voltage);
        info!(
            "Sending {} to DAC to achieve a voltage of {}",
            dac_value, voltage
        );
        dac.set(dac_value);

        if instr.gate_is_high() {
            info!("Note is on");
            switch_trigger.set_high();
        } else {
            info!("Note is off");
            switch_trigger.set_low();
        }
    }
}

#[embassy_executor::task]
async fn note_priority_input_task(
    mut button: ExtiInput<'static>,
    instrument: &'static InstrumentAsyncMutex,
) -> ! {
    loop {
        button.wait_for_rising_edge().await;
        let mut instr = instrument.lock().await;
        let note_priority = instr.config().note_priority;
        instr.config_mut().note_priority = note_priority.cycle();
    }
}

/// Provides a quick and dirty status indicator for user-configurable items.
///
/// Each cycle is divided in half. The LED remains dark for one half. For the other, the
/// LED lights up N times (where N is one more than the index of the selected item).
/// Of course this this won't scale well, but it suits our purposes for now.
#[embassy_executor::task]
async fn note_priority_display_task(
    mut led: Output<'static>,
    instrument: &'static InstrumentAsyncMutex,
) -> ! {
    const BLINK_SLEEP_MS: u64 = 1_000_000;

    loop {
        led.set_low();
        Timer::after_micros(BLINK_SLEEP_MS).await;

        // since the index starts with 0, 1 is added or else the LED wouldn't blink at all for the "first" (i.e., zeroth) configuration option
        let blink_cnt = (instrument.lock().await.config().note_priority as u8).saturating_add(1);
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

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, UsbDriver>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn midi_task(
    mut class: MidiClass<'static, UsbDriver>,
    instrument: &'static InstrumentAsyncMutex,
) -> ! {
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = process_usb_data(&mut class, instrument).await;
        info!("Disconnected");
    }
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
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

async fn process_usb_data<'d, T: usb::Instance + 'd>(
    class: &mut MidiClass<'d, usb::Driver<'d, T>>,
    instrument: &'static InstrumentAsyncMutex,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    loop {
        let n = class.read_packet(&mut buf).await?;
        let mut instr = instrument.lock().await;

        bytes_to_midi_message_iterator(&buf[..n]).for_each(|midi_msg| {
            instr.receive_midi(midi_msg);
        });

        OUTPUT_UPDATE_REQUIRED.signal(true);
    }
}

/// Placeholder task to ensure both DAC channels are used, preventing the DAC itself from being disabled;
/// see https://github.com/embassy-rs/embassy/issues/4577.
#[embassy_executor::task]
async fn tbd_task(dac: DacCh2<'static, DAC1, Async>) -> ! {
    info!("Starting TBD task");
    loop {
        Timer::after_secs(60).await;
        info!("TBD task dummy DAC usage: {}", dac.read());
    }
}
