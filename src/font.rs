use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont, point};

pub struct Glyph {
    pub w: usize,
    pub h: usize,
    /// Offset of the bitmap from the cell's top-left corner.
    pub x: i32,
    pub y: i32,
    pub cov: Vec<u8>,
}

pub struct Font {
    font: FontArc,
    pub size: f32,
    /// Advance width of one cell (the font is assumed monospace).
    pub adv: i32,
    pub line_h: i32,
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
        let s = font.as_scaled(PxScale::from(size));
        let ascent = s.ascent();
        let line_h = (ascent - s.descent() + s.line_gap()).ceil() as i32;
        let mut adv = s.h_advance(font.glyph_id('M')).round() as i32;
        if adv <= 0 {
            adv = (size * 0.6).round() as i32;
        }
        Font {
            font,
            size,
            adv: adv.max(1),
            line_h: line_h.max(1),
            ascent: ascent.round() as i32,
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn with_size(&self, size: f32) -> Font {
        Font::from_arc(self.font.clone(), size)
    }

    pub fn glyph(&self, c: char) -> Rc<Glyph> {
        if let Some(g) = self.cache.borrow().get(&c) {
            return g.clone();
        }
        let g = Rc::new(self.rasterize(c));
        self.cache.borrow_mut().insert(c, g.clone());
        g
    }

    fn rasterize(&self, c: char) -> Glyph {
        let s = self.font.as_scaled(PxScale::from(self.size));
        let mut g = s.scaled_glyph(c);
        g.position = point(0.0, self.ascent as f32);
        let Some(og) = self.font.outline_glyph(g) else {
            return Glyph {
                w: 0,
                h: 0,
                x: 0,
                y: 0,
                cov: vec![],
            };
        };
        let b = og.px_bounds();
        let w = b.width().ceil().max(0.0) as usize;
        let h = b.height().ceil().max(0.0) as usize;
        let mut cov = vec![0u8; w * h];
        og.draw(|x, y, a| {
            let (x, y) = (x as usize, y as usize);
            if x < w && y < h {
                cov[y * w + x] = (a.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        Glyph {
            w,
            h,
            x: b.min.x as i32,
            y: b.min.y as i32,
            cov,
        }
    }
}

fn find_font() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RAKME_FONT") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
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
    for c in candidates {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        for c in [
            ".nix-profile/share/fonts/truetype/DejaVuSansMono.ttf",
            ".local/share/fonts/DejaVuSansMono.ttf",
        ] {
            let p = PathBuf::from(&home).join(c);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // Last resort: ask fontconfig.
    if let Ok(out) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "monospace"])
        .output()
    {
        let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        if p.is_file() {
            return Some(p);
        }
    }
    None
}
