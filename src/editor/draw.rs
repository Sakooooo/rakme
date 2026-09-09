//! Painting the whole editor into a pixmap.

use super::*;

impl Editor {
    pub fn draw(&self, pm: &mut Pixmap) {
        gfx::fill(pm, Rect::new(0, 0, self.w, self.h), gfx::WHITE);

        // Row tag.
        let rh = self.row_h();
        gfx::fill(pm, Rect::new(0, 0, self.w, rh), gfx::PALE_BLUE);
        self.draw_text(pm, TextId::Row, gfx::PALE_BLUE, gfx::PALE_GREY_GREEN);
        gfx::fill(pm, Rect::new(0, rh - 1, self.w, 1), gfx::BLACK);

        for ci in 0..self.cols.len() {
            let col = &self.cols[ci];
            let cr = self.col_rect(ci);
            let ct = self.col_tag_rect(ci);
            gfx::fill(pm, ct, gfx::PALE_BLUE);
            let bx = Rect::new(cr.x + 1, cr.y + TAG_PAD + 1, SB_W - 3, SB_W - 3);
            gfx::fill(pm, bx, gfx::WHITE);
            gfx::outline(pm, bx, gfx::PURPLE_BLUE);
            self.draw_text(
                pm,
                TextId::ColTag(col.id),
                gfx::PALE_BLUE,
                gfx::PALE_GREY_GREEN,
            );
            gfx::fill(pm, Rect::new(cr.x, ct.bottom() - 1, cr.w, 1), gfx::BLACK);

            for wi in 0..col.wins.len() {
                let win = &col.wins[wi];
                let r = self.win_rects(ci, wi);
                gfx::fill(pm, r.tag, gfx::PALE_BLUE);
                if win.dirty() {
                    gfx::fill(pm, r.bx, gfx::DARK_BLUE);
                } else {
                    gfx::fill(pm, r.bx, gfx::WHITE);
                }
                gfx::outline(pm, r.bx, gfx::PURPLE_BLUE);
                self.draw_text(
                    pm,
                    TextId::Tag(win.id),
                    gfx::PALE_BLUE,
                    gfx::PALE_GREY_GREEN,
                );
                gfx::fill(pm, Rect::new(cr.x, r.tag.bottom() - 1, cr.w, 1), gfx::BLACK);

                if !r.body.is_empty() {
                    gfx::fill(pm, r.body, gfx::PALE_YELLOW);
                    self.draw_scrollbar(pm, win, &r);
                    self.draw_text(pm, TextId::Body(win.id), gfx::PALE_YELLOW, gfx::DARK_YELLOW);
                }
                if wi > 0 {
                    gfx::fill(pm, Rect::new(cr.x, r.all.y, cr.w, 1), gfx::BLACK);
                }
            }
            if ci > 0 {
                gfx::fill(pm, Rect::new(cr.x, cr.y, 1, cr.h), gfx::BLACK);
            }
        }
    }

    fn draw_scrollbar(&self, pm: &mut Pixmap, win: &Window, r: &WinRects) {
        gfx::fill(pm, r.sb, gfx::YELLOW_GREEN);
        let Some(g) = self.geom(TextId::Body(win.id)) else {
            return;
        };
        let total = win.body.len().max(1) as f32;
        let lines = frame::lines(&win.body, &g);
        let end = lines
            .last()
            .map(|l| l.end + usize::from(l.nl))
            .unwrap_or(win.origin)
            .min(win.body.len());
        let y0 = r.sb.y + (r.sb.h as f32 * win.origin as f32 / total) as i32;
        let y1 = r.sb.y + (r.sb.h as f32 * end as f32 / total) as i32;
        let y1 = y1.max(y0 + 2).min(r.sb.bottom());
        gfx::fill(
            pm,
            Rect::new(r.sb.x, y0, r.sb.w - 1, y1 - y0),
            gfx::PALE_YELLOW,
        );
    }

    fn draw_text(&self, pm: &mut Pixmap, id: TextId, bg: gfx::Color, sel: gfx::Color) {
        let (Some(t), Some(g)) = (self.text(id), self.geom(id)) else {
            return;
        };
        let st = Style {
            bg,
            sel,
            fg: gfx::BLACK,
            hl: self.sweep_of(id),
            cursor: true,
        };
        frame::draw(pm, &self.font, t, &g, &st);
    }
}
