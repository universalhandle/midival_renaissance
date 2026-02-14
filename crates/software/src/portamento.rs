//! Provides struct for managing intra-note states, i.e., gliding from one note to another.

use crate::configuration::{Keyboard, ProvideNote};
use embassy_time::{Duration, Instant};
use measurements::Voltage;
use wmidi::Note;

/// Contains data necessary to execute a portamento or glide effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Portamento<T> {
    /// Indicates the starting point of the glide.
    ///
    /// Uses [`Voltage`] instead of [`Note`] so that intra-note state can be represented, as
    /// a glide can start from anywhere, e.g., when a new destination is selected mid-glide.
    origin: Voltage,
    /// Indicates the end of the glide; when this [`Note`] is reached, there is nothing left to do.
    destination: Note,
    /// The [`Instant`] at which the glide began.
    start: Instant,
    /// How long after the `start` to stretch the effect.
    duration: Duration,
    /// Keyboard configuration.
    ///
    /// Voltages can't be calculated without the context of the keyboard, but it's possible adding
    /// them to this struct is not the best way of sharing that data.
    keyboard: Keyboard<T>,
}

impl<T> Portamento<T>
where
    T: ProvideNote,
{
    /// Constructs a new [`Portamento`].
    pub fn new(origin: Note, destination: Note, duration: Duration, keyboard: Keyboard<T>) -> Self {
        Self {
            origin: keyboard.voltage(origin),
            destination: destination,
            start: Instant::now(),
            duration,
            keyboard,
        }
    }

    /// Given a new destination, constructs a new [`Portamento`] using the existing one as a template.
    ///
    /// This is especially useful for starting a glide from in-between [`Note`]s.
    pub fn new_destination(self, destination: Note) -> Self {
        Self {
            origin: self.glide(),
            destination,
            start: Instant::now(),
            ..self
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

    /// Returns a [`Voltage`] representing the voicing (which may be between [`Note`]s) at the current position in the glide.
    pub fn glide(&self) -> Voltage {
        let destination = self.keyboard.voltage(self.destination);
        let total_journey = destination - self.origin;
        let journey_so_far = total_journey * self.progress();

        self.origin + journey_so_far
    }

    /// Indicates progress through the duration of the glide as a decimal fraction.
    fn progress(&self) -> f64 {
        let now = Instant::now();
        let time_gliding = now - self.start;

        // if the portamento time has been reduced so much that the glide should have
        // already finished (or if the call to this method was for some reason so delayed),
        // progress is 100% and the portamento should end
        if time_gliding >= self.duration {
            1.0
        } else {
            time_gliding.as_micros() as f64 / self.duration.as_micros() as f64
        }
    }

    /// Returns `true` if glide has arrived at its destination, otherwise `false`.
    fn is_done(&self) -> bool {
        Instant::now() > self.start + self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::NotePriority;
    use embassy_time::MockDriver;

    fn keyboard() -> Keyboard<NotePriority> {
        Keyboard::new(
            NotePriority::Low,
            Note::F3..=Note::C6,
            Voltage::from_volts(1.0),
        )
    }

    fn time_driver() -> &'static MockDriver {
        let driver = MockDriver::get();
        driver.reset();
        driver
    }

    #[test]
    fn new_destination() {
        let driver = time_driver();
        let portamento_in_progress = Portamento {
            origin: Voltage::from_volts(0.75), // this is a D4
            destination: Note::D5,
            start: Instant::now(),
            duration: Duration::from_millis(2500),
            keyboard: keyboard(),
        };

        driver.advance(Duration::from_millis(500));

        assert_eq!(
            Portamento {
                origin: Voltage::from_volts(0.95), // somewhere between E4 and F4
                destination: Note::C4,
                start: Instant::now(),
                duration: Duration::from_millis(2500),
                keyboard: keyboard(),
            },
            portamento_in_progress.new_destination(Note::C4),
            "Expected left but got right"
        );
    }

    #[test]
    fn glide_up() {
        let driver = time_driver();
        let portamento = Portamento {
            origin: Voltage::from_volts(0.75), // this is a D4
            destination: Note::D5,
            start: Instant::now(),
            duration: Duration::from_millis(1000),
            keyboard: keyboard(),
        };

        driver.advance(Duration::from_millis(500));

        assert_eq!(
            Voltage::from_volts(1.25),
            portamento.glide(),
            "Expected glide up the keyboard to increase the voltage linearly"
        );
    }

    #[test]
    fn glide_down() {
        let driver = time_driver();
        let portamento = Portamento {
            origin: Voltage::from_volts(1.75), // this is a D5
            destination: Note::D4,
            start: Instant::now(),
            duration: Duration::from_millis(1000),
            keyboard: keyboard(),
        };

        driver.advance(Duration::from_millis(500));

        assert_eq!(
            Voltage::from_volts(1.25),
            portamento.glide(),
            "Expected glide down the keyboard to decrease the voltage linearly"
        );
    }

    #[test]
    fn glide_disabled() {
        let driver = time_driver();
        let portamento = Portamento {
            origin: Voltage::from_volts(0.75), // this is a D4
            destination: Note::D5,
            start: Instant::now(),
            duration: Duration::from_millis(0),
            keyboard: keyboard(),
        };

        driver.advance(Duration::from_millis(0));

        assert_eq!(
            Voltage::from_volts(1.75),
            portamento.glide(),
            "Expected instant note changed when portamento disabled"
        );
    }

    #[test]
    fn glide_late() {
        let driver = time_driver();
        let portamento = Portamento {
            origin: Voltage::from_volts(0.75), // this is a D4
            destination: Note::D5,
            start: Instant::now(),
            duration: Duration::from_millis(1000),
            keyboard: keyboard(),
        };

        driver.advance(Duration::from_millis(1111));

        assert_eq!(
            Voltage::from_volts(1.75),
            portamento.glide(),
            "Expected glide not to overshoot the destination note"
        );
    }
}
