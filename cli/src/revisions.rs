//! `draftpaper revisions`: what the revision block above the title block letters (issue #45, the shell
//! polls it: qml/RevisionBlock.qml). The focused window's directory, as the compositor names its process
//! over the IPC: a terminal's is its shell's, or the directory of the program the shell runs (an editor
//! moved elsewhere); any other program's its own. In a git repository there, its name and latest commits,
//! as JSON:
//!
//!     {"repo": "draftpaper", "rows": [{"rev": "714fcc7", "description": "A scale bar…", "date": "2026-10-18", "by": "0x64616e6e"}, …]}
//!
//! `{}` outside any repository or a session; `{"focused": false}` with nothing focused (every window
//! parked: the shell keeps the last rows, the sheet then showing the work just left). A terminal server running
//! several windows in one process (foot --server) is read by its first shell.
//!
//! git runs in whatever repository the focused window sits in, so nothing in that repository's own
//! configuration may run a program: signatures are not shown (log.showSignature would run gpg.program),
//! no file system monitor, no optional locks, no pager.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use serde_json::{json, Value};

use crate::util;

/// The shells a terminal runs (the compositor's session.rs keeps the same list).
const SHELLS: &[&str] = &["bash", "zsh", "fish", "sh", "dash", "ksh", "mksh", "tcsh", "nu", "elvish", "xonsh"];
const ROWS: usize = 4;

pub fn main() {
    // nothing focused (every window parked, an empty workspace): said so, and the shell keeps showing the
    // repository last worked in
    if focused_pid().is_none() {
        println!("{{\"focused\":false}}");
        return;
    }
    println!("{}", util::dumps(&revisions().unwrap_or_else(|| json!({}))));
}

fn revisions() -> Option<Value> {
    let pid = focused_pid()?;
    let dir = directory(pid)?;
    let git = |args: &[&str]| {
        let mut cmd = vec!["git", "--no-pager", "--no-optional-locks", "-c", "log.showSignature=false", "-c", "core.fsmonitor=false", "-C", dir.as_str()];
        cmd.extend_from_slice(args);
        util::run(&cmd, 3)
    };
    let top = git(&["rev-parse", "--show-toplevel"])?;
    let repo = Path::new(top.trim()).file_name()?.to_string_lossy().into_owned();
    let log = git(&["log", &format!("-n{ROWS}"), "--format=%h%x1f%s%x1f%as%x1f%an"])?;
    let rows: Vec<Value> = log
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\x1f').collect();
            (f.len() == 4).then(|| json!({ "rev": f[0], "description": f[1], "date": f[2], "by": f[3] }))
        })
        .collect();
    (!rows.is_empty()).then(|| json!({ "repo": repo, "rows": rows }))
}

/// The process of the focused window, from the compositor's IPC (`windows`).
fn focused_pid() -> Option<u32> {
    let path = std::env::var_os("DRAFTPAPER_SOCKET")?;
    let mut s = UnixStream::connect(path).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok()?;
    s.set_write_timeout(Some(std::time::Duration::from_secs(2))).ok()?;
    s.write_all(b"{\"request\": \"windows\"}\n").ok()?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).ok()?;
    let reply: Value = serde_json::from_str(&line).ok()?;
    let windows = reply.get("reply")?.as_array()?;
    let focused = windows.iter().find(|w| w["focused"].as_bool() == Some(true))?;
    focused["pid"].as_u64().map(|p| p as u32)
}

fn children(pid: u32) -> Vec<u32> {
    std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap_or_default().split_whitespace().filter_map(|c| c.parse().ok()).collect()
}
fn cwd(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok().map(|p| p.to_string_lossy().into_owned())
}

/// Where `pid` works: a terminal's shell's directory, or that of the program the shell runs; else its own.
fn directory(pid: u32) -> Option<String> {
    let shell = children(pid).into_iter().find(|c| std::fs::read_to_string(format!("/proc/{c}/comm")).is_ok_and(|comm| SHELLS.contains(&comm.trim())));
    match shell {
        Some(sh) => children(sh).into_iter().last().and_then(cwd).or_else(|| cwd(sh)),
        None => cwd(pid),
    }
}
