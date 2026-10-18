//! The text (config.toml's [text], the settings' Text page): one interface face and one monospace face,
//! each with its size in points, set everywhere at once.
//!
//!     [text]
//!     interface = "IBM Plex Sans"   # GTK (and so Firefox's interface), Qt
//!     size = 11                     # points
//!     monospace = "IBM Plex Mono"   # GTK's and Qt's fixed font, the terminals, Emacs, the sheet's lettering
//!     mono_size = 11                # points
//!     min_degrees = 0.2             # the legibility target the settings letter sizes against
//!
//!   GTK      gsettings font-name, monospace-font-name (running applications follow)
//!   Qt       qt5ct.conf / qt6ct.conf [Fonts] general, fixed (new windows)
//!   Alacritty  [font] size (and family) in draftpaper.toml; alacritty.toml's own [font] family wins
//!   foot     [main] font in draftpaper.ini
//!   Emacs    the default face, at the end of the theme Draftpaper writes (and in a running Emacs)
//!   the shell  theme.json's text: the sheet's lettering in the monospace face (qml/Fonts.qml)
//!
//! What was set before Draftpaper first set any of it is kept (~/.local/state/draftpaper/text.json): a
//! [text] taken out puts that back. [apps] theme = false and skip leave applications alone here too.

use std::path::Path;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::util::{self, Paths};

const IFACE: &str = "org.gnome.desktop.interface";

/// A face and its size, as [text] sets them (either may be left out).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Face {
    pub family: Option<String>,
    pub size: Option<f64>,
}

/// "IBM Plex Sans 11" (GTK's way) as a family and a size.
pub fn parse_gtk(s: &str) -> Option<(String, f64)> {
    let s = s.trim().trim_matches('\'');
    let (family, size) = s.rsplit_once(' ')?;
    Some((family.trim().to_string(), size.parse().ok()?))
}

/// A size as GTK and Qt write it: 11, or 10.5.
fn pt(size: f64) -> String {
    if size.fract() == 0.0 { format!("{}", size as i64) } else { format!("{size}") }
}

/// [text]'s interface and monospace faces.
pub fn faces(cfg: &Map<String, Value>) -> (Face, Face) {
    let t = cfg.get("text").and_then(Value::as_object).cloned().unwrap_or_default();
    let s = |k: &str| t.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    let n = |k: &str| t.get(k).and_then(Value::as_f64).filter(|v| (4.0..=72.0).contains(v));
    (Face { family: s("interface"), size: n("size") }, Face { family: s("monospace"), size: n("mono_size") })
}

fn saved_path(p: &Paths) -> std::path::PathBuf {
    p.state.join("text.json")
}

/// The fonts as they were before Draftpaper set any: GTK's two, and each qtct's general and fixed.
fn current(p: &Paths) -> Value {
    let get = |k: &str| util::run(&["gsettings", "get", IFACE, k], 10).map(|s| s.trim().trim_matches('\'').to_string()).unwrap_or_default();
    let mut qt = Map::new();
    for q in ["qt5ct", "qt6ct"] {
        let text = util::read(&p.config_dir.join(q).join(format!("{q}.conf")));
        let line = |k: &str| Regex::new(&format!(r"(?m)^{k}=(.*)$")).unwrap().captures(&text).map(|c| c[1].to_string()).unwrap_or_default();
        qt.insert(q.into(), json!({ "general": line("general"), "fixed": line("fixed") }));
    }
    json!({ "font-name": get("font-name"), "monospace-font-name": get("monospace-font-name"), "qt": qt })
}

/// What was set before (kept the first time Draftpaper sets the text), or what is set now.
pub fn before(p: &Paths) -> Value {
    serde_json::from_str(&util::read(&saved_path(p))).unwrap_or_else(|_| current(p))
}

/// [text]'s values for the settings: what config.toml sets, the rest as the system has it.
pub fn shown(p: &Paths, cfg: &Map<String, Value>) -> Map<String, Value> {
    let mut t = cfg.get("text").and_then(Value::as_object).cloned().unwrap_or_default();
    let b = before(p);
    let gtk = |k: &str| b[k].as_str().and_then(parse_gtk);
    let (iface, mono) = (gtk("font-name"), gtk("monospace-font-name"));
    let mut fill = |k: &str, v: Value| {
        t.entry(k.to_string()).or_insert(v);
    };
    fill("interface", json!(iface.as_ref().map(|f| f.0.clone()).unwrap_or_else(|| "Sans".into())));
    fill("size", json!(iface.map(|f| f.1).unwrap_or(11.0)));
    fill("monospace", json!(mono.as_ref().map(|f| f.0.clone()).unwrap_or_else(|| "Monospace".into())));
    fill("mono_size", json!(mono.map(|f| f.1).unwrap_or(11.0)));
    fill("min_degrees", json!(0.2));
    t
}

/// `old` with its block between `{c} draftpaper text begin` and `{c} draftpaper text end` (at its end)
/// replaced by `text`, or taken out with None.
pub fn with_block(old: &str, c: &str, text: Option<&str>) -> String {
    let (begin, end) = (format!("{c} draftpaper text begin: [text] in config.toml"), format!("{c} draftpaper text end"));
    let re = Regex::new(&format!(r"(?s)\n*{}.*?{}\n?", regex::escape(&begin), regex::escape(&end))).unwrap();
    let rest = re.replace_all(old, "").trim_end_matches('\n').to_string() + "\n";
    match text {
        Some(t) => format!("{rest}\n{begin}\n{}\n{end}\n", t.trim_end_matches('\n')),
        None => rest,
    }
}

/// The file at `path` with its block as `text`; a file that is not there is left so. Whether it changed.
fn block(path: &Path, c: &str, text: Option<&str>) -> bool {
    if !path.exists() {
        return false;
    }
    let old = util::read(path);
    let new = with_block(&old, c, text);
    new != old && util::write(path, &new)
}

/// What [text] puts at the end of the files Draftpaper writes for the terminals and Emacs (apps.rs writes
/// them with it, so a full apply changes them once, and only when something did).
#[derive(Default)]
pub struct Blocks {
    pub alacritty: Option<String>,
    pub foot: Option<String>,
    pub emacs: Option<String>,
}

pub fn blocks(p: &Paths, cfg: &Map<String, Value>) -> Blocks {
    let (iface, mono) = faces(cfg);
    if iface == Face::default() && mono == Face::default() {
        return Blocks::default();
    }
    let b = before(p);
    let was = b["monospace-font-name"].as_str().and_then(parse_gtk).unwrap_or(("Monospace".into(), 11.0));
    let m = (mono.family.clone().unwrap_or(was.0), mono.size.unwrap_or(was.1));
    let mut alacritty = String::from("[font]\n");
    if let Some(s) = mono.size {
        alacritty += &format!("size = {:.1}\n", s);
    }
    if let Some(f) = &mono.family {
        alacritty += &format!("normal = {{ family = \"{f}\" }}\n");
    }
    let face = mono.family.clone().or(mono.size.map(|_| m.0.clone()));
    Blocks {
        alacritty: (alacritty.lines().count() > 1).then_some(alacritty),
        foot: face.map(|f| format!("[main]\nfont={f}:size={}", pt(m.1))),
        emacs: (mono.family.is_some() || mono.size.is_some())
            .then(|| format!("(set-face-attribute 'default nil :family \"{}\" :height {})", m.0, (m.1 * 10.0).round() as i64)),
    }
}

/// A qtct font line with this family and size, the rest of `was` kept (Qt 6's form when there was none).
fn qt_font(was: &str, family: &str, size: f64) -> String {
    let inner = was.trim().trim_matches('"');
    let rest: Vec<&str> = inner.split(',').skip(2).collect();
    let rest = if inner.is_empty() || rest.is_empty() { "-1,5,400,0,0,0,0,0,0,0,0,0,0,1".to_string() } else { rest.join(",") };
    format!("\"{family},{},{rest}\"", pt(size))
}

/// Set the text everywhere as [text] says; with no [text], put back what was there before.
pub fn apply(p: &Paths, cfg: &Map<String, Value>) {
    let (iface, mono) = faces(cfg);
    let none = iface == Face::default() && mono == Face::default();
    let saved = saved_path(p);
    if none && !saved.exists() {
        return;
    }
    if !none && !saved.exists() {
        util::write(&saved, &util::dumps_indent(&current(p), 1));
    }
    let b = before(p);
    let apps = cfg.get("apps").and_then(Value::as_object).cloned().unwrap_or_default();
    let skip: Vec<String> = apps.get("skip").and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
    let all = apps.get("theme") != Some(&Value::Bool(false));
    let on = |app: &str| all && !skip.iter().any(|s| s == app);
    let conf = &p.config_dir;
    // each face whole: what [text] sets over what was there
    let whole = |f: &Face, key: &str, fallback: &str| -> (String, f64) {
        let was = b[key].as_str().and_then(parse_gtk).unwrap_or((fallback.into(), 11.0));
        (f.family.clone().unwrap_or(was.0), f.size.unwrap_or(was.1))
    };
    let (i, m) = (whole(&iface, "font-name", "Sans"), whole(&mono, "monospace-font-name", "Monospace"));

    if on("gtk") {
        for (k, v) in [("font-name", &i), ("monospace-font-name", &m)] {
            let value = if none { b[k].as_str().unwrap_or("").to_string() } else { format!("{} {}", v.0, pt(v.1)) };
            if !value.is_empty() {
                // only when different: set again, every application restyles
                let now = util::run(&["gsettings", "get", IFACE, k], 10).unwrap_or_default();
                if now.trim().trim_matches('\'') != value {
                    util::run(&["gsettings", "set", IFACE, k, &value], 10);
                }
            }
        }
    }
    if on("qt") {
        for q in ["qt5ct", "qt6ct"] {
            let path = conf.join(q).join(format!("{q}.conf"));
            if !path.exists() {
                continue;
            }
            let was = |k: &str| b["qt"][q][k].as_str().unwrap_or("").to_string();
            let line = |k: &str, f: &(String, f64)| if none { was(k) } else { qt_font(&was(k), &f.0, f.1) };
            let (general, fixed) = (line("general", &i), line("fixed", &m));
            let values: Vec<(&str, String)> = [("general", general), ("fixed", fixed)].into_iter().filter(|(_, v)| !v.is_empty()).collect();
            if !values.is_empty() {
                set_fonts(&path, &values);
            }
        }
    }
    // the terminals and Emacs: their blocks, and a reload only where one changed
    let bl = blocks(p, cfg);
    if on("alacritty") && block(&conf.join("alacritty/draftpaper.toml"), "#", bl.alacritty.as_deref()) {
        let main = conf.join("alacritty/alacritty.toml");
        if main.exists() {
            let _ = std::fs::File::options().append(true).open(&main).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        }
    }
    if on("foot") {
        block(&conf.join("foot/draftpaper.ini"), "#", bl.foot.as_deref());
    }
    if on("emacs") && block(&conf.join("draftpaper/emacs/draftpaper-theme.el"), ";;", bl.emacs.as_deref()) {
        if let Some(s) = &bl.emacs {
            util::run(&["emacsclient", "-q", "-e", s], 10);
        }
    }
    if none {
        let _ = std::fs::remove_file(&saved);
    }
}

/// [Fonts] general and fixed in a qtct.conf, the rest of the file as it is.
fn set_fonts(path: &Path, values: &[(&str, String)]) {
    let text = util::read(path);
    let mut out: Vec<String> = Vec::new();
    let (mut inside, mut found) = (false, false);
    let mut seen: Vec<&str> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if inside {
                out.extend(values.iter().filter(|(k, _)| !seen.contains(k)).map(|(k, v)| format!("{k}={v}")));
            }
            inside = t == "[Fonts]";
            found |= inside;
        } else if inside {
            if let Some((k, v)) = values.iter().find(|(k, _)| t.starts_with(&format!("{k}="))) {
                out.push(format!("{k}={v}"));
                seen.push(k);
                continue;
            }
        }
        out.push(line.to_string());
    }
    if inside {
        out.extend(values.iter().filter(|(k, _)| !seen.contains(k)).map(|(k, v)| format!("{k}={v}")));
    }
    if !found {
        out.push(String::new());
        out.push("[Fonts]".into());
        out.extend(values.iter().map(|(k, v)| format!("{k}={v}")));
    }
    let new = out.join("\n") + "\n";
    if new != text {
        util::write(path, &new);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk_names() {
        assert_eq!(parse_gtk("'IBM Plex Sans 11'"), Some(("IBM Plex Sans".into(), 11.0)));
        assert_eq!(parse_gtk("Cantarell 10.5"), Some(("Cantarell".into(), 10.5)));
        assert_eq!(parse_gtk("Sans"), None);
    }

    #[test]
    fn qt_lines_keep_the_rest() {
        assert_eq!(qt_font("\"IBM Plex Sans,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Regular\"", "Noto Sans", 11.0), "\"Noto Sans,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Regular\"");
        assert_eq!(qt_font("", "Noto Sans", 10.5), "\"Noto Sans,10.5,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\"");
    }

    #[test]
    fn blocks_replace_and_go() {
        let d = std::env::temp_dir().join(format!("draftpaper-text-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let f = d.join("a.toml");
        std::fs::write(&f, "[colors]\nx = 1\n").unwrap();
        block(&f, "#", Some("[font]\nsize = 11.0"));
        block(&f, "#", Some("[font]\nsize = 12.0"));
        let t = std::fs::read_to_string(&f).unwrap();
        assert_eq!(t.matches("draftpaper text begin").count(), 1);
        assert!(t.contains("size = 12.0") && !t.contains("size = 11.0"), "{t}");
        block(&f, "#", None);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "[colors]\nx = 1\n");
        let _ = std::fs::remove_dir_all(&d);
    }
}
