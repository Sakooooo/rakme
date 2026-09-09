//! Laying out, hit-testing, scrolling and drawing a text frame.

use tiny_skia::Pixmap;

use crate::font::Font;
use crate::gfx::{self, Color, Rect};
use crate::text::Text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    /// Exclusive end, not counting the newline.
    pub end: usize,
    pub nl: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Geom {
    /// The text area.
    pub rect: Rect,
    pub origin: usize,
    pub cols: usize,
    pub rows: usize,
    pub tab: usize,
}

pub fn cw(c: char, col: usize, tab: usize) -> usize {
    if c == '\t' { tab - col % tab } else { 1 }
}

/// Wrap the text from `origin` into at most `max_rows` visual lines.
pub fn wrap(
    t: &Text,
    origin: usize,
    cols: usize,
    max_rows: usize,
    tab: usize,
    one_line: bool,
) -> Vec<Line> {
    let cols = cols.max(1);
    let tab = tab.max(1);
    let n = t.len();
    let mut lines = Vec::new();
    let mut i = origin.min(n);
    while lines.len() < max_rows {
        let start = i;
        let mut col = 0;
        let mut nl = false;
        while i < n {
            let c = t.at(i).unwrap();
            if c == '\n' {
                nl = true;
                i += 1;
                break;
            }
            let w = cw(c, col, tab);
            if col + w > cols && col > 0 {
                break;
            }
            col += w;
            i += 1;
        }
        lines.push(Line {
            start,
            end: if nl { i - 1 } else { i },
            nl,
        });
        if nl && one_line {
            break;
        }
        if i >= n {
            if nl && lines.len() < max_rows {
                lines.push(Line {
                    start: i,
                    end: i,
                    nl: false,
                });
            }
            break;
        }
    }
    lines
}

/// Scroll `origin` by `n` visual lines (negative is up).
pub fn scroll(t: &Text, origin: usize, cols: usize, tab: usize, n: i64) -> usize {
    if n > 0 {
        let lines = wrap(t, origin, cols, n as usize + 1, tab, false);
        return lines
            .get(n as usize)
            .or(lines.last())
            .map(|l| l.start)
            .unwrap_or(origin);
    }
    let mut need = (-n) as usize;
    let mut o = origin.min(t.len());
    while need > 0 && o > 0 {
        let ls = t.line_start(o - 1);
        let lines = wrap(t, ls, cols, usize::MAX, tab, true);
        let starts: Vec<usize> = lines.iter().map(|l| l.start).filter(|&s| s < o).collect();
        if starts.is_empty() {
            break;
        }
        let k = need.min(starts.len());
        o = starts[starts.len() - k];
        need -= k;
    }
    o
}

/// An origin that puts `idx` on screen with `rows_above` lines before it.
pub fn origin_for(t: &Text, idx: usize, cols: usize, tab: usize, rows_above: usize) -> usize {
    let idx = idx.min(t.len());
    let ls = t.line_start(idx);
    let lines = wrap(t, ls, cols, usize::MAX, tab, true);
    let vs = lines
        .iter()
        .rfind(|l| l.start <= idx)
        .map(|l| l.start)
        .unwrap_or(ls);
    scroll(t, vs, cols, tab, -(rows_above as i64))
}

/// Character index for a point given as row and fractional column.
pub fn hit(t: &Text, lines: &[Line], row: i64, colf: f32, tab: usize) -> usize {
    if lines.is_empty() || row < 0 {
        return lines.first().map(|l| l.start).unwrap_or(0);
    }
    let Some(l) = lines.get(row as usize) else {
        return lines.last().map(|l| l.end).unwrap_or(t.len());
    };
    let mut col = 0;
    for i in l.start..l.end {
        let w = cw(t.at(i).unwrap(), col, tab);
        if colf < col as f32 + w as f32 / 2.0 {
            return i;
        }
        col += w;
    }
    l.end
}

/// Visual (row, col) of `idx` if it is within the laid-out lines.
pub fn locate(t: &Text, lines: &[Line], idx: usize, tab: usize) -> Option<(usize, usize)> {
    for (r, l) in lines.iter().enumerate() {
        let last = r + 1 == lines.len();
        if idx >= l.start && (idx < l.end || (idx == l.end && (l.nl || last))) {
            let mut col = 0;
            for i in l.start..idx {
                col += cw(t.at(i).unwrap(), col, tab);
            }
            return Some((r, col));
        }
    }
    None
}

pub fn lines(t: &Text, g: &Geom) -> Vec<Line> {
    wrap(t, g.origin, g.cols, g.rows, g.tab, false)
}

/// Index at pixel position (x, y) relative to the frame.
pub fn hit_xy(t: &Text, g: &Geom, font: &Font, x: i32, y: i32) -> usize {
    let lines = lines(t, g);
    let row = ((y - g.rect.y).max(0) / font.line_h) as i64;
    let colf = (x - g.rect.x) as f32 / font.adv as f32;
    hit(t, &lines, row, colf, g.tab)
}

/// Pixel position (top-left of the cell) of `idx`, if visible.
pub fn xy_of(t: &Text, g: &Geom, font: &Font, idx: usize) -> Option<(i32, i32)> {
    let lines = lines(t, g);
    locate(t, &lines, idx, g.tab).map(|(r, c)| {
        (
            g.rect.x + c as i32 * font.adv,
            g.rect.y + r as i32 * font.line_h,
        )
    })
}

pub struct Style {
    pub bg: Color,
    pub sel: Color,
    pub fg: Color,
    /// Extra highlight range (button 2/3 sweeps) drawn over the selection.
    pub hl: Option<(usize, usize, Color)>,
    pub cursor: bool,
}

pub fn draw(pm: &mut Pixmap, font: &Font, t: &Text, g: &Geom, st: &Style) {
    let rect = g.rect;
    gfx::fill(pm, rect, st.bg);
    if rect.is_empty() || g.rows == 0 {
        return;
    }
    let lines = lines(t, g);
    for (r, l) in lines.iter().enumerate() {
        let y = rect.y + r as i32 * font.line_h;
        let mut col = 0;
        for i in l.start..=l.end {
            let c = if i < l.end {
                t.at(i).unwrap()
            } else if l.nl {
                '\n'
            } else {
                break;
            };
            let x = rect.x + col as i32 * font.adv;
            let w = cw(c, col, g.tab);
            let hl = st
                .hl
                .filter(|&(a, b, _)| i >= a && i < b)
                .map(|(_, _, c)| c);
            let (bg, fg) = match hl {
                Some(c) => (Some(c), gfx::WHITE),
                None if i >= t.q0 && i < t.q1 => (Some(st.sel), st.fg),
                None => (None, st.fg),
            };
            if let Some(bg) = bg {
                let wpx = if c == '\n' {
                    rect.right() - x
                } else {
                    w as i32 * font.adv
                };
                gfx::fill(pm, Rect::new(x, y, wpx, font.line_h).inter(rect), bg);
            }
            if c != '\t' && c != '\n' {
                gfx::glyph(pm, font, x, y, c, fg, rect);
            }
            col += w;
        }
    }
    if st.cursor
        && t.q0 == t.q1
        && let Some((r, c)) = locate(t, &lines, t.q0, g.tab)
    {
        let x = rect.x + c as i32 * font.adv;
        let y = rect.y + r as i32 * font.line_h;
        let h = font.line_h;
        gfx::fill(pm, Rect::new(x, y, 1, h).inter(rect), st.fg);
        gfx::fill(pm, Rect::new(x - 1, y, 3, 1).inter(rect), st.fg);
        gfx::fill(pm, Rect::new(x - 1, y + h - 1, 3, 1).inter(rect), st.fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_basic() {
        let t = Text::from_str("abcdef\n\nxy");
        let l = wrap(&t, 0, 4, 10, 4, false);
        assert_eq!(l.len(), 4);
        assert_eq!(
            l[0],
            Line {
                start: 0,
                end: 4,
                nl: false
            }
        );
        assert_eq!(
            l[1],
            Line {
                start: 4,
                end: 6,
                nl: true
            }
        );
        assert_eq!(
            l[2],
            Line {
                start: 7,
                end: 7,
                nl: true
            }
        );
        assert_eq!(
            l[3],
            Line {
                start: 8,
                end: 10,
                nl: false
            }
        );
    }

    #[test]
    fn trailing_newline_gets_empty_line() {
        let t = Text::from_str("a\n");
        let l = wrap(&t, 0, 80, 10, 4, false);
        assert_eq!(l.len(), 2);
        assert_eq!(
            l[1],
            Line {
                start: 2,
                end: 2,
                nl: false
            }
        );
        assert_eq!(locate(&t, &l, 2, 4), Some((1, 0)));
    }

    #[test]
    fn scroll_up_and_down() {
        let t = Text::from_str("abcdef\nxy\nz");
        let o = scroll(&t, 0, 4, 4, 1);
        assert_eq!(o, 4);
        let o = scroll(&t, o, 4, 4, 1);
        assert_eq!(o, 7);
        assert_eq!(scroll(&t, 7, 4, 4, -1), 4);
        assert_eq!(scroll(&t, 7, 4, 4, -2), 0);
        assert_eq!(scroll(&t, 10, 4, 4, -1), 7);
        assert_eq!(origin_for(&t, 10, 4, 4, 1), 7);
    }

    #[test]
    fn hit_and_tabs() {
        let t = Text::from_str("\tab");
        let l = wrap(&t, 0, 80, 10, 4, false);
        assert_eq!(hit(&t, &l, 0, 1.0, 4), 0);
        assert_eq!(hit(&t, &l, 0, 4.4, 4), 1);
        assert_eq!(hit(&t, &l, 0, 9.0, 4), 3);
        assert_eq!(locate(&t, &l, 2, 4), Some((0, 5)));
    }
}
