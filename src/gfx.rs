use tiny_skia::Pixmap;

use crate::font::Font;

pub type Color = [u8; 3];

pub const BLACK: Color = [0x00, 0x00, 0x00];
pub const WHITE: Color = [0xFF, 0xFF, 0xFF];
/// Body background.
pub const PALE_YELLOW: Color = [0xFF, 0xFF, 0xEA];
/// Body selection.
pub const DARK_YELLOW: Color = [0xEE, 0xEE, 0x9E];
/// Body scrollbar trough.
pub const YELLOW_GREEN: Color = [0x99, 0x99, 0x4C];
/// Tag background.
pub const PALE_BLUE: Color = [0xEA, 0xFF, 0xFF];
/// Tag selection.
pub const PALE_GREY_GREEN: Color = [0x9E, 0xEE, 0xEE];
/// Tag border / box outline.
pub const PURPLE_BLUE: Color = [0x88, 0x88, 0xCC];
/// Dirty box.
pub const DARK_BLUE: Color = [0x00, 0x00, 0x99];
/// Button 2 sweep highlight.
pub const BUT2: Color = [0xAA, 0x00, 0x00];
/// Button 3 sweep highlight.
pub const BUT3: Color = [0x00, 0x66, 0x00];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    pub fn inter(&self, o: Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        Rect::new(x, y, r - x, b - y)
    }
}

fn bounds(pm: &Pixmap) -> Rect {
    Rect::new(0, 0, pm.width() as i32, pm.height() as i32)
}

pub fn fill(pm: &mut Pixmap, r: Rect, c: Color) {
    let r = r.inter(bounds(pm));
    if r.is_empty() {
        return;
    }
    let pw = pm.width() as usize;
    let data = pm.data_mut();
    for y in r.y..r.bottom() {
        let row = y as usize * pw;
        for x in r.x..r.right() {
            let i = (row + x as usize) * 4;
            data[i] = c[0];
            data[i + 1] = c[1];
            data[i + 2] = c[2];
            data[i + 3] = 255;
        }
    }
}

/// Draw a 1px outline just inside `r`.
pub fn outline(pm: &mut Pixmap, r: Rect, c: Color) {
    fill(pm, Rect::new(r.x, r.y, r.w, 1), c);
    fill(pm, Rect::new(r.x, r.bottom() - 1, r.w, 1), c);
    fill(pm, Rect::new(r.x, r.y, 1, r.h), c);
    fill(pm, Rect::new(r.right() - 1, r.y, 1, r.h), c);
}

/// Blend one glyph whose cell's top-left corner is at (x, y).
pub fn glyph(pm: &mut Pixmap, font: &Font, x: i32, y: i32, ch: char, color: Color, clip: Rect) {
    let g = font.glyph(ch);
    if g.w == 0 || g.h == 0 {
        return;
    }
    let clip = clip.inter(bounds(pm));
    if clip.is_empty() {
        return;
    }
    let pw = pm.width() as usize;
    let data = pm.data_mut();
    for gy in 0..g.h {
        let py = y + g.y + gy as i32;
        if py < clip.y || py >= clip.bottom() {
            continue;
        }
        for gx in 0..g.w {
            let px = x + g.x + gx as i32;
            if px < clip.x || px >= clip.right() {
                continue;
            }
            let a = g.cov[gy * g.w + gx] as u32;
            if a == 0 {
                continue;
            }
            let i = (py as usize * pw + px as usize) * 4;
            for k in 0..3 {
                let d = data[i + k] as u32;
                data[i + k] = ((d * (255 - a) + color[k] as u32 * a) / 255) as u8;
            }
            data[i + 3] = 255;
        }
    }
}
