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
//!
//! Naming: `win_id` / `col_id` are stable ids that survive re-layout;
//! `win_idx` / `col_idx` are positions in the current `windows` / `columns`
//! vectors.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::exec::{self, Kind};
use crate::font::Font;
use crate::frame::{self, Geom, Style};
use crate::gfx::{self, Rect};
use crate::text::{Text, is_filename_char, is_word_char};

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
pub const SCROLLBAR_W: i32 = 12;
const TEXT_PAD: i32 = 4;
const TAG_PAD: i32 = 2;
const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(400);

/// Identifies one of the editable texts on screen.
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
    /// Character index of the first visible body character.
    pub origin: usize,
    pub tab_width: usize,
    /// Y pixel of the window's top edge.
    pub top: i32,
    pub is_dir: bool,
    /// Body revision at which a Del/Get on a dirty window was refused, so
    /// the second attempt goes through.
    warned_at_revision: Option<u64>,
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
            tab_width: 4,
            top: 0,
            is_dir,
            warned_at_revision: None,
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
            .map(|parent| parent.to_path_buf())
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(cwd)
    }

    // todo: probably make this a field in the struct instead
    fn is_scratch(&self) -> bool {
        self.name.ends_with("+Errors")
    }

    fn dirty(&self) -> bool {
        !self.is_dir && self.body.is_dirty()
    }
}

pub struct Column {
    pub id: usize,
    /// X pixel of the column's left edge.
    pub left: i32,
    pub tag: Text,
    pub windows: Vec<Window>,
}

/// What is under a point on screen.
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

/// The rectangles making up one window on screen.
pub struct WinRects {
    pub whole: Rect,
    /// The small square at the top-left that shows the dirty state.
    pub dirty_box: Rect,
    pub tag: Rect,
    pub tag_text: Rect,
    pub tag_rows: usize,
    pub scrollbar: Rect,
    pub body: Rect,
    pub body_text: Rect,
}

/// What a held mouse button is currently doing.
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
        button: u8,
        start: usize,
        end: usize,
    },
    WinBox {
        win_id: usize,
        press_x: i32,
        press_y: i32,
    },
    ColBox {
        col_id: usize,
        press_x: i32,
    },
}

#[derive(Default)]
struct Mouse {
    /// Bitmask of held buttons: bit 0 is button 1, and so on.
    held_buttons: u8,
    x: i32,
    y: i32,
    action: Action,
    last_click: Option<(Instant, TextId, usize)>,
    /// A chord (cut/paste) happened during this press, so the release
    /// should not execute or look.
    chorded: bool,
    /// Argument captured by a 2-1 chord, appended to the executed command.
    chord_arg: Option<String>,
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
    pub width: i32,
    pub height: i32,
    pub row_tag: Text,
    pub columns: Vec<Column>,
    next_id: usize,
    /// acme's name for the cut/paste buffer.
    snarf: String,
    clipboard: Option<arboard::Clipboard>,
    pub focus: TextId,
    mouse: Mouse,
    /// Where the current run of typing began, so Escape can select it.
    typing_start: Option<(TextId, usize)>,
    /// Pointer warp requested by a search.
    pub warp: Option<(i32, i32)>,
    pub quit: bool,
    exit_warned: bool,
    /// Shell commands waiting for `App` to spawn them.
    pub pending_commands: Vec<exec::Request>,
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Absolute, normalised, forward-slashed path; directories get a trailing `/`.
fn clean_path(path: &Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut cleaned = PathBuf::new();
    for component in abs.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cleaned.pop();
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    let mut result = cleaned.to_string_lossy().replace('\\', "/");
    if cleaned.is_dir() && !result.ends_with('/') {
        result.push('/');
    }
    result
}

/// Like `clean_path` but without the trailing `/`.
fn dir_string(path: &Path) -> String {
    let mut result = clean_path(path);
    while result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}

/// Split a command line into its name and the rest.
fn split_command(line: &str) -> (&str, &str) {
    let line = line.trim();
    match line.find(char::is_whitespace) {
        Some(space) => (&line[..space], line[space..].trim()),
        None => (line, ""),
    }
}

impl Editor {
    pub fn new(font: Font, width: i32, height: i32, files: &[String]) -> Editor {
        let mut editor = Editor {
            font,
            width,
            height,
            row_tag: Text::from_str("Newcol Putall Exit "),
            columns: Vec::new(),
            next_id: 1,
            snarf: String::new(),
            clipboard: arboard::Clipboard::new().ok(),
            focus: TextId::Row,
            mouse: Mouse::default(),
            typing_start: None,
            warp: None,
            quit: false,
            exit_warned: false,
            pending_commands: Vec::new(),
        };
        editor.new_col(0);
        editor.new_col(0);
        let col_idx = editor.columns.len() - 1;
        if files.is_empty() {
            let win_id = editor.open_file(&cwd(), col_idx);
            editor.focus = TextId::Body(win_id);
        } else {
            for file in files {
                let win_id = editor.open_file(Path::new(file), col_idx);
                editor.focus = TextId::Body(win_id);
            }
        }
        editor.fix_layout();
        editor.update_tags();
        editor
    }

    fn alloc_id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id - 1
    }
}
