//! Reading and writing files and directories, and the `+Errors` window.

use super::*;

impl Editor {
    /// Report a message in the `+Errors` window for `dir`.
    pub(super) fn error(&mut self, dir: Option<PathBuf>, col: Option<usize>, msg: &str) {
        let dir = dir.unwrap_or_else(cwd);
        let mut msg = msg.to_string();
        if !msg.ends_with('\n') {
            msg.push('\n');
        }
        self.append_errors(&dir, col, &msg);
    }

    fn errors_win(&mut self, dir: &Path, col: Option<usize>) -> usize {
        let name = format!("{}/+Errors", dir_string(dir));
        for c in &self.cols {
            for w in &c.wins {
                if w.name == name {
                    return w.id;
                }
            }
        }
        let ci = col.unwrap_or(self.cols.len().saturating_sub(1));
        self.new_win(ci, name)
    }

    pub(super) fn append_errors(&mut self, dir: &Path, col: Option<usize>, text: &str) {
        if text.is_empty() {
            return;
        }
        let id = self.errors_win(dir, col);
        let w = self.win_mut(id).unwrap();
        let n = w.body.len();
        w.body.replace_raw(n, n, text);
        let n = w.body.len();
        w.body.set_select(n, n);
        self.show(id, n, 0.999);
        self.update_tags();
    }

    /// Contents of a file, or a listing of a directory; the flag says which.
    fn read_path(path: &Path) -> Result<(String, bool), String> {
        if path.is_dir() {
            let mut names: Vec<String> = std::fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok())
                .map(|e| {
                    let mut n = e.file_name().to_string_lossy().into_owned();
                    if e.path().is_dir() {
                        n.push('/');
                    }
                    n
                })
                .collect();
            names.sort();
            let mut s = names.join("\n");
            if !s.is_empty() {
                s.push('\n');
            }
            Ok((s, true))
        } else {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            Ok((String::from_utf8_lossy(&bytes).into_owned(), false))
        }
    }

    /// Open (or find) a window for `path` in column `ci`.
    pub fn open_file(&mut self, path: &Path, ci: usize) -> usize {
        let name = clean_path(path);
        for c in &self.cols {
            for w in &c.wins {
                if w.name == name {
                    return w.id;
                }
            }
        }
        let id = self.new_win(ci, name.clone());
        match Self::read_path(Path::new(&name)) {
            Ok((s, is_dir)) => {
                let w = self.win_mut(id).unwrap();
                w.body.set_contents(&s);
                w.is_dir = is_dir;
            }
            Err(e) => {
                let msg = format!("{name}: {e}");
                self.error(Some(cwd()), Some(ci), &msg);
            }
        }
        self.update_tags();
        id
    }

    /// `Get [name]`: reload the window from disk. Refused once if dirty.
    pub(super) fn get(&mut self, id: usize, arg: &str) {
        let Some((ci, _)) = self.find_win(id) else {
            return;
        };
        let w = self.win(id).unwrap();
        let dir = w.dir();
        if w.dirty() && w.warned != Some(w.body.seq) {
            self.win_mut(id).unwrap().warned = Some(self.win(id).unwrap().body.seq);
            let msg = format!("{}: file modified", self.win(id).unwrap().name);
            self.error(Some(dir), Some(ci), &msg);
            return;
        }
        let name = if arg.is_empty() {
            self.tag_name(id)
        } else {
            arg.to_string()
        };
        if name.is_empty() {
            self.error(Some(dir), Some(ci), "no file name");
            return;
        }
        let path = if Path::new(&name).is_absolute() {
            PathBuf::from(&name)
        } else {
            dir.join(&name)
        };
        let name = clean_path(&path);
        match Self::read_path(&path) {
            Ok((s, is_dir)) => {
                let w = self.win_mut(id).unwrap();
                w.name = name;
                w.is_dir = is_dir;
                w.body.set_contents(&s);
                w.origin = 0;
                w.warned = None;
            }
            Err(e) => {
                let msg = format!("{name}: {e}");
                self.error(Some(dir), Some(ci), &msg);
            }
        }
    }

    /// `Put [name]`: write the body to the file named in the tag (or `arg`).
    pub(super) fn put(&mut self, id: usize, arg: &str) {
        let Some((ci, _)) = self.find_win(id) else {
            return;
        };
        let w = self.win(id).unwrap();
        let dir = w.dir();
        if w.is_dir {
            self.error(Some(dir), Some(ci), "cannot write a directory");
            return;
        }
        let name = if arg.is_empty() {
            self.tag_name(id)
        } else {
            arg.to_string()
        };
        if name.is_empty() {
            self.error(Some(dir), Some(ci), "no file name");
            return;
        }
        let path = if Path::new(&name).is_absolute() {
            PathBuf::from(&name)
        } else {
            dir.join(&name)
        };
        let contents = self.win(id).unwrap().body.contents();
        match std::fs::write(&path, contents) {
            Ok(()) => {
                let name = clean_path(&path);
                let w = self.win_mut(id).unwrap();
                w.name = name;
                w.body.mark_clean();
                w.warned = None;
            }
            Err(e) => {
                let msg = format!("{}: {e}", path.display());
                self.error(Some(dir), Some(ci), &msg);
            }
        }
    }

    /// `Del` / `Delete`: close the window. Without `force` a dirty window
    /// is refused once.
    pub(super) fn del(&mut self, id: usize, force: bool) {
        let Some((ci, _)) = self.find_win(id) else {
            return;
        };
        let w = self.win(id).unwrap();
        if !force && (w.dirty() && !w.is_scratch()) && w.warned != Some(w.body.seq) {
            let seq = w.body.seq;
            let (dir, name) = (w.dir(), w.name.clone());
            self.win_mut(id).unwrap().warned = Some(seq);
            let msg = format!("{name}: file modified");
            self.error(Some(dir), Some(ci), &msg);
            return;
        }
        self.close_win(id);
    }
}
