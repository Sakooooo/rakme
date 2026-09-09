//! Editing operations: the snarf buffer and the keyboard.

use super::*;

impl Editor {
    // --------------------------------------------------------- snarf buffer

    fn set_snarf(&mut self, text: String) {
        if let Some(clipboard) = &mut self.clipboard {
            let _ = clipboard.set_text(text.clone());
        }
        self.snarf = text;
    }

    pub(super) fn get_snarf(&mut self) -> String {
        if let Some(clipboard) = &mut self.clipboard
            && let Ok(text) = clipboard.get_text()
        {
            self.snarf = text;
        }
        self.snarf.clone()
    }

    pub(super) fn cut(&mut self, id: TextId) {
        let Some(text) = self.text(id) else {
            return;
        };
        if text.sel_start == text.sel_end {
            return;
        }
        let selected = text.selection();
        self.set_snarf(selected);
        let text = self.text_mut(id).unwrap();
        text.delete(text.sel_start, text.sel_end);
        self.show_cursor(id);
    }

    pub(super) fn snarf_selection(&mut self, id: TextId) {
        if let Some(text) = self.text(id)
            && text.sel_start != text.sel_end
        {
            let selected = text.selection();
            self.set_snarf(selected);
        }
    }

    pub(super) fn paste(&mut self, id: TextId) {
        let pasted = self.get_snarf();
        let Some(text) = self.text_mut(id) else {
            return;
        };
        text.replace(text.sel_start, text.sel_end, &pasted);
        self.show_cursor(id);
    }

    pub(super) fn selection_text(&self, id: TextId) -> String {
        self.text(id)
            .map(|text| text.selection())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------ keyboard

    /// Handle a key press in the focused text.
    pub fn key(&mut self, key: Key) {
        let id = self.focus;
        if self.text(id).is_none() {
            return;
        }
        let is_body = matches!(id, TextId::Body(_));
        let rows = self.geom(id).map(|geom| geom.rows).unwrap_or(1) as i64;
        let typing_start = self.typing_start;
        let text = self.text_mut(id).unwrap();
        match key {
            Key::Char(ch) => {
                let sel_start = text.sel_start;
                text.typed(ch);
                if typing_start.is_none_or(|(typing_id, _)| typing_id != id) {
                    self.typing_start = Some((id, sel_start));
                }
            }
            Key::Backspace | Key::Ctrl('h') => {
                if text.sel_start == text.sel_end && text.sel_start > 0 {
                    text.delete(text.sel_start - 1, text.sel_start);
                } else {
                    text.delete(text.sel_start, text.sel_end);
                }
            }
            Key::Delete => {
                if text.sel_start == text.sel_end && text.sel_end < text.len() {
                    text.delete(text.sel_start, text.sel_start + 1);
                } else {
                    text.delete(text.sel_start, text.sel_end);
                }
            }
            Key::Ctrl('u') => {
                let line_start = text.line_start(text.sel_start);
                text.delete(line_start, text.sel_start);
            }
            Key::Ctrl('w') => {
                let mut word_start = text.sel_start;
                while word_start > 0
                    && text
                        .at(word_start - 1)
                        .is_some_and(|ch| ch.is_whitespace() && ch != '\n')
                {
                    word_start -= 1;
                }
                while word_start > 0 && text.at(word_start - 1).is_some_and(is_word_char) {
                    word_start -= 1;
                }
                text.delete(word_start, text.sel_start);
            }
            Key::Ctrl('a') | Key::Home => {
                let pos = text.line_start(text.sel_start);
                text.set_select(pos, pos);
            }
            Key::Ctrl('e') | Key::End => {
                let pos = text.line_end(text.sel_end);
                text.set_select(pos, pos);
            }
            Key::Left => {
                let pos = if text.sel_start == text.sel_end {
                    text.sel_start.saturating_sub(1)
                } else {
                    text.sel_start
                };
                text.set_select(pos, pos);
            }
            Key::Right => {
                let pos = if text.sel_start == text.sel_end {
                    (text.sel_end + 1).min(text.len())
                } else {
                    text.sel_end
                };
                text.set_select(pos, pos);
            }
            Key::Up | Key::Down => {
                let col_offset = text.sel_start - text.line_start(text.sel_start);
                let target_line_start = if matches!(key, Key::Up) {
                    let line_start = text.line_start(text.sel_start);
                    if line_start == 0 {
                        return;
                    }
                    text.line_start(line_start - 1)
                } else {
                    let line_end = text.line_end(text.sel_end);
                    if line_end >= text.len() {
                        return;
                    }
                    line_end + 1
                };
                let pos = (target_line_start + col_offset).min(text.line_end(target_line_start));
                text.set_select(pos, pos);
            }
            Key::PageUp | Key::PageDown => {
                if let TextId::Body(win_id) = id {
                    let delta = if matches!(key, Key::PageUp) {
                        -rows.max(1)
                    } else {
                        rows.max(1)
                    };
                    self.scroll_by(win_id, delta);
                }
                return;
            }
            Key::Escape => {
                if let Some((typing_id, start)) = typing_start
                    && typing_id == id
                    && start <= text.sel_start
                {
                    text.set_select(start, text.sel_start);
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
