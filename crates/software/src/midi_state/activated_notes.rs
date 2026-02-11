//! Provides a struct [`ActivatedNotes`] for managing the activated notes of an instrument. Here "activated notes"
//! means the notes that are currently being played (e.g., depressed on a keyboard), regardless of whether or not
//! those notes are actually voiced. (On a monophonic instrument, many keys might be depressed, but only one will
//! sound.)

use tinyvec::{ArrayVec, array_vec};
use wmidi::{Note, U7};

/// Per the General MIDI Level 2 specification, compliant devices "must be capable of supplying polyphony of
/// 32 or more allocated notes simultaneously." Thus, this will be the default size of an ActivatedNotes instance.
const GM2_SIMUL_NOTE_NUM: usize = 32;

/// A struct for managing the activated notes of an instrument.
///
/// Internally, this struct uses the [`U7`] type because [`tinyvec`] requires that `Items` implement [`Default`].
/// However, [`U7`] can be a bit unwieldy, so public interfaces will deal with the related [`Note`] type instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivatedNotes<const N: usize = GM2_SIMUL_NOTE_NUM> {
    /// [`U7`] representations of the currently activated notes
    data: ArrayVec<[U7; N]>,
}

impl Default for ActivatedNotes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "defmt")]
impl<const N: usize> defmt::Format for ActivatedNotes<N> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "ActivatedNotes {{ ");
        defmt::write!(fmt, "data: [");
        for (i, &note) in self.data.iter().enumerate() {
            if i == 0 {
                defmt::write!(fmt, " ");
            } else {
                defmt::write!(fmt, ", ");
            }
            defmt::write!(fmt, "{} ({})", Note::from(note).to_str(), u8::from(note));
        }
        defmt::write!(fmt, " ]");
        defmt::write!(fmt, " }}");
    }
}

impl ActivatedNotes {
    /// Construct a new `ActivatedNotes`.
    pub fn new() -> Self {
        Self { data: array_vec!() }
    }

    /// Add a [`Note`] to the list of those currently activated. Equivalent to depressing a key on a keyboard.
    pub fn add(&mut self, note: Note) {
        let u7 = U7::from_u8_lossy(note as u8);
        // only add if space allows and if the note isn't (somehow) already registered as active; otherwise, ignore input
        if self.data.len() != self.data.capacity() && !self.data.contains(&u7) {
            self.data.push(u7);
        }
    }

    /// Remove a [`Note`] from the list of those currently activated. Equivalent to releasing a depressed key on a keyboard.
    pub fn remove(&mut self, note: Note) {
        self.data.retain(|&n| n != U7::from_u8_lossy(note as u8));
    }

    /// Determine if any [`Note`]s are activated.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns an [`Iterator`] over the activated [`Note`]s.
    ///
    /// Order is preserved; e.g., the first performed `Note` can be accessed via the first call to `.next()`, and the
    /// last performed `Note` is accessible via `.last()`.
    pub fn iter(&self) -> impl Iterator<Item = Note> {
        self.data.iter().map(|&i| Note::from(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C_NOTE: U7 = U7::from_u8_lossy(60);
    const D_NOTE: U7 = U7::from_u8_lossy(62);
    const E_NOTE: U7 = U7::from_u8_lossy(64);
    const G_NOTE: U7 = U7::from_u8_lossy(67);

    fn chord() -> ActivatedNotes<GM2_SIMUL_NOTE_NUM> {
        ActivatedNotes::<GM2_SIMUL_NOTE_NUM> {
            data: array_vec!([U7; 32] => E_NOTE, C_NOTE, G_NOTE),
        }
    }

    #[test]
    fn new() {
        let expected: ActivatedNotes<32> = ActivatedNotes { data: array_vec!() };
        let actual = ActivatedNotes::new();
        assert_eq!(expected, actual, "Expected left but got right");
    }

    #[test]
    fn add_appends() {
        let expected = ActivatedNotes::<GM2_SIMUL_NOTE_NUM> {
            data: array_vec!([U7; 32] => E_NOTE, C_NOTE, G_NOTE, D_NOTE),
        };

        let mut actual = chord();
        actual.add(D_NOTE.into());

        assert_eq!(expected, actual, "Expected left but got right");
    }

    #[test]
    fn duplicate_add_is_ignored() {
        let expected = chord();
        let mut actual = chord();
        actual.add(C_NOTE.into());

        assert_eq!(expected, actual, "Expected left but got right");
    }

    #[test]
    fn add_ignores_rather_than_overflow() {
        let mut activated_notes = ActivatedNotes::<GM2_SIMUL_NOTE_NUM> {
            data: ArrayVec::from([C_NOTE; GM2_SIMUL_NOTE_NUM]),
        };
        assert_eq!(
            activated_notes.data.len(),
            GM2_SIMUL_NOTE_NUM,
            "Expected data to be inititalized to max capacity"
        );
        // end setup

        activated_notes.add(D_NOTE.into());
        assert_eq!(
            activated_notes.data.len(),
            GM2_SIMUL_NOTE_NUM,
            "Expected data length not to change"
        );
        assert!(
            activated_notes
                .data
                .iter()
                .find(|&&n| n == D_NOTE.into())
                .is_none()
        );
    }

    #[test]
    fn remove() {
        let expected = ActivatedNotes::<GM2_SIMUL_NOTE_NUM> {
            data: array_vec!([U7; 32] => E_NOTE, G_NOTE),
        };

        let mut actual = chord();
        actual.remove(C_NOTE.into());

        assert_eq!(expected, actual, "Expected left but got right");
    }

    #[test]
    fn should_be_empty() {
        let activated_notes = ActivatedNotes::<GM2_SIMUL_NOTE_NUM> { data: array_vec!() };
        assert!(activated_notes.is_empty());
    }

    #[test]
    fn should_not_be_empty() {
        let activated_notes = chord();
        assert!(!activated_notes.is_empty());
    }

    #[test]
    fn iter() {
        let chord = chord();
        let mut iter = chord.iter();
        assert_eq!(Some(Note::E4), iter.next());
        assert_eq!(Some(Note::C4), iter.next());
        assert_eq!(Some(Note::G4), iter.next());
        assert_eq!(None, iter.next());
    }
}
