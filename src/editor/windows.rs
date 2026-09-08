//! Creating, sizing, moving and closing columns and windows, and keeping
//! window tags up to date.

use super::*;

impl Editor {
    pub(super) fn new_col(&mut self, at_x: i32) -> usize {
        let id = self.alloc_id();
        let col = Column {
            id,
            x0: at_x,
            tag: Text::from_str("New Cut Paste Snarf Sort Zerox Delcol "),
            wins: Vec::new(),
        };
        if self.cols.is_empty() {
            self.cols.push(col);
            return 0;
        }
        // Split the widest column.
        let mut best = 0;
        let mut best_w = 0;
        for ci in 0..self.cols.len() {
            let w = self.col_rect(ci).w;
            if w > best_w {
                best_w = w;
                best = ci;
            }
        }
        let cr = self.col_rect(best);
        let mut col = col;
        col.x0 = cr.x + cr.w / 2;
        self.cols.insert(best + 1, col);
        self.fix_layout();
        best + 1
    }

    /// Create an empty window in column `ci`, splitting its tallest window.
    pub(super) fn new_win(&mut self, ci: usize, name: String) -> usize {
        if self.cols.is_empty() {
            self.new_col(0);
        }
        let ci = ci.min(self.cols.len() - 1);
        let id = self.alloc_id();
        let mut win = Window::new(id, name);
        let top = self.wins_top(ci);
        let bottom = self.col_rect(ci).bottom();
        let col = &mut self.cols[ci];
        if col.wins.is_empty() {
            win.y0 = top;
            col.wins.push(win);
        } else {
            let mut best = 0;
            let mut best_h = -1;
            for wi in 0..col.wins.len() {
                let y1 = col.wins.get(wi + 1).map(|w| w.y0).unwrap_or(bottom);
                let h = y1 - col.wins[wi].y0;
                if h > best_h {
                    best_h = h;
                    best = wi;
                }
            }
            let y1 = col.wins.get(best + 1).map(|w| w.y0).unwrap_or(bottom);
            win.y0 = col.wins[best].y0 + (y1 - col.wins[best].y0) / 2;
            col.wins.insert(best + 1, win);
        }
        self.fix_col(ci);
        self.update_tags();
        id
    }

    pub(super) fn close_win(&mut self, id: usize) {
        let Some((ci, wi)) = self.find_win(id) else {
            return;
        };
        self.cols[ci].wins.remove(wi);
        self.fix_col(ci);
        if let TextId::Body(f) | TextId::Tag(f) = self.focus
            && f == id
        {
            self.focus = TextId::ColTag(self.cols[ci].id);
        }
    }

    fn set_win_height(&mut self, ci: usize, wi: usize, new_h: i32) {
        let top = self.wins_top(ci);
        let bottom = self.col_rect(ci).bottom();
        let min_h = self.min_h();
        let n = self.cols[ci].wins.len();
        let wins = &mut self.cols[ci].wins;
        let below = (n - wi - 1) as i32;
        let new_h = new_h.min(bottom - top - (n as i32 - 1) * min_h).max(min_h);
        let mut y0 = wins[wi].y0;
        if y0 + new_h + below * min_h > bottom {
            y0 = (bottom - below * min_h - new_h).max(top + wi as i32 * min_h);
        }
        wins[wi].y0 = y0;
        for j in (0..wi).rev() {
            let limit = wins[j + 1].y0 - min_h;
            wins[j].y0 = wins[j].y0.min(limit);
        }
        for j in wi + 1..n {
            let need = if j == wi + 1 { new_h } else { min_h };
            wins[j].y0 = wins[j].y0.max(wins[j - 1].y0 + need);
        }
        self.fix_col(ci);
    }

    /// Button 1 grows a window somewhat; buttons 2 and 3 fill the column.
    pub(super) fn grow_win(&mut self, id: usize, btn: u8) {
        let Some((ci, wi)) = self.find_win(id) else {
            return;
        };
        let r = self.win_rects(ci, wi);
        let colh = self.col_rect(ci).bottom() - self.wins_top(ci);
        let new_h = match btn {
            1 => (r.all.h + colh / 3).max(r.all.h * 3 / 2),
            _ => colh,
        };
        self.set_win_height(ci, wi, new_h);
    }

    /// Move window `id` so its top is at (x, y), possibly into another column.
    pub(super) fn move_win(&mut self, id: usize, x: i32, y: i32) {
        let Some((sci, swi)) = self.find_win(id) else {
            return;
        };
        let Some(tci) = (0..self.cols.len()).find(|&ci| self.col_rect(ci).contains(x, y)) else {
            return;
        };
        let mut win = self.cols[sci].wins.remove(swi);
        self.fix_col(sci);
        let idx = self.cols[tci].wins.iter().filter(|w| w.y0 < y).count();
        win.y0 = y;
        if idx == 0 {
            if let Some(first) = self.cols[tci].wins.first_mut() {
                first.y0 = y;
            }
            win.y0 = self.wins_top(tci);
        }
        self.cols[tci].wins.insert(idx, win);
        self.fix_col(tci);
    }

    pub(super) fn grow_col(&mut self, ci: usize, btn: u8) {
        let n = self.cols.len();
        if n < 2 {
            return;
        }
        let min_w = self.min_w();
        if btn == 1 {
            let cr = self.col_rect(ci);
            let left = if ci > 0 { self.col_rect(ci - 1).w } else { 0 };
            let right = if ci + 1 < n { self.col_rect(ci + 1).w } else { 0 };
            if right >= left && ci + 1 < n {
                self.cols[ci + 1].x0 = (cr.right() + right / 2).min(self.w - (n - ci - 1) as i32 * min_w);
            } else if ci > 0 {
                self.cols[ci].x0 = (cr.x - left / 2).max(ci as i32 * min_w);
            }
        } else {
            for j in 0..n {
                self.cols[j].x0 = if j <= ci { j as i32 * min_w } else { self.w - (n - j) as i32 * min_w };
            }
        }
        self.fix_layout();
    }

    pub(super) fn move_col(&mut self, ci: usize, x: i32) {
        if ci == 0 {
            return;
        }
        self.cols[ci].x0 = x;
        self.fix_layout();
    }

    // ----------------------------------------------------------------- tags

    /// Rewrite the fixed part of every window tag (name and the commands
    /// that apply right now), leaving whatever follows the `|` alone.
    pub fn update_tags(&mut self) {
        for ci in 0..self.cols.len() {
            for wi in 0..self.cols[ci].wins.len() {
                let w = &mut self.cols[ci].wins[wi];
                let mut prefix = format!("{} Del Snarf", w.name);
                if w.is_dir {
                    prefix.push_str(" Get");
                } else {
                    if w.body.can_undo() {
                        prefix.push_str(" Undo");
                    }
                    if w.body.can_redo() {
                        prefix.push_str(" Redo");
                    }
                    if w.body.is_dirty() {
                        prefix.push_str(" Put");
                    }
                }
                prefix.push(' ');
                let old_len = w.tag.chars().iter().position(|&c| c == '|').unwrap_or(w.tag.len());
                let old = w.tag.slice(0, old_len);
                if old != prefix {
                    w.tag.replace_raw(0, old_len, &prefix);
                }
                if !w.tag.chars().contains(&'|') {
                    let n = w.tag.len();
                    w.tag.replace_raw(n, n, "| Look ");
                }
            }
        }
    }

    /// The file name currently written at the start of the tag.
    pub(super) fn tag_name(&self, id: usize) -> String {
        let w = self.win(id).unwrap();
        w.tag.contents().split_whitespace().next().unwrap_or("").to_string()
    }
}
