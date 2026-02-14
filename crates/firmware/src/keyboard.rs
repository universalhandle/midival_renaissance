//! Controls the device's communication with the KBD input.

use embassy_stm32::{
    dac::{DacCh1, Value},
    mode::Async,
    peripherals::DAC1,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use midival_renaissance_lib::voltage::Voltage;

pub static KBD: Signal<CriticalSectionRawMutex, Voltage> = Signal::new();

/// The reference voltage for the <abbr name="digital-to-analog converter">DAC</abbr> peripheral that services KBD input.
const REFERENCE_VOLTAGE: f64 = 10.0 / 3.0;

const DAC_RESOLUTION: u16 = 4096;
const DAC_MAX_VALUE: u16 = DAC_RESOLUTION - 1;

/// Converts the [`Voltage`] required to play a specific note to a <abbr name="digital-to-analog converter">DAC</abbr> value.
fn voltage_to_dac_value(voltage: Voltage) -> Value {
    Value::Bit12Right(
        (voltage / Voltage::from_volts(REFERENCE_VOLTAGE) * f64::from(DAC_MAX_VALUE)) as u16,
    )
}

/// Task responsible for communicating with the Micromoog's KBD input.
#[embassy_executor::task]
pub async fn keyboard(mut dac: DacCh1<'static, DAC1, Async>) -> ! {
    loop {
        let voltage = KBD.wait().await;
        let dac_value = voltage_to_dac_value(voltage);
        #[cfg(feature = "defmt")]
        defmt::info!(
            "Sending {} to DAC to achieve a voltage of {}",
            dac_value,
            voltage.as_volts()
        );
        dac.set(dac_value);
    }
}
