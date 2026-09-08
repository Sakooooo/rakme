//! The editor: a row of columns of windows, with acme's mouse language.
//!
//! The `Editor` type is defined here; its behaviour is split across the
//! submodules by concern:
//!
//! - `layout`: geometry, hit testing, lookups by id, scrolling
//! - `windows`: creating, sizing, moving and closing columns and windows
//! - `files`: reading, writing and the `+Errors` window
//! - `edit`: the snarf buffer and the keyboard
//! - `mouse`: selecting, sweeping and chording
//! - `commands`: built-in commands and shell execution
//! - `look`: button 3 (files, addresses, search)
//! - `draw`: painting everything into a pixmap

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::exec::{self, Kind};
use crate::font::Font;
use crate::frame::{self, Geom, Style};
use crate::gfx::{self, Rect};
use crate::text::{Text, is_filec, is_word};

mod commands;
mod draw;
mod edit;
mod files;
mod layout;
mod look;
mod mouse;
#[cfg(test)]
mod tests;
mod windows;

/// Width of the scrollbar / box column on the left of every window.
pub const SB_W: i32 = 12;
const TEXT_PAD: i32 = 4;
const TAG_PAD: i32 = 2;
const DCLICK: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextId {
    Row,
    ColTag(usize),
    Tag(usize),
    Body(usize),
}

pub struct Window {
    pub id: usize,
    pub name: String,
    pub tag: Text,
    pub body: Text,
    pub origin: usize,
    pub tab: usize,
    pub y0: i32,
    pub is_dir: bool,
    /// Body sequence number at which a Del/Get on a dirty window was refused.
    warned: Option<u64>,
}

impl Window {
    fn new(id: usize, name: String) -> Window {
        let is_dir = name.ends_with('/');
        Window {
            id,
            name,
            tag: Text::new(),
            body: Text::new(),
            origin: 0,
            tab: 4,
            y0: 0,
            is_dir,
            warned: None,
        }
    }

    pub fn dir(&self) -> PathBuf {
        if self.name.is_empty() {
            return cwd();
        }
        if self.is_dir {
            return PathBuf::from(&self.name);
        }
        Path::new(&self.name)
            .parent()
            .map(|p| p.to_path_buf())
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(cwd)
    }

    fn dirty(&self) -> bool {
        !self.is_dir && self.body.is_dirty()
    }
}

pub struct Column {
    pub id: usize,
    pub x0: i32,
    pub tag: Text,
    pub wins: Vec<Window>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Nothing,
    RowTag,
    ColBox(usize),
    ColTag(usize),
    WinBox(usize, usize),
    WinTag(usize, usize),
    WinScroll(usize, usize),
    WinBody(usize, usize),
}

pub struct WinRects {
    pub all: Rect,
    pub bx: Rect,
    pub tag: Rect,
    pub tag_text: Rect,
    pub tag_rows: usize,
    pub sb: Rect,
    pub body: Rect,
    pub body_text: Rect,
}

#[derive(Clone, Copy, Debug, Default)]
enum Action {
    #[default]
    None,
    Select {
        id: TextId,
    },
    Sweep {
        id: TextId,
        anchor: usize,
        btn: u8,
        q0: usize,
        q1: usize,
    },
    WinBox {
        win: usize,
        sx: i32,
        sy: i32,
    },
    ColBox {
        col: usize,
        sx: i32,
    },
}

#[derive(Default)]
struct Mouse {
    buttons: u8,
    x: i32,
    y: i32,
    action: Action,
    last_click: Option<(Instant, TextId, usize)>,
    chorded: bool,
    arg: Option<String>,
}

pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Escape,
    Ctrl(char),
}

pub struct Editor {
    pub font: Font,
    pub w: i32,
    pub h: i32,
    pub row_tag: Text,
    pub cols: Vec<Column>,
    next_id: usize,
    snarf: String,
    clipboard: Option<arboard::Clipboard>,
    pub focus: TextId,
    mouse: Mouse,
    typing: Option<(TextId, usize)>,
    /// Pointer warp requested by a search.
    pub warp: Option<(i32, i32)>,
    pub quit: bool,
    exit_warned: bool,
    pub pending: Vec<exec::Request>,
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn clean_path(p: &Path) -> String {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    let mut s = out.to_string_lossy().replace('\\', "/");
    if out.is_dir() && !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn dir_string(p: &Path) -> String {
    let mut s = clean_path(p);
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    s
}

fn split_cmd(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim()),
        None => (s, ""),
    }
}

impl Editor {
    pub fn new(font: Font, w: i32, h: i32, files: &[String]) -> Editor {
        let mut ed = Editor {
            font,
            w,
            h,
            row_tag: Text::from_str("Newcol Putall Exit "),
            cols: Vec::new(),
            next_id: 1,
            snarf: String::new(),
            clipboard: arboard::Clipboard::new().ok(),
            focus: TextId::Row,
            mouse: Mouse::default(),
            typing: None,
            warp: None,
            quit: false,
            exit_warned: false,
            pending: Vec::new(),
        };
        ed.new_col(0);
        ed.new_col(0);
        let ci = ed.cols.len() - 1;
        if files.is_empty() {
            let id = ed.open_file(&cwd(), ci);
            ed.focus = TextId::Body(id);
        } else {
            for f in files {
                let id = ed.open_file(Path::new(f), ci);
                ed.focus = TextId::Body(id);
            }
        }
        ed.fix_layout();
        ed.update_tags();
        ed
    }

    fn alloc_id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id - 1
    }
}
