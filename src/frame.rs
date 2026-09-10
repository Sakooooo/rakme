//! Laying out, hit-testing, scrolling and drawing a text frame.

use tiny_skia::Pixmap;

use crate::font::Font;
use crate::gfx::{self, Color, Rect};
use crate::text::Text;

/// One visual (wrapped) line of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    /// Exclusive end, not counting the newline.
    pub end: usize,
    /// Whether the line was ended by a newline character (as opposed to
    /// wrapping or the end of the text).
    pub has_newline: bool,
}

/// Where and how a text is laid out.
#[derive(Clone, Copy, Debug)]
pub struct Geom {
    /// The text area.
    pub rect: Rect,
    /// Character index of the first visible character.
    pub origin: usize,
    /// Visible width and height in character cells.
    pub cols: usize,
    pub rows: usize,
    pub tab_width: usize,
}

/// Width in cells of `ch` when it starts at column `col`.
pub fn char_width(ch: char, col: usize, tab_width: usize) -> usize {
    if ch == '\t' {
        tab_width - col % tab_width
    } else {
        1
    }
}

/// Wrap the text from `origin` into at most `max_rows` visual lines.
pub fn wrap(
    text: &Text,
    origin: usize,
    cols: usize,
    max_rows: usize,
    tab_width: usize,
    one_line: bool,
) -> Vec<Line> {
    let cols = cols.max(1);
    let tab_width = tab_width.max(1);
    let len = text.len();
    let mut lines = Vec::new();
    let mut pos = origin.min(len);
    while lines.len() < max_rows {
        let start = pos;
        let mut col = 0;
        let mut has_newline = false;
        while pos < len {
            let ch = text.at(pos).unwrap();
            if ch == '\n' {
                has_newline = true;
                pos += 1;
                break;
            }
            let width = char_width(ch, col, tab_width);
            if col + width > cols && col > 0 {
                break;
            }
            col += width;
            pos += 1;
        }
        lines.push(Line {
            start,
            end: if has_newline { pos - 1 } else { pos },
            has_newline,
        });
        if has_newline && one_line {
            break;
        }
        if pos >= len {
            if has_newline && lines.len() < max_rows {
                lines.push(Line {
                    start: pos,
                    end: pos,
                    has_newline: false,
                });
            }
            break;
        }
    }
    lines
}

/// Scroll `origin` by `delta` visual lines (negative is up).
pub fn scroll(text: &Text, origin: usize, cols: usize, tab_width: usize, delta: i64) -> usize {
    if delta > 0 {
        let lines = wrap(text, origin, cols, delta as usize + 1, tab_width, false);
        return lines
            .get(delta as usize)
            .or(lines.last())
            .map(|line| line.start)
            .unwrap_or(origin);
    }
    let mut remaining = (-delta) as usize;
    let mut pos = origin.min(text.len());
    while remaining > 0 && pos > 0 {
        let line_start = text.line_start(pos - 1);
        let lines = wrap(text, line_start, cols, usize::MAX, tab_width, true);
        let starts: Vec<usize> = lines
            .iter()
            .map(|line| line.start)
            .filter(|&start| start < pos)
            .collect();
        if starts.is_empty() {
            break;
        }
        let step = remaining.min(starts.len());
        pos = starts[starts.len() - step];
        remaining -= step;
    }
    pos
}

/// An origin that puts `pos` on screen with `rows_above` lines before it.
pub fn origin_for(
    text: &Text,
    pos: usize,
    cols: usize,
    tab_width: usize,
    rows_above: usize,
) -> usize {
    let pos = pos.min(text.len());
    let line_start = text.line_start(pos);
    let lines = wrap(text, line_start, cols, usize::MAX, tab_width, true);
    let visual_start = lines
        .iter()
        .rfind(|line| line.start <= pos)
        .map(|line| line.start)
        .unwrap_or(line_start);
    scroll(text, visual_start, cols, tab_width, -(rows_above as i64))
}

/// Character index for a point given as row and fractional column.
pub fn hit(text: &Text, lines: &[Line], row: i64, col_frac: f32, tab_width: usize) -> usize {
    if lines.is_empty() || row < 0 {
        return lines.first().map(|line| line.start).unwrap_or(0);
    }
    let Some(line) = lines.get(row as usize) else {
        return lines.last().map(|line| line.end).unwrap_or(text.len());
    };
    let mut col = 0;
    for pos in line.start..line.end {
        let width = char_width(text.at(pos).unwrap(), col, tab_width);
        if col_frac < col as f32 + width as f32 / 2.0 {
            return pos;
        }
        col += width;
    }
    line.end
}

/// Visual (row, col) of `pos` if it is within the laid-out lines.
pub fn locate(text: &Text, lines: &[Line], pos: usize, tab_width: usize) -> Option<(usize, usize)> {
    for (row, line) in lines.iter().enumerate() {
        let is_last = row + 1 == lines.len();
        if pos >= line.start
            && (pos < line.end || (pos == line.end && (line.has_newline || is_last)))
        {
            let mut col = 0;
            for i in line.start..pos {
                col += char_width(text.at(i).unwrap(), col, tab_width);
            }
            return Some((row, col));
        }
    }
    None
}

pub fn lines(text: &Text, geom: &Geom) -> Vec<Line> {
    wrap(
        text,
        geom.origin,
        geom.cols,
        geom.rows,
        geom.tab_width,
        false,
    )
}

/// Index at pixel position (x, y) relative to the frame.
pub fn hit_xy(text: &Text, geom: &Geom, font: &Font, x: i32, y: i32) -> usize {
    let lines = lines(text, geom);
    let row = ((y - geom.rect.y).max(0) / font.line_height) as i64;
    let col_frac = (x - geom.rect.x) as f32 / font.cell_width as f32;
    hit(text, &lines, row, col_frac, geom.tab_width)
}

/// Pixel position (top-left of the cell) of `pos`, if visible.
pub fn xy_of(text: &Text, geom: &Geom, font: &Font, pos: usize) -> Option<(i32, i32)> {
    let lines = lines(text, geom);
    locate(text, &lines, pos, geom.tab_width).map(|(row, col)| {
        (
            geom.rect.x + col as i32 * font.cell_width,
            geom.rect.y + row as i32 * font.line_height,
        )
    })
}

pub struct Style<'a> {
    pub bg: Color,
    pub sel_bg: Color,
    pub fg: Color,
    /// Extra highlight range (button 2/3 sweeps) drawn over the selection.
    pub highlight: Option<(usize, usize, Color)>,
    pub show_cursor: bool,
    pub syntax_highlight: &'a [(usize, usize, Color)],
}

pub fn draw(pixmap: &mut Pixmap, font: &Font, text: &Text, geom: &Geom, style: &Style) {
    let rect = geom.rect;
    gfx::fill(pixmap, rect, style.bg);
    if rect.is_empty() || geom.rows == 0 {
        return;
    }
    let lines = lines(text, geom);
    for (row, line) in lines.iter().enumerate() {
        let y = rect.y + row as i32 * font.line_height;
        let mut col = 0;
        for pos in line.start..=line.end {
            let ch = if pos < line.end {
                text.at(pos).unwrap()
            } else if line.has_newline {
                '\n'
            } else {
                break;
            };
            let x = rect.x + col as i32 * font.cell_width;
            let width = char_width(ch, col, geom.tab_width);
            let highlight = style
                .highlight
                .filter(|&(start, end, _)| pos >= start && pos < end)
                .map(|(_, _, color)| color);
            let (bg, fg) = match highlight {
                Some(color) => (Some(color), gfx::WHITE),
                None if pos >= text.sel_start && pos < text.sel_end => {
                    (Some(style.sel_bg), style.fg)
                }
                None => (None, style.fg),
            };
            if let Some(bg) = bg {
                let width_px = if ch == '\n' {
                    rect.right() - x
                } else {
                    width as i32 * font.cell_width
                };
                gfx::fill(
                    pixmap,
                    Rect::new(x, y, width_px, font.line_height).intersect(rect),
                    bg,
                );
            }
            if ch != '\t' && ch != '\n' {
                gfx::glyph(pixmap, font, x, y, ch, fg, rect);
            }
            col += width;
        }
    }
    if style.show_cursor
        && text.sel_start == text.sel_end
        && let Some((row, col)) = locate(text, &lines, text.sel_start, geom.tab_width)
    {
        let x = rect.x + col as i32 * font.cell_width;
        let y = rect.y + row as i32 * font.line_height;
        let height = font.line_height;
        gfx::fill(pixmap, Rect::new(x, y, 1, height).intersect(rect), style.fg);
        gfx::fill(pixmap, Rect::new(x - 1, y, 3, 1).intersect(rect), style.fg);
        gfx::fill(
            pixmap,
            Rect::new(x - 1, y + height - 1, 3, 1).intersect(rect),
            style.fg,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_basic() {
        let text = Text::from_str("abcdef\n\nxy");
        let lines = wrap(&text, 0, 4, 10, 4, false);
        assert_eq!(lines.len(), 4);
        assert_eq!(
            lines[0],
            Line {
                start: 0,
                end: 4,
                has_newline: false
            }
        );
        assert_eq!(
            lines[1],
            Line {
                start: 4,
                end: 6,
                has_newline: true
            }
        );
        assert_eq!(
            lines[2],
            Line {
                start: 7,
                end: 7,
                has_newline: true
            }
        );
        assert_eq!(
            lines[3],
            Line {
                start: 8,
                end: 10,
                has_newline: false
            }
        );
    }

    #[test]
    fn trailing_newline_gets_empty_line() {
        let text = Text::from_str("a\n");
        let lines = wrap(&text, 0, 80, 10, 4, false);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[1],
            Line {
                start: 2,
                end: 2,
                has_newline: false
            }
        );
        assert_eq!(locate(&text, &lines, 2, 4), Some((1, 0)));
    }

    #[test]
    fn scroll_up_and_down() {
        let text = Text::from_str("abcdef\nxy\nz");
        let origin = scroll(&text, 0, 4, 4, 1);
        assert_eq!(origin, 4);
        let origin = scroll(&text, origin, 4, 4, 1);
        assert_eq!(origin, 7);
        assert_eq!(scroll(&text, 7, 4, 4, -1), 4);
        assert_eq!(scroll(&text, 7, 4, 4, -2), 0);
        assert_eq!(scroll(&text, 10, 4, 4, -1), 7);
        assert_eq!(origin_for(&text, 10, 4, 4, 1), 7);
    }

    #[test]
    fn hit_and_tabs() {
        let text = Text::from_str("\tab");
        let lines = wrap(&text, 0, 80, 10, 4, false);
        assert_eq!(hit(&text, &lines, 0, 1.0, 4), 0);
        assert_eq!(hit(&text, &lines, 0, 4.4, 4), 1);
        assert_eq!(hit(&text, &lines, 0, 9.0, 4), 3);
        assert_eq!(locate(&text, &lines, 2, 4), Some((0, 5)));
    }
}
