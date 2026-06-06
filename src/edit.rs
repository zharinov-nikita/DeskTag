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

    /// Clear the whole buffer (Ctrl+Backspace / Ctrl+Delete).
    pub fn clear(&mut self) {
        self.buf.clear();
        self.caret = 0;
        self.fresh = false;
    }

    /// Select all: the next edit replaces/clears everything (Ctrl+A), reusing
    /// the same `fresh` mechanism as entering edit mode.
    pub fn select_all(&mut self) {
        self.fresh = true;
    }

    /// True while the whole text is "selected" (fresh) — the next edit replaces
    /// it. Drives the selection highlight in the badge.
    pub fn is_selected(&self) -> bool {
        self.fresh
    }

    /// Move the caret one char left. A selection collapses to its start.
    pub fn move_left(&mut self) {
        if self.fresh {
            self.fresh = false;
            self.caret = 0;
            return;
        }
        if self.caret > 0 {
            let prev = self.buf[..self.caret]
                .chars()
                .next_back()
                .map(|c| self.caret - c.len_utf8())
                .unwrap_or(0);
            self.caret = prev;
        }
    }

    /// Move the caret one char right. A selection collapses to its end.
    pub fn move_right(&mut self) {
        if self.fresh {
            self.fresh = false;
            self.caret = self.buf.len();
            return;
        }
        if self.caret < self.buf.len() {
            let adv = self.buf[self.caret..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.caret += adv;
        }
    }

    /// Move the caret to the start.
    pub fn home(&mut self) {
        self.fresh = false;
        self.caret = 0;
    }

    /// Move the caret to the end.
    pub fn end(&mut self) {
        self.fresh = false;
        self.caret = self.buf.len();
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

    #[test]
    fn clear_empties_buffer() {
        let mut e = EditState::new("hello");
        e.insert_char('x'); // fresh-replace -> "x"
        e.clear();
        assert_eq!(e.text(), "");
        assert_eq!(e.caret(), 0);
    }

    #[test]
    fn select_all_then_type_replaces() {
        let mut e = EditState::new("");
        e.insert_char('a');
        e.insert_char('b');
        e.select_all();
        e.insert_char('x');
        assert_eq!(e.text(), "x");
    }

    #[test]
    fn select_all_then_backspace_clears() {
        let mut e = EditState::new("");
        e.insert_char('a');
        e.insert_char('b');
        e.select_all();
        e.backspace();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn is_selected_reflects_fresh() {
        let mut e = EditState::new("abc");
        assert!(e.is_selected()); // fresh on entry
        e.insert_char('x');
        assert!(!e.is_selected()); // cleared after first edit
        e.select_all();
        assert!(e.is_selected()); // re-selected
    }

    #[test]
    fn move_left_collapses_selection_to_start() {
        let mut e = EditState::new("abc"); // fresh, caret at end
        e.move_left();
        assert_eq!(e.caret(), 0);
        assert!(!e.is_selected());
        e.insert_char('x');
        assert_eq!(e.text(), "xabc");
    }

    #[test]
    fn move_right_collapses_selection_to_end() {
        let mut e = EditState::new("abc");
        e.move_right();
        assert_eq!(e.caret(), "abc".len());
        e.insert_char('x');
        assert_eq!(e.text(), "abcx");
    }

    #[test]
    fn mid_edit_insert_and_backspace() {
        let mut e = EditState::new("");
        for c in "abc".chars() {
            e.insert_char(c);
        }
        e.move_left(); // "ab|c"
        e.insert_char('X'); // "abXc"
        assert_eq!(e.text(), "abXc");
        e.backspace(); // remove X
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn home_end_move_caret() {
        let mut e = EditState::new("");
        for c in "hi".chars() {
            e.insert_char(c);
        }
        e.home();
        assert_eq!(e.caret(), 0);
        e.end();
        assert_eq!(e.caret(), "hi".len());
    }

    #[test]
    fn move_caret_cyrillic_boundaries() {
        let mut e = EditState::new("");
        for c in "абв".chars() {
            e.insert_char(c);
        }
        e.move_left();
        assert_eq!(e.caret(), "аб".len()); // before в
        e.move_left();
        assert_eq!(e.caret(), "а".len());
        e.move_right();
        assert_eq!(e.caret(), "аб".len());
    }
}
