//! tmux's sessions kept across a reboot (config.toml [session] apps). A tmux server outlives a logout but
//! not a reboot: `draftpaper tmux keep`, started with the server (tmux-session.conf), writes its sessions
//! to $XDG_STATE_HOME/draftpaper/tmux.json every few seconds; `draftpaper tmux restore`, run as the server
//! starts (before `tmux attach` looks for a session), makes them again: each window with its name,
//! layout and panes, each pane in its directory and running again the program it ran (an editor, a
//! pager, a monitor: programs::RESUMED), the active window and pane as they were.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::util::{self, Paths};

/// How often `keep` saves.
const EVERY: std::time::Duration = std::time::Duration::from_secs(10);

fn tmux(socket: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("tmux").arg("-S").arg(socket).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn file(p: &Paths) -> std::path::PathBuf {
    p.state.join("tmux.json")
}

/// The server's sessions as they are now; None when it has none (or is gone).
/// Between the fields tmux prints (tabs and control characters come out as `_` without a UTF-8 locale).
const SEP: &str = "<|>";

fn now(socket: &str) -> Option<Value> {
    const FIELDS: &str = "#{session_name}<|>#{window_index}<|>#{window_name}<|>#{automatic-rename}<|>#{window_layout}<|>#{window_active}<|>#{window_width}<|>#{window_height}<|>#{pane_index}<|>#{pane_active}<|>#{pane_current_path}<|>#{pane_pid}";
    let out = tmux(socket, &["list-panes", "-a", "-F", FIELDS])?;
    let mut sessions: Vec<Value> = Vec::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        if f.len() < 12 {
            continue;
        }
        let n = |s: &str| s.parse::<u64>().unwrap_or(0);
        if sessions.last().is_none_or(|s| s["name"] != f[0]) {
            sessions.push(json!({ "name": f[0], "windows": [] }));
        }
        let windows = sessions.last_mut().unwrap()["windows"].as_array_mut().unwrap();
        if windows.last().is_none_or(|w| w["index"] != n(f[1])) {
            windows.push(json!({
                "index": n(f[1]), "name": f[2], "auto": f[3] == "1", "layout": f[4], "active": f[5] == "1",
                "width": n(f[6]), "height": n(f[7]), "panes": [],
            }));
        }
        let command = f[11].parse::<u32>().ok().and_then(crate::programs::resumed);
        windows.last_mut().unwrap()["panes"].as_array_mut().unwrap().push(json!({ "index": n(f[8]), "active": f[9] == "1", "dir": f[10], "command": command }));
    }
    (!sessions.is_empty()).then(|| json!({ "version": 1, "sessions": sessions }))
}

/// `draftpaper tmux save [SOCKET]`
pub fn save(p: &Paths, socket: &str) -> bool {
    let Some(v) = now(socket) else { return false };
    let text = util::dumps_indent(&v, 2);
    if util::read(&file(p)) != text {
        util::write_atomic(&file(p), &text);
        let _ = std::fs::set_permissions(file(p), std::os::unix::fs::PermissionsExt::from_mode(0o600));
    }
    true
}

/// `draftpaper tmux keep SOCKET`: saves while the server lives; one per server.
pub fn keep(p: &Paths, socket: &str) {
    let lock = p.runtime.join(format!("tmux-keep-{}", socket.replace('/', "%")));
    let _ = std::fs::create_dir_all(&p.runtime);
    if let Some(pid) = util::read(&lock).trim().parse::<u32>().ok().filter(|pid| *pid != std::process::id()) {
        let alive = std::fs::read_to_string(format!("/proc/{pid}/cmdline")).is_ok_and(|c| c.contains("tmux\0keep"));
        if alive {
            return;
        }
    }
    let _ = std::fs::write(&lock, std::process::id().to_string());
    // the first save after the restore has had its time
    std::thread::sleep(EVERY);
    while tmux(socket, &["has-session"]).is_some() {
        save(p, socket);
        std::thread::sleep(EVERY);
    }
    let _ = std::fs::remove_file(&lock);
}

/// A word as both POSIX shells and fish read it; None when it cannot be said simply. The words are typed
/// into the shell (send-keys), so a control character (in a file name) would be a key: Ctrl+C and Enter
/// would run what follows. Such a word is none: the pane gets its shell alone.
fn quote(a: &str) -> Option<String> {
    if a.chars().any(char::is_control) {
        None
    } else if !a.is_empty() && a.chars().all(|c| c.is_ascii_alphanumeric() || "_./=:,+@%-".contains(c)) {
        Some(a.to_string())
    } else if !a.contains('\'') && !a.contains('\\') {
        Some(format!("'{a}'"))
    } else {
        None
    }
}

/// `draftpaper tmux restore SOCKET`: the saved sessions made again, once per server; those of a name the
/// server has already are left.
pub fn restore(p: &Paths, socket: &str) {
    if tmux(socket, &["show", "-gqv", "@draftpaper-restored"]).is_some_and(|v| v.trim() == "1") {
        return;
    }
    let _ = tmux(socket, &["set", "-g", "@draftpaper-restored", "1"]);
    let Ok(v) = serde_json::from_str::<Value>(&util::read(&file(p))) else { return };
    let have: Vec<String> = tmux(socket, &["list-sessions", "-F", "#{session_name}"]).unwrap_or_default().lines().map(String::from).collect();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    for s in v["sessions"].as_array().into_iter().flatten() {
        let Some(name) = s["name"].as_str().filter(|n| !have.iter().any(|h| h == n)) else { continue };
        let mut active = None;
        for (i, w) in s["windows"].as_array().into_iter().flatten().enumerate() {
            let index = w["index"].as_u64().unwrap_or(0).to_string();
            let panes: Vec<&Value> = w["panes"].as_array().into_iter().flatten().collect();
            let dir = |p: &Value| p["dir"].as_str().filter(|d| Path::new(d).is_dir()).unwrap_or(&home).to_string();
            let first = panes.first().map(|p| dir(p)).unwrap_or_else(|| home.clone());
            let target = format!("={name}:{index}");
            let mut args: Vec<String> = if i == 0 {
                let (wd, ht) = (w["width"].as_u64().unwrap_or(80).to_string(), w["height"].as_u64().unwrap_or(24).to_string());
                ["new-session", "-d", "-s", name, "-c", &first, "-x", &wd, "-y", &ht, "-P", "-F", "#{window_index}"].map(String::from).to_vec()
            } else {
                ["new-window", "-d", "-t", &target, "-c", &first, "-P", "-F", "#{window_index}"].map(String::from).to_vec()
            };
            if !w["auto"].as_bool().unwrap_or(true) {
                args.extend(["-n".to_string(), w["name"].as_str().unwrap_or_default().to_string()]);
            }
            let Some(got) = tmux(socket, &args.iter().map(String::as_str).collect::<Vec<_>>()) else { continue };
            if got.trim() != index {
                let _ = tmux(socket, &["move-window", "-s", &format!("={name}:{}", got.trim()), "-t", &target]);
            }
            for p in panes.iter().skip(1) {
                let _ = tmux(socket, &["split-window", "-d", "-t", &target, "-c", &dir(p)]);
            }
            if let Some(layout) = w["layout"].as_str() {
                let _ = tmux(socket, &["select-layout", "-t", &target, layout]);
            }
            // each pane's program again, typed into its shell (the shell stays when it ends)
            let indices = tmux(socket, &["list-panes", "-t", &target, "-F", "#{pane_index}"]).unwrap_or_default();
            for (p, real) in panes.iter().zip(indices.lines()) {
                let pane = format!("{target}.{real}");
                let words: Option<Vec<String>> = p["command"].as_array().map(|c| c.iter().filter_map(Value::as_str).map(quote).collect::<Option<Vec<_>>>()).unwrap_or(None);
                if let Some(words) = words.filter(|w| !w.is_empty()) {
                    let _ = tmux(socket, &["send-keys", "-t", &pane, "-l", &words.join(" ")]);
                    let _ = tmux(socket, &["send-keys", "-t", &pane, "Enter"]);
                }
                if p["active"].as_bool() == Some(true) {
                    let _ = tmux(socket, &["select-pane", "-t", &pane]);
                }
            }
            if w["active"].as_bool() == Some(true) {
                active = Some(target);
            }
        }
        if let Some(t) = active {
            let _ = tmux(socket, &["select-window", "-t", &t]);
        }
    }
}

/// `draftpaper tmux save|keep|restore [SOCKET]` (the socket: tmux's #{socket_path}; else $TMUX's)
pub fn command(p: &Paths, args: &[&str]) {
    let socket = args.get(1).map(|s| s.to_string()).or_else(|| std::env::var("TMUX").ok().and_then(|t| t.split(',').next().map(String::from))).filter(|s| !s.is_empty());
    let Some(socket) = socket else { util::die("draftpaper: tmux: no server (run inside tmux, or give its socket)") };
    match args.first() {
        Some(&"save") => {
            if !save(p, &socket) {
                util::die("draftpaper: tmux: no sessions to save");
            }
        }
        Some(&"keep") => keep(p, &socket),
        Some(&"restore") => restore(p, &socket),
        _ => util::die("draftpaper tmux save|keep|restore [SOCKET]"),
    }
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn words_typed_back() {
        assert_eq!(quote("notes.md").as_deref(), Some("notes.md"));
        assert_eq!(quote("my notes.md").as_deref(), Some("'my notes.md'"));
        assert_eq!(quote("$(rm -rf ~)").as_deref(), Some("'$(rm -rf ~)'"));
        assert_eq!(quote("it's"), None);
        // a control character would be a key in the shell: Ctrl+C, then a command and Enter
        assert_eq!(quote("x\u{3}touch pwned\r"), None);
        assert_eq!(quote("a\nb"), None);
        assert_eq!(quote("a\u{1b}[A"), None);
    }
}
