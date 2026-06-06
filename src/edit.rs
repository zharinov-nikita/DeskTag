//! Pure inline-edit buffer for the badge's rename mode. OS-independent;
//! unit-tested like `label.rs`. Caret moves only by typing/backspace at the
//! end (arrow keys / selection are out of scope — see spec non-goals).

/// Max desktop name length, counted in chars (not bytes).
pub const MAX_LEN: usize = 50;

/// Editable string with a caret byte-offset (always on a char boundary).
pub struct EditState {
    buf: String,
    caret: usize,
    fresh: bool, // true until first edit; emulates "whole text selected"
}

impl EditState {
    /// Begin editing `initial`; caret at end, whole text "selected".
    pub fn new(initial: &str) -> Self {
        Self {
            buf: initial.to_string(),
            caret: initial.len(),
            fresh: true,
        }
    }

    /// Insert a char at the caret. The first edit while `fresh` replaces the
    /// whole buffer. Input past `MAX_LEN` chars is ignored.
    pub fn insert_char(&mut self, c: char) {
        if self.fresh {
            self.buf.clear();
            self.caret = 0;
            self.fresh = false;
        }
        if self.buf.chars().count() >= MAX_LEN {
            return;
        }
        self.buf.insert(self.caret, c);
        self.caret += c.len_utf8();
    }

    /// Delete the char before the caret. The first edit while `fresh` clears
    /// the whole buffer.
    pub fn backspace(&mut self) {
        if self.fresh {
            self.buf.clear();
            self.caret = 0;
            self.fresh = false;
            return;
        }
        if self.caret == 0 {
            return;
        }
        let prev = self.buf[..self.caret]
            .chars()
            .next_back()
            .map(|c| self.caret - c.len_utf8())
            .unwrap_or(0);
        self.buf.replace_range(prev..self.caret, "");
        self.caret = prev;
    }

    pub fn text(&self) -> &str {
        &self.buf
    }

    /// Caret position as a byte offset on a char boundary.
    pub fn caret(&self) -> usize {
        self.caret
    }
}

#[cfg(test)]
mod tests {
    use super::{EditState, MAX_LEN};

    #[test]
    fn fresh_first_char_replaces_all() {
        let mut e = EditState::new("old");
        e.insert_char('x');
        assert_eq!(e.text(), "x");
    }

    #[test]
    fn fresh_backspace_clears_all() {
        let mut e = EditState::new("old");
        e.backspace();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn typing_appends_after_fresh() {
        let mut e = EditState::new("");
        e.insert_char('a');
        e.insert_char('b');
        assert_eq!(e.text(), "ab");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut e = EditState::new("");
        e.insert_char('a');
        e.insert_char('b');
        e.backspace();
        assert_eq!(e.text(), "a");
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut e = EditState::new("");
        e.backspace(); // clears fresh
        e.backspace(); // empty, no panic
        assert_eq!(e.text(), "");
    }

    #[test]
    fn respects_max_len() {
        let mut e = EditState::new("");
        for _ in 0..(MAX_LEN + 10) {
            e.insert_char('a');
        }
        assert_eq!(e.text().chars().count(), MAX_LEN);
    }

    #[test]
    fn cyrillic_caret_stays_on_char_boundary() {
        let mut e = EditState::new("");
        e.insert_char('п');
        e.insert_char('р');
        e.insert_char('и');
        assert_eq!(e.text(), "при");
        assert_eq!(e.caret(), "при".len()); // 6 bytes
        e.backspace();
        assert_eq!(e.text(), "пр");
        assert_eq!(e.caret(), "пр".len()); // 4 bytes, on boundary
    }
}
