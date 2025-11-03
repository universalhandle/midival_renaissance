//! This module contains the set of instruments that may be used with the MIDIval Renaissance. For the foreseeable future,
//! only the [`Micromoog`][`self::Micromoog`] is supported.

use crate::configuration::{Config, InstrumentConfig};
use enum_dispatch::enum_dispatch;
use micromoog::Micromoog;

mod micromoog;

/// Enum for selecting the instrument to receive input from the MIDIval Renaissance.
///
/// This design decision was likely a premature optimization. If supporting a second instrument becomes a goal of this
/// project—and frankly that seems a long way off—it seems unlikely this will be the mechanism by which it occurs.
/// [`mod@enum_dispatch`] hasn't yet provided benefit to the codebase (as there is only one instrument at present) but it
/// imposes limitations that already are frustrating, such as the inability to use trait constants or generics. A
/// better approach might be to create an Instrument subtrait that requires `Midi`, `Gate`, etc.
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
/// otherwise it won't compile. See <https://gitlab.com/antonok/enum_dispatch/-/issues/81>.
#[allow(dead_code)]
#[enum_dispatch(Instrument)]
trait EnumDispatchHack {}
