//! Session restore (#38): at login, the applications open at the last logout are started again, and
//! each window goes back where it was: its screen, its workspace and its zones, floating, fullscreen or
//! parked in its cell. Terminals start in the directory their shell was in, running again the program
//! they ran if one of RESUMED (an editor, a pager, a monitor, a multiplexer); an application started
//! with documents opens them again.
//!
//! Saving: every PERIOD the session is read: each window's app id, title and place, and its process's
//! command line and shell directory. It is written to $XDG_STATE_HOME/draftpaper/session.json once it
//! has held for a whole period. A logout or a shutdown ends the applications as it ends the compositor:
//! the windows that close on the way out never stay long enough to be written. `quit` (Mod+Shift+e)
//! writes the session as it is first; so does the action `session_save`.
//!
//! Restoring (config.toml's [session] restore, true by default): each process the session had is started
//! again in its shell's directory, by its desktop entry, else by its command line. The entry is found by
//! the window's app id: the file's name, StartupWMClass, then the program. X11 processes start once
//! Xwayland is up. A window that maps takes the place of a saved window of the same app id: one of its own
//! process first, then one of the same title, then in order. Whatever has not come back after FORGET is
//! forgotten; its zones stay paper. Nothing is written until then. The opening (opening.rs) waits for the
//! restored windows up to RESTORE_WAIT.
//!
//! The applications take up their own state: `draftpaper apply` switches it on in each (Firefox, Chromium,
//! Emacs, Neovim, tmux; cli/src/restore.rs), as config.toml's [session] apps says.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use smithay::desktop::Window;

use crate::{layout::Zones, Draftpaper};

/// How often the session is read; it is written once it has held for this long.
pub const PERIOD: Duration = Duration::from_secs(2);
/// How long the windows of a restored session are waited for.
const FORGET: Duration = Duration::from_secs(60);
/// How long the opening waits for them.
const RESTORE_WAIT: Duration = Duration::from_secs(3);

/// The shells whose directory a terminal reopens in.
const SHELLS: &[&str] = &["bash", "zsh", "fish", "sh", "dash", "ksh", "mksh", "tcsh", "nu", "elvish", "xonsh"];

/// The programs a terminal runs again when it comes back: editors, pagers, file managers, monitors,
/// multiplexers. Programs that change things (a build, a script, ssh) never run again on their own. The
/// CLI keeps the same list for tmux's panes (cli/src/programs.rs), multiplexers aside.
const RESUMED: &[&str] = &[
    "nvim", "vim", "vi", "view", "nano", "micro", "hx", "helix", "kak", "emacs", "mg", "less", "more", "man", "htop", "btop", "top", "ranger", "lf",
    "nnn", "yazi", "mc", "ncdu", "cmus", "tmux", "screen", "zellij",
];

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect()
}

#[derive(Default)]
pub struct Session {
    /// saving and restoring at all: a login session (--session), or DRAFTPAPER_SESSION=1 (tests)
    pub on: bool,
    /// what the session was at the last reading, and what was last written
    read: Option<String>,
    written: Option<String>,
    restore: Option<Restore>,
}

struct Restore {
    since: Instant,
    slots: Vec<Slot>,
    /// the processes started: pid, index in the file
    pids: Vec<(u32, usize)>,
    /// X11 processes, waiting for Xwayland
    x11: Vec<Process>,
}

#[derive(Debug, Clone)]
struct Slot {
    app_id: String,
    title: String,
    process: usize,
    output: String,
    workspace: u32,
    place: Place,
    fullscreen: bool,
    focused: bool,
}

#[derive(Debug, Clone)]
enum Place {
    Tiled(Zones, (u32, u32)),
    Floating,
    Parked(usize, Option<(Zones, (u32, u32))>),
}

#[derive(Debug, Clone)]
struct Process {
    index: usize,
    app_id: String,
    command: Vec<String>,
    dir: Option<String>,
    x11: bool,
    /// a terminal's: its shell, and the program the shell ran
    shell: Option<String>,
    program: Vec<String>,
    /// the documents it was started with
    files: Vec<String>,
}

/// $XDG_STATE_HOME/draftpaper/session.json
pub fn path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    state.join("draftpaper/session.json")
}

fn write(text: &str) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = path();
    let Some(dir) = path.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    let tmp = dir.join(format!(".session.json.{}", std::process::id()));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .and_then(|_| std::fs::rename(&tmp, &path));
    if let Err(e) = written {
        eprintln!("draftpaper: session: {}: {e}", path.display());
        let _ = std::fs::remove_file(&tmp);
    }
}

/// The process of a window: its client's (Wayland), or the one X11 names.
pub(crate) fn pid_of(state: &Draftpaper, window: &Window) -> Option<u32> {
    use smithay::{reexports::wayland_server::Resource, wayland::seat::WaylandFocus};
    if let Some(x) = window.x11_surface() {
        return x.pid();
    }
    let client = window.wl_surface()?.client()?;
    client.get_credentials(&state.display_handle).ok().map(|c| c.pid as u32)
}

/// A window with a parent (a dialog): it comes with its application, not on its own.
fn is_dialog(window: &Window) -> bool {
    window.toplevel().is_some_and(|t| t.parent().is_some()) || window.x11_surface().is_some_and(|x| x.is_transient_for().is_some())
}

fn command_line(pid: u32) -> Vec<String> {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| b.split(|c| *c == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect())
        .unwrap_or_default()
}

fn children(pid: u32) -> Vec<u32> {
    std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap_or_default().split_whitespace().filter_map(|c| c.parse().ok()).collect()
}

/// A terminal's shell (the first among the children of `pid`): its directory, its program, and the
/// program it runs if one to run again (RESUMED).
fn shell_of(pid: u32) -> Option<(String, Option<String>, Option<Vec<String>>)> {
    children(pid).into_iter().find_map(|c| {
        let comm = std::fs::read_to_string(format!("/proc/{c}/comm")).ok()?;
        if !SHELLS.contains(&comm.trim()) {
            return None;
        }
        let dir = std::fs::read_link(format!("/proc/{c}/cwd")).ok()?.to_string_lossy().into_owned();
        let exe = std::fs::read_link(format!("/proc/{c}/exe")).ok().map(|e| e.to_string_lossy().into_owned());
        let program = children(c).into_iter().map(command_line).find(|a| resumable(a));
        Some((dir, exe, program))
    })
}

/// Whether a command line is one a terminal runs again: an editor, a pager, a monitor, a multiplexer;
/// Emacs only in the terminal (a graphical one comes back on its own).
fn resumable(argv: &[String]) -> bool {
    let Some(program) = argv.first().map(|a| a.rsplit('/').next().unwrap_or(a)) else { return false };
    RESUMED.contains(&program) && (program != "emacs" || argv.iter().any(|a| a == "-nw" || a == "--no-window-system" || a == "-t" || a == "--tty"))
}

/// The documents a process was started with: its arguments that are files of the user's (in the home,
/// or on a mounted disk; not hidden, not the program's own), as absolute paths.
fn documents(pid: u32, argv: &[String]) -> Vec<String> {
    // a relative name: from the directory of the process, else of a child (a terminal leaves its own)
    let cwd = |p: u32| std::fs::read_link(format!("/proc/{p}/cwd")).ok();
    let dirs: Vec<PathBuf> = std::iter::once(pid).chain(children(pid)).filter_map(cwd).collect();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let places = [home.clone(), "/tmp".into(), "/media".into(), "/mnt".into(), "/run/media".into(), "/srv".into()];
    argv.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-') && !a.contains("://"))
        .filter_map(|a| dirs.iter().map(|d| d.join(a)).find(|p| p.is_file()))
        .filter(|p| {
            p.is_absolute()
                && p.is_file()
                && places.iter().any(|d| d.as_os_str().len() > 1 && p.starts_with(d))
                && !p.strip_prefix(&home).unwrap_or(p).components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// How a terminal is told to run a command (its arguments before the command), by its program.
fn terminal_exec(program: &str) -> Option<&'static [&'static str]> {
    Some(match program.rsplit('/').next().unwrap_or(program) {
        "alacritty" | "xterm" | "uxterm" | "urxvt" | "st" | "ghostty" | "konsole" | "terminator" => &["-e"],
        "foot" | "footclient" | "kitty" => &[],
        "wezterm" => &["start", "--"],
        "gnome-terminal" | "kgx" | "ptyxis" | "xfce4-terminal" => &["--"],
        _ => return None,
    })
}

impl Draftpaper {
    /// The session as it is now (the file's contents).
    fn session_now(&self) -> Value {
        let focused = self.focused_window();
        let mut processes: Vec<(Option<u32>, Value)> = Vec::new();
        let mut windows = Vec::new();
        for s in &self.screens {
            for w in &s.workspaces {
                let sheet = w.layout.sheet();
                let mut all: Vec<(&Window, Value)> = Vec::new();
                all.extend(w.layout.tiles().map(|(x, z)| (x, json!({ "tiled": z, "sheet": sheet }))));
                all.extend(w.floating.iter().map(|x| (x, json!({ "floating": true }))));
                all.extend(w.parked.iter().map(|p| match p.zones() {
                    Some((z, sheet)) => (&p.window, json!({ "parked": p.cell, "tiled": z, "sheet": sheet })),
                    None => (&p.window, json!({ "parked": p.cell })),
                }));
                for (window, mut place) in all {
                    let (app_id, title, x11) = crate::ipc::names(window);
                    if app_id.is_empty() || is_dialog(window) || self.config.session.skip.iter().any(|a| a.eq_ignore_ascii_case(&app_id)) {
                        continue;
                    }
                    // one entry per process: a browser's windows come back with one start
                    let pid = pid_of(self, window);
                    let process = match processes.iter().position(|(p, _)| pid.is_some() && *p == pid) {
                        Some(k) => k,
                        None => {
                            let command = pid.map(command_line).unwrap_or_default();
                            let shell = pid.and_then(shell_of);
                            let files = pid.map(|p| documents(p, &command)).unwrap_or_default();
                            let (dir, sh, program) = match shell {
                                Some((d, s, p)) => (Some(d), s, p),
                                None => (None, None, None),
                            };
                            processes.push((pid, json!({ "app_id": app_id, "command": command, "dir": dir, "x11": x11, "shell": sh, "program": program, "files": files })));
                            processes.len() - 1
                        }
                    };
                    let p = place.as_object_mut().unwrap();
                    p.insert("app_id".into(), json!(app_id));
                    p.insert("title".into(), json!(title));
                    p.insert("process".into(), json!(process));
                    p.insert("output".into(), json!(s.output.name()));
                    p.insert("workspace".into(), json!(w.number));
                    p.insert("fullscreen".into(), json!(w.fullscreen.as_ref() == Some(window)));
                    p.insert("focused".into(), json!(focused.as_ref() == Some(window)));
                    windows.push(place);
                }
            }
        }
        let screens: Vec<Value> = self.screens.iter().map(|s| json!({ "output": s.output.name(), "workspace": s.workspace().number })).collect();
        let active = self.screens.get(self.active).map(|s| s.output.name());
        json!({
            "version": 1,
            "screens": screens,
            "active": active,
            "processes": processes.into_iter().map(|(_, p)| p).collect::<Vec<_>>(),
            "windows": windows,
        })
    }

    /// Every PERIOD: the session written when it has held since the last reading (and changed).
    pub fn session_tick(&mut self) {
        if !self.saved.on {
            return;
        }
        if let Some(r) = &self.saved.restore {
            if r.since.elapsed() < FORGET && !(r.slots.is_empty() && r.x11.is_empty()) {
                return;
            }
            if !r.slots.is_empty() {
                let left: Vec<&str> = r.slots.iter().map(|s| s.app_id.as_str()).collect();
                eprintln!("draftpaper: session: not come back, forgotten: {}", left.join(", "));
            }
            self.saved.restore = None;
        }
        let now = serde_json::to_string_pretty(&self.session_now()).unwrap_or_default();
        if self.saved.read.as_ref() == Some(&now) && self.saved.written.as_ref() != Some(&now) {
            write(&now);
            self.saved.written = Some(now.clone());
        }
        self.saved.read = Some(now);
    }

    /// Write the session as it is now (`quit`, `session_save`); not while one is being restored (it
    /// would be written half back) unless `anyway`.
    pub fn session_save(&mut self, anyway: bool) {
        if !self.saved.on || (self.saved.restore.is_some() && !anyway) {
            return;
        }
        let now = serde_json::to_string_pretty(&self.session_now()).unwrap_or_default();
        write(&now);
        self.saved.read = Some(now.clone());
        self.saved.written = Some(now);
    }

    /// The restored windows are back, or have been waited for long enough (the opening's reveal).
    pub fn session_settled(&self) -> bool {
        self.saved.restore.as_ref().is_none_or(|r| r.slots.is_empty() || r.since.elapsed() >= RESTORE_WAIT)
    }

    /// At login: the session file read, its processes started, its workspaces shown.
    pub fn session_restore(&mut self) {
        if !self.saved.on || !self.config.session.restore {
            return;
        }
        let Ok(text) = std::fs::read_to_string(path()) else { return };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            eprintln!("draftpaper: session: {} does not parse", path().display());
            return;
        };
        let zones = |v: &Value| -> Option<(Zones, (u32, u32))> {
            let n = |v: &Value| v.as_u64().map(|n| n as u32);
            let t = v.get("tiled")?;
            let z = [[n(&t[0][0])?, n(&t[0][1])?], [n(&t[1][0])?, n(&t[1][1])?]];
            let s = v.get("sheet")?;
            Some((z, (n(&s[0])?, n(&s[1])?)))
        };
        let skip = &self.config.session.skip;
        let slots: Vec<Slot> = v["windows"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                let place = match (w.get("parked").and_then(Value::as_u64), zones(w)) {
                    (Some(cell), z) => Place::Parked(cell as usize, z),
                    (None, Some((z, sheet))) => Place::Tiled(z, sheet),
                    (None, None) => Place::Floating,
                };
                Some(Slot {
                    app_id: w["app_id"].as_str()?.to_string(),
                    title: w["title"].as_str().unwrap_or_default().to_string(),
                    process: w["process"].as_u64()? as usize,
                    output: w["output"].as_str().unwrap_or_default().to_string(),
                    workspace: w["workspace"].as_u64().unwrap_or(1) as u32,
                    place,
                    fullscreen: w["fullscreen"].as_bool().unwrap_or(false),
                    focused: w["focused"].as_bool().unwrap_or(false),
                })
            })
            .filter(|s| !skip.iter().any(|a| a.eq_ignore_ascii_case(&s.app_id)))
            .collect();
        let processes: Vec<Process> = v["processes"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(index, _)| slots.iter().any(|s| s.process == *index))
            .map(|(index, p)| Process {
                index,
                app_id: p["app_id"].as_str().unwrap_or_default().to_string(),
                command: p["command"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect(),
                dir: p["dir"].as_str().map(str::to_string),
                x11: p["x11"].as_bool().unwrap_or(false),
                shell: p["shell"].as_str().map(str::to_string),
                program: strings(&p["program"]),
                files: strings(&p["files"]),
            })
            .collect();
        if slots.is_empty() {
            return;
        }
        eprintln!("draftpaper: session: restoring {} windows of {} processes", slots.len(), processes.len());

        // the workspaces shown, each on its screen, and the screen active
        for s in v["screens"].as_array().into_iter().flatten() {
            let (Some(output), Some(number)) = (s["output"].as_str(), s["workspace"].as_u64()) else { continue };
            if let Some(i) = self.screens.iter().position(|x| x.output.name() == output) {
                if self.screens[i].workspace().number != number as u32 && !self.screens.iter().any(|x| x.workspaces.iter().any(|w| w.number == number as u32)) {
                    self.active = i;
                    self.switch_to(number as u32);
                }
            }
        }
        if let Some(i) = v["active"].as_str().and_then(|a| self.screens.iter().position(|x| x.output.name() == a)) {
            self.active = i;
        }

        let (x11, wayland): (Vec<Process>, Vec<Process>) = processes.into_iter().partition(|p| p.x11);
        self.saved.restore = Some(Restore { since: Instant::now(), slots, pids: Vec::new(), x11: if self.xwm.is_some() { Vec::new() } else { x11.clone() } });
        let now: Vec<Process> = if self.xwm.is_some() { wayland.into_iter().chain(x11).collect() } else { wayland };
        for p in now {
            self.session_start(&p);
        }
    }

    /// Xwayland is up: the X11 processes of a session being restored.
    pub fn session_xwayland_ready(&mut self) {
        let Some(r) = self.saved.restore.as_mut() else { return };
        for p in std::mem::take(&mut r.x11) {
            self.session_start(&p);
        }
    }

    /// Start a process of the session again: its desktop entry, else its command line, in its shell's
    /// directory (else the home).
    fn session_start(&mut self, p: &Process) {
        let argv = desktop_command(&p.app_id, &p.files).or_else(|| (!p.command.is_empty()).then(|| p.command.clone()));
        // a terminal runs its program again, in its shell (which stays when the program ends): unless its
        // command runs one of its own already
        let argv = argv.map(|mut argv| {
            if let (Some(shell), false, Some(exec)) = (&p.shell, p.program.is_empty(), argv.first().and_then(|a| terminal_exec(a))) {
                // (an argument not an option may be a command, or an option's value: either way, left as it is)
                if !argv.iter().skip(1).any(|a| a == "-e" || a == "--" || a == "-x" || !a.starts_with('-')) {
                    argv.extend(exec.iter().map(|s| s.to_string()));
                    argv.extend(["sh".into(), "-c".into(), "\"$@\"; exec \"$0\"".into(), shell.clone()]);
                    argv.extend(p.program.iter().cloned());
                }
            }
            argv
        });
        let Some(argv) = argv else {
            eprintln!("draftpaper: session: nothing to start {} with", p.app_id);
            return;
        };
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into());
        let dir = p.dir.as_ref().map(PathBuf::from).filter(|d| d.is_dir()).unwrap_or(home);
        let mut command = std::process::Command::new(&argv[0]);
        command.args(&argv[1..]).current_dir(&dir).env("WAYLAND_DISPLAY", &self.socket_name).stdin(std::process::Stdio::null());
        match command.spawn() {
            Ok(mut child) => {
                eprintln!("draftpaper: session: started {} ({}) in {}", p.app_id, argv.join(" "), dir.display());
                if let Some(r) = self.saved.restore.as_mut() {
                    r.pids.push((child.id(), p.index));
                }
                // reaped when it ends
                std::thread::spawn(move || child.wait());
            }
            Err(e) => eprintln!("draftpaper: session: {}: {e}", argv[0]),
        }
    }

    /// A window that maps while a session is restored: the saved window whose place it takes (of the
    /// same app id: its own process's first, then the same title, then the first).
    fn session_claim(&mut self, window: &Window) -> Option<Slot> {
        if self.saved.restore.as_ref()?.slots.is_empty() || is_dialog(window) {
            return None;
        }
        let (app_id, title, _) = crate::ipc::names(window);
        let pid = pid_of(self, window);
        let r = self.saved.restore.as_mut()?;
        let process = r.pids.iter().find(|(p, _)| Some(*p) == pid).map(|(_, k)| *k);
        let candidates: Vec<usize> = (0..r.slots.len()).filter(|&k| r.slots[k].app_id == app_id).collect();
        let k = candidates
            .iter()
            .copied()
            .max_by_key(|&k| (process == Some(r.slots[k].process), r.slots[k].title == title, std::cmp::Reverse(k)))?;
        Some(r.slots.remove(k))
    }

    /// Place a new window where a restored session had it, when it is one of that session's (see
    /// session_claim): true when placed.
    pub fn session_place(&mut self, window: &Window, float: bool, d: &crate::rules::Decision) -> bool {
        let Some(slot) = self.session_claim(window) else { return false };
        if self.screens.is_empty() {
            return false;
        }
        // its workspace, where its number is, else new on its screen (or the active one)
        let at = self.screens.iter().enumerate().find_map(|(i, s)| s.workspaces.iter().position(|w| w.number == slot.workspace).map(|k| (i, k)));
        let (i, k) = at.unwrap_or_else(|| {
            let i = self.screens.iter().position(|s| s.output.name() == slot.output).unwrap_or(self.active);
            let s = &mut self.screens[i];
            let current = s.workspace().number;
            s.workspaces.push(crate::screens::Workspace::new(slot.workspace));
            s.workspaces.sort_by_key(|w| w.number);
            s.current = s.workspaces.iter().position(|w| w.number == current).unwrap_or(0);
            (i, s.workspaces.iter().position(|w| w.number == slot.workspace).unwrap())
        });
        if float && d.sticky {
            window.user_data().insert_if_missing(|| crate::rules::Sticky);
        }
        if float && d.place == crate::rules::Place::Corner {
            window.user_data().insert_if_missing(|| crate::rules::Cornered);
        }
        let grid = self.screens[i].grid;
        let ws = &mut self.screens[i].workspaces[k];
        let parked = matches!(slot.place, Place::Parked(..));
        match slot.place {
            Place::Parked(cell, zones) => ws.parked.push(crate::parking::Parked::restored(window.clone(), cell, zones)),
            Place::Tiled(z, sheet) if !float => {
                if !ws.layout.insert_at(window.clone(), z, sheet, &grid) && !ws.layout.add(window.clone(), &grid) {
                    ws.floating.push(window.clone());
                }
            }
            _ => ws.floating.push(window.clone()),
        }
        if slot.fullscreen && !parked {
            ws.fullscreen = Some(window.clone());
        }
        self.relayout();
        let shown = self.screens[i].current == k;
        if shown && !parked && d.focus && (slot.focused || self.focused_window().is_none()) {
            self.active = i;
            self.focus(Some(window.clone()));
        }
        true
    }
}

/// The command of the desktop entry of `app_id` (field codes left out): the entry of that name, else
/// the one whose StartupWMClass it is, else one whose name or program ends as it does. The user's
/// entries over the system's.
pub fn desktop_command(app_id: &str, files: &[String]) -> Option<Vec<String>> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".local/share"));
    let data_dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut entries: Vec<(String, Entry)> = Vec::new();
    for dir in std::iter::once(data_home).chain(data_dirs.split(':').map(PathBuf::from)) {
        let Ok(read) = std::fs::read_dir(dir.join("applications")) else { continue };
        let mut files: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "desktop")).collect();
        files.sort();
        for file in files {
            let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            if entries.iter().any(|(s, _)| *s == stem) {
                continue;
            }
            if let Some(e) = std::fs::read_to_string(&file).ok().and_then(|t| Entry::parse(&t)) {
                entries.push((stem, e));
            }
        }
    }
    let usable: Vec<&(String, Entry)> = entries.iter().filter(|(_, e)| e.usable()).collect();
    let id = app_id.to_lowercase();
    let last = |s: &str| s.rsplit('.').next().unwrap_or(s).to_lowercase();
    let program = |e: &Entry| e.command().first().map(|p| p.rsplit('/').next().unwrap_or(p).to_lowercase());
    let found = usable
        .iter()
        .find(|(stem, _)| stem.to_lowercase() == id)
        .or_else(|| usable.iter().find(|(_, e)| e.wm_class.as_ref().is_some_and(|c| c.to_lowercase() == id)))
        .or_else(|| usable.iter().find(|(stem, _)| last(stem) == last(&id)))
        .or_else(|| usable.iter().find(|(_, e)| program(e).is_some_and(|p| p == id || p == last(&id))))?;
    let command = found.1.command_with(files);
    (!command.is_empty()).then_some(command)
}

/// The sessions a display manager would offer, for the login screen to choose from (qml/greeter.qml):
/// the entries in wayland-sessions under each data directory, the first of a name winning, as
/// {"id", "name", "command"}. Left out: Hidden and NoDisplay entries, and those whose TryExec is not
/// installed (a session whose program was removed with its package).
pub fn wayland_sessions() -> Vec<Value> {
    let data_dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut seen: Vec<String> = Vec::new();
    let mut sessions = Vec::new();
    for dir in data_dirs.split(':').filter(|d| !d.is_empty()) {
        let Ok(read) = std::fs::read_dir(PathBuf::from(dir).join("wayland-sessions")) else { continue };
        let mut files: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "desktop")).collect();
        files.sort();
        for file in files {
            let id = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            if seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());
            let Some(e) = std::fs::read_to_string(&file).ok().and_then(|t| Entry::parse(&t)) else { continue };
            if e.hidden || e.no_display || e.try_exec.as_deref().is_some_and(|p| !installed(p)) {
                continue;
            }
            let command = e.command();
            if command.is_empty() {
                continue;
            }
            sessions.push(json!({ "id": id, "name": e.name.clone().unwrap_or_else(|| id.clone()), "command": command }));
        }
    }
    sessions
}

/// A program as TryExec names it: a path that is an executable file, or a name found on PATH.
fn installed(program: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let executable = |p: &std::path::Path| p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    if program.contains('/') {
        return executable(std::path::Path::new(program));
    }
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| executable(&d.join(program))))
}

#[derive(Debug, Default)]
struct Entry {
    exec: String,
    name: Option<String>,
    try_exec: Option<String>,
    wm_class: Option<String>,
    application: bool,
    hidden: bool,
    no_display: bool,
    terminal: bool,
}

impl Entry {
    /// The [Desktop Entry] group of a .desktop file.
    fn parse(text: &str) -> Option<Entry> {
        let mut e = Entry::default();
        let mut inside = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                inside = line == "[Desktop Entry]";
                continue;
            }
            let Some((k, v)) = line.split_once('=').filter(|_| inside) else { continue };
            let v = v.trim();
            match k.trim() {
                "Exec" => e.exec = v.to_string(),
                // the untranslated name (Name[fr] and the like are other keys)
                "Name" => e.name = Some(v.to_string()),
                "TryExec" => e.try_exec = Some(v.to_string()),
                "NoDisplay" => e.no_display = v == "true",
                "StartupWMClass" => e.wm_class = Some(v.to_string()),
                "Type" => e.application = v == "Application",
                "Hidden" => e.hidden = v == "true",
                "Terminal" => e.terminal = v == "true",
                _ => {}
            }
        }
        (!e.exec.is_empty()).then_some(e)
    }

    fn usable(&self) -> bool {
        self.application && !self.hidden && !self.terminal
    }

    /// Exec split into arguments (double quotes, backslash escapes inside them), field codes left out.
    fn command(&self) -> Vec<String> {
        self.command_with(&[])
    }

    /// The command with `files` for its field codes: %f and %u the first, %F and %U all.
    fn command_with(&self, files: &[String]) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        let (mut arg, mut quoted, mut started) = (String::new(), false, false);
        let mut chars = self.exec.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    quoted = !quoted;
                    started = true;
                }
                '\\' if quoted => arg.extend(chars.next()),
                ' ' | '\t' if !quoted => {
                    if started || !arg.is_empty() {
                        args.push(std::mem::take(&mut arg));
                    }
                    started = false;
                }
                c => arg.push(c),
            }
        }
        if started || !arg.is_empty() {
            args.push(arg);
        }
        args.into_iter()
            .flat_map(|a| -> Vec<String> {
                // a field code alone is the files (or goes); in an argument, %% is a %, any other code goes
                match a.as_str() {
                    "%f" | "%u" => return files.iter().take(1).cloned().collect(),
                    "%F" | "%U" => return files.to_vec(),
                    "%%" => return vec!["%".into()],
                    _ if a.len() == 2 && a.starts_with('%') => return Vec::new(),
                    _ => {}
                }
                let mut out = String::new();
                let mut cs = a.chars();
                while let Some(c) = cs.next() {
                    if c == '%' {
                        if cs.next() == Some('%') {
                            out.push('%');
                        }
                    } else {
                        out.push(c);
                    }
                }
                vec![out]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_lines() {
        let e = |exec: &str| Entry { exec: exec.into(), application: true, ..Default::default() }.command();
        assert_eq!(e("firefox-esr %u"), ["firefox-esr"]);
        assert_eq!(e("foot --app-id=drafttest"), ["foot", "--app-id=drafttest"]);
        assert_eq!(e(r#"sh -c "echo \"a b\"; x" %F"#), ["sh", "-c", r#"echo "a b"; x"#]);
        assert_eq!(e("prog --rate=50%% --icon=%i"), ["prog", "--rate=50%", "--icon="]);
        assert_eq!(e(r#"prog "" end"#), ["prog", "", "end"]);
    }

    #[test]
    fn exec_with_files() {
        let e = |exec: &str, f: &[&str]| Entry { exec: exec.into(), application: true, ..Default::default() }.command_with(&f.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(e("evince %U", &["/h/a.pdf", "/h/b.pdf"]), ["evince", "/h/a.pdf", "/h/b.pdf"]);
        assert_eq!(e("gimp %f", &["/h/a.png", "/h/b.png"]), ["gimp", "/h/a.png"]);
        assert_eq!(e("viewer %F --x", &[]), ["viewer", "--x"]);
        assert_eq!(e("prog %%", &[]), ["prog", "%"]);
    }

    #[test]
    fn which_come_back() {
        let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert!(resumable(&v("nvim notes.txt")));
        assert!(resumable(&v("/usr/bin/tmux a")));
        assert!(resumable(&v("emacs -nw x.el")));
        assert!(!resumable(&v("emacs x.el")));
        assert!(!resumable(&v("make deploy")));
        assert!(!resumable(&v("ssh host")));
    }

    #[test]
    fn entries() {
        let e = Entry::parse("[Desktop Entry]\nType=Application\nExec=alacritty\nStartupWMClass=Alacritty\n[Desktop Action New]\nExec=other\n").unwrap();
        assert_eq!(e.exec, "alacritty");
        assert_eq!(e.wm_class.as_deref(), Some("Alacritty"));
        assert!(e.usable());
        assert!(!Entry::parse("[Desktop Entry]\nType=Application\nExec=htop\nTerminal=true\n").unwrap().usable());
        assert!(Entry::parse("[Desktop Entry]\nType=Application\n").is_none());
    }

    #[test]
    fn session_entries() {
        let e = Entry::parse("[Desktop Entry]\nName=Tom's WM (Wayland)\nName[fr]=Le WM de Tom\nExec=twm-session\nTryExec=twm\nType=Application\n").unwrap();
        assert_eq!(e.name.as_deref(), Some("Tom's WM (Wayland)"));
        assert_eq!(e.try_exec.as_deref(), Some("twm"));
        assert_eq!(e.command(), ["twm-session"]);
        assert!(Entry::parse("[Desktop Entry]\nExec=x\nNoDisplay=true\n").unwrap().no_display);
        assert!(installed("sh") && installed("/bin/sh"));
        assert!(!installed("no-such-program-anywhere") && !installed("/no/such/program"));
    }
}
