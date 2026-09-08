//! Geometry: where columns, windows, tags and bodies are on screen, hit
//! testing, lookups by id, and scrolling.

use super::*;

impl Editor {
    fn tag_h(&self, rows: usize) -> i32 {
        rows as i32 * self.font.line_h + 2 * TAG_PAD + 1
    }

    pub(super) fn min_h(&self) -> i32 {
        self.tag_h(1)
    }

    pub(super) fn min_w(&self) -> i32 {
        SB_W * 4
    }

    fn tag_cols(&self, width: i32) -> usize {
        ((width - SB_W - 4) / self.font.adv).max(1) as usize
    }

    fn tag_rows(&self, t: &Text, width: i32) -> usize {
        frame::wrap(t, 0, self.tag_cols(width), 3, 4, false).len().max(1)
    }

    pub(super) fn row_h(&self) -> i32 {
        self.tag_h(self.tag_rows(&self.row_tag, self.w))
    }

    pub fn col_rect(&self, ci: usize) -> Rect {
        let x0 = self.cols[ci].x0;
        let x1 = self.cols.get(ci + 1).map(|c| c.x0).unwrap_or(self.w);
        let y = self.row_h();
        Rect::new(x0, y, x1 - x0, self.h - y)
    }

    pub(super) fn col_tag_rect(&self, ci: usize) -> Rect {
        let cr = self.col_rect(ci);
        let rows = self.tag_rows(&self.cols[ci].tag, cr.w);
        Rect::new(cr.x, cr.y, cr.w, self.tag_h(rows))
    }

    pub(super) fn wins_top(&self, ci: usize) -> i32 {
        self.col_tag_rect(ci).bottom()
    }

    pub fn win_rects(&self, ci: usize, wi: usize) -> WinRects {
        let cr = self.col_rect(ci);
        let col = &self.cols[ci];
        let win = &col.wins[wi];
        let y0 = win.y0;
        let y1 = col.wins.get(wi + 1).map(|w| w.y0).unwrap_or(cr.bottom());
        let tag_rows = self.tag_rows(&win.tag, cr.w);
        let th = self.tag_h(tag_rows).min(y1 - y0);
        let all = Rect::new(cr.x, y0, cr.w, y1 - y0);
        let tag = Rect::new(cr.x, y0, cr.w, th);
        let bx = Rect::new(cr.x + 1, y0 + TAG_PAD + 1, SB_W - 3, SB_W - 3);
        let text_h = (tag_rows as i32 * self.font.line_h).min((th - 2 * TAG_PAD - 1).max(0));
        let tag_text = Rect::new(cr.x + SB_W + 2, y0 + TAG_PAD, cr.w - SB_W - 4, text_h);
        let body = Rect::new(cr.x, y0 + th, cr.w, y1 - y0 - th);
        let sb = Rect::new(cr.x, body.y, SB_W, body.h);
        let body_text = Rect::new(cr.x + SB_W + TEXT_PAD, body.y, cr.w - SB_W - TEXT_PAD - 2, body.h);
        WinRects { all, bx, tag, tag_text, tag_rows, sb, body, body_text }
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        let (ow, oh) = (self.w.max(1), self.h.max(1));
        let top = self.row_h();
        self.w = w;
        self.h = h;
        for col in &mut self.cols {
            col.x0 = (col.x0 as i64 * w as i64 / ow as i64) as i32;
            for win in &mut col.wins {
                let rel = (win.y0 - top).max(0) as i64;
                win.y0 = top + (rel * (h - top).max(1) as i64 / (oh - top).max(1) as i64) as i32;
            }
        }
        self.fix_layout();
    }

    /// Clamp column and window positions so everything fits and nothing
    /// is smaller than a tag line.
    pub(super) fn fix_layout(&mut self) {
        let min_w = self.min_w();
        let n = self.cols.len();
        for i in 0..n {
            if i == 0 {
                self.cols[i].x0 = 0;
            } else {
                let prev = self.cols[i - 1].x0;
                self.cols[i].x0 = self.cols[i].x0.max(prev + min_w);
            }
        }
        for i in (1..n).rev() {
            let limit = self.w - (n - i) as i32 * min_w;
            self.cols[i].x0 = self.cols[i].x0.min(limit);
        }
        for ci in 0..n {
            self.fix_col(ci);
        }
    }

    pub(super) fn fix_col(&mut self, ci: usize) {
        let top = self.wins_top(ci);
        let bottom = self.col_rect(ci).bottom();
        let min_h = self.min_h();
        let n = self.cols[ci].wins.len();
        let wins = &mut self.cols[ci].wins;
        for i in 0..n {
            if i == 0 {
                wins[i].y0 = top;
            } else {
                let prev = wins[i - 1].y0;
                wins[i].y0 = wins[i].y0.max(prev + min_h);
            }
        }
        for i in (1..n).rev() {
            let limit = bottom - (n - i) as i32 * min_h;
            wins[i].y0 = wins[i].y0.min(limit).max(top);
        }
    }

    pub fn hit(&self, x: i32, y: i32) -> Hit {
        if y < self.row_h() {
            return Hit::RowTag;
        }
        for ci in 0..self.cols.len() {
            let cr = self.col_rect(ci);
            if !cr.contains(x, y) {
                continue;
            }
            let ct = self.col_tag_rect(ci);
            if ct.contains(x, y) {
                return if x < cr.x + SB_W { Hit::ColBox(ci) } else { Hit::ColTag(ci) };
            }
            for wi in 0..self.cols[ci].wins.len() {
                let r = self.win_rects(ci, wi);
                if !r.all.contains(x, y) {
                    continue;
                }
                if r.tag.contains(x, y) {
                    return if x < cr.x + SB_W { Hit::WinBox(ci, wi) } else { Hit::WinTag(ci, wi) };
                }
                if x < cr.x + SB_W {
                    return Hit::WinScroll(ci, wi);
                }
                return Hit::WinBody(ci, wi);
            }
            return Hit::Nothing;
        }
        Hit::Nothing
    }

    // ------------------------------------------------------------- lookups

    pub fn find_win(&self, id: usize) -> Option<(usize, usize)> {
        for (ci, c) in self.cols.iter().enumerate() {
            if let Some(wi) = c.wins.iter().position(|w| w.id == id) {
                return Some((ci, wi));
            }
        }
        None
    }

    pub(super) fn find_col(&self, id: usize) -> Option<usize> {
        self.cols.iter().position(|c| c.id == id)
    }

    pub(super) fn win(&self, id: usize) -> Option<&Window> {
        let (ci, wi) = self.find_win(id)?;
        Some(&self.cols[ci].wins[wi])
    }

    pub(super) fn win_mut(&mut self, id: usize) -> Option<&mut Window> {
        let (ci, wi) = self.find_win(id)?;
        Some(&mut self.cols[ci].wins[wi])
    }

    pub fn text(&self, id: TextId) -> Option<&Text> {
        match id {
            TextId::Row => Some(&self.row_tag),
            TextId::ColTag(c) => self.find_col(c).map(|ci| &self.cols[ci].tag),
            TextId::Tag(w) => self.win(w).map(|w| &w.tag),
            TextId::Body(w) => self.win(w).map(|w| &w.body),
        }
    }

    pub fn text_mut(&mut self, id: TextId) -> Option<&mut Text> {
        match id {
            TextId::Row => Some(&mut self.row_tag),
            TextId::ColTag(c) => {
                let ci = self.find_col(c)?;
                Some(&mut self.cols[ci].tag)
            }
            TextId::Tag(w) => self.win_mut(w).map(|w| &mut w.tag),
            TextId::Body(w) => self.win_mut(w).map(|w| &mut w.body),
        }
    }

    /// (window id, column index) that a text belongs to.
    pub(super) fn ctx(&self, id: TextId) -> (Option<usize>, Option<usize>) {
        match id {
            TextId::Row => (None, None),
            TextId::ColTag(c) => (None, self.find_col(c)),
            TextId::Tag(w) | TextId::Body(w) => {
                let f = self.find_win(w);
                (f.map(|_| w), f.map(|(ci, _)| ci))
            }
        }
    }

    pub(super) fn id_of_hit(&self, h: Hit) -> Option<TextId> {
        Some(match h {
            Hit::RowTag => TextId::Row,
            Hit::ColTag(ci) | Hit::ColBox(ci) => TextId::ColTag(self.cols[ci].id),
            Hit::WinTag(ci, wi) | Hit::WinBox(ci, wi) => TextId::Tag(self.cols[ci].wins[wi].id),
            Hit::WinBody(ci, wi) | Hit::WinScroll(ci, wi) => TextId::Body(self.cols[ci].wins[wi].id),
            Hit::Nothing => return None,
        })
    }

    pub fn geom(&self, id: TextId) -> Option<Geom> {
        Some(match id {
            TextId::Row => Geom {
                rect: Rect::new(SB_W + 2, TAG_PAD, self.w - SB_W - 4, self.row_h() - 2 * TAG_PAD - 1),
                origin: 0,
                cols: self.tag_cols(self.w),
                rows: self.tag_rows(&self.row_tag, self.w),
                tab: 4,
            },
            TextId::ColTag(c) => {
                let ci = self.find_col(c)?;
                let cr = self.col_rect(ci);
                let rows = self.tag_rows(&self.cols[ci].tag, cr.w);
                Geom {
                    rect: Rect::new(cr.x + SB_W + 2, cr.y + TAG_PAD, cr.w - SB_W - 4, rows as i32 * self.font.line_h),
                    origin: 0,
                    cols: self.tag_cols(cr.w),
                    rows,
                    tab: 4,
                }
            }
            TextId::Tag(w) => {
                let (ci, wi) = self.find_win(w)?;
                let r = self.win_rects(ci, wi);
                Geom {
                    rect: r.tag_text,
                    origin: 0,
                    cols: self.tag_cols(r.all.w),
                    rows: r.tag_rows,
                    tab: 4,
                }
            }
            TextId::Body(w) => {
                let (ci, wi) = self.find_win(w)?;
                let r = self.win_rects(ci, wi);
                let win = &self.cols[ci].wins[wi];
                Geom {
                    rect: r.body_text,
                    origin: win.origin,
                    cols: (r.body_text.w / self.font.adv).max(1) as usize,
                    rows: (r.body_text.h / self.font.line_h).max(0) as usize,
                    tab: win.tab,
                }
            }
        })
    }

    pub(super) fn hit_pos(&self, id: TextId, x: i32, y: i32) -> usize {
        let (Some(t), Some(g)) = (self.text(id), self.geom(id)) else {
            return 0;
        };
        frame::hit_xy(t, &g, &self.font, x, y)
    }

    pub(super) fn xy_of(&self, id: TextId, idx: usize) -> Option<(i32, i32)> {
        let (t, g) = (self.text(id)?, self.geom(id)?);
        frame::xy_of(t, &g, &self.font, idx)
    }

    // ------------------------------------------------------------ scrolling

    pub fn scroll_by(&mut self, win: usize, n: i64) {
        let Some(g) = self.geom(TextId::Body(win)) else {
            return;
        };
        let w = self.win_mut(win).unwrap();
        w.origin = frame::scroll(&w.body, g.origin, g.cols, g.tab, n);
    }

    /// Make sure `idx` in the body is visible, placing it `frac` of the way
    /// down the frame if it has to scroll.
    pub(super) fn show(&mut self, win: usize, idx: usize, frac: f32) {
        let Some(g) = self.geom(TextId::Body(win)) else {
            return;
        };
        let w = self.win_mut(win).unwrap();
        let idx = idx.min(w.body.len());
        let lines = frame::lines(&w.body, &g);
        if frame::locate(&w.body, &lines, idx, g.tab).is_some() && g.rows > 0 {
            return;
        }
        let above = ((g.rows as f32 * frac) as usize).min(g.rows.saturating_sub(1));
        w.origin = frame::origin_for(&w.body, idx, g.cols, g.tab, above);
    }

    pub(super) fn show_cursor(&mut self, id: TextId) {
        if let TextId::Body(w) = id {
            let q = self.text(id).map(|t| t.q1).unwrap_or(0);
            let above = if q < self.win(w).map(|w| w.origin).unwrap_or(0) { 0.25 } else { 0.75 };
            self.show(w, q, above);
        }
    }
}
