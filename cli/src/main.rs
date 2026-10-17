//! draftpaper: the desktop as a drafting sheet. Draftpaper's command, beside the compositor and the
//! shell: themes and colour schemes, the files the shell, launcher, lock screen and idle read, the
//! applications' colours, and the readings the shell polls.

mod apps;
mod config;
mod greeter;
mod programs;
mod readings;
mod restore;
mod revisions;
mod scheme;
mod theme;
mod tmux;
mod util;

use std::{
    io::{Read, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    process::{Command, Stdio},
};

use serde_json::{json, Map, Value};

use util::Paths;

const USAGE: &str = "draftpaper — the desktop as a drafting sheet.

  draftpaper start            apply the theme and run the shell (draftpaper-shell.service, in the session)
  draftpaper stop             stop the shell
  draftpaper apply            write the theme's files again (the shell, launcher, lock screen, idle,
                              applications: their colours and session restore)
  draftpaper theme [list]     list the themes, the current one marked
  draftpaper theme <name> [--preview]
                              switch theme: the sheet and the applications; a --preview (the settings
                              while browsing) switches the sheet only; config.toml's [hooks] theme
                              runs after a full switch
  draftpaper theme next       the next theme (the shell's theme cell, right click)
  draftpaper theme json       every theme's values, for the settings
  draftpaper theme check [<name> ...]
                              the themes against the contrast rules (words 4.5:1, dimmed text 3:1)
  draftpaper theme revert <name>
                              a shipped theme as shipped again: your copy of it removed
  draftpaper scheme options|json|preview|keep [choice=value ...]
                              a colour scheme built from choices (the settings): paper, tone, accent,
                              windows, shade, terminal, text, contrast; keep writes [scheme] and the
                              \"custom\" theme
  draftpaper settings         open or close the settings (Mod+t, the theme cell): the colour scheme
  draftpaper launcher         open or close the launcher (Mod+d): the applications, as a register
  draftpaper menu [--prompt TEXT]
                              a menu of stdin's lines in the launcher's register; the choice on stdout
                              (exit 1: closed without one)
  draftpaper audio save|restore
                              the default output's volume and mute, kept from one session to the next
                              (draftpaper-audio.service: the switch to the login screen mutes it)
  draftpaper wallpaper [FILE | none]
                              a picture in the frame, or none (the compositor picks it up at once)
  draftpaper wallpaper json   the wallpaper's settings as set, and the pictures to choose from
  draftpaper wallpaper set NAME=VALUE ...
                              picture=FILE|none, margins=paper|ambient, tint, fit=fill|fit|centre,
                              position=centre|top|bottom|left|right, brightness, wash, blur (mm):
                              any of them in one write (the settings' wallpaper page)
  draftpaper margins paper|ambient
                              with a wallpaper: the margins the theme's paper or the picture's colours
  draftpaper tmux save|keep|restore [SOCKET]
                              tmux's sessions kept across a reboot ([session] apps: run by tmux itself)
  draftpaper readings         the readings the shell polls, as JSON
  draftpaper revisions        the focused window's repository and latest commits, as JSON (the revision block)
  draftpaper lock             lock the screen (the lock screen, qml/lock.qml); returns once locked
  draftpaper lock-on-sleep [on|off|toggle]
                              lock before every suspend (the lid) until turned off or the next login
  draftpaper version

Settings: ~/.config/draftpaper/config.toml (see /usr/share/doc/draftpaper/README.md).";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let p = Paths::new();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        [] | ["-h" | "--help" | "help", ..] => println!("{USAGE}"),
        ["start", ..] => start(&p),
        ["stop", ..] => {
            let _ = Command::new("qs").args(["kill", "-p"]).arg(p.qml()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
        ["apply", ..] => {
            let t = theme::apply(&p, None, true);
            println!("{} applied.", t["label"].as_str().unwrap_or(""));
        }
        ["tmux", rest @ ..] => tmux::command(&p, rest),
        ["theme", rest @ ..] => themes(&p, rest),
        ["scheme", rest @ ..] => schemes(&p, rest),
        ["wallpaper"] => println!("{}", config::load(&p).get("wallpaper").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("none")),
        ["wallpaper", "json"] => wallpaper_json(&p),
        ["wallpaper", "set", pairs @ ..] => {
            wallpaper_set(&p, pairs);
            greeter::sync(&p);
        }
        ["wallpaper", "none", ..] => {
            config::set_value(&p, "wallpaper", None);
            greeter::sync(&p);
        }
        ["wallpaper", file, ..] => {
            let path = expand(file, &p);
            if !path.is_file() {
                util::die(format!("draftpaper: no such file {}", path.display()));
            }
            config::set_value(&p, "wallpaper", Some(&path.display().to_string()));
            greeter::sync(&p);
        }
        ["margins", m @ ("paper" | "ambient"), ..] => {
            config::set_value(&p, "margins", Some(m));
            greeter::sync(&p);
        }
        ["margins", ..] => util::die("draftpaper margins paper|ambient"),
        ["audio", how, ..] => audio(&p, how),
        ["audio"] => util::die("draftpaper audio save|restore"),
        ["settings" | "picker", ..] => {
            // the shell's IpcHandler "settings" (qml/shell.qml); `picker` is the name it had
            let code = Command::new("qs").args(["ipc", "-p"]).arg(p.qml()).args(["call", "settings", "toggle"]).status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
            std::process::exit(code);
        }
        ["launcher", ..] => {
            // the shell's IpcHandler "launcher" (qml/shell.qml)
            let code = Command::new("qs").args(["ipc", "-p"]).arg(p.qml()).args(["call", "launcher", "toggle"]).status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
            std::process::exit(code);
        }
        ["menu", rest @ ..] => menu(&p, rest),
        ["lock", ..] => lock(&p),
        ["lock-on-sleep", how, ..] => lock_on_sleep(&p, how),
        ["lock-on-sleep"] => lock_on_sleep(&p, "toggle"),
        ["readings", ..] => readings::main(&p),
        ["revisions", ..] => revisions::main(),
        ["version", ..] => println!("{}", env!("CARGO_PKG_VERSION")),
        _ => util::die(USAGE),
    }
}

fn expand(file: &str, p: &Paths) -> std::path::PathBuf {
    let path = match file.strip_prefix("~/") {
        Some(rest) => p.home.join(rest),
        None if file == "~" => p.home.clone(),
        None => file.into(),
    };
    let path = if path.is_absolute() { path } else { std::env::current_dir().unwrap_or_default().join(path) };
    // the path made plain (no "." or ".."), as os.path.abspath does, without following links
    let mut out = std::path::PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// The wallpaper's settings in config.toml: (the settings page's name, config.toml's key, the default).
/// Strings are quoted when written, numbers not.
const WALLPAPER: [(&str, &str, &str); 7] = [
    ("margins", "margins", "paper"),
    ("tint", "title_tint", "0.7"),
    ("fit", "wallpaper_fit", "fill"),
    ("position", "wallpaper_position", "centre"),
    ("brightness", "wallpaper_brightness", "1.0"),
    ("wash", "wallpaper_wash", "0.0"),
    ("blur", "wallpaper_blur", "0.0"),
];
const WALLPAPER_DIRS: [&str; 2] = ["~/Pictures/Wallpapers", "/usr/share/backgrounds"];

/// `draftpaper wallpaper set picture=FILE|none margins=paper|ambient tint=0.7 fit=fill|fit|centre
/// position=centre|top|bottom|left|right brightness=1 wash=0 blur=0`: any of them, all in one write (one
/// reload), as given (a restore may name a picture that has gone since).
fn wallpaper_set(p: &Paths, pairs: &[&str]) {
    let mut values: Vec<(&str, Option<String>)> = Vec::new();
    for pair in pairs {
        let Some((k, v)) = pair.split_once('=') else { util::die(format!("draftpaper: wallpaper set NAME=VALUE ..., not {pair}")) };
        if k == "picture" {
            values.push(("wallpaper", if v == "none" || v.is_empty() { None } else { Some(format!("\"{}\"", expand(v, p).display())) }));
            continue;
        }
        let Some((_, key, _)) = WALLPAPER.iter().find(|(name, ..)| *name == k) else { util::die(format!("draftpaper: no wallpaper setting {k}")) };
        let ok = match k {
            "margins" => ["paper", "ambient"].contains(&v),
            "fit" => ["fill", "fit", "centre"].contains(&v),
            "position" => ["centre", "top", "bottom", "left", "right"].contains(&v),
            _ => v.parse::<f64>().is_ok_and(|x| (0.0..=30.0).contains(&x)),
        };
        if !ok {
            util::die(format!("draftpaper: wallpaper {k} = {v}?"));
        }
        let raw = match v.parse::<f64>() {
            Ok(x) => util::float(x),
            Err(_) => format!("\"{v}\""),
        };
        values.push((key, Some(raw)));
    }
    config::set_many(p, &values);
}

/// What the settings' wallpaper page shows: the wallpaper's settings as they are, and the pictures to
/// choose from: config.toml's wallpaper_dirs (by default ~/Pictures/Wallpapers, a folder of their own so
/// the screenshots in ~/Pictures stay out, and the system's backgrounds), and the current picture
/// wherever it is.
fn wallpaper_json(p: &Paths) {
    let cfg = config::load(p);
    let current = cfg.get("wallpaper").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| expand(s, p));
    let dirs: Vec<String> = cfg
        .get("wallpaper_dirs")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|d| d.as_str().map(String::from)).collect())
        .unwrap_or_else(|| WALLPAPER_DIRS.iter().map(|d| d.to_string()).collect());
    let picture = |f: &std::path::Path| f.is_file() && f.extension().and_then(|e| e.to_str()).is_some_and(|e| ["jpg", "jpeg", "png", "webp"].contains(&e.to_lowercase().as_str()));
    let list = |d: &std::path::Path, depth: usize| {
        let mut out = Vec::new();
        let mut dirs = vec![(d.to_path_buf(), 0)];
        while let Some((d, n)) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else { continue };
            for e in entries.flatten() {
                let f = e.path();
                if f.is_dir() && n < depth {
                    dirs.push((f, n + 1));
                } else if picture(&f) {
                    out.push(f);
                }
            }
        }
        out.sort();
        out
    };
    let mut pictures: Vec<std::path::PathBuf> = Vec::new();
    for d in &dirs {
        for f in list(&expand(d, p), 1) {
            if !pictures.contains(&f) {
                pictures.push(f);
            }
        }
    }
    if let Some(c) = current.as_ref().filter(|c| c.is_file() && !pictures.contains(c)) {
        pictures.insert(0, c.clone());
    }
    pictures.truncate(80);
    let mut out = Map::new();
    out.insert("current".into(), json!(current.as_ref().map(|c| c.display().to_string()).unwrap_or_default()));
    out.insert("exists".into(), json!(current.as_ref().is_some_and(|c| c.is_file())));
    // each setting as set, or its default (a number as a number)
    for (name, key, default) in WALLPAPER {
        let v = match cfg.get(key) {
            Some(v @ (Value::String(_) | Value::Number(_))) => v.clone(),
            _ => default.parse::<f64>().map(|x| json!(x)).unwrap_or_else(|_| json!(default)),
        };
        out.insert(name.into(), v);
    }
    out.insert("dirs".into(), json!(dirs));
    out.insert("pictures".into(), json!(pictures.iter().map(|f| f.display().to_string()).collect::<Vec<_>>()));
    println!("{}", util::dumps(&Value::Object(out)));
}

/// The shell, in the foreground: the session's service keeps it running.
fn start(p: &Paths) {
    if util::which("qs").is_none() {
        util::die("draftpaper: quickshell (qs) is not installed");
    }
    if std::env::var_os("DRAFTPAPER_SOCKET").is_none() {
        util::die("draftpaper: DRAFTPAPER_SOCKET is not set (start the shell inside a Draftpaper session)");
    }
    theme::apply(p, None, true);
    let bin = std::env::current_exe().ok().and_then(|e| e.canonicalize().ok()).map(|e| e.display().to_string()).unwrap_or_else(|| "draftpaper".into());
    let err = Command::new("qs")
        .args(["-p"])
        .arg(p.qml())
        .arg("-n")
        .env("DRAFTPAPER_BIN", bin)
        .env("DRAFTPAPER_CACHE", &p.cache)
        .env("DRAFTPAPER_RUNTIME", &p.runtime)
        .exec();
    util::die(format!("draftpaper: qs: {err}"));
}

/// Keep the default output's volume and mute across sessions. Logging out hands the sound card to the
/// login screen, and the handover mutes it; the session saves the state as it ends (while it still has
/// the card) and puts it back as the next one starts.
fn audio(p: &Paths, how: &str) {
    let state = p.state.join("audio.json");
    match how {
        "save" => {
            let out = util::run(&["wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@"], 10).unwrap_or_default();
            let re = regex::Regex::new(r"Volume:\s*([0-9.]+)(\s*\[MUTED\])?").unwrap();
            if let Some(c) = re.captures(&out) {
                let volume: f64 = c[1].parse().unwrap_or(0.0);
                util::write(&state, &util::dumps(&json!({"volume": volume, "muted": c.get(2).is_some()})));
            }
        }
        "restore" => {
            // the card back first: WirePlumber, kept running from the last session (another session of the
            // user's outlived it), saw the card go to the login screen and probed it without access: no
            // outputs, only the dummy. Restarted, it probes the card again, now this session's
            if dummy_output() {
                eprintln!("draftpaper: no sound card output (WirePlumber lost it): restarting WirePlumber");
                util::run(&["systemctl", "--user", "restart", "wireplumber.service"], 20);
                for _ in 0..24 {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    if !dummy_output() {
                        break;
                    }
                }
            }
            let Ok(s) = serde_json::from_str::<Value>(&util::read(&state)) else { return };
            let (Some(volume), Some(muted)) = (s.get("volume").and_then(Value::as_f64), s.get("muted").and_then(Value::as_bool)) else { return };
            util::run(&["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", &util::float(volume)], 10);
            util::run(&["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", if muted { "1" } else { "0" }], 10);
        }
        _ => util::die("draftpaper audio save|restore"),
    }
}

/// A menu of stdin's lines in the launcher's register (the shell's IpcHandler "launcher"): the choice
/// comes back through a FIFO in the runtime directory and goes to stdout. Exit 1 when the menu was
/// closed without a choice, 2 when the shell could not show it.
fn menu(p: &Paths, rest: &[&str]) {
    let fail = |msg: String| -> ! {
        eprintln!("draftpaper menu: {msg}");
        std::process::exit(2)
    };
    let prompt = match rest {
        [] => "",
        ["--prompt" | "-p", text, ..] => text,
        _ => fail("draftpaper menu [--prompt TEXT] < lines".into()),
    };
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        fail("stdin is not text".into());
    }
    let _ = std::fs::create_dir_all(&p.runtime);
    let base = p.runtime.join(format!("menu-{}", std::process::id()));
    let (list, fifo) = (base.with_extension("lines"), base.with_extension("fifo"));
    let clean = || {
        let _ = std::fs::remove_file(&list);
        let _ = std::fs::remove_file(&fifo);
    };
    clean();
    if std::fs::write(&list, &input).is_err() || !Command::new("mkfifo").arg("-m").arg("600").arg(&fifo).status().is_ok_and(|s| s.success()) {
        clean();
        fail(format!("cannot write {}", base.display()));
    }
    // `qs ipc call` exits 0 even when the call fails, and says why on its output: a call that returns
    // nothing and says nothing went through
    let said = Command::new("qs").args(["ipc", "-p"]).arg(p.qml()).args(["call", "launcher", "menu", prompt]).arg(&list).arg(&fifo).output();
    match said {
        Ok(o) if o.status.success() && o.stdout.is_empty() && o.stderr.is_empty() => {}
        Ok(o) => {
            clean();
            fail(format!("the shell did not show it: {}{}", String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim()));
        }
        Err(e) => {
            clean();
            fail(format!("qs: {e}"));
        }
    }
    // blocks until the shell writes the choice: a line, or an empty one when the menu closed
    let choice = std::fs::read_to_string(&fifo).unwrap_or_default();
    clean();
    let choice = choice.trim_end_matches('\n');
    if choice.is_empty() {
        std::process::exit(1);
    }
    println!("{choice}");
}

/// A flag in the runtime directory that the idle service's before-sleep hook looks for.
fn lock_on_sleep(p: &Paths, how: &str) {
    p.ensure();
    let flag = p.runtime.join("lock-on-sleep");
    let on = match how {
        "on" => true,
        "off" => false,
        _ => !flag.exists(),
    };
    if on {
        let _ = std::fs::write(&flag, "");
    } else {
        let _ = std::fs::remove_file(&flag);
    }
    let (title, body) = if on {
        ("Lock on lid close: ON", "The screen locks before every suspend")
    } else {
        ("Lock on lid close: OFF", "Suspend and resume without a lock screen")
    };
    util::run(&["notify-send", "-a", "draftpaper", title, body], 10);
    println!("{title}");
}

fn current(p: &Paths) -> String {
    config::load(p).get("theme").and_then(Value::as_str).unwrap_or("softglow").to_string()
}

fn themes(p: &Paths, args: &[&str]) {
    let current = current(p);
    let names = theme::listed(p);
    match args {
        ["json", ..] => {
            let all: Vec<Value> = names
                .iter()
                .map(|n| {
                    let mut t = theme::load(p, n);
                    let figures = scheme::figures(&t);
                    t.insert("current".into(), json!(*n == current));
                    t.insert("own".into(), json!(theme::own(p, n).is_file()));
                    t.insert("shipped".into(), json!(theme::shipped(p, n)));
                    t.insert("figures".into(), figures);
                    Value::Object(t)
                })
                .collect();
            println!("{}", util::dumps(&Value::Array(all)));
        }
        [] | ["list", ..] => {
            for n in &names {
                println!("{}{n}", if *n == current { "* " } else { "  " });
            }
        }
        ["check", rest @ ..] => {
            // the contrast rules (scheme.rs), for the named themes or all of them
            let which: Vec<String> = if rest.is_empty() { names.clone() } else { rest.iter().map(|s| s.to_string()).collect() };
            let mut failed = false;
            for n in which {
                for (what, r, m, hard) in scheme::check(&theme::load(p, &n)) {
                    println!("{n}: {what} {r:.2}:1, {} {}:1", if hard { "needs" } else { "advised" }, if m.fract() == 0.0 { format!("{}", m as i64) } else { format!("{m}") });
                    failed |= hard;
                }
            }
            std::process::exit(if failed { 1 } else { 0 });
        }
        ["revert", name] => {
            // the shipped theme again, the user's copy gone
            if !theme::shipped(p, name) {
                util::die(format!("draftpaper: {name} is not a shipped theme"));
            }
            let _ = std::fs::remove_file(theme::own(p, name));
            switch(p, name, false);
        }
        _ => {
            let preview = args.contains(&"--preview");
            let rest: Vec<&str> = args.iter().copied().filter(|a| *a != "--preview").collect();
            let Some(name) = rest.first() else { util::die(USAGE) };
            let name = if *name == "next" {
                match names.iter().position(|n| *n == current) {
                    Some(i) => names[(i + 1) % names.len()].clone(),
                    None => names.first().cloned().unwrap_or_default(),
                }
            } else {
                name.to_string()
            };
            switch(p, &name, preview);
        }
    }
}

/// draftpaper scheme options | json | preview | keep [CHOICE=VALUE ...]: a colour scheme built from
/// choices (scheme.rs), for the settings. The choices given override config.toml's [scheme].
fn schemes(p: &Paths, args: &[&str]) {
    let cfg = config::load(p);
    let (sub, pairs) = match args {
        [] => ("json", &[][..]),
        [s, rest @ ..] => (*s, rest),
    };
    let mut given: Vec<(String, String)> = cfg
        .get("scheme")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect())
        .unwrap_or_default();
    given.extend(pairs.iter().filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))));
    let choices = scheme::complete(&given);
    let with = |k: &str, v: &str| {
        let mut c = choices.clone();
        c.push((k.to_string(), v.to_string()));
        c
    };
    let custom = |t: &Map<String, Value>| {
        let mut t = t.clone();
        let figures = scheme::figures(&t);
        t.insert("name".into(), json!("custom"));
        t.insert("figures".into(), figures);
        Value::Object(t)
    };
    match sub {
        "options" => {
            // each choice's values, with a colour that stands for each (the scheme with that value swapped in)
            let mut out = Map::new();
            for (k, values) in scheme::choices() {
                let mut opts = Vec::new();
                for v in values {
                    let t = scheme::build(&with(k, v));
                    let a = &t["apps"];
                    let s = |x: &Value| x.as_str().unwrap_or("").to_string();
                    let colour = match k {
                        "paper" | "tone" => s(&t["paper"]),
                        "accent" => s(&t["windows"]["focused"]),
                        "windows" | "shade" => s(&a["bg"]),
                        _ => s(&a["term_fg"]), // the terminal's: Aa on its background
                    };
                    let f = scheme::figures(&t);
                    let ratio = match k {
                        "paper" | "tone" => f["ink"].clone(),
                        "text" | "terminal" | "contrast" => f["terminal"].clone(),
                        "accent" => json!(util::round_to(scheme::contrast(&s(&t["windows"]["focused"]), &s(&a["bg"])), 1)),
                        _ => f["text"].clone(),
                    };
                    // a shade is called by its light-side name when the windows are light
                    let light = t["choices"]["windows"] == "light";
                    let label = if light && k == "shade" { scheme::light_name(v) } else { v };
                    let on = if matches!(k, "text" | "terminal") { a["term_bg"].clone() } else { Value::Null };
                    opts.push(json!({"value": v, "label": label, "colour": colour, "ratio": ratio, "on": on}));
                }
                out.insert(k.into(), Value::Array(opts));
            }
            // and the scheme the choices build, so the settings need one command per click
            let t = scheme::build(&choices);
            println!("{}", util::dumps(&json!({"choices": t["choices"], "options": out, "theme": custom(&t)})));
        }
        "json" => println!("{}", util::dumps(&custom(&scheme::build(&choices)))),
        "preview" => {
            // the sheet only, from a draft beside the user's themes (theme.rs)
            let t = scheme::build(&choices);
            util::write(&theme::own(p, &format!("custom{}", theme::DRAFT)), &scheme::toml(&t));
            switch(p, &format!("custom{}", theme::DRAFT), true);
        }
        "keep" => {
            // the choices kept, the theme written, switched to whole
            let t = scheme::build(&choices);
            let kept: Vec<(String, String)> = t["choices"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect();
            config::set_table(p, "scheme", &kept);
            util::write(&theme::own(p, "custom"), &scheme::toml(&t));
            switch(p, "custom", false);
            let _ = std::fs::remove_file(theme::own(p, &format!("custom{}", theme::DRAFT)));
        }
        _ => util::die("draftpaper scheme options | json | preview | keep [CHOICE=VALUE ...]"),
    }
}

/// Switch to theme `name`: the sheet; and unless a preview, the applications and [hooks] theme.
fn switch(p: &Paths, name: &str, preview: bool) {
    let t = theme::apply(p, Some(name), !preview); // a preview (the settings browsing) leaves the applications
    config::set_theme(p, name);
    compositor("reload"); // the sheet at once, not at the compositor's next look at the file
    if !preview {
        let hook: Vec<String> = config::load(p)
            .get("hooks")
            .and_then(|h| h.get("theme"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|s| s.as_str().map(|s| s.replace("{name}", name))).collect())
            .unwrap_or_default();
        if let Some((cmd, rest)) = hook.split_first() {
            // the applications too (detached: it may take a while, the sheet has switched already)
            let _ = Command::new(cmd).args(rest).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn();
        }
    }
    println!("{} applied.", t["label"].as_str().unwrap_or(""));
}

/// Run an action in the compositor over its IPC, when there is one (in a Draftpaper session).
fn compositor(action: &str) {
    let Some(path) = std::env::var_os("DRAFTPAPER_SOCKET") else { return };
    let Ok(mut s) = UnixStream::connect(path) else { return };
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let _ = s.set_write_timeout(Some(std::time::Duration::from_secs(2)));
    let msg = util::dumps(&json!({"request": "action", "action": action})) + "\n";
    if s.write_all(msg.as_bytes()).is_ok() {
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
    }
}

/// Whether the default output is PipeWire's dummy (no sound card output), or there is none.
fn dummy_output() -> bool {
    let out = util::run(&["wpctl", "inspect", "@DEFAULT_AUDIO_SINK@"], 10).unwrap_or_default();
    out.is_empty() || out.contains("\"auto_null\"")
}

/// The lock screen (qml/lock.qml), started detached; back once the compositor has confirmed the lock (so
/// swayidle's before-sleep waits for it), or at once when it is locked already. Exit 1 when it did not
/// lock within 5 seconds, and swaylock (when there is one) did not either.
fn lock(p: &Paths) {
    if util::which("qs").is_none() {
        util::die("draftpaper: quickshell (qs) is not installed");
    }
    let qml = p.root.join("qml/lock.qml");
    let state = || util::run(&["qs", "ipc", "-p", &qml.display().to_string(), "call", "lock", "state"], 3).unwrap_or_default();
    if state().starts_with("locked") {
        return;
    }
    let started = Command::new("qs")
        .args(["-p"])
        .arg(&qml)
        .args(["-n", "-d"])
        .env("DRAFTPAPER_CACHE", &p.cache)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if started.is_err() {
        util::die("draftpaper: the lock screen did not start");
    }
    for _ in 0..50 {
        if state().starts_with("locked") {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // never left unlocked (before sleep above all): swaylock, when there is one, themed
    if util::which("swaylock").is_some() {
        eprintln!("draftpaper: the lock screen did not lock within 5 seconds: swaylock instead");
        let conf = p.cache.join("swaylock.conf");
        let ok = Command::new("swaylock").arg("-f").arg("-C").arg(&conf).status().is_ok_and(|s| s.success());
        if ok {
            return;
        }
    }
    util::die("draftpaper: the screen did not lock within 5 seconds");
}
