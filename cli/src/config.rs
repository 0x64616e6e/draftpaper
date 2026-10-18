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
//! Writing goes line by line (set_value, set_in, set_table, set_theme): the user's comments and order stay.

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

/// A key as TOML writes it: bare when it can be, quoted otherwise ("Mod+Return").
fn bare_key(key: &str) -> String {
    if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        key.to_string()
    } else {
        format!("\"{}\"", key.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// A dotted key's parts: `outputs."ASUS MB166C".scale` → outputs, ASUS MB166C, scale (a quoted part may
/// hold dots and spaces); None when a quote is not closed.
pub fn key_parts(s: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut chars = s.trim().chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let mut part = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            loop {
                match chars.next()? {
                    '\\' => part.push(chars.next()?),
                    '"' => break,
                    c => part.push(c),
                }
            }
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c == '.' {
                    break;
                }
                part.push(c);
                chars.next();
            }
            part = part.trim().to_string();
        }
        parts.push(part);
        match chars.next() {
            Some('.') => continue,
            None => return Some(parts),
            Some(_) => return None,
        }
    }
}

/// Is `line` the header of table `name` (`[name]`, a comment after it allowed; not `[[name]]`)? Compared
/// part by part, so spaces and quotes as written do not matter.
fn is_header(line: &str, name: &str) -> bool {
    let t = line.trim();
    if t.starts_with("[[") || !t.starts_with('[') {
        return false;
    }
    let Some(end) = t.find(']') else { return false };
    key_parts(&t[1..end]).is_some_and(|h| Some(h) == key_parts(name))
}

/// A table's name as a header writes it: each part bare when it can be, quoted otherwise.
fn table_name(parts: &[String]) -> String {
    parts.iter().map(|p| bare_key(p)).collect::<Vec<_>>().join(".")
}

/// `key = raw` set inside table [table] (or removed, with None), the rest of the file as it was: the key's
/// line replaced, its comment kept; else a commented example of it in the table set in its place; else a
/// new line after the table's last key; a missing table goes at the end. Table "" is the top level.
pub fn set_in(text: &str, table: &str, key: &str, raw: Option<&str>) -> String {
    if table.is_empty() {
        return set_line(text, key, raw);
    }
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let k = bare_key(key);
    let commented = |l: &str| l.trim_start().strip_prefix('#').is_some_and(|h| is_header(h, table));
    if !lines.iter().any(|l| is_header(l, table)) && raw.is_some() {
        if let Some(i) = lines.iter().position(|l| commented(l)) {
            let body = (i + 1..lines.len()).take_while(|&j| !lines[j].trim_start().starts_with('['));
            if body.clone().all(|j| lines[j].trim().is_empty() || lines[j].trim_start().starts_with('#')) {
                lines[i] = format!("[{table}]");
            }
        }
    }
    let Some(start) = lines.iter().position(|l| is_header(l, table)) else {
        let Some(v) = raw else { return text.to_string() };
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("[{table}]"));
        lines.push(format!("{k} = {v}"));
        return lines.join("\n") + "\n";
    };
    let end = (start + 1..lines.len()).find(|&i| lines[i].trim_start().starts_with('[')).unwrap_or(lines.len());
    let ek = regex::escape(&k);
    let set = regex::Regex::new(&format!(r#"^{ek}\s*=\s*(?:"(?:[^"\\]|\\.)*"|[^#]*?)(\s+#.*)?$"#)).unwrap();
    let example = regex::Regex::new(&format!(r#"^#\s*{ek}\s*=\s*(?:"(?:[^"\\]|\\.)*"|[^#]*?)(\s+#.*)?$"#)).unwrap();
    let keyed = regex::Regex::new(&format!(r"^{ek}\s*=")).unwrap();
    let with = |c: Option<regex::Captures>, v: &str| {
        let head = format!("{k} = {v}");
        match c.and_then(|c| c.get(1)) {
            // the comment stays in its column when it can
            Some(m) => {
                let col = m.start() + m.as_str().len() - m.as_str().trim_start().len();
                format!("{head}{}{}", " ".repeat(col.saturating_sub(head.len()).max(1)), m.as_str().trim_start())
            }
            None => head,
        }
    };
    if let Some(i) = (start + 1..end).find(|&i| keyed.is_match(lines[i].trim_start())) {
        match raw {
            None => {
                lines.remove(i);
            }
            Some(v) => {
                let line = lines[i].trim_start().to_string();
                lines[i] = with(set.captures(&line), v);
            }
        }
    } else if let Some(v) = raw {
        if let Some(i) = (start + 1..end).find(|&i| example.is_match(lines[i].trim_start())) {
            let line = lines[i].trim_start().to_string();
            lines[i] = with(example.captures(&line), v);
        } else {
            // at the table's end, before the comments leading into the next table (a blank line before them)
            let blank = |i: usize| lines[i].trim().is_empty();
            let comment = |i: usize| lines[i].trim_start().starts_with('#');
            let mut e = end;
            while e > start + 1 && blank(e - 1) {
                e -= 1;
            }
            let mut c = e;
            while c > start + 1 && comment(c - 1) {
                c -= 1;
            }
            if c < e && c > start + 1 && blank(c - 1) && end < lines.len() {
                e = c;
                while e > start + 1 && blank(e - 1) {
                    e -= 1;
                }
            }
            lines.insert(e, format!("{k} = {v}"));
        }
    }
    lines.join("\n") + "\n"
}

/// A value as the settings give it, written as TOML: true, false, numbers, lists and quoted strings as
/// they are, anything else quoted.
pub fn raw_value(v: &str) -> String {
    let t = v.trim();
    let plain = t == "true" || t == "false" || t.parse::<f64>().is_ok_and(f64::is_finite) || t.starts_with('[') || t.starts_with('"');
    if plain {
        t.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
    }
}

/// `draftpaper config set KEY=VALUE ...`: dotted keys (title.scale, outputs.eDP-1.scale, margin), the table
/// being everything before the last dot; an empty value removes the key (its default again). One write,
/// refused when the result would not read back as TOML.
pub fn set_pairs(p: &Paths, pairs: &[&str]) {
    let mut text = util::read(&p.config);
    for pair in pairs {
        let Some((key, v)) = pair.split_once('=') else { util::die(format!("draftpaper: config set KEY=VALUE ..., not {pair}")) };
        // the table: the parts before the last (quoted where they must be: outputs."ASUS MB166C")
        let Some(mut parts) = key_parts(key) else { util::die(format!("draftpaper: config set: an unclosed quote in {key}")) };
        let key = parts.pop().unwrap_or_default();
        let table = table_name(&parts);
        let (table, key) = (table.as_str(), key.as_str());
        if key.is_empty() {
            util::die(format!("draftpaper: config set: no key in {pair}"));
        }
        let raw = (!v.is_empty()).then(|| raw_value(v));
        text = set_in(&text, table, key, raw.as_deref());
    }
    let text = regex::Regex::new(r"\n{3,}").unwrap().replace_all(&text, "\n\n").into_owned();
    if let Err(e) = text.parse::<toml::Table>() {
        util::die(format!("draftpaper: config set: that would not read back: {e}"));
    }
    write_config(p, &text);
}

/// The copy of config.toml the settings put back on Escape (`config mark`, `config back`, `config keep`).
fn mark_path(p: &Paths) -> std::path::PathBuf {
    p.runtime.join("config.mark")
}

/// `config mark`: config.toml as it is now, kept (an earlier mark stays: the settings' first change wins).
pub fn mark(p: &Paths) {
    let m = mark_path(p);
    if m.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&p.runtime);
    // a file that was not there marks as empty: back removes nothing the user had
    util::write_atomic(&m, &util::read(&p.config));
}

/// `config back`: config.toml as marked; false when there was no mark.
pub fn back(p: &Paths) -> bool {
    let m = mark_path(p);
    if !m.exists() {
        return false;
    }
    let text = util::read(&m);
    if text != util::read(&p.config) {
        write_config(p, &text);
    }
    let _ = std::fs::remove_file(&m);
    true
}

/// `config keep`: the changes stay, the mark goes.
pub fn keep(p: &Paths) {
    let _ = std::fs::remove_file(mark_path(p));
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "theme = \"softglow\"\n\n[outputs.eDP-1]\nscale = 1.5                # 0.5 to 4\n# position = [0, 0]        # logical pixels\n\n# ---- keys ----\n[bindings]\n# \"Mod+Return\" = \"spawn alacritty\"\n\n[title]\nrevisions = true            # the revision block\n";

    #[test]
    fn replaces_keeping_the_comment() {
        let t = set_in(FILE, "outputs.eDP-1", "scale", Some("1.25"));
        assert!(t.contains("scale = 1.25               # 0.5 to 4\n"), "{t}");
        assert_eq!(t.lines().count(), FILE.lines().count());
        let t = set_in(FILE, "title", "revisions", Some("false"));
        assert!(t.contains("revisions = false           # the revision block\n"), "{t}");
    }

    #[test]
    fn sets_a_commented_example_in_its_place() {
        let t = set_in(FILE, "outputs.eDP-1", "position", Some("[0, 0]"));
        assert!(t.contains("scale = 1.5                # 0.5 to 4\nposition = [0, 0]          # logical pixels\n"), "{t}");
        let t = set_in(FILE, "bindings", "Mod+Return", Some("\"spawn foot\""));
        assert!(t.contains("[bindings]\n\"Mod+Return\" = \"spawn foot\"\n"), "{t}");
    }

    #[test]
    fn adds_after_the_last_key_before_the_next_tables_comments() {
        let t = set_in(FILE, "outputs.eDP-1", "mm", Some("6.4"));
        assert!(t.contains("# position = [0, 0]        # logical pixels\nmm = 6.4\n\n# ---- keys ----\n"), "{t}");
        assert!(t.parse::<toml::Table>().is_ok());
    }

    #[test]
    fn removes_and_adds_tables() {
        let t = set_in(FILE, "title", "revisions", None);
        assert!(!t.contains("revisions"), "{t}");
        assert!(t.contains("[title]\n"));
        let t = set_in(FILE, "rulers", "step", Some("1"));
        assert!(t.ends_with("revisions = true            # the revision block\n\n[rulers]\nstep = 1\n"), "{t}");
        assert_eq!(set_in(FILE, "nowhere", "x", None), FILE);
        let t = set_in(FILE, "", "margin", Some("8"));
        assert!(t.starts_with("theme = \"softglow\"\nmargin = 8\n"), "{t}");
    }

    #[test]
    fn uncomments_a_commented_table() {
        let f = "theme = \"x\"\n\n# [idle]\n# lock_after = 0          # minutes\n\n[apps]\nskip = []\n";
        let t = set_in(f, "idle", "lock_after", Some("5"));
        assert!(t.contains("[idle]\nlock_after = 5            # minutes\n"), "{t}");
        // not when a key sits under it: that key would move into the table
        let f = "# [idle]\n# lock_after = 0\nwallpaper = \"a\"\n[apps]\n";
        let t = set_in(f, "idle", "lock_after", Some("5"));
        assert!(t.starts_with("# [idle]\n") && t.ends_with("[idle]\nlock_after = 5\n"), "{t}");
    }

    #[test]
    fn quoted_names() {
        assert_eq!(key_parts("outputs.\"ASUS MB166C\".scale"), Some(vec!["outputs".into(), "ASUS MB166C".into(), "scale".into()]));
        assert_eq!(key_parts("margin"), Some(vec!["margin".into()]));
        assert_eq!(key_parts("outputs.\"unclosed"), None);
        let parts = vec!["outputs".to_string(), "ASUS MB166C".to_string()];
        assert_eq!(table_name(&parts), "outputs.\"ASUS MB166C\"");
        // a new table, quoted; set again in its place
        let t = set_in(FILE, &table_name(&parts), "scale", Some("1"));
        assert!(t.ends_with("[outputs.\"ASUS MB166C\"]\nscale = 1\n"), "{t}");
        let t = set_in(&t, &table_name(&parts), "scale", Some("1.25"));
        assert_eq!(t.matches("ASUS MB166C").count(), 1);
        assert!(t.contains("[outputs.\"ASUS MB166C\"]\nscale = 1.25\n"), "{t}");
        assert!(t.parse::<toml::Table>().is_ok());
        // a header written with spaces is the same table
        assert!(is_header("[ outputs . \"ASUS MB166C\" ]   # the Asus", "outputs.\"ASUS MB166C\""));
        assert!(!is_header("[[outputs]]", "outputs"));
    }

    #[test]
    fn values() {
        assert_eq!(raw_value("true"), "true");
        assert_eq!(raw_value("1.5"), "1.5");
        assert_eq!(raw_value("[1, 2]"), "[1, 2]");
        assert_eq!(raw_value("IBM Plex Mono"), "\"IBM Plex Mono\"");
        assert_eq!(raw_value("a \"b\""), "\"a \\\"b\\\"\"");
        assert_eq!(raw_value("inf"), "\"inf\"");
    }
}
