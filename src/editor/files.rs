//! Reading and writing files and directories, and the `+Errors` window.

use super::*;

impl Editor {
    /// Report a message in the `+Errors` window for `dir`.
    pub(super) fn error(&mut self, dir: Option<PathBuf>, col_idx: Option<usize>, msg: &str) {
        let dir = dir.unwrap_or_else(cwd);
        let mut msg = msg.to_string();
        if !msg.ends_with('\n') {
            msg.push('\n');
        }
        self.append_errors(&dir, col_idx, &msg);
    }

    /// Id of the `+Errors` window for `dir`, creating it in column
    /// `col_idx` (or the last column) if needed.
    fn errors_win_id(&mut self, dir: &Path, col_idx: Option<usize>) -> usize {
        let name = format!("{}/+Errors", dir_string(dir));
        for col in &self.columns {
            for win in &col.windows {
                if win.name == name {
                    return win.id;
                }
            }
        }
        let col_idx = col_idx.unwrap_or(self.columns.len().saturating_sub(1));
        self.new_win(col_idx, name)
    }

    pub(super) fn append_errors(&mut self, dir: &Path, col_idx: Option<usize>, text: &str) {
        if text.is_empty() {
            return;
        }
        let win_id = self.errors_win_id(dir, col_idx);
        let win = self.win_mut(win_id).unwrap();
        let len = win.body.len();
        win.body.replace_raw(len, len, text);
        let len = win.body.len();
        win.body.set_select(len, len);
        self.show(win_id, len, 0.999);
        self.update_tags();
    }

    /// Contents of a file, or a listing of a directory; the flag says which.
    fn read_path(path: &Path) -> Result<(String, bool), String> {
        if path.is_dir() {
            let mut names: Vec<String> = std::fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let mut name = entry.file_name().to_string_lossy().into_owned();
                    if entry.path().is_dir() {
                        name.push('/');
                    }
                    name
                })
                .collect();
            names.sort();
            let mut listing = names.join("\n");
            if !listing.is_empty() {
                listing.push('\n');
            }
            Ok((listing, true))
        } else {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            Ok((String::from_utf8_lossy(&bytes).into_owned(), false))
        }
    }

    /// Open (or find) a window for `path` in column `col_idx`. Returns the
    /// window's id.
    pub fn open_file(&mut self, path: &Path, col_idx: usize) -> usize {
        let name = clean_path(path);
        for col in &self.columns {
            for win in &col.windows {
                if win.name == name {
                    return win.id;
                }
            }
        }
        let win_id = self.new_win(col_idx, name.clone());
        match Self::read_path(Path::new(&name)) {
            Ok((contents, is_dir)) => {
                let win = self.win_mut(win_id).unwrap();
                win.body.set_contents(&contents);
                win.is_dir = is_dir;
            }
            Err(e) => {
                let msg = format!("{name}: {e}");
                self.error(Some(cwd()), Some(col_idx), &msg);
            }
        }
        self.update_tags();
        win_id
    }

    /// `Get [name]`: reload the window from disk. Refused once if dirty.
    pub(super) fn get(&mut self, win_id: usize, arg: &str) {
        let Some((col_idx, _)) = self.find_win(win_id) else {
            return;
        };
        let win = self.win(win_id).unwrap();
        let dir = win.dir();
        if win.dirty() && win.warned_at_revision != Some(win.body.revision) {
            let revision = win.body.revision;
            let name = win.name.clone();
            self.win_mut(win_id).unwrap().warned_at_revision = Some(revision);
            let msg = format!("{name}: file modified");
            self.error(Some(dir), Some(col_idx), &msg);
            return;
        }
        let name = if arg.is_empty() {
            self.tag_name(win_id)
        } else {
            arg.to_string()
        };
        if name.is_empty() {
            self.error(Some(dir), Some(col_idx), "no file name");
            return;
        }
        let path = if Path::new(&name).is_absolute() {
            PathBuf::from(&name)
        } else {
            dir.join(&name)
        };
        let name = clean_path(&path);
        match Self::read_path(&path) {
            Ok((contents, is_dir)) => {
                let win = self.win_mut(win_id).unwrap();
                win.name = name;
                win.is_dir = is_dir;
                win.body.set_contents(&contents);
                win.origin = 0;
                win.warned_at_revision = None;
            }
            Err(e) => {
                let msg = format!("{name}: {e}");
                self.error(Some(dir), Some(col_idx), &msg);
            }
        }
    }

    /// `Put [name]`: write the body to the file named in the tag (or `arg`).
    pub(super) fn put(&mut self, win_id: usize, arg: &str) {
        let Some((col_idx, _)) = self.find_win(win_id) else {
            return;
        };
        let win = self.win(win_id).unwrap();
        let dir = win.dir();
        if win.is_dir {
            self.error(Some(dir), Some(col_idx), "cannot write a directory");
            return;
        }
        let name = if arg.is_empty() {
            self.tag_name(win_id)
        } else {
            arg.to_string()
        };
        if name.is_empty() {
            self.error(Some(dir), Some(col_idx), "no file name");
            return;
        }
        let path = if Path::new(&name).is_absolute() {
            PathBuf::from(&name)
        } else {
            dir.join(&name)
        };
        let contents = self.win(win_id).unwrap().body.contents();
        match std::fs::write(&path, contents) {
            Ok(()) => {
                let name = clean_path(&path);
                let win = self.win_mut(win_id).unwrap();
                win.name = name;
                win.body.mark_clean();
                win.warned_at_revision = None;
            }
            Err(e) => {
                let msg = format!("{}: {e}", path.display());
                self.error(Some(dir), Some(col_idx), &msg);
            }
        }
    }

    /// `Del` / `Delete`: close the window. Without `force` a dirty window
    /// is refused once.
    pub(super) fn del(&mut self, win_id: usize, force: bool) {
        let Some((col_idx, _)) = self.find_win(win_id) else {
            return;
        };
        let win = self.win(win_id).unwrap();
        if !force
            && (win.dirty() && !win.is_scratch())
            && win.warned_at_revision != Some(win.body.revision)
        {
            let revision = win.body.revision;
            let (dir, name) = (win.dir(), win.name.clone());
            self.win_mut(win_id).unwrap().warned_at_revision = Some(revision);
            let msg = format!("{name}: file modified");
            self.error(Some(dir), Some(col_idx), &msg);
            return;
        }
        self.close_win(win_id);
    }
}
