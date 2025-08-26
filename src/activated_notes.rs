use tinyvec::{ArrayVec, array_vec};
use wmidi::{Note, U7};

/// Per the General MIDI Level 2 specification, compliant devices "must be capable of supplying polyphony of
/// 32 or more allocated notes simultaneously." Thus, this will be the default size of an ActivatedNotes instance.
const GM2_SIMUL_NOTE_NUM: usize = 32;

/// A struct for managing the activated notes of an instrument (e.g., the state of a keyboard).
///
/// Internally, this struct use the [`U7`] type because [`tinyvec`] requires that `Items` implement [`Default`].
/// However, [`U7`] can be a bit unwieldy, so public interfaces will deal with the related [`Note`] type instead.
pub struct ActivatedNotes<const N: usize = GM2_SIMUL_NOTE_NUM> {
    /// Because tinyvec requires the wrapped value to implement Default, and Note doesn't, U7 is used.
    data: ArrayVec<[U7; N]>,
}

impl Default for ActivatedNotes {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivatedNotes {
    pub fn new() -> Self {
        Self { data: array_vec!() }
    }

    pub fn add(&mut self, note: Note) {
        let u7 = U7::from_u8_lossy(note as u8);
        // only add if space allows and if the note isn't (somehow) already registered as active; otherwise, ignore input
        if self.data.len() != self.data.capacity() && !self.data.contains(&u7) {
            self.data.push(u7);
        }
    }

    /// Returns the note that was activated first
    pub fn first(&mut self) -> Option<Note> {
        self.data.first().map(|&u7| u7.into())
    }

    /// Returns the note that was activated last
    pub fn last(&mut self) -> Option<Note> {
        self.data.last().map(|&u7| u7.into())
    }

    /// Returns the highest activated note
    pub fn highest(&mut self) -> Option<Note> {
        self.data.iter().max().map(|&u7| u7.into())
    }

    /// Returns the lowest activated note
    pub fn lowest(&mut self) -> Option<Note> {
        self.data.iter().min().map(|&u7| u7.into())
    }

    pub fn remove(&mut self, note: Note) {
        self.data.retain(|&n| n != U7::from_u8_lossy(note as u8));
    }

    pub fn is_empty(&mut self) -> bool {
        self.data.is_empty()
    }
}
