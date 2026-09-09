//! Geometry: where columns, windows, tags and bodies are on screen, hit
//! testing, lookups by id, and scrolling.

use super::*;

impl Editor {
    /// Pixel height of a tag with `rows` lines of text.
    fn tag_height(&self, rows: usize) -> i32 {
        rows as i32 * self.font.line_height + 2 * TAG_PAD + 1
    }

    pub(super) fn min_win_height(&self) -> i32 {
        self.tag_height(1)
    }

    pub(super) fn min_col_width(&self) -> i32 {
        SCROLLBAR_W * 4
    }

    /// Text columns available to a tag in something `width` pixels wide.
    fn tag_text_cols(&self, width: i32) -> usize {
        ((width - SCROLLBAR_W - 4) / self.font.cell_width).max(1) as usize
    }

    /// Rows a tag needs (wrapped, capped at 3).
    fn tag_rows(&self, tag: &Text, width: i32) -> usize {
        frame::wrap(tag, 0, self.tag_text_cols(width), 3, 4, false)
            .len()
            .max(1)
    }

    pub(super) fn row_tag_height(&self) -> i32 {
        self.tag_height(self.tag_rows(&self.row_tag, self.width))
    }

    pub fn col_rect(&self, col_idx: usize) -> Rect {
        let left = self.columns[col_idx].left;
        let right = self
            .columns
            .get(col_idx + 1)
            .map(|col| col.left)
            .unwrap_or(self.width);
        let top = self.row_tag_height();
        Rect::new(left, top, right - left, self.height - top)
    }

    pub(super) fn col_tag_rect(&self, col_idx: usize) -> Rect {
        let col_bounds = self.col_rect(col_idx);
        let rows = self.tag_rows(&self.columns[col_idx].tag, col_bounds.w);
        Rect::new(
            col_bounds.x,
            col_bounds.y,
            col_bounds.w,
            self.tag_height(rows),
        )
    }

    /// Y pixel where the first window of a column may start.
    pub(super) fn windows_top(&self, col_idx: usize) -> i32 {
        self.col_tag_rect(col_idx).bottom()
    }

    pub fn win_rects(&self, col_idx: usize, win_idx: usize) -> WinRects {
        let col_bounds = self.col_rect(col_idx);
        let col = &self.columns[col_idx];
        let win = &col.windows[win_idx];
        let top = win.top;
        let bottom = col
            .windows
            .get(win_idx + 1)
            .map(|next| next.top)
            .unwrap_or(col_bounds.bottom());
        let tag_rows = self.tag_rows(&win.tag, col_bounds.w);
        let tag_height = self.tag_height(tag_rows).min(bottom - top);
        let whole = Rect::new(col_bounds.x, top, col_bounds.w, bottom - top);
        let tag = Rect::new(col_bounds.x, top, col_bounds.w, tag_height);
        let dirty_box = Rect::new(
            col_bounds.x + 1,
            top + TAG_PAD + 1,
            SCROLLBAR_W - 3,
            SCROLLBAR_W - 3,
        );
        let tag_text_height =
            (tag_rows as i32 * self.font.line_height).min((tag_height - 2 * TAG_PAD - 1).max(0));
        let tag_text = Rect::new(
            col_bounds.x + SCROLLBAR_W + 2,
            top + TAG_PAD,
            col_bounds.w - SCROLLBAR_W - 4,
            tag_text_height,
        );
        let body = Rect::new(
            col_bounds.x,
            top + tag_height,
            col_bounds.w,
            bottom - top - tag_height,
        );
        let scrollbar = Rect::new(col_bounds.x, body.y, SCROLLBAR_W, body.h);
        let body_text = Rect::new(
            col_bounds.x + SCROLLBAR_W + TEXT_PAD,
            body.y,
            col_bounds.w - SCROLLBAR_W - TEXT_PAD - 2,
            body.h,
        );
        WinRects {
            whole,
            dirty_box,
            tag,
            tag_text,
            tag_rows,
            scrollbar,
            body,
            body_text,
        }
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        let (old_width, old_height) = (self.width.max(1), self.height.max(1));
        let top = self.row_tag_height();
        self.width = width;
        self.height = height;
        for col in &mut self.columns {
            col.left = (col.left as i64 * width as i64 / old_width as i64) as i32;
            for win in &mut col.windows {
                let offset = (win.top - top).max(0) as i64;
                win.top = top
                    + (offset * (height - top).max(1) as i64 / (old_height - top).max(1) as i64)
                        as i32;
            }
        }
        self.fix_layout();
    }

    /// Clamp column and window positions so everything fits and nothing
    /// is smaller than a tag line.
    pub(super) fn fix_layout(&mut self) {
        let min_width = self.min_col_width();
        let count = self.columns.len();
        for col_idx in 0..count {
            if col_idx == 0 {
                self.columns[col_idx].left = 0;
            } else {
                let prev_left = self.columns[col_idx - 1].left;
                self.columns[col_idx].left = self.columns[col_idx].left.max(prev_left + min_width);
            }
        }
        for col_idx in (1..count).rev() {
            let max_left = self.width - (count - col_idx) as i32 * min_width;
            self.columns[col_idx].left = self.columns[col_idx].left.min(max_left);
        }
        for col_idx in 0..count {
            self.fix_col(col_idx);
        }
    }

    /// Clamp window positions in one column the same way.
    pub(super) fn fix_col(&mut self, col_idx: usize) {
        let top = self.windows_top(col_idx);
        let bottom = self.col_rect(col_idx).bottom();
        let min_height = self.min_win_height();
        let count = self.columns[col_idx].windows.len();
        let windows = &mut self.columns[col_idx].windows;
        for win_idx in 0..count {
            if win_idx == 0 {
                windows[win_idx].top = top;
            } else {
                let prev_top = windows[win_idx - 1].top;
                windows[win_idx].top = windows[win_idx].top.max(prev_top + min_height);
            }
        }
        for win_idx in (1..count).rev() {
            let max_top = bottom - (count - win_idx) as i32 * min_height;
            windows[win_idx].top = windows[win_idx].top.min(max_top).max(top);
        }
    }

    pub fn hit(&self, x: i32, y: i32) -> Hit {
        if y < self.row_tag_height() {
            return Hit::RowTag;
        }
        for col_idx in 0..self.columns.len() {
            let col_bounds = self.col_rect(col_idx);
            if !col_bounds.contains(x, y) {
                continue;
            }
            let col_tag_bounds = self.col_tag_rect(col_idx);
            if col_tag_bounds.contains(x, y) {
                return if x < col_bounds.x + SCROLLBAR_W {
                    Hit::ColBox(col_idx)
                } else {
                    Hit::ColTag(col_idx)
                };
            }
            for win_idx in 0..self.columns[col_idx].windows.len() {
                let rects = self.win_rects(col_idx, win_idx);
                if !rects.whole.contains(x, y) {
                    continue;
                }
                if rects.tag.contains(x, y) {
                    return if x < col_bounds.x + SCROLLBAR_W {
                        Hit::WinBox(col_idx, win_idx)
                    } else {
                        Hit::WinTag(col_idx, win_idx)
                    };
                }
                if x < col_bounds.x + SCROLLBAR_W {
                    return Hit::WinScroll(col_idx, win_idx);
                }
                return Hit::WinBody(col_idx, win_idx);
            }
            return Hit::Nothing;
        }
        Hit::Nothing
    }

    // ------------------------------------------------------------- lookups

    /// (column index, window index) of the window with id `win_id`.
    pub fn find_win(&self, win_id: usize) -> Option<(usize, usize)> {
        for (col_idx, col) in self.columns.iter().enumerate() {
            if let Some(win_idx) = col.windows.iter().position(|win| win.id == win_id) {
                return Some((col_idx, win_idx));
            }
        }
        None
    }

    /// Column index of the column with id `col_id`.
    pub(super) fn find_col(&self, col_id: usize) -> Option<usize> {
        self.columns.iter().position(|col| col.id == col_id)
    }

    pub(super) fn win(&self, win_id: usize) -> Option<&Window> {
        let (col_idx, win_idx) = self.find_win(win_id)?;
        Some(&self.columns[col_idx].windows[win_idx])
    }

    pub(super) fn win_mut(&mut self, win_id: usize) -> Option<&mut Window> {
        let (col_idx, win_idx) = self.find_win(win_id)?;
        Some(&mut self.columns[col_idx].windows[win_idx])
    }

    pub fn text(&self, id: TextId) -> Option<&Text> {
        match id {
            TextId::Row => Some(&self.row_tag),
            TextId::ColTag(col_id) => self
                .find_col(col_id)
                .map(|col_idx| &self.columns[col_idx].tag),
            TextId::Tag(win_id) => self.win(win_id).map(|win| &win.tag),
            TextId::Body(win_id) => self.win(win_id).map(|win| &win.body),
        }
    }

    pub fn text_mut(&mut self, id: TextId) -> Option<&mut Text> {
        match id {
            TextId::Row => Some(&mut self.row_tag),
            TextId::ColTag(col_id) => {
                let col_idx = self.find_col(col_id)?;
                Some(&mut self.columns[col_idx].tag)
            }
            TextId::Tag(win_id) => self.win_mut(win_id).map(|win| &mut win.tag),
            TextId::Body(win_id) => self.win_mut(win_id).map(|win| &mut win.body),
        }
    }

    /// (window id, column index) that a text belongs to.
    pub(super) fn win_and_col(&self, id: TextId) -> (Option<usize>, Option<usize>) {
        match id {
            TextId::Row => (None, None),
            TextId::ColTag(col_id) => (None, self.find_col(col_id)),
            TextId::Tag(win_id) | TextId::Body(win_id) => {
                let found = self.find_win(win_id);
                (found.map(|_| win_id), found.map(|(col_idx, _)| col_idx))
            }
        }
    }

    pub(super) fn id_of_hit(&self, hit: Hit) -> Option<TextId> {
        Some(match hit {
            Hit::RowTag => TextId::Row,
            Hit::ColTag(col_idx) | Hit::ColBox(col_idx) => TextId::ColTag(self.columns[col_idx].id),
            Hit::WinTag(col_idx, win_idx) | Hit::WinBox(col_idx, win_idx) => {
                TextId::Tag(self.columns[col_idx].windows[win_idx].id)
            }
            Hit::WinBody(col_idx, win_idx) | Hit::WinScroll(col_idx, win_idx) => {
                TextId::Body(self.columns[col_idx].windows[win_idx].id)
            }
            Hit::Nothing => return None,
        })
    }

    pub fn geom(&self, id: TextId) -> Option<Geom> {
        Some(match id {
            TextId::Row => Geom {
                rect: Rect::new(
                    SCROLLBAR_W + 2,
                    TAG_PAD,
                    self.width - SCROLLBAR_W - 4,
                    self.row_tag_height() - 2 * TAG_PAD - 1,
                ),
                origin: 0,
                cols: self.tag_text_cols(self.width),
                rows: self.tag_rows(&self.row_tag, self.width),
                tab_width: 4,
            },
            TextId::ColTag(col_id) => {
                let col_idx = self.find_col(col_id)?;
                let col_bounds = self.col_rect(col_idx);
                let rows = self.tag_rows(&self.columns[col_idx].tag, col_bounds.w);
                Geom {
                    rect: Rect::new(
                        col_bounds.x + SCROLLBAR_W + 2,
                        col_bounds.y + TAG_PAD,
                        col_bounds.w - SCROLLBAR_W - 4,
                        rows as i32 * self.font.line_height,
                    ),
                    origin: 0,
                    cols: self.tag_text_cols(col_bounds.w),
                    rows,
                    tab_width: 4,
                }
            }
            TextId::Tag(win_id) => {
                let (col_idx, win_idx) = self.find_win(win_id)?;
                let rects = self.win_rects(col_idx, win_idx);
                Geom {
                    rect: rects.tag_text,
                    origin: 0,
                    cols: self.tag_text_cols(rects.whole.w),
                    rows: rects.tag_rows,
                    tab_width: 4,
                }
            }
            TextId::Body(win_id) => {
                let (col_idx, win_idx) = self.find_win(win_id)?;
                let rects = self.win_rects(col_idx, win_idx);
                let win = &self.columns[col_idx].windows[win_idx];
                Geom {
                    rect: rects.body_text,
                    origin: win.origin,
                    cols: (rects.body_text.w / self.font.cell_width).max(1) as usize,
                    rows: (rects.body_text.h / self.font.line_height).max(0) as usize,
                    tab_width: win.tab_width,
                }
            }
        })
    }

    /// Character index under pixel (x, y) in text `id`.
    pub(super) fn hit_pos(&self, id: TextId, x: i32, y: i32) -> usize {
        let (Some(text), Some(geom)) = (self.text(id), self.geom(id)) else {
            return 0;
        };
        frame::hit_xy(text, &geom, &self.font, x, y)
    }

    /// Pixel position of character `pos` in text `id`, if visible.
    pub(super) fn xy_of(&self, id: TextId, pos: usize) -> Option<(i32, i32)> {
        let (text, geom) = (self.text(id)?, self.geom(id)?);
        frame::xy_of(text, &geom, &self.font, pos)
    }

    // ------------------------------------------------------------ scrolling

    /// Scroll the body of window `win_id` by `delta` visual lines.
    pub fn scroll_by(&mut self, win_id: usize, delta: i64) {
        let Some(geom) = self.geom(TextId::Body(win_id)) else {
            return;
        };
        let win = self.win_mut(win_id).unwrap();
        win.origin = frame::scroll(&win.body, geom.origin, geom.cols, geom.tab_width, delta);
    }

    /// Make sure `pos` in the body is visible, placing it `frac` of the way
    /// down the frame if it has to scroll.
    pub(super) fn show(&mut self, win_id: usize, pos: usize, frac: f32) {
        let Some(geom) = self.geom(TextId::Body(win_id)) else {
            return;
        };
        let win = self.win_mut(win_id).unwrap();
        let pos = pos.min(win.body.len());
        let lines = frame::lines(&win.body, &geom);
        if frame::locate(&win.body, &lines, pos, geom.tab_width).is_some() && geom.rows > 0 {
            return;
        }
        let rows_above = ((geom.rows as f32 * frac) as usize).min(geom.rows.saturating_sub(1));
        win.origin = frame::origin_for(&win.body, pos, geom.cols, geom.tab_width, rows_above);
    }

    /// Scroll so the end of the selection in `id` is visible (bodies only).
    pub(super) fn show_cursor(&mut self, id: TextId) {
        if let TextId::Body(win_id) = id {
            let pos = self.text(id).map(|text| text.sel_end).unwrap_or(0);
            let origin = self.win(win_id).map(|win| win.origin).unwrap_or(0);
            // Scrolling up puts the cursor near the top; down, near the bottom.
            let frac = if pos < origin { 0.25 } else { 0.75 };
            self.show(win_id, pos, frac);
        }
    }
}
