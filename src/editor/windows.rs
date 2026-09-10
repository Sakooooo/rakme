//! Creating, sizing, moving and closing columns and windows, and keeping
//! window tags up to date.

use super::*;

impl Editor {
    /// Create a column. The first column takes `left`; later ones split the
    /// widest existing column. Returns the new column's index.
    pub(super) fn new_col(&mut self, left: i32) -> usize {
        let id = self.alloc_id();
        let mut col = Column {
            id,
            left,
            tag: Text::from_str("New Cut Paste Snarf Sort Zerox Delcol "),
            windows: Vec::new(),
        };
        if self.columns.is_empty() {
            self.columns.push(col);
            return 0;
        }
        let mut widest_idx = 0;
        let mut widest_width = 0;
        for col_idx in 0..self.columns.len() {
            let width = self.col_rect(col_idx).w;
            if width > widest_width {
                widest_width = width;
                widest_idx = col_idx;
            }
        }
        let widest_bounds = self.col_rect(widest_idx);
        col.left = widest_bounds.x + widest_bounds.w / 2;
        self.columns.insert(widest_idx + 1, col);
        self.fix_layout();
        widest_idx + 1
    }

    /// Create an empty window in column `col_idx`, splitting its tallest
    /// window. Returns the new window's id.
    pub(super) fn new_win(&mut self, col_idx: usize, name: String) -> usize {
        if self.columns.is_empty() {
            self.new_col(0);
        }
        let col_idx = col_idx.min(self.columns.len() - 1);
        let id = self.alloc_id();
        let file_type = match &name.split(".").last() {
            Some("rs") => FileType::Rust,
            _ => FileType::Generic,
        };
        let mut win = Window::new(id, name);
        let top = self.windows_top(col_idx);
        let bottom = self.col_rect(col_idx).bottom();
        let col = &mut self.columns[col_idx];
        win.file_type = file_type;
        if col.windows.is_empty() {
            win.top = top;
            col.windows.push(win);
        } else {
            let mut tallest_idx = 0;
            let mut tallest_height = -1;
            for win_idx in 0..col.windows.len() {
                let next_top = col
                    .windows
                    .get(win_idx + 1)
                    .map(|next| next.top)
                    .unwrap_or(bottom);
                let height = next_top - col.windows[win_idx].top;
                if height > tallest_height {
                    tallest_height = height;
                    tallest_idx = win_idx;
                }
            }
            let next_top = col
                .windows
                .get(tallest_idx + 1)
                .map(|next| next.top)
                .unwrap_or(bottom);
            let tallest_top = col.windows[tallest_idx].top;
            win.top = tallest_top + (next_top - tallest_top) / 2;
            col.windows.insert(tallest_idx + 1, win);
        }
        self.fix_col(col_idx);
        self.update_tags();
        id
    }

    pub(super) fn close_win(&mut self, win_id: usize) {
        let Some((col_idx, win_idx)) = self.find_win(win_id) else {
            return;
        };
        self.columns[col_idx].windows.remove(win_idx);
        self.fix_col(col_idx);
        if let TextId::Body(focused_id) | TextId::Tag(focused_id) = self.focus
            && focused_id == win_id
        {
            self.focus = TextId::ColTag(self.columns[col_idx].id);
        }
    }

    fn set_win_height(&mut self, col_idx: usize, win_idx: usize, new_height: i32) {
        let top = self.windows_top(col_idx);
        let bottom = self.col_rect(col_idx).bottom();
        let min_height = self.min_win_height();
        let count = self.columns[col_idx].windows.len();
        let windows = &mut self.columns[col_idx].windows;
        let windows_below = (count - win_idx - 1) as i32;
        let new_height = new_height
            .min(bottom - top - (count as i32 - 1) * min_height)
            .max(min_height);
        let mut win_top = windows[win_idx].top;
        if win_top + new_height + windows_below * min_height > bottom {
            win_top = (bottom - windows_below * min_height - new_height)
                .max(top + win_idx as i32 * min_height);
        }
        windows[win_idx].top = win_top;
        // Push the windows above up and the ones below down to make room.
        for other in (0..win_idx).rev() {
            let max_top = windows[other + 1].top - min_height;
            windows[other].top = windows[other].top.min(max_top);
        }
        for other in win_idx + 1..count {
            let min_gap = if other == win_idx + 1 {
                new_height
            } else {
                min_height
            };
            windows[other].top = windows[other].top.max(windows[other - 1].top + min_gap);
        }
        self.fix_col(col_idx);
    }

    /// Button 1 grows a window somewhat; buttons 2 and 3 fill the column.
    pub(super) fn grow_win(&mut self, win_id: usize, button: u8) {
        let Some((col_idx, win_idx)) = self.find_win(win_id) else {
            return;
        };
        let rects = self.win_rects(col_idx, win_idx);
        let col_height = self.col_rect(col_idx).bottom() - self.windows_top(col_idx);
        let new_height = match button {
            1 => (rects.whole.h + col_height / 3).max(rects.whole.h * 3 / 2),
            _ => col_height,
        };
        self.set_win_height(col_idx, win_idx, new_height);
    }

    /// Move window `win_id` so its top is at (x, y), possibly into another
    /// column.
    pub(super) fn move_win(&mut self, win_id: usize, x: i32, y: i32) {
        let Some((src_col, src_win)) = self.find_win(win_id) else {
            return;
        };
        let Some(dst_col) =
            (0..self.columns.len()).find(|&col_idx| self.col_rect(col_idx).contains(x, y))
        else {
            return;
        };
        let mut win = self.columns[src_col].windows.remove(src_win);
        self.fix_col(src_col);
        let insert_at = self.columns[dst_col]
            .windows
            .iter()
            .filter(|other| other.top < y)
            .count();
        win.top = y;
        if insert_at == 0 {
            // Dropped above every window: the old first window moves down
            // to the drop point and the moved one takes the top.
            if let Some(first) = self.columns[dst_col].windows.first_mut() {
                first.top = y;
            }
            win.top = self.windows_top(dst_col);
        }
        self.columns[dst_col].windows.insert(insert_at, win);
        self.fix_col(dst_col);
    }

    /// Button 1 widens a column into its wider neighbour; buttons 2 and 3
    /// give it nearly the whole screen.
    pub(super) fn grow_col(&mut self, col_idx: usize, button: u8) {
        let count = self.columns.len();
        if count < 2 {
            return;
        }
        let min_width = self.min_col_width();
        if button == 1 {
            let col_bounds = self.col_rect(col_idx);
            let left_width = if col_idx > 0 {
                self.col_rect(col_idx - 1).w
            } else {
                0
            };
            let right_width = if col_idx + 1 < count {
                self.col_rect(col_idx + 1).w
            } else {
                0
            };
            if right_width >= left_width && col_idx + 1 < count {
                self.columns[col_idx + 1].left = (col_bounds.right() + right_width / 2)
                    .min(self.width - (count - col_idx - 1) as i32 * min_width);
            } else if col_idx > 0 {
                self.columns[col_idx].left =
                    (col_bounds.x - left_width / 2).max(col_idx as i32 * min_width);
            }
        } else {
            for other in 0..count {
                self.columns[other].left = if other <= col_idx {
                    other as i32 * min_width
                } else {
                    self.width - (count - other) as i32 * min_width
                };
            }
        }
        self.fix_layout();
    }

    pub(super) fn move_col(&mut self, col_idx: usize, x: i32) {
        if col_idx == 0 {
            return;
        }
        self.columns[col_idx].left = x;
        self.fix_layout();
    }

    // ----------------------------------------------------------------- tags

    /// Rewrite the fixed part of every window tag (name and the commands
    /// that apply right now), leaving whatever follows the `|` alone.
    pub fn update_tags(&mut self) {
        for col_idx in 0..self.columns.len() {
            for win_idx in 0..self.columns[col_idx].windows.len() {
                let win = &mut self.columns[col_idx].windows[win_idx];
                let mut prefix = format!("{} Del Snarf", win.name);
                if win.is_dir {
                    prefix.push_str(" Get");
                } else {
                    if win.body.can_undo() {
                        prefix.push_str(" Undo");
                    }
                    if win.body.can_redo() {
                        prefix.push_str(" Redo");
                    }
                    if win.body.is_dirty() {
                        prefix.push_str(" Put");
                    }
                }
                prefix.push(' ');
                let old_prefix_len = win
                    .tag
                    .chars()
                    .iter()
                    .position(|&ch| ch == '|')
                    .unwrap_or(win.tag.len());
                let old_prefix = win.tag.slice(0, old_prefix_len);
                if old_prefix != prefix {
                    win.tag.replace_raw(0, old_prefix_len, &prefix);
                }
                if !win.tag.chars().contains(&'|') {
                    let len = win.tag.len();
                    win.tag.replace_raw(len, len, "| Look ");
                }
            }
        }
    }

    /// The file name currently written at the start of the tag.
    pub(super) fn tag_name(&self, win_id: usize) -> String {
        let win = self.win(win_id).unwrap();
        win.tag
            .contents()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    }
}
