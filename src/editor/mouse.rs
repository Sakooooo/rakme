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
                let anchor = self.mouse.last_click.map(|(_, _, p)| p).unwrap_or(pos);
                if let Some(t) = self.text_mut(id) {
                    t.set_select(anchor.min(pos), anchor.max(pos));
                }
            }
            Action::Sweep {
                id, anchor, btn, ..
            } => {
                let pos = self.hit_pos(id, x, y);
                self.mouse.action = Action::Sweep {
                    id,
                    anchor,
                    btn,
                    q0: anchor.min(pos),
                    q1: anchor.max(pos),
                };
            }
            _ => {}
        }
    }

    pub fn mouse_press(&mut self, btn: u8, x: i32, y: i32) {
        self.mouse.x = x;
        self.mouse.y = y;
        let prev = self.mouse.buttons;
        self.mouse.buttons |= 1 << (btn - 1);
        match btn {
            1 => {
                if prev & 0b010 != 0 {
                    // 2-1 chord: the selection becomes the command's argument.
                    self.mouse.arg = Some(self.selection_text(self.focus));
                    return;
                }
                if prev != 0 {
                    return;
                }
                self.press1(x, y);
            }
            2 | 3 => {
                if prev & 0b001 != 0 {
                    // 1-2 cuts, 1-3 pastes; 1-2-3 therefore snarfs.
                    if let Action::Select { id } = self.mouse.action {
                        if btn == 2 {
                            self.cut(id);
                        } else {
                            self.paste(id);
                        }
                        self.mouse.chorded = true;
                    }
                    return;
                }
                if prev != 0 {
                    return;
                }
                self.press23(btn, x, y);
            }
            _ => {}
        }
    }

    fn press1(&mut self, x: i32, y: i32) {
        let h = self.hit(x, y);
        match h {
            Hit::RowTag | Hit::ColTag(_) | Hit::WinTag(..) | Hit::WinBody(..) => {
                let id = self.id_of_hit(h).unwrap();
                let pos = self.hit_pos(id, x, y);
                self.focus = id;
                self.typing = Some((id, pos));
                let now = Instant::now();
                let double = matches!(self.mouse.last_click, Some((t, lid, lp)) if lid == id && lp == pos && now - t < DCLICK);
                self.mouse.last_click = Some((now, id, pos));
                let t = self.text_mut(id).unwrap();
                if double {
                    let (a, b) = t.dclick(pos);
                    t.set_select(a, b);
                    self.mouse.action = Action::None;
                    self.mouse.last_click = None;
                } else {
                    t.set_select(pos, pos);
                    self.mouse.action = Action::Select { id };
                }
            }
            Hit::WinBox(ci, wi) => {
                let win = self.cols[ci].wins[wi].id;
                self.mouse.action = Action::WinBox { win, sx: x, sy: y };
            }
            Hit::ColBox(ci) => {
                let col = self.cols[ci].id;
                self.mouse.action = Action::ColBox { col, sx: x };
            }
            Hit::WinScroll(ci, wi) => self.scrollbar(ci, wi, 1, y),
            Hit::Nothing => {}
        }
    }

    fn press23(&mut self, btn: u8, x: i32, y: i32) {
        let h = self.hit(x, y);
        match h {
            Hit::RowTag | Hit::ColTag(_) | Hit::WinTag(..) | Hit::WinBody(..) => {
                let id = self.id_of_hit(h).unwrap();
                let pos = self.hit_pos(id, x, y);
                self.mouse.action = Action::Sweep {
                    id,
                    anchor: pos,
                    btn,
                    q0: pos,
                    q1: pos,
                };
            }
            Hit::WinBox(ci, wi) => {
                let win = self.cols[ci].wins[wi].id;
                self.grow_win(win, btn);
            }
            Hit::ColBox(ci) => self.grow_col(ci, btn),
            Hit::WinScroll(ci, wi) => self.scrollbar(ci, wi, btn, y),
            Hit::Nothing => {}
        }
    }

    /// Button 1 scrolls up, button 3 scrolls down (by an amount that grows
    /// with the pointer's height), button 2 jumps to that fraction.
    fn scrollbar(&mut self, ci: usize, wi: usize, btn: u8, y: i32) {
        let r = self.win_rects(ci, wi);
        let id = self.cols[ci].wins[wi].id;
        let Some(g) = self.geom(TextId::Body(id)) else {
            return;
        };
        let h = r.sb.h.max(1);
        let frac = ((y - r.sb.y) as f32 / h as f32).clamp(0.0, 1.0);
        match btn {
            1 => {
                let n = ((frac * g.rows as f32) as i64).max(1);
                self.scroll_by(id, -n);
            }
            3 => {
                let n = ((frac * g.rows as f32) as i64).max(1);
                self.scroll_by(id, n);
            }
            _ => {
                let w = self.win_mut(id).unwrap();
                let idx = (frac * w.body.len() as f32) as usize;
                let ls = w.body.line_start(idx);
                w.origin = frame::origin_for(&w.body, ls, g.cols, g.tab, 0);
            }
        }
    }

    pub fn wheel(&mut self, x: i32, y: i32, lines: i64) {
        if lines == 0 {
            return;
        }
        if let Hit::WinBody(ci, wi)
        | Hit::WinScroll(ci, wi)
        | Hit::WinTag(ci, wi)
        | Hit::WinBox(ci, wi) = self.hit(x, y)
        {
            let id = self.cols[ci].wins[wi].id;
            self.scroll_by(id, lines);
        }
    }

    pub fn mouse_release(&mut self, btn: u8, x: i32, y: i32) {
        self.mouse.x = x;
        self.mouse.y = y;
        self.mouse.buttons &= !(1 << (btn - 1));
        let action = self.mouse.action;
        match (btn, action) {
            (1, Action::Select { .. }) => self.mouse.action = Action::None,
            (1, Action::WinBox { win, sx, sy }) => {
                self.mouse.action = Action::None;
                if (x - sx).abs() > 3 || (y - sy).abs() > 3 {
                    self.move_win(win, x, y);
                } else {
                    self.grow_win(win, 1);
                }
            }
            (1, Action::ColBox { col, sx }) => {
                self.mouse.action = Action::None;
                if let Some(ci) = self.find_col(col) {
                    if (x - sx).abs() > 3 {
                        self.move_col(ci, x);
                    } else {
                        self.grow_col(ci, 1);
                    }
                }
            }
            (
                b,
                Action::Sweep {
                    id, btn, q0, q1, ..
                },
            ) if b == btn => {
                self.mouse.action = Action::None;
                let arg = self.mouse.arg.take();
                if !self.mouse.chorded && self.text(id).is_some() {
                    let (a, b) = if q0 == q1 {
                        self.expand(id, q0, btn)
                    } else {
                        (q0, q1)
                    };
                    let text = self.text(id).unwrap().slice(a, b);
                    if btn == 3 {
                        // Looking selects what was looked at, so the search
                        // continues from there.
                        self.text_mut(id).unwrap().set_select(a, b);
                    }
                    if btn == 2 {
                        let cmd = match arg {
                            Some(a) if !a.is_empty() => format!("{text} {a}"),
                            _ => text,
                        };
                        self.execute(id, &cmd);
                    } else {
                        self.look3(id, &text);
                    }
                }
            }
            _ => {}
        }
        if self.mouse.buttons == 0 {
            self.mouse.chorded = false;
            self.mouse.arg = None;
            if matches!(self.mouse.action, Action::Sweep { .. }) {
                self.mouse.action = Action::None;
            }
        }
    }

    /// Expand a null sweep at `pos` into the range to execute or look for.
    fn expand(&self, id: TextId, pos: usize, btn: u8) -> (usize, usize) {
        let t = self.text(id).unwrap();
        if t.q0 < t.q1 && pos >= t.q0 && pos <= t.q1 {
            return (t.q0, t.q1);
        }
        if btn == 2 {
            t.expand(pos, |c| !c.is_whitespace())
        } else {
            let (a, b) = t.expand(pos, is_filec);
            if a == b {
                t.expand(pos, is_word)
            } else {
                (a, b)
            }
        }
    }

    /// The sweep highlight to draw for `id`, if any.
    pub(super) fn sweep_of(&self, id: TextId) -> Option<(usize, usize, gfx::Color)> {
        match self.mouse.action {
            Action::Sweep {
                id: sid,
                btn,
                q0,
                q1,
                ..
            } if sid == id => Some((q0, q1, if btn == 2 { gfx::BUT2 } else { gfx::BUT3 })),
            _ => None,
        }
    }
}
