//! Button 3: opening files, jumping to addresses, and searching.

use super::*;

impl Editor {
    /// Select the next occurrence of `pattern` in the body of `win_id`,
    /// scroll to it and ask for the pointer to be warped there.
    pub(super) fn search(&mut self, win_id: usize, pattern: &str) {
        let pattern: Vec<char> = pattern.chars().collect();
        if pattern.is_empty() {
            return;
        }
        let Some(win) = self.win_mut(win_id) else {
            return;
        };
        if let Some(match_pos) = win.body.find(&pattern, win.body.sel_end) {
            win.body.set_select(match_pos, match_pos + pattern.len());
            self.focus = TextId::Body(win_id);
            self.show(win_id, match_pos, 0.33);
            if let Some((x, y)) = self.xy_of(TextId::Body(win_id), match_pos) {
                self.warp = Some((x + 1, y + self.font.line_height / 2));
            }
        }
    }

    /// Parse `name:addr`, returning the existing path and the address.
    fn resolve_file(&self, dir: &Path, text: &str) -> Option<(PathBuf, String)> {
        let existing_path = |name: &str| -> Option<PathBuf> {
            if name.is_empty() {
                return None;
            }
            let path = if Path::new(name).is_absolute() {
                PathBuf::from(name)
            } else {
                dir.join(name)
            };
            if path.exists() { Some(path) } else { None }
        };
        if let Some(path) = existing_path(text) {
            return Some((path, String::new()));
        }
        if let Some(colon) = text.rfind(':')
            && colon > 0
            && let Some(path) = existing_path(&text[..colon])
        {
            return Some((path, text[colon + 1..].to_string()));
        }
        None
    }

    /// Apply an address: a line number, `$`, or `/text`.
    fn address(&mut self, win_id: usize, addr: &str) {
        let Some(win) = self.win_mut(win_id) else {
            return;
        };
        let body = &mut win.body;
        if let Ok(line_num) = addr.parse::<usize>() {
            let (start, end) = body.line_range(line_num.max(1));
            body.set_select(start, end);
        } else if addr == "$" {
            let len = body.len();
            body.set_select(len, len);
        } else if let Some(pattern) = addr.strip_prefix('/') {
            let pattern = pattern.strip_suffix('/').unwrap_or(pattern);
            let pattern: Vec<char> = pattern.chars().collect();
            if let Some(match_pos) = body.find(&pattern, 0) {
                body.set_select(match_pos, match_pos + pattern.len());
            }
        } else {
            return;
        }
        let sel_start = body.sel_start;
        self.show(win_id, sel_start, 0.33);
    }

    /// Button 3 on `text` in `id`: an address, a file, or a search.
    pub(super) fn look3(&mut self, id: TextId, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let (win_id, col_idx) = self.win_and_col(id);
        let dir = win_id
            .and_then(|id| self.win(id))
            .map(|win| win.dir())
            .unwrap_or_else(cwd);
        let col_idx = col_idx.unwrap_or(self.columns.len().saturating_sub(1));
        if let Some(win_id) = win_id
            && let Some(addr) = text.strip_prefix(':')
        {
            self.address(win_id, addr);
            return;
        }
        if let Some((path, addr)) = self.resolve_file(&dir, text) {
            let target_id = self.open_file(&path, col_idx);
            if !addr.is_empty() {
                self.address(target_id, &addr);
            }
            self.focus = TextId::Body(target_id);
            let sel_start = self
                .win(target_id)
                .map(|win| win.body.sel_start)
                .unwrap_or(0);
            if let Some((x, y)) = self.xy_of(TextId::Body(target_id), sel_start) {
                self.warp = Some((x + 1, y + self.font.line_height / 2));
            } else if let Some((col_idx, win_idx)) = self.find_win(target_id) {
                let rects = self.win_rects(col_idx, win_idx);
                self.warp = Some((
                    rects.tag_text.x + 1,
                    rects.tag_text.y + self.font.line_height / 2,
                ));
            }
            return;
        }
        if let Some(win_id) = win_id {
            self.search(win_id, text);
        }
    }
}
