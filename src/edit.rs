//! Pure inline-edit buffer for the badge's rename mode. OS-independent;
//! unit-tested like `label.rs`. Supports caret movement, word jumps, and a
//! selection range (`anchor..caret`), all on UTF-8 char boundaries.

/// Max desktop name length, counted in chars (not bytes).
pub const MAX_LEN: usize = 50;

/// Editable string with a caret and an optional selection anchor. When `anchor`
/// is set and differs from `caret`, the span between them is selected.
pub struct EditState {
    buf: String,
    caret: usize,          // byte offset on a char boundary
    anchor: Option<usize>, // selection spans anchor..caret when set and != caret
}

impl EditState {
    /// Begin editing `initial`; the whole text starts selected (anchor at 0,
    /// caret at end) so the first keystroke replaces it — like focusing a field.
    pub fn new(initial: &str) -> Self {
        let len = initial.len();
        Self {
            buf: initial.to_string(),
            caret: len,
            anchor: if len == 0 { None } else { Some(0) },
        }
    }

    pub fn text(&self) -> &str {
        &self.buf
    }

    /// Caret position as a byte offset on a char boundary.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selected span as `(lo, hi)` byte offsets, if a non-empty range is
    /// selected. `None` means just a caret.
    pub fn selection(&self) -> Option<(usize, usize)> {
        match self.anchor {
            Some(a) if a != self.caret => Some((a.min(self.caret), a.max(self.caret))),
            _ => None,
        }
    }

    // --- char/word boundary helpers ---

    fn prev_char(&self, pos: usize) -> usize {
        self.buf[..pos]
            .chars()
            .next_back()
            .map_or(0, |c| pos - c.len_utf8())
    }

    fn next_char(&self, pos: usize) -> usize {
        self.buf[pos..]
            .chars()
            .next()
            .map_or(pos, |c| pos + c.len_utf8())
    }

    /// Is the char starting at `pos` whitespace?
    fn ws_at(&self, pos: usize) -> bool {
        self.buf[pos..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace())
    }

    /// Start of the previous word: skip whitespace left, then non-whitespace.
    fn prev_word(&self, mut pos: usize) -> usize {
        while pos > 0 && self.ws_at(self.prev_char(pos)) {
            pos = self.prev_char(pos);
        }
        while pos > 0 && !self.ws_at(self.prev_char(pos)) {
            pos = self.prev_char(pos);
        }
        pos
    }

    /// Start of the next word: skip non-whitespace right, then whitespace.
    fn next_word(&self, mut pos: usize) -> usize {
        let len = self.buf.len();
        while pos < len && !self.ws_at(pos) {
            pos = self.next_char(pos);
        }
        while pos < len && self.ws_at(pos) {
            pos = self.next_char(pos);
        }
        pos
    }

    // --- editing ---

    /// Delete the selection if any. Returns whether something was removed.
    fn delete_selection(&mut self) -> bool {
        if let Some((lo, hi)) = self.selection() {
            self.buf.replace_range(lo..hi, "");
            self.caret = lo;
            self.anchor = None;
            true
        } else {
            self.anchor = None;
            false
        }
    }

    /// Insert a char at the caret, replacing any selection first. Input past
    /// `MAX_LEN` chars is ignored.
    pub fn insert_char(&mut self, c: char) {
        self.delete_selection();
        if self.buf.chars().count() >= MAX_LEN {
            return;
        }
        self.buf.insert(self.caret, c);
        self.caret += c.len_utf8();
    }

    /// Delete the selection, or the char before the caret.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret > 0 {
            let p = self.prev_char(self.caret);
            self.buf.replace_range(p..self.caret, "");
            self.caret = p;
        }
    }

    /// Delete the selection, or the char after the caret (forward delete).
    pub fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret < self.buf.len() {
            let n = self.next_char(self.caret);
            self.buf.replace_range(self.caret..n, "");
        }
    }

    /// Clear the whole buffer (Ctrl+Backspace / Ctrl+Delete).
    pub fn clear(&mut self) {
        self.buf.clear();
        self.caret = 0;
        self.anchor = None;
    }

    /// Select all (Ctrl+A).
    pub fn select_all(&mut self) {
        if self.buf.is_empty() {
            self.anchor = None;
            self.caret = 0;
        } else {
            self.anchor = Some(0);
            self.caret = self.buf.len();
        }
    }

    // --- caret movement (collapses any selection) ---

    pub fn move_left(&mut self) {
        self.caret = match self.selection() {
            Some((lo, _)) => lo,
            None => self.prev_char(self.caret),
        };
        self.anchor = None;
    }

    pub fn move_right(&mut self) {
        self.caret = match self.selection() {
            Some((_, hi)) => hi,
            None => self.next_char(self.caret),
        };
        self.anchor = None;
    }

    pub fn move_word_left(&mut self) {
        self.caret = self.prev_word(self.caret);
        self.anchor = None;
    }

    pub fn move_word_right(&mut self) {
        self.caret = self.next_word(self.caret);
        self.anchor = None;
    }

    pub fn home(&mut self) {
        self.caret = 0;
        self.anchor = None;
    }

    pub fn end(&mut self) {
        self.caret = self.buf.len();
        self.anchor = None;
    }

    // --- selection extension (Shift + movement) ---

    fn ensure_anchor(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.caret);
        }
    }

    pub fn extend_left(&mut self) {
        self.ensure_anchor();
        self.caret = self.prev_char(self.caret);
    }

    pub fn extend_right(&mut self) {
        self.ensure_anchor();
        self.caret = self.next_char(self.caret);
    }

    pub fn extend_word_left(&mut self) {
        self.ensure_anchor();
        self.caret = self.prev_word(self.caret);
    }

    pub fn extend_word_right(&mut self) {
        self.ensure_anchor();
        self.caret = self.next_word(self.caret);
    }

    pub fn extend_home(&mut self) {
        self.ensure_anchor();
        self.caret = 0;
    }

    pub fn extend_end(&mut self) {
        self.ensure_anchor();
        self.caret = self.buf.len();
    }
}

#[cfg(test)]
mod tests {
    use super::{EditState, MAX_LEN};

    #[test]
    fn new_selects_all_nonempty() {
        let e = EditState::new("abc");
        assert_eq!(e.selection(), Some((0, "abc".len())));
        assert_eq!(e.caret(), "abc".len());
    }

    #[test]
    fn new_empty_has_no_selection() {
        let e = EditState::new("");
        assert_eq!(e.selection(), None);
        assert_eq!(e.caret(), 0);
    }

    #[test]
    fn typing_replaces_selection() {
        let mut e = EditState::new("old");
        e.insert_char('x'); // whole selection replaced
        assert_eq!(e.text(), "x");
        assert_eq!(e.selection(), None);
    }

    #[test]
    fn backspace_deletes_selection() {
        let mut e = EditState::new("old");
        e.backspace();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn typing_appends_without_selection() {
        let mut e = EditState::new("");
        e.insert_char('a');
        e.insert_char('b');
        assert_eq!(e.text(), "ab");
    }

    #[test]
    fn backspace_removes_prev_char() {
        let mut e = EditState::new("");
        e.insert_char('a');
        e.insert_char('b');
        e.backspace();
        assert_eq!(e.text(), "a");
    }

    #[test]
    fn forward_delete_removes_next_char() {
        let mut e = EditState::new("");
        for c in "abc".chars() {
            e.insert_char(c);
        }
        e.home();
        e.delete();
        assert_eq!(e.text(), "bc");
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
    fn cyrillic_caret_on_char_boundary() {
        let mut e = EditState::new("");
        for c in "при".chars() {
            e.insert_char(c);
        }
        assert_eq!(e.caret(), "при".len());
        e.backspace();
        assert_eq!(e.text(), "пр");
        assert_eq!(e.caret(), "пр".len());
    }

    #[test]
    fn move_left_collapses_selection_to_start() {
        let mut e = EditState::new("abc");
        e.move_left();
        assert_eq!(e.caret(), 0);
        assert_eq!(e.selection(), None);
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
        e.backspace();
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn extend_then_type_replaces_range() {
        let mut e = EditState::new("");
        for c in "abcd".chars() {
            e.insert_char(c);
        }
        e.home(); // caret 0
        e.extend_right(); // select "a"
        e.extend_right(); // select "ab"
        assert_eq!(e.selection(), Some((0, 2)));
        e.insert_char('Z');
        assert_eq!(e.text(), "Zcd");
    }

    #[test]
    fn extend_left_selects_backwards() {
        let mut e = EditState::new("");
        for c in "abcd".chars() {
            e.insert_char(c);
        }
        // caret at end (4)
        e.extend_left();
        e.extend_left();
        assert_eq!(e.selection(), Some((2, 4))); // "cd"
        e.backspace();
        assert_eq!(e.text(), "ab");
    }

    #[test]
    fn word_movement() {
        let mut e = EditState::new("");
        for c in "foo bar baz".chars() {
            e.insert_char(c);
        }
        // caret at end (11)
        e.move_word_left();
        assert_eq!(e.caret(), "foo bar ".len()); // start of "baz"
        e.move_word_left();
        assert_eq!(e.caret(), "foo ".len()); // start of "bar"
        e.move_word_right();
        assert_eq!(e.caret(), "foo bar ".len()); // start of "baz"
    }

    #[test]
    fn extend_word_left_selects_word() {
        let mut e = EditState::new("");
        for c in "foo bar".chars() {
            e.insert_char(c);
        }
        e.extend_word_left(); // select "bar"
        assert_eq!(e.selection(), Some(("foo ".len(), "foo bar".len())));
        e.insert_char('!');
        assert_eq!(e.text(), "foo !");
    }

    #[test]
    fn home_end_and_extend() {
        let mut e = EditState::new("");
        for c in "hi".chars() {
            e.insert_char(c);
        }
        e.home();
        assert_eq!(e.caret(), 0);
        e.extend_end();
        assert_eq!(e.selection(), Some((0, "hi".len())));
        e.end();
        assert_eq!(e.caret(), "hi".len());
        assert_eq!(e.selection(), None);
    }

    #[test]
    fn select_all_and_clear() {
        let mut e = EditState::new("");
        for c in "xy".chars() {
            e.insert_char(c);
        }
        e.select_all();
        assert_eq!(e.selection(), Some((0, "xy".len())));
        e.clear();
        assert_eq!(e.text(), "");
        assert_eq!(e.selection(), None);
    }

    #[test]
    fn word_movement_cyrillic() {
        let mut e = EditState::new("");
        for c in "абв гд".chars() {
            e.insert_char(c);
        }
        e.move_word_left();
        assert_eq!(e.caret(), "абв ".len()); // start of "гд"
        assert!(e.caret() <= e.text().len());
    }
}
