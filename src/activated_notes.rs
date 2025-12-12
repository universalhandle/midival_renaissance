//! Provides a struct [`ActivatedNotes`] for managing the activated notes of an instrument. Here "activated notes"
//! means the notes that are currently being played (e.g., depressed on a keyboard), regardless of whether or not
//! those notes are actually voiced. (On a monophonic instrument, many keys might be depressed, but only one will
//! sound.)

use embassy_time::Instant;
use tinyvec::{ArrayVec, array_vec};
use wmidi::{Note, U7};

/// Per the General MIDI Level 2 specification, compliant devices "must be capable of supplying polyphony of
/// 32 or more allocated notes simultaneously." Thus, this will be the default size of an ActivatedNotes instance.
const GM2_SIMUL_NOTE_NUM: usize = 32;

/// A struct for managing the activated notes of an instrument.
///
/// Internally, this struct uses the [`U7`] type because [`tinyvec`] requires that `Items` implement [`Default`].
/// However, [`U7`] can be a bit unwieldy, so public interfaces will deal with the related [`Note`] type instead.
pub struct ActivatedNotes<const N: usize = GM2_SIMUL_NOTE_NUM> {
    /// [`U7`] representations of the currently activated notes
    data: ArrayVec<[U7; N]>,
    updated_at: Option<Instant>,
}

impl Default for ActivatedNotes {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivatedNotes {
    pub fn new() -> Self {
        Self {
            data: array_vec!(),
            updated_at: None,
        }
    }

    /// Add a [`Note`] to the list of those currently activated. Equivalent to depressing a key on a keyboard.
    pub fn add(&mut self, note: Note) {
        let u7 = U7::from_u8_lossy(note as u8);
        // only add if space allows and if the note isn't (somehow) already registered as active; otherwise, ignore input
        if self.data.len() != self.data.capacity() && !self.data.contains(&u7) {
            self.data.push(u7);
            self.updated_at = Some(Instant::now());
        }
    }

    /// Return the [`Note`] that was activated first.
    pub fn first(&mut self) -> Option<Note> {
        self.data.first().map(|&u7| u7.into())
    }

    /// Return the [`Note`] that was activated last.
    pub fn last(&mut self) -> Option<Note> {
        self.data.last().map(|&u7| u7.into())
    }

    /// Return the instant of the last update to ActivatedNotes.
    pub fn updated_at(&self) -> Option<Instant> {
        self.updated_at
    }

    /// Return the highest activated [`Note`] (i.e., the rightmost on a keyboard).
    pub fn highest(&mut self) -> Option<Note> {
        self.data.iter().max().map(|&u7| u7.into())
    }

    /// Return the lowest activated [`Note`] (i.e., the leftmost on a keyboard).
    pub fn lowest(&mut self) -> Option<Note> {
        self.data.iter().min().map(|&u7| u7.into())
    }

    /// Remove a [`Note`] from the list of those currently activated. Equivalent to releasing a depressed key on a keyboard.
    pub fn remove(&mut self, note: Note) {
        self.data.retain(|&n| n != U7::from_u8_lossy(note as u8));
        self.updated_at = Some(Instant::now());
    }

    /// Determine if any [`Note`]s are activated.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
