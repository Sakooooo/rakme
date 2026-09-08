//! Running shell commands in the background.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Output goes to the +Errors window.
    Plain,
    /// `|cmd`: selection is stdin, stdout replaces the selection.
    Pipe,
    /// `<cmd`: stdout replaces the selection.
    Input,
    /// `>cmd`: selection is stdin, output goes to +Errors.
    Output,
}

pub struct Request {
    pub cmd: String,
    pub dir: PathBuf,
    pub input: Option<String>,
    pub kind: Kind,
    pub win: Option<usize>,
    pub col: Option<usize>,
    pub range: (usize, usize),
    pub env: Vec<(String, String)>,
}

pub struct Result {
    pub kind: Kind,
    pub win: Option<usize>,
    pub col: Option<usize>,
    pub dir: PathBuf,
    pub range: (usize, usize),
    pub out: String,
    pub err: String,
}

pub fn spawn(req: Request, done: impl FnOnce(Result) + Send + 'static) {
    std::thread::spawn(move || {
        let (out, err) = run(&req);
        done(Result {
            kind: req.kind,
            win: req.win,
            col: req.col,
            dir: req.dir,
            range: req.range,
            out,
            err,
        });
    });
}

fn run(req: &Request) -> (String, String) {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", &req.cmd]);
        c
    } else {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
        let mut c = Command::new(sh);
        c.args(["-c", &req.cmd]);
        c
    };
    if req.dir.is_dir() {
        c.current_dir(&req.dir);
    }
    for (k, v) in &req.env {
        c.env(k, v);
    }
    c.stdin(if req.input.is_some() { Stdio::piped() } else { Stdio::null() });
    c.stdout(Stdio::piped());
    c.stderr(Stdio::piped());
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return (String::new(), format!("{}: {e}\n", req.cmd)),
    };
    if let Some(input) = &req.input
        && let Some(mut stdin) = child.stdin.take()
    {
        let input = input.clone();
        std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    match child.wait_with_output() {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        ),
        Err(e) => (String::new(), format!("{}: {e}\n", req.cmd)),
    }
}
