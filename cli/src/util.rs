//! Small things the rest share: where files are, running programs, writing files, and numbers printed
//! the way the themes and the shell expect them.

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;

/// Where Draftpaper finds its files and keeps its state. Installed, the shared files live in
/// /usr/share/draftpaper; run from a checkout (cli/target/...), in the repository itself, unless
/// DRAFTPAPER_ROOT says otherwise. Per user: settings in ~/.config/draftpaper/config.toml, themes of
/// their own in ~/.config/draftpaper/themes, generated files (theme.json) in ~/.cache/draftpaper, runtime
/// state in $XDG_RUNTIME_DIR/draftpaper, state kept across sessions in $XDG_STATE_HOME/draftpaper.
pub struct Paths {
    pub home: PathBuf,
    pub root: PathBuf,
    pub config_dir: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
    pub runtime: PathBuf,
    pub state: PathBuf,
}

impl Paths {
    pub fn new() -> Paths {
        let home = PathBuf::from(env::var_os("HOME").unwrap_or_else(|| "/".into()));
        let xdg = |var: &str, default: PathBuf| env::var_os(var).map(PathBuf::from).unwrap_or(default);
        let root = env::var_os("DRAFTPAPER_ROOT").map(PathBuf::from).or_else(checkout).unwrap_or_else(|| "/usr/share/draftpaper".into());
        let config_dir = xdg("XDG_CONFIG_HOME", home.join(".config"));
        Paths {
            config: config_dir.join("draftpaper/config.toml"),
            cache: xdg("XDG_CACHE_HOME", home.join(".cache")).join("draftpaper"),
            // DRAFTPAPER_RUNTIME: as the shell is given it (start), and for the tests
            runtime: env::var_os("DRAFTPAPER_RUNTIME").map(PathBuf::from).unwrap_or_else(|| xdg("XDG_RUNTIME_DIR", "/tmp".into()).join("draftpaper")),
            state: xdg("XDG_STATE_HOME", home.join(".local/state")).join("draftpaper"),
            config_dir,
            root,
            home,
        }
    }
    pub fn qml(&self) -> PathBuf {
        self.root.join("qml/shell.qml")
    }
    /// The user's themes first (they shadow the shipped ones of the same name), then the shipped.
    pub fn themes(&self) -> [PathBuf; 2] {
        [self.config_dir.join("draftpaper/themes"), self.root.join("themes")]
    }
    pub fn theme_json(&self) -> PathBuf {
        self.cache.join("theme.json")
    }
    pub fn ensure(&self) {
        let _ = std::fs::create_dir_all(&self.cache);
        let _ = std::fs::create_dir_all(&self.runtime);
    }
}

/// Run from a checkout: the repository is the directory above cli/ holding qml/ and themes/.
fn checkout() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?.canonicalize().ok()?;
    exe.ancestors().skip(1).take(5).find(|d| d.join("qml/shell.qml").is_file() && d.join("themes").is_dir() && d.join("cli").is_dir()).map(Path::to_path_buf)
}

/// A program on the PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| env::split_paths(&paths).map(|d| d.join(name)).find(|p| p.is_file()))
}

/// Run a program, its output kept, waiting at most `secs` (a hung one is killed); None when it does not
/// exist or did not finish.
pub fn run(cmd: &[&str], secs: u64) -> Option<String> {
    which(cmd[0])?;
    let mut child = Command::new(cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(1)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    use std::io::Read;
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Write a file (its directory made), only when it changes.
pub fn write(path: &Path, text: &str) -> bool {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return false;
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(path, text) {
        eprintln!("draftpaper: {}: {e}", path.display());
        return false;
    }
    true
}

/// Write a file whole by renaming a sibling into place, so a reader never sees half of it.
pub fn write_atomic(path: &Path, text: &str) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("{msg}");
    std::process::exit(1)
}

/// Round half to even, as the themes' numbers have always been rounded.
pub fn round_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        r
    }
}

/// `x` to `digits` decimal places, a tie to even (formatting rounds the exact value, as the themes' numbers always were).
pub fn round_to(x: f64, digits: i32) -> f64 {
    format!("{:.*}", digits as usize, x).parse().unwrap_or(x)
}

/// A float as the shortest text that reads back as it, with ".0" when it is whole (TOML and JSON both
/// take it so: 54.0, 0.45).
pub fn float(x: f64) -> String {
    if x.fract() == 0.0 && x.is_finite() && x.abs() < 1e16 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

/// A TOML value as JSON (tables keep their order).
pub fn toml_to_json(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map(Value::Number).unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => Value::Object(t.iter().map(|(k, v)| (k.clone(), toml_to_json(v))).collect()),
    }
}

/// A JSON value as text, as Python's json.dumps writes it (", " and ": " between items, non-ASCII as
/// \u escapes), so the shell and the files see what they always have.
pub fn dumps(v: &Value) -> String {
    let mut s = String::new();
    write_json(&mut s, v, None, 0);
    s
}

/// The same, indented by `indent` spaces a level.
pub fn dumps_indent(v: &Value, indent: usize) -> String {
    let mut s = String::new();
    write_json(&mut s, v, Some(indent), 0);
    s
}

fn write_json(s: &mut String, v: &Value, indent: Option<usize>, level: usize) {
    let nl = |s: &mut String, level: usize| {
        if let Some(n) = indent {
            s.push('\n');
            s.push_str(&" ".repeat(n * level));
        }
    };
    let sep = if indent.is_some() { "," } else { ", " };
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => s.push_str(&i.to_string()),
            (_, Some(u), _) => s.push_str(&u.to_string()),
            (_, _, Some(f)) => s.push_str(&float(f)),
            _ => s.push_str("null"),
        },
        Value::String(t) => quote(s, t),
        Value::Array(a) => {
            if a.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(sep);
                }
                nl(s, level + 1);
                write_json(s, x, indent, level + 1);
            }
            nl(s, level);
            s.push(']');
        }
        Value::Object(o) => {
            if o.is_empty() {
                s.push_str("{}");
                return;
            }
            s.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    s.push_str(sep);
                }
                nl(s, level + 1);
                quote(s, k);
                s.push_str(": ");
                write_json(s, x, indent, level + 1);
            }
            nl(s, level);
            s.push('}');
        }
    }
}

fn quote(s: &mut String, t: &str) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    s.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_json_as_before() {
        assert_eq!(round_even(2.5), 2.0);
        assert_eq!(round_even(3.5), 4.0);
        assert_eq!(round_even(2.4), 2.0);
        assert_eq!(float(54.0), "54.0");
        assert_eq!(float(0.45), "0.45");
        assert_eq!(round_to(7.75, 1), 7.8);
        let v: Value = serde_json::from_str(r#"{"a": [1, 2.0, "x°"], "b": {}}"#).unwrap();
        assert_eq!(dumps(&v), "{\"a\": [1, 2.0, \"x\\u00b0\"], \"b\": {}}");
        assert_eq!(dumps_indent(&v, 1), "{\n \"a\": [\n  1,\n  2.0,\n  \"x\\u00b0\"\n ],\n \"b\": {}\n}");
    }
}
