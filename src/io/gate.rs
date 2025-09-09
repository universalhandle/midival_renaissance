use crate::instrument::Instrument;
use enum_dispatch::enum_dispatch;

#[derive(Debug, PartialEq)]
pub enum GateState {
    /// When the gate is high, the instrument will sound.
    High,
    /// When the gate is low, the instrument will rest.
    Low,
}

/// A trait for using a gate signal to indicate whether or not an instrument should be in an active state.
#[enum_dispatch(Instrument)]
pub trait Gate {
    /// Returns the state the gate is in.
    fn gate_state(&self) -> GateState;

    /// Convenience function to test whether gate is currently high.
    fn gate_is_high(&self) -> bool {
        self.gate_state() == GateState::High
    }

    /// Convenience function to test whether the gate is currently low.
    fn gate_is_low(&self) -> bool {
        self.gate_state() == GateState::Low
    }
}
