use enum_dispatch::enum_dispatch;
use micromoog::Micromoog;

mod micromoog;

#[enum_dispatch]
pub enum Instrument {
    Micromoog(Micromoog),
}

impl Default for Instrument {
    fn default() -> Self {
        Self::Micromoog(Micromoog::default())
    }
}

/// Evidently, the `enum_display` macro needs to have Instrument and all of the types used by its variant constructors in scope,
/// otherwise it won't compile. See https://gitlab.com/antonok/enum_dispatch/-/issues/81.
#[enum_dispatch(Instrument)]
trait EnumDispatchHack {}
