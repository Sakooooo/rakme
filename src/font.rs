use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont, point};

pub struct Glyph {
    pub width: usize,
    pub height: usize,
    /// Offset of the bitmap from the cell's top-left corner.
    pub offset_x: i32,
    pub offset_y: i32,
    /// Coverage (alpha) per pixel, row-major, `width * height` bytes.
    pub coverage: Vec<u8>,
}

pub struct Font {
    font: FontArc,
    pub size: f32,
    /// Advance width of one cell (the font is assumed monospace).
    pub cell_width: i32,
    pub line_height: i32,
    pub ascent: i32,
    cache: RefCell<HashMap<char, Rc<Glyph>>>,
}

impl Font {
    pub fn load(size: f32) -> Result<Font, String> {
        let path = find_font()
            .ok_or_else(|| "no monospace font found; set RAKME_FONT to a .ttf file".to_string())?;
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let font = FontArc::try_from_vec(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Font::from_arc(font, size))
    }

    pub fn load_file(path: &str, size: f32) -> Result<Font, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let font = FontArc::try_from_vec(bytes).map_err(|e| format!("{path}: {e}"))?;
        Ok(Font::from_arc(font, size))
    }

    fn from_arc(font: FontArc, size: f32) -> Font {
        let scaled = font.as_scaled(PxScale::from(size));
        let ascent = scaled.ascent();
        let line_height = (ascent - scaled.descent() + scaled.line_gap()).ceil() as i32;
        let mut cell_width = scaled.h_advance(font.glyph_id('M')).round() as i32;
        if cell_width <= 0 {
            cell_width = (size * 0.6).round() as i32;
        }
        Font {
            font,
            size,
            cell_width: cell_width.max(1),
            line_height: line_height.max(1),
            ascent: ascent.round() as i32,
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn with_size(&self, size: f32) -> Font {
        Font::from_arc(self.font.clone(), size)
    }

    pub fn glyph(&self, ch: char) -> Rc<Glyph> {
        if let Some(glyph) = self.cache.borrow().get(&ch) {
            return glyph.clone();
        }
        let glyph = Rc::new(self.rasterize(ch));
        self.cache.borrow_mut().insert(ch, glyph.clone());
        glyph
    }

    fn rasterize(&self, ch: char) -> Glyph {
        let scaled = self.font.as_scaled(PxScale::from(self.size));
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = point(0.0, self.ascent as f32);
        let Some(outlined) = self.font.outline_glyph(glyph) else {
            return Glyph {
                width: 0,
                height: 0,
                offset_x: 0,
                offset_y: 0,
                coverage: vec![],
            };
        };
        let bounds = outlined.px_bounds();
        let width = bounds.width().ceil().max(0.0) as usize;
        let height = bounds.height().ceil().max(0.0) as usize;
        let mut coverage = vec![0u8; width * height];
        outlined.draw(|x, y, alpha| {
            let (x, y) = (x as usize, y as usize);
            if x < width && y < height {
                coverage[y * width + x] = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        Glyph {
            width,
            height,
            offset_x: bounds.min.x as i32,
            offset_y: bounds.min.y as i32,
            coverage,
        }
    }
}

fn find_font() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("RAKME_FONT") {
        let path = PathBuf::from(env_path);
        if path.is_file() {
            return Some(path);
        }
    }
    let candidates: &[&str] = &[
        "C:/Windows/Fonts/consola.ttf",
        "C:/Windows/Fonts/lucon.ttf",
        "C:/Windows/Fonts/cour.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/liberation-fonts/LiberationMono-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
        "/run/current-system/sw/share/X11/fonts/DejaVuSansMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "/System/Library/Fonts/Monaco.ttf",
        "/Library/Fonts/Courier New.ttf",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        for candidate in [
            ".nix-profile/share/fonts/truetype/DejaVuSansMono.ttf",
            ".local/share/fonts/DejaVuSansMono.ttf",
        ] {
            let path = PathBuf::from(&home).join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    // Last resort: ask fontconfig.
    if let Ok(output) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "monospace"])
        .output()
    {
        let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if path.is_file() {
            return Some(path);
        }
    }
    None
}
