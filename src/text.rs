//! A text buffer with a selection, undo/redo, and acme's selection rules.

use ropey::Rope;

struct Change {
    pos: usize,
    del: Vec<char>,
    ins: Vec<char>,
}

pub struct Text {
    // buf: Vec<char>,
    buf: Rope,
    pub q0: usize,
    pub q1: usize,
    undo: Vec<Change>,
    redo: Vec<Change>,
    /// Length of the undo stack when the text was last saved.
    clean: Option<usize>,
    /// Bumped on every modification.
    pub seq: u64,
}

impl Default for Text {
    fn default() -> Self {
        Text::new()
    }
}

pub fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Characters that may appear in a file name (acme's `isfilec`).
pub fn is_filec(c: char) -> bool {
    is_word(c) || ".-+/:@~$%\\".contains(c)
}

impl Text {
    pub fn new() -> Text {
        Text {
            buf: Rope::new(),
            q0: 0,
            q1: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            clean: Some(0),
            seq: 0,
        }
    }

    pub fn from_str(s: &str) -> Text {
        let mut t = Text::new();
        t.buf = Rope::from_str(s);
        t
    }

    pub fn len(&self) -> usize {
        self.buf.len_chars()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buf.len_chars() == 0
    }

    pub fn at(&self, i: usize) -> Option<char> {
        self.buf.get_char(i)
    }

    pub fn chars(&self) -> Vec<char> {
        self.buf.chars().collect()
        // &self.buf
    }

    pub fn slice(&self, a: usize, b: usize) -> String {
        let (a, b) = self.clamp(a, b);
        self.buf.slice(a..b).to_string()
        // self.buf[a..b].iter().collect()
    }

    pub fn contents(&self) -> String {
        // self.buf.iter().collect()
        self.buf.to_string()
    }

    pub fn selection(&self) -> String {
        self.slice(self.q0, self.q1)
    }

    /// Replace everything, dropping history.
    pub fn set_contents(&mut self, s: &str) {
        self.buf = Rope::from_str(s);
        self.q0 = 0;
        self.q1 = 0;
        self.undo.clear();
        self.redo.clear();
        self.clean = Some(0);
        self.seq += 1;
    }

    fn clamp(&self, a: usize, b: usize) -> (usize, usize) {
        let n = self.len();
        let a = a.min(n);
        let b = b.min(n);
        if a <= b { (a, b) } else { (b, a) }
    }

    pub fn set_select(&mut self, a: usize, b: usize) {
        let (a, b) = self.clamp(a, b);
        self.q0 = a;
        self.q1 = b;
    }

    // TODO: make this not &[char] maybe
    fn splice(&mut self, a: usize, b: usize, ins: &[char]) -> Vec<char> {
        self.seq += 1;
        let del: Vec<char> = self.buf.slice(a..b).chars().collect();
        self.buf.remove(a..b);
        let owned: String = ins.iter().collect();
        self.buf.insert(a, &owned);
        del
    }

    fn push_change(&mut self, ch: Change) {
        self.redo.clear();
        if let Some(k) = self.clean
            && k > self.undo.len()
        {
            self.clean = None;
        }
        self.undo.push(ch);
    }

    /// Replace `[a, b)` with `s`, recording undo. Afterwards the inserted text
    /// is selected.
    pub fn replace(&mut self, a: usize, b: usize, s: &str) {
        let (a, b) = self.clamp(a, b);
        let ins: Vec<char> = s.chars().collect();
        if a == b && ins.is_empty() {
            return;
        }
        let del = self.splice(a, b, &ins);
        let n = ins.len();
        self.push_change(Change { pos: a, del, ins });
        self.q0 = a;
        self.q1 = a + n;
    }

    /// Replace without recording undo, keeping the selection sensible.
    pub fn replace_raw(&mut self, a: usize, b: usize, s: &str) {
        let (a, b) = self.clamp(a, b);
        let ins: Vec<char> = s.chars().collect();
        let n = ins.len();
        self.splice(a, b, &ins);
        let adj = |q: usize| -> usize {
            if q >= b {
                q - (b - a) + n
            } else if q > a {
                q.min(a + n)
            } else {
                q
            }
        };
        self.q0 = adj(self.q0);
        self.q1 = adj(self.q1);
    }

    /// Type one character at the selection, replacing it. Consecutive typed
    /// characters are merged into one undo step.
    pub fn typed(&mut self, c: char) {
        let (a, b) = (self.q0, self.q1);
        let mergeable = a == b
            && c != '\n'
            && self.redo.is_empty()
            && self.clean != Some(self.undo.len())
            && self.undo.last().is_some_and(|ch| {
                ch.del.is_empty() && !ch.ins.is_empty() && ch.pos + ch.ins.len() == a
            });
        if mergeable {
            self.splice(a, a, &[c]);
            self.undo.last_mut().unwrap().ins.push(c);
        } else {
            self.replace(a, b, &c.to_string());
        }
        self.q0 = a + 1;
        self.q1 = a + 1;
    }

    /// Delete `[a, b)`, recording undo, and put the cursor at `a`.
    pub fn delete(&mut self, a: usize, b: usize) {
        let (a, b) = self.clamp(a, b);
        if a == b {
            return;
        }
        self.replace(a, b, "");
        self.q0 = a;
        self.q1 = a;
    }

    pub fn undo(&mut self) -> bool {
        let Some(ch) = self.undo.pop() else {
            return false;
        };
        self.splice(ch.pos, ch.pos + ch.ins.len(), &ch.del);
        self.q0 = ch.pos;
        self.q1 = ch.pos + ch.del.len();
        self.redo.push(ch);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(ch) = self.redo.pop() else {
            return false;
        };
        self.splice(ch.pos, ch.pos + ch.del.len(), &ch.ins);
        self.q0 = ch.pos;
        self.q1 = ch.pos + ch.ins.len();
        self.undo.push(ch);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.clean != Some(self.undo.len())
    }

    pub fn mark_clean(&mut self) {
        self.clean = Some(self.undo.len());
    }

    /// Start of the logical line containing position `p`.
    pub fn line_start(&self, p: usize) -> usize {
        let p = p.min(self.len());
        self.buf.line_to_char(self.buf.char_to_line(p))
    }

    /// Index of the newline ending the line containing `p`, or the length.
    pub fn line_end(&self, p: usize) -> usize {
        let p = p.min(self.len());
        self.line_end_of(self.buf.char_to_line(p))
    }

    /// Index of the newline ending 0-based line `line`, or the length.
    fn line_end_of(&self, line: usize) -> usize {
        if line + 1 < self.buf.len_lines() {
            self.buf.line_to_char(line + 1) - 1
        } else {
            self.len()
        }
    }

    /// Range (including the newline) of 1-based line `n`.
    pub fn line_range(&self, n: usize) -> (usize, usize) {
        if n == 0 || n > self.buf.len_lines() {
            return (self.len(), self.len());
        }
        let start = self.buf.line_to_char(n - 1);
        let end = self.line_end_of(n - 1);
        (start, (end + 1).min(self.len()))
    }

    /// Expand `p` to the run of characters satisfying `f` around it.
    pub fn expand(&self, p: usize, f: impl Fn(char) -> bool) -> (usize, usize) {
        let p = p.min(self.len());
        let mut a = p;
        while a > 0
            && let Some(current_char) = self.buf.get_char(a - 1)
            && f(current_char)
        {
            a -= 1;
        }
        let mut b = p;
        while b < self.len()
            && let Some(current_char) = self.buf.get_char(b)
            && f(current_char)
        {
            b += 1;
        }
        (a, b)
    }

    /// Double-click selection: brackets, quotes, whole lines, or words.
    pub fn dclick(&self, p: usize) -> (usize, usize) {
        let p = p.min(self.len());
        const L: &str = "([{<";
        const R: &str = ")]}>";
        const Q: &str = "\"'`";
        if p > 0 {
            // let c = self.buf[p - 1];
            let current_char = self.buf.get_char(p - 1);
            if let Some(c) = current_char {
                if let Some(k) = L.find(c) {
                    let (l, r) = (c, R.chars().nth(k).unwrap());
                    if let Some(m) = self.match_forward(p, l, r) {
                        return (p, m);
                    }
                } else if Q.contains(c)
                    && let Some(m) = self.buf.chars_at(p).position(|x| x == c)
                {
                    return (p, p + m);
                }
            }
        }
        if p < self.len() {
            let current_char = self.buf.get_char(p);
            if let Some(c) = current_char {
                if let Some(k) = R.find(c) {
                    let (l, r) = (L.chars().nth(k).unwrap(), c);
                    if let Some(m) = self.match_backward(p, l, r) {
                        return (m, p);
                    }
                } else if Q.contains(c)
                    && let Some(m) = self.buf.slice(..p).chars().position(|x| x == c)
                {
                    return (m + 1, p);
                }
            }
        }
        let ls = self.line_start(p);
        let le = self.line_end(p);
        if p == ls || p == le {
            return (ls, (le + 1).min(self.len()));
        }
        let current_char = self.buf.get_char(p);
        let prev_char = self.buf.get_char(p - 1);
        if let Some(cc) = current_char
            && let Some(pc) = prev_char
            && (is_word(cc) || (p > 0 && is_word(pc)))
        {
            return self.expand(p, is_word);
        }
        (p, p)
    }

    fn match_forward(&self, from: usize, l: char, r: char) -> Option<usize> {
        let mut depth = 1;
        for (i, c) in self.buf.chars().enumerate().skip(from) {
            if c == l {
                depth += 1;
            } else if c == r {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    fn match_backward(&self, from: usize, l: char, r: char) -> Option<usize> {
        let mut depth = 1;
        for i in (0..from).rev() {
            if let Some(c) = self.buf.get_char(i) {
                if c == r {
                    depth += 1;
                } else if c == l {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
            }
        }
        None
    }

    /// Find `pat` starting at `from`, wrapping around the end.
    pub fn find(&self, pat: &[char], from: usize) -> Option<usize> {
        if pat.is_empty() || pat.len() > self.len() {
            return None;
        }
        let n = self.len();
        let last = n - pat.len();
        let from = from.min(n);
        // let matches = |i: usize| self.buf[i..i + pat.len()] == *pat;
        let matches = |i: usize| {
            self.buf
                .slice(i..i + pat.len())
                .chars()
                .eq(pat.iter().copied())
        };
        (from..=last)
            .chain(0..from.min(last + 1))
            .find(|&i| matches(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_dirty() {
        let mut t = Text::from_str("hello");
        assert!(!t.is_dirty());
        t.set_select(5, 5);
        t.typed(' ');
        t.typed('w');
        assert_eq!(t.contents(), "hello w");
        assert!(t.is_dirty());
        assert!(t.undo());
        assert_eq!(t.contents(), "hello");
        assert!(!t.is_dirty());
        assert!(t.redo());
        assert_eq!(t.contents(), "hello w");
        t.mark_clean();
        t.typed('x');
        assert!(t.is_dirty());
        t.undo();
        assert!(!t.is_dirty());
    }

    #[test]
    fn undo_restores_deleted_text() {
        let mut t = Text::from_str("hello world");
        t.delete(0, 6);
        assert_eq!(t.contents(), "world");
        assert!(t.undo());
        assert_eq!(t.contents(), "hello world");
        assert_eq!((t.q0, t.q1), (0, 6));
        t.replace(6, 11, "there");
        assert_eq!(t.contents(), "hello there");
        assert!(t.undo());
        assert_eq!(t.contents(), "hello world");
        assert!(t.redo());
        assert_eq!(t.contents(), "hello there");
    }

    #[test]
    fn dclick_rules() {
        let t = Text::from_str("foo(bar baz) \"qux\"\nline");
        assert_eq!(t.dclick(1), (0, 3));
        assert_eq!(t.dclick(4), (4, 11));
        assert_eq!(t.dclick(11), (4, 11));
        assert_eq!(t.dclick(14), (14, 17));
        assert_eq!(t.dclick(19), (19, 23));
        assert_eq!(t.dclick(23), (19, 23));
    }

    #[test]
    fn find_wraps() {
        let t = Text::from_str("abc abc");
        let pat: Vec<char> = "abc".chars().collect();
        assert_eq!(t.find(&pat, 1), Some(4));
        assert_eq!(t.find(&pat, 5), Some(0));
    }

    #[test]
    fn line_range() {
        let t = Text::from_str("a\nbb\nccc");
        assert_eq!(t.line_range(2), (2, 5));
        assert_eq!(t.line_range(3), (5, 8));
        assert_eq!(t.line_range(9), (8, 8));
    }
}
