//! This module contains both user-configurable settings (implemented as enums) and traits to make them easier to work with in code.

mod chord_cleanup;
pub use chord_cleanup::*;

mod envelope_trigger;
pub use envelope_trigger::*;

mod input_mode;
pub use input_mode::*;

mod keyboard;
pub use keyboard::*;

use num_traits::{FromPrimitive, ToPrimitive};

/// A trait which allows infinite cycling of an enum's variants.
///
/// Useful for pushbutton user interfaces, allowing presses to advance from the current to the next variant,
/// cycling back to the beginning when all variants have been exhausted.
pub trait CycleConfig {
    /// Return the next variant, cycling back to the beginning as needed.
    fn cycle(self) -> Self
    where
        Self: FromPrimitive + ToPrimitive + Sized,
    {
        let index = self
            .to_u8()
            .expect("enum variants should be castable to u8");
        match <Self as FromPrimitive>::from_u8(index + 1) {
            Some(new_selection) => new_selection,
            None => FromPrimitive::from_u8(0).expect("enum should not be empty"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_derive::{FromPrimitive, ToPrimitive};

    #[derive(Debug, Clone, Copy, ToPrimitive, FromPrimitive, PartialEq)]
    enum Alpha {
        A,
        B,
        C,
    }
    impl CycleConfig for Alpha {}

    #[test]
    fn cycle() {
        let config = Alpha::A.cycle();
        assert_eq!(
            Alpha::B,
            config,
            "Should advance to next variant; expected left but got right"
        );

        let config = config.cycle();
        assert_eq!(
            Alpha::C,
            config,
            "Should advance to next variant; expected left but got right"
        );

        let config = config.cycle();
        assert_eq!(
            Alpha::A,
            config,
            "Should wrap around to first variant; expected left but got right"
        );
    }
}
