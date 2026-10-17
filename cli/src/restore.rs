//! The applications as they were left (config.toml [session] apps, true by default): the compositor
//! starts them again at login (compositor/src/session.rs); `draftpaper apply` has each one take up its
//! own state, in files Draftpaper owns and a line in the application's config where it needs one.
//!
//!   Firefox   its windows and tabs (browser.startup.page = 3, a marked block in each profile's user.js)
//!   Chromium  its windows and tabs (session.restore_on_startup = 1 in Preferences, while it is not running)
//!   Emacs     the files open, each at its place, its windows (desktop-save-mode, save-place-mode):
//!             ~/.config/draftpaper/emacs/draftpaper-session.el, loaded by a line added to init.el
//!   Neovim    a file's cursor where it was; started with no file, the session it last had in that
//!             directory: nvim/plugin/draftpaper-session.lua
//!   tmux      its sessions, windows and panes across a reboot (tmux.rs): tmux/draftpaper-session.conf,
//!             sourced by a line added to tmux.conf
//!
//! `apps = false`, or `restore = false`, takes the files back out; [session] skip leaves an application
//! out by name (firefox, chromium, emacs, neovim, tmux).

use std::path::Path;

use regex::Regex;
use serde_json::{Map, Value};

use crate::apps::ensure_line;
use crate::util::{self, Paths};

const FIREFOX_BEGIN: &str = "// draftpaper session begin: written by `draftpaper apply`";
const FIREFOX_END: &str = "// draftpaper session end";

/// A marked block at the end of a profile's user.js, or none.
fn user_js(path: &Path, block: Option<&str>) {
    let old = util::read(path);
    let re = Regex::new(&format!(r"(?s){}.*?{}\n?", regex::escape(FIREFOX_BEGIN), regex::escape(FIREFOX_END))).unwrap();
    let rest = re.replace_all(&old, "").trim_end_matches('\n').to_string();
    let new = match block {
        Some(b) => format!("{}{}{FIREFOX_BEGIN}\n{b}\n{FIREFOX_END}\n", rest, if rest.is_empty() { "" } else { "\n" }),
        None if rest.is_empty() => String::new(),
        None => format!("{rest}\n"),
    };
    if new != old && (block.is_some() || path.exists()) {
        let _ = std::fs::write(path, new);
    }
}

/// A program of ours (this user's) running.
fn running(comm: &str) -> bool {
    let uid = std::fs::metadata("/proc/self").map(|m| std::os::unix::fs::MetadataExt::uid(&m)).unwrap_or(u32::MAX);
    std::fs::read_dir("/proc").into_iter().flatten().flatten().any(|e| {
        let d = e.path();
        std::fs::metadata(&d).is_ok_and(|m| std::os::unix::fs::MetadataExt::uid(&m) == uid) && std::fs::read_to_string(d.join("comm")).is_ok_and(|c| c.trim() == comm)
    })
}

fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

pub fn apply(p: &Paths, cfg: &Map<String, Value>) {
    let settings = cfg.get("session").and_then(Value::as_object).cloned().unwrap_or_default();
    let all = settings.get("restore") != Some(&Value::Bool(false)) && settings.get("apps") != Some(&Value::Bool(false));
    let skip: Vec<String> = settings.get("skip").and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_lowercase)).collect()).unwrap_or_default();
    let on = |app: &str| all && !skip.iter().any(|s| s == app);
    let (conf, home) = (&p.config_dir, &p.home);

    // Firefox: every profile's windows and tabs
    for root in [home.join(".mozilla/firefox")] {
        for profile in std::fs::read_dir(&root).into_iter().flatten().flatten().map(|e| e.path()) {
            if profile.is_dir() && profile.join("prefs.js").exists() {
                let block = r#"user_pref("browser.startup.page", 3);"#;
                user_js(&profile.join("user.js"), on("firefox").then_some(block));
            }
        }
    }

    // Chromium (and Chrome): written while it is not running, else it writes its own over it
    for (dir, comm) in [(conf.join("chromium"), "chromium"), (conf.join("google-chrome"), "chrome")] {
        let prefs = dir.join("Default/Preferences");
        if !on("chromium") || !prefs.exists() || running(comm) {
            continue;
        }
        if let Ok(Value::Object(mut v)) = serde_json::from_str::<Value>(&util::read(&prefs)) {
            let session = v.entry("session").or_insert_with(|| Value::Object(Map::new()));
            if let Some(s) = session.as_object_mut() {
                if s.get("restore_on_startup") != Some(&Value::from(1)) {
                    s.insert("restore_on_startup".into(), Value::from(1));
                    util::write(&prefs, &Value::Object(v).to_string());
                }
            }
        }
    }

    // Emacs
    let el = conf.join("draftpaper/emacs/draftpaper-session.el");
    if on("emacs") {
        util::write(&el, &util::read(&p.root.join("apps/emacs-session.el")));
        if let Some(init) = [conf.join("emacs/init.el"), home.join(".emacs.d/init.el")].into_iter().find(|p| p.exists()) {
            ensure_line(&init, &format!(";; the files as they were left (draftpaper apply; config.toml [session] apps)\n(load \"{}\" t t)", el.display()), "draftpaper-session.el", false);
        }
    } else {
        remove(&el);
    }

    // Neovim
    let lua = conf.join("nvim/plugin/draftpaper-session.lua");
    if on("neovim") && (conf.join("nvim").is_dir() || util::which("nvim").is_some()) {
        util::write(&lua, &util::read(&p.root.join("apps/nvim-session.lua")));
    } else {
        remove(&lua);
    }

    // tmux
    let tconf = conf.join("tmux/draftpaper-session.conf");
    if on("tmux") && util::which("tmux").is_some() {
        util::write(&tconf, &util::read(&p.root.join("apps/tmux-session.conf")));
        let main = if conf.join("tmux/tmux.conf").exists() || !home.join(".tmux.conf").exists() { conf.join("tmux/tmux.conf") } else { home.join(".tmux.conf") };
        ensure_line(&main, "source-file -q ~/.config/tmux/draftpaper-session.conf   # sessions kept across a reboot (draftpaper apply)", "draftpaper-session.conf", false);
        // a server running already keeps its sessions from now on
        if std::env::var_os("TMUX").is_some() || util::run(&["tmux", "has-session"], 5).is_some() {
            util::run(&["tmux", "source-file", &tconf.display().to_string()], 10);
        }
    } else {
        remove(&tconf);
    }
}
