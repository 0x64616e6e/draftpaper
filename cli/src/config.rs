//! The user's settings (~/.config/draftpaper/config.toml), with defaults for everything Draftpaper's
//! command reads (the compositor reads the file itself):
//!
//!     theme = "softglow"            # a file in ~/.config/draftpaper/themes or /usr/share/draftpaper/themes
//!     margin = 6                    # millimetres between the screen edge and the frame (3 to 20)
//!     glyphs = true                 # Nerd Font glyphs before the readings, when a Nerd Font is installed
//!     wallpaper = "~/Pictures/x.jpg"  # optional: a picture in the frame (draftpaper wallpaper FILE | none)
//!     margins = "paper"             # with a wallpaper: "paper", or "ambient" (the picture's colours)
//!
//!     [title]                       # the title block's fixed cells: project (the host name), drawn (you);
//!                                   # scale = false takes the scale bar away (the compositor's);
//!                                   # revisions = true shows the revision block (qml/RevisionBlock.qml);
//!                                   # key_plan = false takes the key plan away (the compositor's)
//!     [[cell]]                      # cells of your own: label, command, interval, left, right
//!     [alerts]                      # cpu = 85, nvme = 60 (°C), fan = 5000 (RPM), battery = 15 (%)
//!     [idle]                        # screens_off = 15, lock_after = 0 (minutes), lock_on_sleep = false
//!     [apps]                        # theme = true, skip = [...], firefox_text, firefox_ui_pt
//!     [hooks]                       # theme = ["cmd", "{name}"]: run after a full theme switch
//!     [commands]                    # screenshot = [...]: the shell's SHOT cell
//!     [scheme]                      # the colour scheme's choices (the settings tool)
//!
//! Writing goes line by line (set_value, set_table, set_theme): the user's comments and order stay.

use serde_json::{json, Map, Value};

use crate::util::{self, Paths};

pub fn defaults() -> Map<String, Value> {
    let v = json!({
        "theme": "softglow",
        "margin": 6.0,
        "glyphs": true,
        "title": {},
        "cell": [],
        "alerts": {"cpu": 85, "nvme": 60, "fan": 5000, "battery": 15},
        "idle": {"screens_off": 15, "lock_after": 0, "lock_on_sleep": false},
        "hooks": {},
        // a region picked with slurp, captured by grim (the compositor's screencopy), annotated in swappy
        "commands": {"screenshot": ["sh", "-c", "g=$(slurp) && grim -g \"$g\" - | swappy -f -"]},
    });
    match v {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

/// The settings: config.toml over the defaults (a table merged key by key), the margin kept in range,
/// the title block's cells filled in.
pub fn load(p: &Paths) -> Map<String, Value> {
    let mut cfg = defaults();
    let text = util::read(&p.config);
    if !text.is_empty() {
        match text.parse::<toml::Table>() {
            Ok(user) => {
                for (k, v) in &user {
                    let v = util::toml_to_json(v);
                    match (cfg.get_mut(k), v) {
                        (Some(Value::Object(d)), Value::Object(u)) => d.extend(u),
                        (_, v) => {
                            cfg.insert(k.clone(), v);
                        }
                    }
                }
            }
            Err(e) => util::die(format!("draftpaper: {}: {e}", p.config.display())),
        }
    }
    let margin = cfg.get("margin").and_then(Value::as_f64).unwrap_or(6.0).clamp(3.0, 20.0);
    cfg.insert("margin".into(), json!(margin));
    if let Some(Value::Object(title)) = cfg.get_mut("title") {
        title.entry("project").or_insert_with(|| json!(hostname().to_uppercase()));
        title.entry("drawn").or_insert_with(|| json!(username()));
    }
    cfg
}

fn hostname() -> String {
    util::read("/proc/sys/kernel/hostname".as_ref()).trim().to_string()
}

/// The user's name: as Python's getpass does, the environment first, then the password file.
fn username() -> String {
    for var in ["LOGNAME", "USER", "LNAME", "USERNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    util::read("/etc/passwd".as_ref())
        .lines()
        .find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.len() > 2 && f[2].parse() == Ok(uid)).then(|| f[0].to_string())
        })
        .unwrap_or_default()
}

/// Write `key = "value"` (or remove the key, with None) into config.toml, keeping the rest: the first
/// line starting `key =` is replaced or removed; a new top-level key goes before the first [table].
pub fn set_value(p: &Paths, key: &str, value: Option<&str>) {
    set_many(p, &[(key, value.map(|v| format!("\"{v}\"")))]);
}

/// Several top-level keys at once, each `key = raw` (raw: the TOML value as written, a quoted string or
/// a number), or removed with None; one write, so the compositor reads the file once.
pub fn set_many(p: &Paths, values: &[(&str, Option<String>)]) {
    let mut text = util::read(&p.config);
    for (key, raw) in values {
        text = set_line(&text, key, raw.as_deref());
    }
    let text = regex::Regex::new(r"\n{3,}").unwrap().replace_all(&text, "\n\n").into_owned();
    write_config(p, &text);
}

fn set_line(text: &str, key: &str, raw: Option<&str>) -> String {
    let text = text.to_string();
    let k = regex::escape(key);
    let any = regex::Regex::new(&format!(r"(?m)^{k}\s*=.*\n?")).unwrap();
    match raw {
        None => any.replacen(&text, 1, "").into_owned(),
        Some(v) => {
            let line = format!("{key} = {v}");
            if any.is_match(&text) {
                // a comment after the value stays
                let re = regex::Regex::new(&format!(r#"(?m)^{k}\s*=\s*(?:"(?:[^"\\]|\\.)*"|[^#\n]*?)(\s+#.*)?$"#)).unwrap();
                re.replacen(&text, 1, |c: &regex::Captures| format!("{line}{}", c.get(1).map_or("", |m| m.as_str()))).into_owned()
            } else if let Some(c) = regex::Regex::new(&format!(r#"(?m)^#\s*{k}\s*=\s*(?:"(?:[^"\\]|\\.)*"|[^#\n]*?)(\s+#.*)?$"#)).unwrap().captures(&text) {
                // a commented-out example of the key (config.toml lists them all): set in its place
                let m = c.get(0).unwrap();
                format!("{}{line}{}{}", &text[..m.start()], c.get(1).map_or("", |m| m.as_str()), &text[m.end()..])
            } else {
                let table = regex::Regex::new(r"(?m)^\[").unwrap().find(&text).map(|m| m.start());
                let at = table.unwrap_or(text.len());
                let before = text[..at].trim_end_matches('\n');
                let mut out = if before.is_empty() { String::new() } else { format!("{before}\n") };
                out += &line;
                out.push('\n');
                if table.is_some() {
                    out.push('\n');
                    out += &text[at..];
                }
                out
            }
        }
    }
}

/// Write `theme = "name"` into config.toml, keeping everything else in the file.
pub fn set_theme(p: &Paths, name: &str) {
    let text = util::read(&p.config);
    let line = format!("theme = \"{name}\"");
    let re = regex::Regex::new(r"(?m)^theme\s*=.*$").unwrap();
    let text = if re.is_match(&text) { re.replacen(&text, 1, regex::NoExpand(&line)).into_owned() } else { format!("{line}\n{text}") };
    write_config(p, &text);
}

/// Write table [name] with `values` (strings) into config.toml, in place of the one there (its lines up
/// to the next table, the comments and blank lines leading into that one kept), or at the end.
pub fn set_table(p: &Paths, name: &str, values: &[(String, String)]) {
    let text = util::read(&p.config);
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut block = vec![format!("[{name}]")];
    block.extend(values.iter().map(|(k, v)| format!("{k} = \"{v}\"")));
    let header = |l: &str| {
        let t = l.trim();
        t.starts_with('[') && t.split('#').next().is_some_and(|h| h.trim() == format!("[{name}]"))
    };
    match lines.iter().position(|l| header(l)) {
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.extend(block);
        }
        Some(start) => {
            let mut end = (start + 1..lines.len()).find(|&i| lines[i].trim_start().starts_with('[')).unwrap_or(lines.len());
            while end > start + 1 && (lines[end - 1].trim().is_empty() || lines[end - 1].trim_start().starts_with('#')) {
                end -= 1;
            }
            lines.splice(start..end, block);
        }
    }
    write_config(p, &(lines.join("\n") + "\n"));
}

fn write_config(p: &Paths, text: &str) {
    if let Some(d) = p.config.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&p.config, text) {
        util::die(format!("draftpaper: {}: {e}", p.config.display()));
    }
}
