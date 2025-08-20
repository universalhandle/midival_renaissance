#![no_std]
#![no_main]

mod active_keys;
mod configuration;
mod instrument;
mod module;

use core::fmt;

use crate::instrument::{
    Midi,
    micromoog::{self, Micromoog},
};
use defmt::{panic, *};
use embassy_executor::Spawner;
use embassy_stm32::{
    Config, bind_interrupts,
    dac::{Dac, DacCh1, DacCh2, Value},
    gpio::{Level, Output, Speed},
    mode::Async,
    peripherals::{self, DAC1},
    time::Hertz,
    usb,
};
use embassy_time::Timer;
use embassy_usb::{Builder, UsbDevice, class::midi::MidiClass, driver::EndpointError};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

type UsbDriver = usb::Driver<'static, peripherals::USB_OTG_FS>;

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, UsbDriver>) -> ! {
    info!("Starting USB task");
    usb.run().await
}

#[embassy_executor::task]
async fn echo_task(
    mut class: MidiClass<'static, UsbDriver>,
    mut dac: DacCh1<'static, DAC1, Async>,
    mut switch_trigger: Output<'static>,
) -> ! {
    info!("Starting echo task");
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = midi_echo(&mut class, &mut dac, &mut switch_trigger).await;
        info!("Disconnected");
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

// If you are trying this and your USB device doesn't connect, the most
// common issues are the RCC config and vbus_detection
//
// See https://embassy.dev/book/#_the_usb_examples_are_not_working_on_my_board_is_there_anything_else_i_need_to_configure
// for more information.
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
    unwrap!(spawner.spawn(echo_task(class, dac_ch1, switch_trigger)));
    unwrap!(spawner.spawn(tbd_task(dac_ch2)));
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

async fn midi_echo<'d, T: usb::Instance + 'd>(
    class: &mut MidiClass<'d, usb::Driver<'d, T>>,
    dac: &mut DacCh1<'d, DAC1, Async>,
    switch_trigger: &mut Output<'d>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    let mut synth = Micromoog::new(micromoog::Settings::default());
    loop {
        let n = class.read_packet(&mut buf).await?;
        let instructions = synth.handle_midi(&buf[..n]);
        info!("Sending {} to DAC", instructions.keyboard_voltage());
        info!("Note is {}", instructions.note_on());
        dac.set(Value::Bit12Right(instructions.keyboard_voltage()));
        if instructions.note_on() {
            switch_trigger.set_high();
        } else {
            switch_trigger.set_low();
        }
    }
}
