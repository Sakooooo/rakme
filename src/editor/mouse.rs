//! The mouse language: selecting with button 1, sweeping with buttons 2
//! and 3, chording, and clicks on boxes and scrollbars.

use super::*;

impl Editor {
    pub fn mouse_move(&mut self, x: i32, y: i32) {
        self.mouse.x = x;
        self.mouse.y = y;
        match self.mouse.action {
            Action::Select { id } => {
                let pos = self.hit_pos(id, x, y);
                let anchor = self
                    .mouse
                    .last_click
                    .map(|(_, _, click_pos)| click_pos)
                    .unwrap_or(pos);
                if let Some(text) = self.text_mut(id) {
                    text.set_select(anchor.min(pos), anchor.max(pos));
                }
            }
            Action::Sweep {
                id, anchor, button, ..
            } => {
                let pos = self.hit_pos(id, x, y);
                self.mouse.action = Action::Sweep {
                    id,
                    anchor,
                    button,
                    start: anchor.min(pos),
                    end: anchor.max(pos),
                };
            }
            _ => {}
        }
    }

    pub fn mouse_press(&mut self, button: u8, x: i32, y: i32) {
        self.mouse.x = x;
        self.mouse.y = y;
        let prev_buttons = self.mouse.held_buttons;
        self.mouse.held_buttons |= 1 << (button - 1);
        match button {
            1 => {
                if prev_buttons & 0b010 != 0 {
                    // 2-1 chord: the selection becomes the command's argument.
                    self.mouse.chord_arg = Some(self.selection_text(self.focus));
                    return;
                }
                if prev_buttons != 0 {
                    return;
                }
                self.press_button1(x, y);
            }
            2 | 3 => {
                if prev_buttons & 0b001 != 0 {
                    // 1-2 cuts, 1-3 pastes; 1-2-3 therefore snarfs.
                    if let Action::Select { id } = self.mouse.action {
                        if button == 2 {
                            self.cut(id);
                        } else {
                            self.paste(id);
                        }
                        self.mouse.chorded = true;
                    }
                    return;
                }
                if prev_buttons != 0 {
                    return;
                }
                self.press_button23(button, x, y);
            }
            _ => {}
        }
    }

    fn press_button1(&mut self, x: i32, y: i32) {
        let hit = self.hit(x, y);
        match hit {
            Hit::RowTag | Hit::ColTag(_) | Hit::WinTag(..) | Hit::WinBody(..) => {
                let id = self.id_of_hit(hit).unwrap();
                let pos = self.hit_pos(id, x, y);
                self.focus = id;
                self.typing_start = Some((id, pos));
                let now = Instant::now();
                let is_double_click = matches!(
                    self.mouse.last_click,
                    Some((last_time, last_id, last_pos))
                        if last_id == id && last_pos == pos && now - last_time < DOUBLE_CLICK_TIME
                );
                self.mouse.last_click = Some((now, id, pos));
                let text = self.text_mut(id).unwrap();
                if is_double_click {
                    let (start, end) = text.double_click(pos);
                    text.set_select(start, end);
                    self.mouse.action = Action::None;
                    self.mouse.last_click = None;
                } else {
                    text.set_select(pos, pos);
                    self.mouse.action = Action::Select { id };
                }
            }
            Hit::WinBox(col_idx, win_idx) => {
                let win_id = self.columns[col_idx].windows[win_idx].id;
                self.mouse.action = Action::WinBox {
                    win_id,
                    press_x: x,
                    press_y: y,
                };
            }
            Hit::ColBox(col_idx) => {
                let col_id = self.columns[col_idx].id;
                self.mouse.action = Action::ColBox { col_id, press_x: x };
            }
            Hit::WinScroll(col_idx, win_idx) => self.scrollbar(col_idx, win_idx, 1, y),
            Hit::Nothing => {}
        }
    }

    fn press_button23(&mut self, button: u8, x: i32, y: i32) {
        let hit = self.hit(x, y);
        match hit {
            Hit::RowTag | Hit::ColTag(_) | Hit::WinTag(..) | Hit::WinBody(..) => {
                let id = self.id_of_hit(hit).unwrap();
                let pos = self.hit_pos(id, x, y);
                self.mouse.action = Action::Sweep {
                    id,
                    anchor: pos,
                    button,
                    start: pos,
                    end: pos,
                };
            }
            Hit::WinBox(col_idx, win_idx) => {
                let win_id = self.columns[col_idx].windows[win_idx].id;
                self.grow_win(win_id, button);
            }
            Hit::ColBox(col_idx) => self.grow_col(col_idx, button),
            Hit::WinScroll(col_idx, win_idx) => self.scrollbar(col_idx, win_idx, button, y),
            Hit::Nothing => {}
        }
    }

    /// Button 1 scrolls up, button 3 scrolls down (by an amount that grows
    /// with the pointer's height), button 2 jumps to that fraction.
    fn scrollbar(&mut self, col_idx: usize, win_idx: usize, button: u8, y: i32) {
        let rects = self.win_rects(col_idx, win_idx);
        let win_id = self.columns[col_idx].windows[win_idx].id;
        let Some(geom) = self.geom(TextId::Body(win_id)) else {
            return;
        };
        let trough_height = rects.scrollbar.h.max(1);
        let frac = ((y - rects.scrollbar.y) as f32 / trough_height as f32).clamp(0.0, 1.0);
        match button {
            1 => {
                let lines = ((frac * geom.rows as f32) as i64).max(1);
                self.scroll_by(win_id, -lines);
            }
            3 => {
                let lines = ((frac * geom.rows as f32) as i64).max(1);
                self.scroll_by(win_id, lines);
            }
            _ => {
                let win = self.win_mut(win_id).unwrap();
                let pos = (frac * win.body.len() as f32) as usize;
                let line_start = win.body.line_start(pos);
                win.origin = frame::origin_for(&win.body, line_start, geom.cols, geom.tab_width, 0);
            }
        }
    }

    pub fn wheel(&mut self, x: i32, y: i32, lines: i64) {
        if lines == 0 {
            return;
        }
        if let Hit::WinBody(col_idx, win_idx)
        | Hit::WinScroll(col_idx, win_idx)
        | Hit::WinTag(col_idx, win_idx)
        | Hit::WinBox(col_idx, win_idx) = self.hit(x, y)
        {
            let win_id = self.columns[col_idx].windows[win_idx].id;
            self.scroll_by(win_id, lines);
        }
    }

    pub fn mouse_release(&mut self, button: u8, x: i32, y: i32) {
        self.mouse.x = x;
        self.mouse.y = y;
        self.mouse.held_buttons &= !(1 << (button - 1));
        let action = self.mouse.action;
        match (button, action) {
            (1, Action::Select { .. }) => self.mouse.action = Action::None,
            (
                1,
                Action::WinBox {
                    win_id,
                    press_x,
                    press_y,
                },
            ) => {
                self.mouse.action = Action::None;
                if (x - press_x).abs() > 3 || (y - press_y).abs() > 3 {
                    self.move_win(win_id, x, y);
                } else {
                    self.grow_win(win_id, 1);
                }
            }
            (1, Action::ColBox { col_id, press_x }) => {
                self.mouse.action = Action::None;
                if let Some(col_idx) = self.find_col(col_id) {
                    if (x - press_x).abs() > 3 {
                        self.move_col(col_idx, x);
                    } else {
                        self.grow_col(col_idx, 1);
                    }
                }
            }
            (
                released,
                Action::Sweep {
                    id,
                    button: sweep_button,
                    start,
                    end,
                    ..
                },
            ) if released == sweep_button => {
                self.mouse.action = Action::None;
                let chord_arg = self.mouse.chord_arg.take();
                if !self.mouse.chorded && self.text(id).is_some() {
                    let (start, end) = if start == end {
                        self.sweep_range(id, start, sweep_button)
                    } else {
                        (start, end)
                    };
                    let swept = self.text(id).unwrap().slice(start, end);
                    if sweep_button == 3 {
                        // Looking selects what was looked at, so the search
                        // continues from there.
                        self.text_mut(id).unwrap().set_select(start, end);
                    }
                    if sweep_button == 2 {
                        let command_line = match chord_arg {
                            Some(arg) if !arg.is_empty() => format!("{swept} {arg}"),
                            _ => swept,
                        };
                        self.execute(id, &command_line);
                    } else {
                        self.look3(id, &swept);
                    }
                }
            }
            _ => {}
        }
        if self.mouse.held_buttons == 0 {
            self.mouse.chorded = false;
            self.mouse.chord_arg = None;
            if matches!(self.mouse.action, Action::Sweep { .. }) {
                self.mouse.action = Action::None;
            }
        }
    }

    /// Expand a null sweep at `pos` into the range to execute or look for.
    fn sweep_range(&self, id: TextId, pos: usize, button: u8) -> (usize, usize) {
        let text = self.text(id).unwrap();
        if text.sel_start < text.sel_end && pos >= text.sel_start && pos <= text.sel_end {
            return (text.sel_start, text.sel_end);
        }
        if button == 2 {
            text.expand(pos, |ch| !ch.is_whitespace())
        } else {
            let (start, end) = text.expand(pos, is_filename_char);
            if start == end {
                text.expand(pos, is_word_char)
            } else {
                (start, end)
            }
        }
    }

    /// The sweep highlight to draw for `id`, if any.
    pub(super) fn sweep_highlight(&self, id: TextId) -> Option<(usize, usize, gfx::Color)> {
        match self.mouse.action {
            Action::Sweep {
                id: sweep_id,
                button,
                start,
                end,
                ..
            } if sweep_id == id => Some((
                start,
                end,
                if button == 2 {
                    gfx::BUTTON2_SWEEP
                } else {
                    gfx::BUTTON3_SWEEP
                },
            )),
            _ => None,
        }
    }
}
