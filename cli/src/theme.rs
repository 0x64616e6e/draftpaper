//! Themes and applying them.
//!
//! A theme (themes/<name>.toml) names the paper, the ink and the window colours. The compositor reads
//! the theme named in config.toml itself (the sheet and the window borders, again whenever the file
//! changes); `apply` writes what the shell reads:
//!
//!   ~/.cache/draftpaper/theme.json    the ink, alert and accent colours and the shell's settings (title
//!                                     block, cells, alerts, commands); shell.qml reads it live
//!   ~/.cache/draftpaper/fuzzel.ini    the launcher (Mod+d) drawn like the sheet: paper, ink, a square
//!                                     frame, matches in the alert colour, the selection inverted
//!   ~/.cache/draftpaper/swaylock.conf the lock screen (Mod+Shift+x): the paper, an ink ring, the accent
//!                                     while typing, the alert colour for a wrong password
//!   ~/.cache/draftpaper/swayidle.conf idle (config.toml [idle]): the displays off, a lock, a lock before
//!                                     sleep; draftpaper-idle.service runs it and is restarted on apply
//!   and the applications, from the theme's [apps] palette (apps.rs)
//!
//! Previews that are not a theme on disk (the settings' scheme being built) go through a draft beside
//! the user's own themes, <name>.draft.toml, left out of the lists.

use std::{collections::BTreeMap, path::PathBuf};

use serde_json::{json, Map, Value};

use crate::util::{self, Paths};

pub const DRAFT: &str = ".draft";
const SHEET_DEFAULTS: [(&str, f64); 7] = [
    ("angle", 54.0), ("midpoint", 0.45), ("blend", 1.0), ("shading", 1.0), ("glow_strength", 1.0), ("glow_size", 1.0), ("pattern_strength", 0.5),
];
const WINDOW_KEYS: [&str; 7] = ["focused", "focused_inactive", "unfocused", "urgent", "text", "text_inactive", "background"];

/// Every theme by name: the shipped ones, then the user's, which shadow them.
pub fn available(p: &Paths) -> BTreeMap<String, PathBuf> {
    let mut names = BTreeMap::new();
    for d in p.themes().iter().rev() {
        let Ok(entries) = std::fs::read_dir(d) else { continue };
        let mut files: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().is_some_and(|x| x == "toml")).collect();
        files.sort();
        for f in files {
            if let Some(stem) = f.file_stem().and_then(|s| s.to_str()) {
                names.insert(stem.to_string(), f);
            }
        }
    }
    names
}

/// The themes to offer: every one but the drafts.
pub fn listed(p: &Paths) -> Vec<String> {
    available(p).into_keys().filter(|n| !n.ends_with(DRAFT)).collect()
}

/// The user's own file for `name` (whether or not it exists).
pub fn own(p: &Paths, name: &str) -> PathBuf {
    p.themes()[0].join(format!("{name}.toml"))
}

pub fn shipped(p: &Paths, name: &str) -> bool {
    p.themes()[1].join(format!("{name}.toml")).is_file()
}

/// A theme's values, with its name and the defaults it leaves out.
pub fn load(p: &Paths, name: &str) -> Map<String, Value> {
    let found = available(p);
    let Some(path) = found.get(name) else { util::die(format!("draftpaper: no theme named '{name}' (draftpaper theme list)")) };
    let table: toml::Table = util::read(path).parse().unwrap_or_else(|e| util::die(format!("draftpaper: {}: {e}", path.display())));
    let Value::Object(mut t) = util::toml_to_json(&toml::Value::Table(table)) else { unreachable!() };
    t.insert("name".into(), json!(name));
    t.entry("label").or_insert_with(|| json!(name));
    let mut sheet = Map::new();
    for (k, v) in SHEET_DEFAULTS {
        sheet.insert(k.into(), json!(v));
    }
    if let Some(Value::Object(s)) = t.get("sheet") {
        sheet.extend(s.clone());
    }
    t.insert("sheet".into(), Value::Object(sheet));
    if !t.contains_key("ink") {
        let shade = hex(&t, "paper_shade");
        let ink: String = shade.trim_start_matches('#').as_bytes().chunks(2).take(3).map(|c| {
            let v = u8::from_str_radix(std::str::from_utf8(c).unwrap_or("0"), 16).unwrap_or(0) as f64;
            format!("{:02X}", util::round_even(v * 0.7) as u8)
        }).collect();
        t.insert("ink".into(), json!(format!("#{ink}")));
    }
    t.entry("alert").or_insert_with(|| json!("#E2231A"));
    for k in WINDOW_KEYS {
        if t.get("windows").and_then(|w| w.get(k)).is_none() {
            util::die(format!("draftpaper: theme {name} lacks windows.{k}"));
        }
    }
    t
}

fn hex(t: &Map<String, Value>, k: &str) -> String {
    t.get(k).and_then(Value::as_str).unwrap_or("#000000").to_string()
}

fn str_of<'a>(t: &'a Map<String, Value>, path: &[&str]) -> &'a str {
    let mut v: Option<&Value> = t.get(path[0]);
    for k in &path[1..] {
        v = v.and_then(|x| x.get(k));
    }
    v.and_then(Value::as_str).unwrap_or("#000000")
}

/// The sheet's lettering face, as the shell picks it (qml/Fonts.qml).
fn mono_font() -> &'static str {
    let families = util::run(&["fc-list", ":", "family"], 10).unwrap_or_default();
    let plex = families.lines().flat_map(|l| l.split(',')).any(|f| f.trim() == "IBM Plex Mono");
    if plex { "IBM Plex Mono" } else { "DejaVu Sans Mono" }
}

/// fuzzel's config for theme t. Colours are RRGGBBAA.
fn fuzzel_ini(t: &Map<String, Value>, font: &str) -> String {
    let c = |k: &str, alpha: &str| format!("{}{alpha}", str_of(t, &[k]).trim_start_matches('#').to_lowercase());
    let terminal = ["alacritty", "foot"].iter().find(|term| util::which(term).is_some()).map(|t| format!("{t} -e")).unwrap_or_else(|| "x-terminal-emulator -e".into());
    format!(
        r#"# generated by `draftpaper apply` ({label}): the launcher (Mod+d) drawn like the sheet
[main]
font={font}:size=11
prompt="› "
placeholder=launch
terminal={terminal}
layer=overlay
width=44
lines=12
horizontal-pad=24
vertical-pad=18
inner-pad=10
icons-enabled=yes

[colors]
background={bg}
text={ink}
prompt={ink}
input={ink}
placeholder={ph}
match={alert}
selection={ink}
selection-text={paper}
selection-match={light}
counter={counter}
border={ink}

[border]
width=2
radius=0
"#,
        label = str_of(t, &["label"]),
        bg = c("paper", "f5"),
        ink = c("ink", "ff"),
        ph = c("ink", "80"),
        alert = c("alert", "ff"),
        paper = c("paper", "ff"),
        light = c("paper_light", "ff"),
        counter = c("ink", "99"),
    )
}

const LOCK: &str = r#"swaylock -f -C "${XDG_CACHE_HOME:-$HOME/.cache}/draftpaper/swaylock.conf""#;

/// swaylock's config for theme t (colours RRGGBB[AA]).
fn swaylock_conf(t: &Map<String, Value>, font: &str) -> String {
    let c = |path: &[&str]| str_of(t, path).trim_start_matches('#').to_lowercase();
    let (paper, ink, alert, accent) = (c(&["paper"]), c(&["ink"]), c(&["alert"]), c(&["windows", "focused"]));
    format!(
        "# generated by `draftpaper apply` ({label}): the lock screen (Mod+Shift+x)
color={paper}
font={font}
font-size=22
indicator-idle-visible
indicator-radius=96
indicator-thickness=6
show-failed-attempts
inside-color={paper}
inside-clear-color={paper}
inside-caps-lock-color={paper}
inside-ver-color={paper}
inside-wrong-color={paper}
ring-color={ink}
ring-clear-color={ink}
ring-caps-lock-color={alert}
ring-ver-color={accent}
ring-wrong-color={alert}
key-hl-color={accent}
caps-lock-key-hl-color={alert}
bs-hl-color={alert}
caps-lock-bs-hl-color={alert}
line-color=00000000
line-clear-color=00000000
line-caps-lock-color=00000000
line-ver-color=00000000
line-wrong-color=00000000
separator-color=00000000
text-color={ink}
text-clear-color={ink}
text-caps-lock-color={alert}
text-ver-color={ink}
text-wrong-color={alert}
",
        label = str_of(t, &["label"]),
    )
}

/// swayidle's config from config.toml [idle]: the displays off, a lock, a lock before sleep.
fn swayidle_conf(cfg: &Map<String, Value>) -> String {
    let idle = cfg.get("idle").cloned().unwrap_or(json!({}));
    let num = |k: &str| idle.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let mut lines = vec!["# generated by `draftpaper apply` from ~/.config/draftpaper/config.toml [idle]".to_string()];
    if num("lock_after") > 0.0 {
        lines.push(format!("timeout {} '{LOCK}'", (num("lock_after") * 60.0) as i64));
    }
    if num("screens_off") > 0.0 {
        lines.push(format!(
            "timeout {} 'draftpaper-compositor msg power off' resume 'draftpaper-compositor msg power on'",
            (num("screens_off") * 60.0) as i64
        ));
    }
    // before suspend (the lid): always when lock_on_sleep, else only while `draftpaper lock-on-sleep` is on
    let flag = r#""$XDG_RUNTIME_DIR/draftpaper/lock-on-sleep""#;
    let truthy = idle.get("lock_on_sleep").is_some_and(|v| v.as_bool().unwrap_or(false) || v.as_f64().is_some_and(|x| x != 0.0));
    lines.push(if truthy { format!("before-sleep '{LOCK}'") } else { format!("before-sleep 'if [ -e {flag} ]; then {LOCK}; fi'") });
    lines.join("\n") + "\n"
}

/// Write the theme's files (the shell, the launcher, the lock screen, idle) and, unless only previewing,
/// theme the applications.
pub fn apply(p: &Paths, name: Option<&str>, apps_too: bool) -> Map<String, Value> {
    let cfg = crate::config::load(p);
    let name = name.map(String::from).unwrap_or_else(|| cfg.get("theme").and_then(Value::as_str).unwrap_or("softglow").to_string());
    let t = load(p, &name);
    p.ensure();
    let get = |k: &str| cfg.get(k).cloned().unwrap_or(Value::Null);
    let truthy = |v: &Value| match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    };
    let animations = cfg.get("animations").and_then(|a| a.get("enabled")).map(truthy).unwrap_or(true);
    let shell = json!({
        "name": t["name"], "label": t["label"],
        "paper": t["paper"], "ink": t["ink"], "inkAlpha": t["sheet"]["pattern_strength"], "alert": t["alert"],
        "strong": t.get("strong").cloned().unwrap_or(json!("#000000")),
        "accent": t["windows"]["focused"],
        "title": get("title"), "cells": get("cell"), "alerts": get("alerts"), "commands": get("commands"),
        "glyphs": truthy(&get("glyphs")),
        // motion in the shell (the settings) follows the compositor's [animations]
        "animations": animations,
        "themes": listed(p),
    });
    util::write_atomic(&p.theme_json(), &util::dumps_indent(&shell, 1));
    let font = mono_font();
    util::write(&p.cache.join("fuzzel.ini"), &fuzzel_ini(&t, font));
    if apps_too {
        crate::apps::apply(p, &t, &cfg);
    }
    util::write(&p.cache.join("swaylock.conf"), &swaylock_conf(&t, font));
    let idle = p.cache.join("swayidle.conf");
    let text = swayidle_conf(&cfg);
    if util::read(&idle) != text || !idle.exists() {
        util::write(&idle, &text);
        // the idle service reads its file at start
        util::run(&["systemctl", "--user", "try-restart", "draftpaper-idle.service"], 10);
    }
    t
}
