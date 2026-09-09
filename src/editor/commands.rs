//! Built-in commands and shell execution (button 2).

use super::*;

impl Editor {
    /// Execute `command_line` as clicked in text `id`.
    pub fn execute(&mut self, id: TextId, command_line: &str) {
        let command_line = command_line.trim();
        if command_line.is_empty() {
            return;
        }
        let (win_id, col_idx) = self.win_and_col(id);
        let first_char = command_line.chars().next().unwrap();
        if "|<>".contains(first_char) {
            let kind = match first_char {
                '|' => Kind::Pipe,
                '<' => Kind::Input,
                _ => Kind::Output,
            };
            self.shell(id, command_line[1..].trim(), kind);
            return;
        }
        let (name, arg) = split_command(command_line);
        // The window whose body a command acts on: the one executed in, else
        // the one holding the selection.
        let target_win_id = win_id.or(match self.focus {
            TextId::Body(focused) | TextId::Tag(focused) => Some(focused),
            _ => None,
        });
        let dir = win_id.and_then(|id| self.win(id)).map(|win| win.dir());
        match name {
            "Exit" => {
                let dirty_names: Vec<String> = self
                    .columns
                    .iter()
                    .flat_map(|col| col.windows.iter())
                    .filter(|win| win.dirty())
                    .map(|win| win.name.clone())
                    .collect();
                if dirty_names.is_empty() || self.exit_warned {
                    self.quit = true;
                } else {
                    self.exit_warned = true;
                    for dirty_name in dirty_names {
                        let msg = format!("{dirty_name}: file modified");
                        self.error(None, col_idx, &msg);
                    }
                }
            }
            "Newcol" => {
                self.new_col(0);
            }
            "Delcol" => {
                if let Some(col_idx) = col_idx {
                    if let Some(win) = self.columns[col_idx].windows.iter().find(|win| win.dirty())
                    {
                        let msg = format!("{}: file modified", win.name);
                        self.error(dir, Some(col_idx), &msg);
                    } else {
                        self.columns.remove(col_idx);
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
                let col_idx = col_idx.unwrap_or(self.columns.len().saturating_sub(1));
                if arg.is_empty() {
                    let new_id = self.new_win(col_idx, String::new());
                    self.focus = TextId::Body(new_id);
                } else {
                    let base = dir.clone().unwrap_or_else(cwd);
                    let path = if Path::new(arg).is_absolute() {
                        PathBuf::from(arg)
                    } else {
                        base.join(arg)
                    };
                    let new_id = if path.exists() {
                        self.open_file(&path, col_idx)
                    } else {
                        self.new_win(col_idx, clean_path(&path))
                    };
                    self.focus = TextId::Body(new_id);
                }
            }
            "Del" => {
                if let Some(win_id) = win_id {
                    self.del(win_id, false);
                }
            }
            "Delete" => {
                if let Some(win_id) = win_id {
                    self.del(win_id, true);
                }
            }
            "Get" => {
                if let Some(win_id) = win_id {
                    self.get(win_id, arg);
                }
            }
            "Put" => {
                if let Some(win_id) = win_id {
                    self.put(win_id, arg);
                }
            }
            "Putall" => {
                let dirty_ids: Vec<usize> = self
                    .columns
                    .iter()
                    .flat_map(|col| col.windows.iter())
                    .filter(|win| {
                        win.dirty() && !win.name.is_empty() && !win.name.ends_with("+Errors")
                    })
                    .map(|win| win.id)
                    .collect();
                for dirty_id in dirty_ids {
                    self.put(dirty_id, "");
                }
            }
            "Undo" | "Redo" => {
                if let Some(target_id) = target_win_id {
                    let body = &mut self.win_mut(target_id).unwrap().body;
                    if name == "Undo" {
                        body.undo()
                    } else {
                        body.redo()
                    };
                    self.show_cursor(TextId::Body(target_id));
                }
            }
            "Cut" => self.cut(self.focus),
            "Paste" => self.paste(self.focus),
            "Snarf" => self.snarf_selection(self.focus),
            "Look" => {
                if let Some(target_id) = target_win_id {
                    let pattern = if arg.is_empty() {
                        self.selection_text(self.focus)
                    } else {
                        arg.to_string()
                    };
                    self.search(target_id, &pattern);
                }
            }
            "Send" => {
                if let Some(target_id) = target_win_id {
                    let mut text = self.selection_text(self.focus);
                    if text.is_empty() {
                        text = self.get_snarf();
                    }
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                    let body = &mut self.win_mut(target_id).unwrap().body;
                    let len = body.len();
                    body.replace(len, len, &text);
                    let len = body.len();
                    body.set_select(len, len);
                    self.show_cursor(TextId::Body(target_id));
                }
            }
            "Sort" => {
                if let Some(col_idx) = col_idx {
                    let top = self.windows_top(col_idx);
                    let bottom = self.col_rect(col_idx).bottom();
                    let windows = &mut self.columns[col_idx].windows;
                    windows.sort_by(|a, b| a.name.cmp(&b.name));
                    let count = windows.len().max(1) as i32;
                    for (win_idx, win) in windows.iter_mut().enumerate() {
                        win.top = top + (bottom - top) * win_idx as i32 / count;
                    }
                    self.fix_col(col_idx);
                }
            }
            "Zerox" => {
                if let (Some(src_id), Some(col_idx)) = (
                    target_win_id,
                    col_idx.or_else(|| self.find_win(target_win_id?).map(|(col_idx, _)| col_idx)),
                ) {
                    let src = self.win(src_id).unwrap();
                    let (name, body, tab_width, is_dir) = (
                        src.name.clone(),
                        src.body.contents(),
                        src.tab_width,
                        src.is_dir,
                    );
                    let copy_id = self.new_win(col_idx, name);
                    let copy = self.win_mut(copy_id).unwrap();
                    copy.body.set_contents(&body);
                    copy.tab_width = tab_width;
                    copy.is_dir = is_dir;
                }
            }
            "Tab" => {
                if let (Some(target_id), Ok(tab_width)) = (target_win_id, arg.parse::<usize>()) {
                    self.win_mut(target_id).unwrap().tab_width = tab_width.max(1);
                }
            }
            "Font" => self.font_command(arg, dir, col_idx),
            "Id" => {
                if let Some(win_id) = win_id {
                    let msg = format!("{win_id}");
                    self.error(dir, col_idx, &msg);
                }
            }
            "Kill" => self.error(dir, col_idx, "Kill: not supported"),
            _ => self.shell(id, command_line, Kind::Plain),
        }
        self.update_tags();
    }

    /// `Font`: cycle sizes; `Font 18`: set a size; `Font path.ttf`: load a
    /// font file.
    fn font_command(&mut self, arg: &str, dir: Option<PathBuf>, col_idx: Option<usize>) {
        let size = self.font.size;
        let loaded = if arg.is_empty() {
            let next_size = if size < 13.0 {
                14.0
            } else if size < 15.0 {
                16.0
            } else if size < 17.0 {
                18.0
            } else {
                12.0
            };
            Ok(self.font.with_size(next_size))
        } else if let Ok(requested_size) = arg.parse::<f32>() {
            Ok(self.font.with_size(requested_size.clamp(6.0, 72.0)))
        } else {
            Font::load_file(arg, size)
        };
        match loaded {
            Ok(font) => {
                self.font = font;
                self.fix_layout();
            }
            Err(e) => self.error(dir, col_idx, &e),
        }
    }

    /// Queue a shell command; `App` spawns it and delivers the result to
    /// `exec_done`.
    fn shell(&mut self, id: TextId, command: &str, kind: Kind) {
        let (win_id, col_idx) = self.win_and_col(id);
        let dir = win_id
            .and_then(|id| self.win(id))
            .map(|win| win.dir())
            .unwrap_or_else(cwd);
        let (input, sel_range) = match (kind, win_id) {
            (Kind::Plain, _) => (None, (0, 0)),
            (_, None) => {
                self.error(Some(dir), col_idx, "no window for pipe command");
                return;
            }
            (kind, Some(win_id)) => {
                let body = &self.win(win_id).unwrap().body;
                let sel_range = (body.sel_start, body.sel_end);
                let input = if kind == Kind::Input {
                    None
                } else {
                    Some(body.selection())
                };
                (input, sel_range)
            }
        };
        let mut env = Vec::new();
        if let Some(win) = win_id.and_then(|id| self.win(id)) {
            env.push(("samfile".to_string(), win.name.clone()));
            env.push(("winid".to_string(), win.id.to_string()));
        }
        self.pending_commands.push(exec::Request {
            command: command.to_string(),
            dir,
            input,
            kind,
            win_id,
            col_id: col_idx.map(|col_idx| self.columns[col_idx].id),
            sel_range,
            env,
        });
    }

    pub fn exec_done(&mut self, result: exec::Result) {
        let col_idx = result.col_id.and_then(|col_id| self.find_col(col_id));
        match result.kind {
            Kind::Pipe | Kind::Input => {
                if let Some(win_id) = result.win_id
                    && self.find_win(win_id).is_some()
                {
                    let body = &mut self.win_mut(win_id).unwrap().body;
                    body.replace(result.sel_range.0, result.sel_range.1, &result.stdout);
                    self.show_cursor(TextId::Body(win_id));
                }
                self.append_errors(&result.dir, col_idx, &result.stderr);
            }
            Kind::Plain | Kind::Output => {
                let output = format!("{}{}", result.stdout, result.stderr);
                self.append_errors(&result.dir, col_idx, &output);
            }
        }
        self.update_tags();
    }
}
