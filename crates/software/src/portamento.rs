//! Provides struct for managing intra-note states, i.e., gliding from one note to another.

use embassy_time::{Duration, Instant};
use wmidi::Note;

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
