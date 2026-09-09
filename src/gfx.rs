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
pub const BUTTON2_SWEEP: Color = [0xAA, 0x00, 0x00];
/// Button 3 sweep highlight.
pub const BUTTON3_SWEEP: Color = [0x00, 0x66, 0x00];

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

    pub fn intersect(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }
}

fn bounds(pixmap: &Pixmap) -> Rect {
    Rect::new(0, 0, pixmap.width() as i32, pixmap.height() as i32)
}

pub fn fill(pixmap: &mut Pixmap, rect: Rect, color: Color) {
    let rect = rect.intersect(bounds(pixmap));
    if rect.is_empty() {
        return;
    }
    let pixmap_width = pixmap.width() as usize;
    let data = pixmap.data_mut();
    for y in rect.y..rect.bottom() {
        let row_start = y as usize * pixmap_width;
        for x in rect.x..rect.right() {
            let idx = (row_start + x as usize) * 4;
            data[idx] = color[0];
            data[idx + 1] = color[1];
            data[idx + 2] = color[2];
            data[idx + 3] = 255;
        }
    }
}

/// Draw a 1px outline just inside `rect`.
pub fn outline(pixmap: &mut Pixmap, rect: Rect, color: Color) {
    fill(pixmap, Rect::new(rect.x, rect.y, rect.w, 1), color);
    fill(
        pixmap,
        Rect::new(rect.x, rect.bottom() - 1, rect.w, 1),
        color,
    );
    fill(pixmap, Rect::new(rect.x, rect.y, 1, rect.h), color);
    fill(
        pixmap,
        Rect::new(rect.right() - 1, rect.y, 1, rect.h),
        color,
    );
}

/// Blend one glyph whose cell's top-left corner is at (x, y).
pub fn glyph(pixmap: &mut Pixmap, font: &Font, x: i32, y: i32, ch: char, color: Color, clip: Rect) {
    let glyph = font.glyph(ch);
    if glyph.width == 0 || glyph.height == 0 {
        return;
    }
    let clip = clip.intersect(bounds(pixmap));
    if clip.is_empty() {
        return;
    }
    let pixmap_width = pixmap.width() as usize;
    let data = pixmap.data_mut();
    for glyph_y in 0..glyph.height {
        let pixel_y = y + glyph.offset_y + glyph_y as i32;
        if pixel_y < clip.y || pixel_y >= clip.bottom() {
            continue;
        }
        for glyph_x in 0..glyph.width {
            let pixel_x = x + glyph.offset_x + glyph_x as i32;
            if pixel_x < clip.x || pixel_x >= clip.right() {
                continue;
            }
            let alpha = glyph.coverage[glyph_y * glyph.width + glyph_x] as u32;
            if alpha == 0 {
                continue;
            }
            let idx = (pixel_y as usize * pixmap_width + pixel_x as usize) * 4;
            for channel in 0..3 {
                let dst = data[idx + channel] as u32;
                data[idx + channel] =
                    ((dst * (255 - alpha) + color[channel] as u32 * alpha) / 255) as u8;
            }
            data[idx + 3] = 255;
        }
    }
}
