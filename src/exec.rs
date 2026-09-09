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
    pub command: String,
    pub dir: PathBuf,
    /// Text fed to the command's stdin, if any.
    pub input: Option<String>,
    pub kind: Kind,
    /// Id of the window the command was run from, if any.
    pub win_id: Option<usize>,
    /// Id of the column the command was run from, if any.
    pub col_id: Option<usize>,
    /// The selection the output should replace (for `Pipe` and `Input`).
    pub sel_range: (usize, usize),
    pub env: Vec<(String, String)>,
}

pub struct Result {
    pub kind: Kind,
    pub win_id: Option<usize>,
    pub col_id: Option<usize>,
    pub dir: PathBuf,
    pub sel_range: (usize, usize),
    pub stdout: String,
    pub stderr: String,
}

pub fn spawn(request: Request, done: impl FnOnce(Result) + Send + 'static) {
    std::thread::spawn(move || {
        let (stdout, stderr) = run(&request);
        done(Result {
            kind: request.kind,
            win_id: request.win_id,
            col_id: request.col_id,
            dir: request.dir,
            sel_range: request.sel_range,
            stdout,
            stderr,
        });
    });
}

fn run(request: &Request) -> (String, String) {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", &request.command]);
        command
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
        let mut command = Command::new(shell);
        command.args(["-c", &request.command]);
        command
    };
    if request.dir.is_dir() {
        command.current_dir(&request.dir);
    }
    for (key, value) in &request.env {
        command.env(key, value);
    }
    command.stdin(if request.input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return (String::new(), format!("{}: {e}\n", request.command)),
    };
    if let Some(input) = &request.input
        && let Some(mut stdin) = child.stdin.take()
    {
        let input = input.clone();
        std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    match child.wait_with_output() {
        Ok(output) => (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ),
        Err(e) => (String::new(), format!("{}: {e}\n", request.command)),
    }
}
