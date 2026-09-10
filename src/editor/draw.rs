//! Painting the whole editor into a pixmap.

use super::*;

impl Editor {
    pub fn draw(&self, pixmap: &mut Pixmap) {
        gfx::fill(pixmap, Rect::new(0, 0, self.width, self.height), gfx::WHITE);

        // Row tag.
        let row_height = self.row_tag_height();
        gfx::fill(
            pixmap,
            Rect::new(0, 0, self.width, row_height),
            gfx::PALE_BLUE,
        );
        self.draw_text(pixmap, TextId::Row, gfx::PALE_BLUE, gfx::PALE_GREY_GREEN);
        gfx::fill(
            pixmap,
            Rect::new(0, row_height - 1, self.width, 1),
            gfx::BLACK,
        );

        for col_idx in 0..self.columns.len() {
            let col = &self.columns[col_idx];
            let col_bounds = self.col_rect(col_idx);
            let col_tag_bounds = self.col_tag_rect(col_idx);
            gfx::fill(pixmap, col_tag_bounds, gfx::PALE_BLUE);
            let col_box = Rect::new(
                col_bounds.x + 1,
                col_bounds.y + TAG_PAD + 1,
                SCROLLBAR_W - 3,
                SCROLLBAR_W - 3,
            );
            gfx::fill(pixmap, col_box, gfx::WHITE);
            gfx::outline(pixmap, col_box, gfx::PURPLE_BLUE);
            self.draw_text(
                pixmap,
                TextId::ColTag(col.id),
                gfx::PALE_BLUE,
                gfx::PALE_GREY_GREEN,
            );
            gfx::fill(
                pixmap,
                Rect::new(col_bounds.x, col_tag_bounds.bottom() - 1, col_bounds.w, 1),
                gfx::BLACK,
            );

            for win_idx in 0..col.windows.len() {
                let win = &col.windows[win_idx];
                let rects = self.win_rects(col_idx, win_idx);
                gfx::fill(pixmap, rects.tag, gfx::PALE_BLUE);
                if win.dirty() {
                    gfx::fill(pixmap, rects.dirty_box, gfx::DARK_BLUE);
                } else {
                    gfx::fill(pixmap, rects.dirty_box, gfx::WHITE);
                }
                gfx::outline(pixmap, rects.dirty_box, gfx::PURPLE_BLUE);
                self.draw_text(
                    pixmap,
                    TextId::Tag(win.id),
                    gfx::PALE_BLUE,
                    gfx::PALE_GREY_GREEN,
                );
                gfx::fill(
                    pixmap,
                    Rect::new(col_bounds.x, rects.tag.bottom() - 1, col_bounds.w, 1),
                    gfx::BLACK,
                );

                if !rects.body.is_empty() {
                    gfx::fill(pixmap, rects.body, gfx::PALE_YELLOW);
                    self.draw_scrollbar(pixmap, win, &rects);
                    self.draw_text(
                        pixmap,
                        TextId::Body(win.id),
                        gfx::PALE_YELLOW,
                        gfx::DARK_YELLOW,
                    );
                }
                if win_idx > 0 {
                    gfx::fill(
                        pixmap,
                        Rect::new(col_bounds.x, rects.whole.y, col_bounds.w, 1),
                        gfx::BLACK,
                    );
                }
            }
            if col_idx > 0 {
                gfx::fill(
                    pixmap,
                    Rect::new(col_bounds.x, col_bounds.y, 1, col_bounds.h),
                    gfx::BLACK,
                );
            }
        }
    }

    fn draw_scrollbar(&self, pixmap: &mut Pixmap, win: &Window, rects: &WinRects) {
        let Some(geom) = self.geom(TextId::Body(win.id)) else {
            return;
        };
        let total = win.body.len().max(1) as f32;
        let lines = frame::lines(&win.body, &geom);
        let visible_end = lines
            .last()
            .map(|line| line.end + usize::from(line.has_newline))
            .unwrap_or(win.origin)
            .min(win.body.len());
        let trough = rects.scrollbar;
        let thumb_top = trough.y + (trough.h as f32 * win.origin as f32 / total) as i32;
        let thumb_bottom = trough.y + (trough.h as f32 * visible_end as f32 / total) as i32;
        let thumb_bottom = thumb_bottom.max(thumb_top + 2).min(trough.bottom());

        gfx::fill(pixmap, rects.scrollbar, gfx::YELLOW_GREEN);
        gfx::fill(
            pixmap,
            Rect::new(trough.x, thumb_top, trough.w - 1, thumb_bottom - thumb_top),
            gfx::PALE_YELLOW,
        );
    }

    fn draw_text(&self, pixmap: &mut Pixmap, id: TextId, bg: gfx::Color, sel_bg: gfx::Color) {
        let (Some(text), Some(geom)) = (self.text(id), self.geom(id)) else {
            return;
        };
        let style = Style {
            bg,
            sel_bg,
            fg: gfx::BLACK,
            highlight: self.sweep_highlight(id),
            show_cursor: true,
        };
        frame::draw(pixmap, &self.font, text, &geom, &style);
    }
}
