use crate::instrument::Instrument;
use embedded_hal::digital::OutputPin;
use enum_dispatch::enum_dispatch;

/// A trait for using a gate signal to control an instrument's on/off state.
#[enum_dispatch(Instrument)]
pub trait Gate {
    /// Opens or closes the gate according to internal state
    fn gate<T: OutputPin>(&self, switch_trigger: &mut T);
}
