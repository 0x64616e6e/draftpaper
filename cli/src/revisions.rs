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
//! The repository is found by walking up for `.git`, and its head read from `.git` itself; git is asked
//! for the rows only when that head has moved since the last answer, which is kept beside the theme. The
//! shell asks every four seconds, so git runs on a commit, a checkout or a pull, not nine hundred times
//! an hour.
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
const ROWS: usize = 30;

/// What the revision schedule letters; `{"focused": false}` with nothing focused, so whatever is on the
/// sheet stays there.
pub fn json(p: &util::Paths) -> Value {
    if focused_pid().is_none() {
        return json!({ "focused": false });
    }
    revisions(p).unwrap_or_else(|| json!({}))
}

pub fn main(p: &util::Paths) {
    println!("{}", util::dumps(&json(p)));
}

fn revisions(p: &util::Paths) -> Option<Value> {
    let pid = focused_pid()?;
    let dir = directory(pid)?;
    let (top, gitdir) = repository(Path::new(&dir))?;
    let repo = top.file_name()?.to_string_lossy().into_owned();
    // the rows change only when the repository's head does, and the shell asks every four seconds: the
    // last answer is kept beside the theme and given back until then, so git runs on a commit, a
    // checkout or a pull, not nine hundred times an hour
    let stamp = format!("{}|{}", top.display(), head(&gitdir));
    let note = p.cache.join("revisions.json");
    let kept: Value = serde_json::from_str(&util::read(&note)).unwrap_or(Value::Null);
    if kept["stamp"] == json!(stamp) {
        return kept.get("out").cloned();
    }
    let log = util::run(
        &[
            "git",
            "--no-pager",
            "--no-optional-locks",
            "-c",
            "log.showSignature=false",
            "-c",
            "core.fsmonitor=false",
            "-C",
            dir.as_str(),
            "log",
            &format!("-n{ROWS}"),
            "--format=%h%x1f%s%x1f%as%x1f%an",
        ],
        3,
    )?;
    let rows: Vec<Value> = log
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\x1f').collect();
            (f.len() == 4).then(|| json!({ "rev": f[0], "description": f[1], "date": f[2], "by": f[3] }))
        })
        .collect();
    let out = (!rows.is_empty()).then(|| json!({ "repo": repo, "rows": rows }))?;
    let _ = std::fs::create_dir_all(&p.cache);
    util::write(&note, &util::dumps(&json!({ "stamp": stamp, "out": out })));
    Some(out)
}

/// The repository a directory sits in: its top and its git directory, found by walking up for `.git`, as
/// `git rev-parse --show-toplevel` does without starting git. A `.git` file instead of a directory points
/// elsewhere (a worktree, a submodule).
fn repository(dir: &Path) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    for top in dir.ancestors() {
        let dot = top.join(".git");
        if dot.is_dir() {
            return Some((top.to_path_buf(), dot));
        }
        if dot.is_file() {
            let pointed = util::read(&dot).trim().strip_prefix("gitdir:")?.trim().to_string();
            let pointed = std::path::PathBuf::from(&pointed);
            let pointed = if pointed.is_absolute() { pointed } else { top.join(pointed) };
            return Some((top.to_path_buf(), pointed));
        }
    }
    None
}

/// What the repository's head is now, as cheaply as it can be read: the branch it is on and the commit
/// that branch is at, or the commit itself where the head is detached. A branch whose file has been
/// packed away is covered by when packed-refs was last written.
fn head(gitdir: &Path) -> String {
    let head = util::read(&gitdir.join("HEAD"));
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref:") else { return head.to_string() };
    let at = util::read(&gitdir.join(reference.trim()));
    let packed = std::fs::metadata(gitdir.join("packed-refs"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{head}|{}|{packed}", at.trim())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory with a `.git` of the shape the test wants, and a path inside it.
    fn repo(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("draftpaper-rev-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src/deep")).unwrap();
        d
    }

    #[test]
    fn the_repository_is_the_first_git_at_or_above() {
        let d = repo("plain");
        std::fs::create_dir_all(d.join(".git")).unwrap();
        let (top, gitdir) = repository(&d.join("src/deep")).unwrap();
        assert_eq!(top, d);
        assert_eq!(gitdir, d.join(".git"));
        // nothing above it: not a repository at all
        assert!(repository(std::path::Path::new("/proc")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_git_file_points_elsewhere() {
        // a worktree or a submodule: ".git" is a file naming the real directory
        let d = repo("worktree");
        let elsewhere = d.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(d.join(".git"), format!("gitdir: {}\n", elsewhere.display())).unwrap();
        let (top, gitdir) = repository(&d.join("src")).unwrap();
        assert_eq!(top, d);
        assert_eq!(gitdir, elsewhere);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_head_moves_with_the_branch_and_the_commit() {
        let d = repo("head");
        let g = d.join(".git");
        std::fs::create_dir_all(g.join("refs/heads")).unwrap();
        std::fs::write(g.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(g.join("refs/heads/main"), "aaaa1111\n").unwrap();
        let first = head(&g);
        // the same repository, untouched: the same answer, so git is not asked again
        assert_eq!(first, head(&g));
        // a commit moves the branch
        std::fs::write(g.join("refs/heads/main"), "bbbb2222\n").unwrap();
        assert_ne!(first, head(&g));
        // and a checkout moves the head itself
        std::fs::write(g.join("HEAD"), "ref: refs/heads/other\n").unwrap();
        let other = head(&g);
        assert_ne!(other, head(&g.join("nowhere")));
        // detached: the commit is the head
        std::fs::write(g.join("HEAD"), "cccc3333\n").unwrap();
        assert_eq!(head(&g), "cccc3333");
        let _ = std::fs::remove_dir_all(&d);
    }
}
