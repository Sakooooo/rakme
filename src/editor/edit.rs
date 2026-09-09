//! Editing operations: the snarf buffer and the keyboard.

use super::*;

impl Editor {
    // --------------------------------------------------------- snarf buffer

    fn set_snarf(&mut self, s: String) {
        if let Some(cb) = &mut self.clipboard {
            let _ = cb.set_text(s.clone());
        }
        self.snarf = s;
    }

    pub(super) fn get_snarf(&mut self) -> String {
        if let Some(cb) = &mut self.clipboard
            && let Ok(s) = cb.get_text()
        {
            self.snarf = s;
        }
        self.snarf.clone()
    }

    pub(super) fn cut(&mut self, id: TextId) {
        let Some(t) = self.text(id) else {
            return;
        };
        if t.q0 == t.q1 {
            return;
        }
        let s = t.selection();
        self.set_snarf(s);
        let t = self.text_mut(id).unwrap();
        t.delete(t.q0, t.q1);
        self.show_cursor(id);
    }

    pub(super) fn snarf_sel(&mut self, id: TextId) {
        if let Some(t) = self.text(id)
            && t.q0 != t.q1
        {
            let s = t.selection();
            self.set_snarf(s);
        }
    }

    pub(super) fn paste(&mut self, id: TextId) {
        let s = self.get_snarf();
        let Some(t) = self.text_mut(id) else {
            return;
        };
        t.replace(t.q0, t.q1, &s);
        self.show_cursor(id);
    }

    pub(super) fn selection_text(&self, id: TextId) -> String {
        self.text(id).map(|t| t.selection()).unwrap_or_default()
    }

    // ------------------------------------------------------------ keyboard

    /// Handle a key press in the focused text.
    pub fn key(&mut self, k: Key) {
        let id = self.focus;
        if self.text(id).is_none() {
            return;
        }
        let is_body = matches!(id, TextId::Body(_));
        let rows = self.geom(id).map(|g| g.rows).unwrap_or(1) as i64;
        let typing = self.typing;
        let t = self.text_mut(id).unwrap();
        match k {
            Key::Char(c) => {
                let q0 = t.q0;
                t.typed(c);
                if typing.is_none_or(|(tid, _)| tid != id) {
                    self.typing = Some((id, q0));
                }
            }
            Key::Backspace | Key::Ctrl('h') => {
                if t.q0 == t.q1 && t.q0 > 0 {
                    t.delete(t.q0 - 1, t.q0);
                } else {
                    t.delete(t.q0, t.q1);
                }
            }
            Key::Delete => {
                if t.q0 == t.q1 && t.q1 < t.len() {
                    t.delete(t.q0, t.q0 + 1);
                } else {
                    t.delete(t.q0, t.q1);
                }
            }
            Key::Ctrl('u') => {
                let ls = t.line_start(t.q0);
                t.delete(ls, t.q0);
            }
            Key::Ctrl('w') => {
                let mut a = t.q0;
                while a > 0 && t.at(a - 1).is_some_and(|c| c.is_whitespace() && c != '\n') {
                    a -= 1;
                }
                while a > 0 && t.at(a - 1).is_some_and(is_word) {
                    a -= 1;
                }
                t.delete(a, t.q0);
            }
            Key::Ctrl('a') | Key::Home => {
                let p = t.line_start(t.q0);
                t.set_select(p, p);
            }
            Key::Ctrl('e') | Key::End => {
                let p = t.line_end(t.q1);
                t.set_select(p, p);
            }
            Key::Left => {
                let p = if t.q0 == t.q1 {
                    t.q0.saturating_sub(1)
                } else {
                    t.q0
                };
                t.set_select(p, p);
            }
            Key::Right => {
                let p = if t.q0 == t.q1 {
                    (t.q1 + 1).min(t.len())
                } else {
                    t.q1
                };
                t.set_select(p, p);
            }
            Key::Up | Key::Down => {
                let col = t.q0 - t.line_start(t.q0);
                let target = if matches!(k, Key::Up) {
                    let ls = t.line_start(t.q0);
                    if ls == 0 {
                        return;
                    }
                    t.line_start(ls - 1)
                } else {
                    let le = t.line_end(t.q1);
                    if le >= t.len() {
                        return;
                    }
                    le + 1
                };
                let p = (target + col).min(t.line_end(target));
                t.set_select(p, p);
            }
            Key::PageUp | Key::PageDown => {
                if let TextId::Body(w) = id {
                    let n = if matches!(k, Key::PageUp) {
                        -rows.max(1)
                    } else {
                        rows.max(1)
                    };
                    self.scroll_by(w, n);
                }
                return;
            }
            Key::Escape => {
                if let Some((tid, start)) = typing
                    && tid == id
                    && start <= t.q0
                {
                    t.set_select(start, t.q0);
                }
                return;
            }
            Key::Ctrl(_) => return,
        }
        if is_body {
            self.show_cursor(id);
        }
        self.update_tags();
    }
}
