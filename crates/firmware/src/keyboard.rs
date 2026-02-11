//! Controls the device's communication with the KBD input.

use embassy_stm32::{
    dac::{DacCh1, Value},
    mode::Async,
    peripherals::DAC1,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Instant};
use wmidi::Note;

pub static KBD: Signal<CriticalSectionRawMutex, Value> = Signal::new();

/// Contains data necessary to execute a portamento or glide effect.
#[derive(Clone, Copy, Debug)]
pub struct Portamento {
    /// Indicates the starting point of the glide.
    ///
    /// Uses `u16` internally for compatibility with the expected DAC value. Also, when a new note is performed during
    /// a glide, a new glide should begin from exactly that point. The choice of `u16` offers a level of granularity that
    /// [`Note`] does not.
    origin: u16,
    /// Indicates the end of the glide; when this [`Note`] is reached, there is nothing left to do.
    destination: Note,
    /// The [`Instant`] at which the glide began.
    start: Instant,
    /// How long after the `start` to stretch the effect.
    duration: Duration,
}

impl Portamento {
    /// Constructs a new [`Portamento`].
    pub fn new(origin: Note, destination: Note, duration: Duration) -> Self {
        Self {
            origin: u8::from(origin).into(),
            destination: destination,
            start: Instant::now(),
            duration,
        }
    }

    /// Given a new destination, constructs a new [`Portamento`] based on the existing one.
    ///
    /// This is especially useful for starting a glide from in-between [`Note`]s.
    pub fn new_destination(self, destination: Note) -> Self {
        Self {
            origin: self.glide(),
            destination: destination,
            start: Instant::now(),
            duration: self.duration,
        }
    }

    /// Getter.
    pub fn destination(&self) -> Note {
        self.destination
    }

    /// Getter.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Setter.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// Returns the current position in the glide, i.e., the `u16` representation of the DAC value for
    /// the current [`Note`] or between-[`Note`] value.
    pub fn glide(&self) -> u16 {
        let total_distance =
            u16::from(<Note as Into<u8>>::into(self.destination)).abs_diff(self.origin);
        let slew_adj = f32::from(total_distance) * self.progress();

        self.origin + slew_adj as u16
    }

    /// Indicates progress through the glide, where 0.0 is the origin and 1.0 is the destination.
    fn progress(&self) -> f32 {
        let now = Instant::now();
        let time_gliding = now - self.start;

        // if the portamento time has been reduced so much that the glide should have
        // already finished, progress is 100% and the portamento should end
        if time_gliding > self.duration {
            1.0
        } else {
            time_gliding.as_micros() as f32 / self.duration.as_micros() as f32
        }
    }

    /// Returns `true` if glide has arrived at its destination, otherwise `false`.
    fn is_done(&self) -> bool {
        Instant::now() > self.start + self.duration
    }
}

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
