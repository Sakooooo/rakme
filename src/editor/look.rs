//! Button 3: opening files, jumping to addresses, and searching.

use super::*;

impl Editor {
    /// Select the next occurrence of `pat` in the body of `win`, scroll to
    /// it and ask for the pointer to be warped there.
    pub(super) fn search(&mut self, win: usize, pat: &str) {
        let pat: Vec<char> = pat.chars().collect();
        if pat.is_empty() {
            return;
        }
        let Some(w) = self.win_mut(win) else {
            return;
        };
        if let Some(i) = w.body.find(&pat, w.body.q1) {
            w.body.set_select(i, i + pat.len());
            self.focus = TextId::Body(win);
            self.show(win, i, 0.33);
            if let Some((x, y)) = self.xy_of(TextId::Body(win), i) {
                self.warp = Some((x + 1, y + self.font.line_h / 2));
            }
        }
    }

    /// Parse `name:addr`, returning the existing path and the address.
    fn resolve_file(&self, dir: &Path, text: &str) -> Option<(PathBuf, String)> {
        let try_path = |s: &str| -> Option<PathBuf> {
            if s.is_empty() {
                return None;
            }
            let p = if Path::new(s).is_absolute() {
                PathBuf::from(s)
            } else {
                dir.join(s)
            };
            if p.exists() { Some(p) } else { None }
        };
        if let Some(p) = try_path(text) {
            return Some((p, String::new()));
        }
        if let Some(i) = text.rfind(':')
            && i > 0
            && let Some(p) = try_path(&text[..i])
        {
            return Some((p, text[i + 1..].to_string()));
        }
        None
    }

    /// Apply an address: a line number, `$`, or `/text`.
    fn address(&mut self, win: usize, addr: &str) {
        let Some(w) = self.win_mut(win) else {
            return;
        };
        let t = &mut w.body;
        if let Ok(n) = addr.parse::<usize>() {
            let (a, b) = t.line_range(n.max(1));
            t.set_select(a, b);
        } else if addr == "$" {
            let n = t.len();
            t.set_select(n, n);
        } else if let Some(pat) = addr.strip_prefix('/') {
            let pat = pat.strip_suffix('/').unwrap_or(pat);
            let pat: Vec<char> = pat.chars().collect();
            if let Some(i) = t.find(&pat, 0) {
                t.set_select(i, i + pat.len());
            }
        } else {
            return;
        }
        let q0 = t.q0;
        self.show(win, q0, 0.33);
    }

    /// Button 3 on `text` in `id`: an address, a file, or a search.
    pub(super) fn look3(&mut self, id: TextId, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let (win, col) = self.ctx(id);
        let dir = win
            .and_then(|w| self.win(w))
            .map(|w| w.dir())
            .unwrap_or_else(cwd);
        let ci = col.unwrap_or(self.cols.len().saturating_sub(1));
        if let Some(w) = win
            && let Some(addr) = text.strip_prefix(':')
        {
            self.address(w, addr);
            return;
        }
        if let Some((path, addr)) = self.resolve_file(&dir, text) {
            let target = self.open_file(&path, ci);
            if !addr.is_empty() {
                self.address(target, &addr);
            }
            self.focus = TextId::Body(target);
            let q = self.win(target).map(|w| w.body.q0).unwrap_or(0);
            if let Some((x, y)) = self.xy_of(TextId::Body(target), q) {
                self.warp = Some((x + 1, y + self.font.line_h / 2));
            } else if let Some((tci, twi)) = self.find_win(target) {
                let r = self.win_rects(tci, twi);
                self.warp = Some((r.tag_text.x + 1, r.tag_text.y + self.font.line_h / 2));
            }
            return;
        }
        if let Some(w) = win {
            self.search(w, text);
        }
    }
}
