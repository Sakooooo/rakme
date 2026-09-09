//! Built-in commands and shell execution (button 2).

use super::*;

impl Editor {
    /// Execute `cmd` as clicked in text `id`.
    pub fn execute(&mut self, id: TextId, cmd: &str) {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return;
        }
        let (win, col) = self.ctx(id);
        let first = cmd.chars().next().unwrap();
        if "|<>".contains(first) {
            let kind = match first {
                '|' => Kind::Pipe,
                '<' => Kind::Input,
                _ => Kind::Output,
            };
            self.shell(id, cmd[1..].trim(), kind);
            return;
        }
        let (name, arg) = split_cmd(cmd);
        // The window whose body a command acts on: the one executed in, else
        // the one holding the selection.
        let target = win.or(match self.focus {
            TextId::Body(w) | TextId::Tag(w) => Some(w),
            _ => None,
        });
        let dir = win.and_then(|w| self.win(w)).map(|w| w.dir());
        match name {
            "Exit" => {
                let dirty: Vec<String> = self
                    .cols
                    .iter()
                    .flat_map(|c| c.wins.iter())
                    .filter(|w| w.dirty())
                    .map(|w| w.name.clone())
                    .collect();
                if dirty.is_empty() || self.exit_warned {
                    self.quit = true;
                } else {
                    self.exit_warned = true;
                    for n in dirty {
                        let msg = format!("{n}: file modified");
                        self.error(None, col, &msg);
                    }
                }
            }
            "Newcol" => {
                self.new_col(0);
            }
            "Delcol" => {
                if let Some(ci) = col {
                    if let Some(w) = self.cols[ci].wins.iter().find(|w| w.dirty()) {
                        let msg = format!("{}: file modified", w.name);
                        self.error(dir, Some(ci), &msg);
                    } else {
                        self.cols.remove(ci);
                        if let TextId::ColTag(_) | TextId::Body(_) | TextId::Tag(_) = self.focus
                            && self.text(self.focus).is_none()
                        {
                            self.focus = TextId::Row;
                        }
                        self.fix_layout();
                    }
                }
            }
            "New" => {
                let ci = col.unwrap_or(self.cols.len().saturating_sub(1));
                if arg.is_empty() {
                    let id = self.new_win(ci, String::new());
                    self.focus = TextId::Body(id);
                } else {
                    let base = dir.clone().unwrap_or_else(cwd);
                    let path = if Path::new(arg).is_absolute() {
                        PathBuf::from(arg)
                    } else {
                        base.join(arg)
                    };
                    let id = if path.exists() {
                        self.open_file(&path, ci)
                    } else {
                        self.new_win(ci, clean_path(&path))
                    };
                    self.focus = TextId::Body(id);
                }
            }
            "Del" => {
                if let Some(w) = win {
                    self.del(w, false);
                }
            }
            "Delete" => {
                if let Some(w) = win {
                    self.del(w, true);
                }
            }
            "Get" => {
                if let Some(w) = win {
                    self.get(w, arg);
                }
            }
            "Put" => {
                if let Some(w) = win {
                    self.put(w, arg);
                }
            }
            "Putall" => {
                let ids: Vec<usize> = self
                    .cols
                    .iter()
                    .flat_map(|c| c.wins.iter())
                    .filter(|w| w.dirty() && !w.name.is_empty() && !w.name.ends_with("+Errors"))
                    .map(|w| w.id)
                    .collect();
                for id in ids {
                    self.put(id, "");
                }
            }
            "Undo" | "Redo" => {
                if let Some(w) = target {
                    let t = &mut self.win_mut(w).unwrap().body;
                    if name == "Undo" {
                        t.undo()
                    } else {
                        t.redo()
                    };
                    self.show_cursor(TextId::Body(w));
                }
            }
            "Cut" => self.cut(self.focus),
            "Paste" => self.paste(self.focus),
            "Snarf" => self.snarf_sel(self.focus),
            "Look" => {
                if let Some(w) = target {
                    let pat = if arg.is_empty() {
                        self.selection_text(self.focus)
                    } else {
                        arg.to_string()
                    };
                    self.search(w, &pat);
                }
            }
            "Send" => {
                if let Some(w) = target {
                    let mut s = self.selection_text(self.focus);
                    if s.is_empty() {
                        s = self.get_snarf();
                    }
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                    let t = &mut self.win_mut(w).unwrap().body;
                    let n = t.len();
                    t.replace(n, n, &s);
                    let n = t.len();
                    t.set_select(n, n);
                    self.show_cursor(TextId::Body(w));
                }
            }
            "Sort" => {
                if let Some(ci) = col {
                    let top = self.wins_top(ci);
                    let bottom = self.col_rect(ci).bottom();
                    let wins = &mut self.cols[ci].wins;
                    wins.sort_by(|a, b| a.name.cmp(&b.name));
                    let n = wins.len().max(1) as i32;
                    for (i, w) in wins.iter_mut().enumerate() {
                        w.y0 = top + (bottom - top) * i as i32 / n;
                    }
                    self.fix_col(ci);
                }
            }
            "Zerox" => {
                if let (Some(w), Some(ci)) = (
                    target,
                    col.or_else(|| self.find_win(target?).map(|(c, _)| c)),
                ) {
                    let src = self.win(w).unwrap();
                    let (name, body, tab, is_dir) =
                        (src.name.clone(), src.body.contents(), src.tab, src.is_dir);
                    let nid = self.new_win(ci, name);
                    let nw = self.win_mut(nid).unwrap();
                    nw.body.set_contents(&body);
                    nw.tab = tab;
                    nw.is_dir = is_dir;
                }
            }
            "Tab" => {
                if let (Some(w), Ok(n)) = (target, arg.parse::<usize>()) {
                    self.win_mut(w).unwrap().tab = n.max(1);
                }
            }
            "Font" => self.font_cmd(arg, dir, col),
            "Id" => {
                if let Some(w) = win {
                    let msg = format!("{w}");
                    self.error(dir, col, &msg);
                }
            }
            "Kill" => self.error(dir, col, "Kill: not supported"),
            _ => self.shell(id, cmd, Kind::Plain),
        }
        self.update_tags();
    }

    /// `Font`: cycle sizes; `Font 18`: set a size; `Font path.ttf`: load a
    /// font file.
    fn font_cmd(&mut self, arg: &str, dir: Option<PathBuf>, col: Option<usize>) {
        let size = self.font.size;
        let new = if arg.is_empty() {
            let next = if size < 13.0 {
                14.0
            } else if size < 15.0 {
                16.0
            } else if size < 17.0 {
                18.0
            } else {
                12.0
            };
            Ok(self.font.with_size(next))
        } else if let Ok(n) = arg.parse::<f32>() {
            Ok(self.font.with_size(n.clamp(6.0, 72.0)))
        } else {
            Font::load_file(arg, size)
        };
        match new {
            Ok(f) => {
                self.font = f;
                self.fix_layout();
            }
            Err(e) => self.error(dir, col, &e),
        }
    }

    /// Queue a shell command; `App` spawns it and delivers the result to
    /// `exec_done`.
    fn shell(&mut self, id: TextId, cmd: &str, kind: Kind) {
        let (win, col) = self.ctx(id);
        let dir = win
            .and_then(|w| self.win(w))
            .map(|w| w.dir())
            .unwrap_or_else(cwd);
        let (input, range) = match (kind, win) {
            (Kind::Plain, _) => (None, (0, 0)),
            (_, None) => {
                self.error(Some(dir), col, "no window for pipe command");
                return;
            }
            (k, Some(w)) => {
                let t = &self.win(w).unwrap().body;
                let range = (t.q0, t.q1);
                let input = if k == Kind::Input {
                    None
                } else {
                    Some(t.selection())
                };
                (input, range)
            }
        };
        let mut env = Vec::new();
        if let Some(w) = win.and_then(|w| self.win(w)) {
            env.push(("samfile".to_string(), w.name.clone()));
            env.push(("winid".to_string(), w.id.to_string()));
        }
        self.pending.push(exec::Request {
            cmd: cmd.to_string(),
            dir,
            input,
            kind,
            win,
            col: col.map(|ci| self.cols[ci].id),
            range,
            env,
        });
    }

    pub fn exec_done(&mut self, r: exec::Result) {
        let col = r.col.and_then(|c| self.find_col(c));
        match r.kind {
            Kind::Pipe | Kind::Input => {
                if let Some(w) = r.win
                    && self.find_win(w).is_some()
                {
                    let t = &mut self.win_mut(w).unwrap().body;
                    t.replace(r.range.0, r.range.1, &r.out);
                    self.show_cursor(TextId::Body(w));
                }
                self.append_errors(&r.dir, col, &r.err);
            }
            Kind::Plain | Kind::Output => {
                let text = format!("{}{}", r.out, r.err);
                self.append_errors(&r.dir, col, &text);
            }
        }
        self.update_tags();
    }
}
