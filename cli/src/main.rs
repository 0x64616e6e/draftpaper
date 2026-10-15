//! draftpaper: the desktop as a drafting sheet. Draftpaper's command, beside the compositor and the
//! shell: themes and colour schemes, the files the shell, launcher, lock screen and idle read, the
//! applications' colours, and the readings the shell polls.

mod apps;
mod config;
mod readings;
mod scheme;
mod theme;
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
                              applications)
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
  draftpaper audio save|restore
                              the default output's volume and mute, kept from one session to the next
                              (draftpaper-audio.service: the switch to the login screen mutes it)
  draftpaper wallpaper [FILE | none]
                              a picture in the frame, or none (the compositor picks it up at once)
  draftpaper margins paper|ambient
                              with a wallpaper: the margins the theme's paper or the picture's colours
  draftpaper readings         the readings the shell polls, as JSON
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
        ["theme", rest @ ..] => themes(&p, rest),
        ["scheme", rest @ ..] => schemes(&p, rest),
        ["wallpaper"] => println!("{}", config::load(&p).get("wallpaper").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("none")),
        ["wallpaper", "none", ..] => config::set_value(&p, "wallpaper", None),
        ["wallpaper", file, ..] => {
            let path = expand(file, &p);
            if !path.is_file() {
                util::die(format!("draftpaper: no such file {}", path.display()));
            }
            config::set_value(&p, "wallpaper", Some(&path.display().to_string()));
        }
        ["margins", m @ ("paper" | "ambient"), ..] => config::set_value(&p, "margins", Some(m)),
        ["margins", ..] => util::die("draftpaper margins paper|ambient"),
        ["audio", how, ..] => audio(&p, how),
        ["audio"] => util::die("draftpaper audio save|restore"),
        ["settings" | "picker", ..] => {
            // the shell's IpcHandler "settings" (qml/shell.qml); `picker` is the name it had
            let code = Command::new("qs").args(["ipc", "-p"]).arg(p.qml()).args(["call", "settings", "toggle"]).status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
            std::process::exit(code);
        }
        ["lock-on-sleep", how, ..] => lock_on_sleep(&p, how),
        ["lock-on-sleep"] => lock_on_sleep(&p, "toggle"),
        ["readings", ..] => readings::main(&p),
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
            let Ok(s) = serde_json::from_str::<Value>(&util::read(&state)) else { return };
            let (Some(volume), Some(muted)) = (s.get("volume").and_then(Value::as_f64), s.get("muted").and_then(Value::as_bool)) else { return };
            util::run(&["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", &util::float(volume)], 10);
            util::run(&["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", if muted { "1" } else { "0" }], 10);
        }
        _ => util::die("draftpaper audio save|restore"),
    }
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
