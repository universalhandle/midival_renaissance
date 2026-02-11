//! Controls the device's communication with the KBD input.

use embassy_stm32::{
    dac::{DacCh1, Value},
    mode::Async,
    peripherals::DAC1,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

pub static KBD: Signal<CriticalSectionRawMutex, Value> = Signal::new();

/// Helper function to convert the voltage required for an instrument to play a specific note to a <abbr name="digital-to-analog converter">DAC</abbr> value.
///
/// There's an uncomfortable amount of hardcoding here. Ideally we could do without it, but, if not, this is the most appropriate place for it, as this is
/// where all the hardware-specific code goes.
pub fn voltage_to_dac_value(voltage: f32) -> Value {
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
pub async fn keyboard(mut dac: DacCh1<'static, DAC1, Async>) -> ! {
    loop {
        let value = KBD.wait().await;
        dac.set(value);
    }
}
