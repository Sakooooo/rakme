//! A text buffer with a selection, undo/redo, and acme's selection rules.

use ropey::Rope;

struct Change {
    pos: usize,
    deleted: Vec<char>,
    inserted: Vec<char>,
}

pub struct Text {
    buf: Rope,
    /// Start of the selection (character index).
    pub sel_start: usize,
    /// End of the selection, exclusive. Equal to `sel_start` for a caret.
    pub sel_end: usize,
    undo_stack: Vec<Change>,
    redo_stack: Vec<Change>,
    /// Length of the undo stack when the text was last saved, or `None` if
    /// that state can no longer be reached by undoing.
    clean_undo_len: Option<usize>,
    /// Bumped on every modification.
    pub revision: u64,
}

impl Default for Text {
    fn default() -> Self {
        Text::new()
    }
}

pub fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Characters that may appear in a file name (acme's `isfilec`).
pub fn is_filename_char(ch: char) -> bool {
    is_word_char(ch) || ".-+/:@~$%\\".contains(ch)
}

impl Text {
    pub fn new() -> Text {
        Text {
            buf: Rope::new(),
            sel_start: 0,
            sel_end: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clean_undo_len: Some(0),
            revision: 0,
        }
    }

    pub fn from_str(s: &str) -> Text {
        let mut text = Text::new();
        text.buf = Rope::from_str(s);
        text
    }

    pub fn len(&self) -> usize {
        self.buf.len_chars()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buf.len_chars() == 0
    }

    pub fn at(&self, pos: usize) -> Option<char> {
        self.buf.get_char(pos)
    }

    pub fn chars(&self) -> Vec<char> {
        self.buf.chars().collect()
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        let (start, end) = self.clamp_range(start, end);
        self.buf.slice(start..end).to_string()
    }

    pub fn contents(&self) -> String {
        self.buf.to_string()
    }

    pub fn selection(&self) -> String {
        self.slice(self.sel_start, self.sel_end)
    }

    /// Replace everything, dropping history.
    pub fn set_contents(&mut self, s: &str) {
        self.buf = Rope::from_str(s);
        self.sel_start = 0;
        self.sel_end = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.clean_undo_len = Some(0);
        self.revision += 1;
    }

    /// Clamp both ends to the text and order them.
    fn clamp_range(&self, start: usize, end: usize) -> (usize, usize) {
        let len = self.len();
        let start = start.min(len);
        let end = end.min(len);
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }

    pub fn set_select(&mut self, start: usize, end: usize) {
        let (start, end) = self.clamp_range(start, end);
        self.sel_start = start;
        self.sel_end = end;
    }

    // TODO: make this not &[char] maybe
    fn splice(&mut self, start: usize, end: usize, inserted: &[char]) -> Vec<char> {
        self.revision += 1;
        let deleted: Vec<char> = self.buf.slice(start..end).chars().collect();
        self.buf.remove(start..end);
        let inserted_str: String = inserted.iter().collect();
        self.buf.insert(start, &inserted_str);
        deleted
    }

    fn push_change(&mut self, change: Change) {
        self.redo_stack.clear();
        if let Some(clean_len) = self.clean_undo_len
            && clean_len > self.undo_stack.len()
        {
            self.clean_undo_len = None;
        }
        self.undo_stack.push(change);
    }

    /// Replace `[start, end)` with `text`, recording undo. Afterwards the
    /// inserted text is selected.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let (start, end) = self.clamp_range(start, end);
        let inserted: Vec<char> = text.chars().collect();
        if start == end && inserted.is_empty() {
            return;
        }
        let deleted = self.splice(start, end, &inserted);
        let inserted_len = inserted.len();
        self.push_change(Change {
            pos: start,
            deleted,
            inserted,
        });
        self.sel_start = start;
        self.sel_end = start + inserted_len;
    }

    /// Replace without recording undo, keeping the selection sensible.
    pub fn replace_raw(&mut self, start: usize, end: usize, text: &str) {
        let (start, end) = self.clamp_range(start, end);
        let inserted: Vec<char> = text.chars().collect();
        let inserted_len = inserted.len();
        self.splice(start, end, &inserted);
        let adjust = |pos: usize| -> usize {
            if pos >= end {
                pos - (end - start) + inserted_len
            } else if pos > start {
                pos.min(start + inserted_len)
            } else {
                pos
            }
        };
        self.sel_start = adjust(self.sel_start);
        self.sel_end = adjust(self.sel_end);
    }

    /// Type one character at the selection, replacing it. Consecutive typed
    /// characters are merged into one undo step.
    pub fn typed(&mut self, ch: char) {
        let (start, end) = (self.sel_start, self.sel_end);
        let mergeable = start == end
            && ch != '\n'
            && self.redo_stack.is_empty()
            && self.clean_undo_len != Some(self.undo_stack.len())
            && self.undo_stack.last().is_some_and(|change| {
                change.deleted.is_empty()
                    && !change.inserted.is_empty()
                    && change.pos + change.inserted.len() == start
            });
        if mergeable {
            self.splice(start, start, &[ch]);
            self.undo_stack.last_mut().unwrap().inserted.push(ch);
        } else {
            self.replace(start, end, &ch.to_string());
        }
        self.sel_start = start + 1;
        self.sel_end = start + 1;
    }

    /// Delete `[start, end)`, recording undo, and put the cursor at `start`.
    pub fn delete(&mut self, start: usize, end: usize) {
        let (start, end) = self.clamp_range(start, end);
        if start == end {
            return;
        }
        self.replace(start, end, "");
        self.sel_start = start;
        self.sel_end = start;
    }

    pub fn undo(&mut self) -> bool {
        let Some(change) = self.undo_stack.pop() else {
            return false;
        };
        self.splice(
            change.pos,
            change.pos + change.inserted.len(),
            &change.deleted,
        );
        self.sel_start = change.pos;
        self.sel_end = change.pos + change.deleted.len();
        self.redo_stack.push(change);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(change) = self.redo_stack.pop() else {
            return false;
        };
        self.splice(
            change.pos,
            change.pos + change.deleted.len(),
            &change.inserted,
        );
        self.sel_start = change.pos;
        self.sel_end = change.pos + change.inserted.len();
        self.undo_stack.push(change);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.clean_undo_len != Some(self.undo_stack.len())
    }

    pub fn mark_clean(&mut self) {
        self.clean_undo_len = Some(self.undo_stack.len());
    }

    /// Start of the logical line containing position `pos`.
    pub fn line_start(&self, pos: usize) -> usize {
        let pos = pos.min(self.len());
        self.buf.line_to_char(self.buf.char_to_line(pos))
    }

    /// Index of the newline ending the line containing `pos`, or the length.
    pub fn line_end(&self, pos: usize) -> usize {
        let pos = pos.min(self.len());
        self.line_end_of(self.buf.char_to_line(pos))
    }

    /// Index of the newline ending 0-based line `line`, or the length.
    fn line_end_of(&self, line: usize) -> usize {
        if line + 1 < self.buf.len_lines() {
            self.buf.line_to_char(line + 1) - 1
        } else {
            self.len()
        }
    }

    /// Range (including the newline) of 1-based line `line_num`.
    pub fn line_range(&self, line_num: usize) -> (usize, usize) {
        if line_num == 0 || line_num > self.buf.len_lines() {
            return (self.len(), self.len());
        }
        let start = self.buf.line_to_char(line_num - 1);
        let end = self.line_end_of(line_num - 1);
        (start, (end + 1).min(self.len()))
    }

    /// Expand `pos` to the run of characters satisfying `pred` around it.
    pub fn expand(&self, pos: usize, pred: impl Fn(char) -> bool) -> (usize, usize) {
        let pos = pos.min(self.len());
        let mut start = pos;
        while start > 0
            && let Some(ch) = self.buf.get_char(start - 1)
            && pred(ch)
        {
            start -= 1;
        }
        let mut end = pos;
        while end < self.len()
            && let Some(ch) = self.buf.get_char(end)
            && pred(ch)
        {
            end += 1;
        }
        (start, end)
    }

    /// Double-click selection: brackets, quotes, whole lines, or words.
    pub fn double_click(&self, pos: usize) -> (usize, usize) {
        let pos = pos.min(self.len());
        const OPENERS: &str = "([{<";
        const CLOSERS: &str = ")]}>";
        const QUOTES: &str = "\"'`";
        if pos > 0
            && let Some(prev) = self.buf.get_char(pos - 1)
        {
            if let Some(idx) = OPENERS.find(prev) {
                let (open, close) = (prev, CLOSERS.chars().nth(idx).unwrap());
                if let Some(close_pos) = self.match_forward(pos, open, close) {
                    return (pos, close_pos);
                }
            } else if QUOTES.contains(prev)
                && let Some(offset) = self.buf.chars_at(pos).position(|ch| ch == prev)
            {
                return (pos, pos + offset);
            }
        }
        if pos < self.len()
            && let Some(cur) = self.buf.get_char(pos)
        {
            if let Some(idx) = CLOSERS.find(cur) {
                let (open, close) = (OPENERS.chars().nth(idx).unwrap(), cur);
                if let Some(open_pos) = self.match_backward(pos, open, close) {
                    return (open_pos, pos);
                }
            } else if QUOTES.contains(cur)
                && let Some(open_pos) = self.buf.slice(..pos).chars().position(|ch| ch == cur)
            {
                return (open_pos + 1, pos);
            }
        }
        let line_start = self.line_start(pos);
        let line_end = self.line_end(pos);
        if pos == line_start || pos == line_end {
            return (line_start, (line_end + 1).min(self.len()));
        }
        let cur = self.buf.get_char(pos);
        let prev = self.buf.get_char(pos - 1);
        if let Some(cur) = cur
            && let Some(prev) = prev
            && (is_word_char(cur) || (pos > 0 && is_word_char(prev)))
        {
            return self.expand(pos, is_word_char);
        }
        (pos, pos)
    }

    /// Index of the `close` matching an `open` just before `from`.
    fn match_forward(&self, from: usize, open: char, close: char) -> Option<usize> {
        let mut depth = 1;
        for (pos, ch) in self.buf.chars().enumerate().skip(from) {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Index just after the `open` matching a `close` at `from`.
    fn match_backward(&self, from: usize, open: char, close: char) -> Option<usize> {
        let mut depth = 1;
        for pos in (0..from).rev() {
            if let Some(ch) = self.buf.get_char(pos) {
                if ch == close {
                    depth += 1;
                } else if ch == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(pos + 1);
                    }
                }
            }
        }
        None
    }

    /// Find `pattern` starting at `from`, wrapping around the end.
    pub fn find(&self, pattern: &[char], from: usize) -> Option<usize> {
        if pattern.is_empty() || pattern.len() > self.len() {
            return None;
        }
        let len = self.len();
        let last_start = len - pattern.len();
        let from = from.min(len);
        let matches_at = |pos: usize| {
            self.buf
                .slice(pos..pos + pattern.len())
                .chars()
                .eq(pattern.iter().copied())
        };
        (from..=last_start)
            .chain(0..from.min(last_start + 1))
            .find(|&pos| matches_at(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_dirty() {
        let mut text = Text::from_str("hello");
        assert!(!text.is_dirty());
        text.set_select(5, 5);
        text.typed(' ');
        text.typed('w');
        assert_eq!(text.contents(), "hello w");
        assert!(text.is_dirty());
        assert!(text.undo());
        assert_eq!(text.contents(), "hello");
        assert!(!text.is_dirty());
        assert!(text.redo());
        assert_eq!(text.contents(), "hello w");
        text.mark_clean();
        text.typed('x');
        assert!(text.is_dirty());
        text.undo();
        assert!(!text.is_dirty());
    }

    #[test]
    fn undo_restores_deleted_text() {
        let mut text = Text::from_str("hello world");
        text.delete(0, 6);
        assert_eq!(text.contents(), "world");
        assert!(text.undo());
        assert_eq!(text.contents(), "hello world");
        assert_eq!((text.sel_start, text.sel_end), (0, 6));
        text.replace(6, 11, "there");
        assert_eq!(text.contents(), "hello there");
        assert!(text.undo());
        assert_eq!(text.contents(), "hello world");
        assert!(text.redo());
        assert_eq!(text.contents(), "hello there");
    }

    #[test]
    fn double_click_rules() {
        let text = Text::from_str("foo(bar baz) \"qux\"\nline");
        assert_eq!(text.double_click(1), (0, 3));
        assert_eq!(text.double_click(4), (4, 11));
        assert_eq!(text.double_click(11), (4, 11));
        assert_eq!(text.double_click(14), (14, 17));
        assert_eq!(text.double_click(19), (19, 23));
        assert_eq!(text.double_click(23), (19, 23));
    }

    #[test]
    fn find_wraps() {
        let text = Text::from_str("abc abc");
        let pattern: Vec<char> = "abc".chars().collect();
        assert_eq!(text.find(&pattern, 1), Some(4));
        assert_eq!(text.find(&pattern, 5), Some(0));
    }

    #[test]
    fn line_range() {
        let text = Text::from_str("a\nbb\nccc");
        assert_eq!(text.line_range(2), (2, 5));
        assert_eq!(text.line_range(3), (5, 8));
        assert_eq!(text.line_range(9), (8, 8));
    }
}
