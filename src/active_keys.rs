use tinyvec::{ArrayVec, array_vec};
use wmidi::U7;

/// Per the General MIDI Level 2 specification, compliant devices "must be capable of supplying polyphony of
/// 32 or more allocated notes simultaneously." Thus, this will be the default size of an ActiveKeys instance.
const GM2_SIMUL_NOTE_NUM: usize = 32;

pub struct ActiveKeys<const N: usize = GM2_SIMUL_NOTE_NUM> {
    /// Because tinyvec requires the wrapped value to implement Default, and Note doesn't, U7 is used.
    data: ArrayVec<[U7; N]>,
}

impl Default for ActiveKeys {
    fn default() -> Self {
        Self { data: array_vec!() }
    }
}

impl ActiveKeys {
    pub fn new() -> Self {
        Self { data: array_vec!() }
    }

    pub fn add(&mut self, note: U7) {
        // only add if space allows and if the note isn't (somehow) already registered as active; otherwise, ignore input
        if self.data.len() != self.data.capacity() && !self.data.contains(&note) {
            self.data.push(note);
        }
    }

    pub fn get(&self) -> &[U7] {
        self.data.as_slice()
    }

    pub fn remove(&mut self, note: U7) {
        self.data.retain(|&n| n != note);
    }

    pub fn is_empty(&mut self) -> bool {
        self.data.is_empty()
    }
}
