//! What the sheet says, in one place (docs/sheet-content.md).
//!
//! The lines of the sheet are the compositor's and are published in physical pixels; everything else on
//! it is text set into places the compositor has already measured. Which words those are was decided
//! twice — once in QML for the shell, once in Rust for the sheet's own client — and the two accounts
//! disagreed in ten separate ways over two days. This is the one account. Both renderers read it and
//! neither decides anything.
//!
//! Nothing here knows where anything goes, what colour it is, or what font can draw it. An entry names
//! its slot and says what is lettered in it; the ink is a role the theme resolves; a reading that would
//! carry a glyph carries the glyph inline in `text` and the plain words in `words`, and the renderer —
//! the only one that knows which faces are installed — picks between them.
//!
//! The rule for picking: letter `text` where the face has every character in it, else `words`. Where
//! there is no `words` there is nothing to fall back to and `text` is all there is.

use serde_json::{json, Value};

/// The glyphs the readings are lettered behind, by what they stand for. These are Nerd Font characters
/// in the private use area; a machine with no Nerd Font installed letters `words` instead and loses
/// nothing (qml/shell.qml's `g`, sheet/src/glyph.rs).
const BOLT: char = '\u{f140b}';
const CHARGING: char = '\u{f0084}';
const CELLS: [char; 5] = ['\u{f007a}', '\u{f007c}', '\u{f007e}', '\u{f0080}', '\u{f0079}'];
const SAVER: char = '\u{f032a}';
const BALANCED: char = '\u{f0f85}';
const PERFORMANCE: char = '\u{f04c5}';
const PROCESSOR: char = '\u{f0ee0}';
const MEMORY: char = '\u{f035b}';
const ARCS: [char; 5] = ['\u{f092f}', '\u{f091f}', '\u{f0922}', '\u{f0925}', '\u{f0928}'];
const WIRED: char = '\u{f0201}';
const OFFLINE: char = '\u{f092e}';
const BT_ON: char = '\u{f00b1}';
const BT_OFF: char = '\u{f00af}';
const BATTERY_LOW: char = '\u{f0083}';
const SHOT: char = '\u{f0e51}';
const TEMPERATURES: [(&str, char); 3] = [("CPU", '\u{f061a}'), ("NVME", '\u{f02ca}'), ("FAN", '\u{f0210}')];

/// A glyph and the space after it, as the sheet sets one before a reading.
fn mark(ch: char) -> String {
    format!("{ch} ")
}

/// One of `gs` by a level in 0..1: the first for empty, the last for full.
fn ramp(v: f64, gs: &[char]) -> char {
    let i = (v * gs.len() as f64).floor().max(0.0) as usize;
    gs[i.min(gs.len() - 1)]
}

/// One thing the sheet letters. `text` carries its glyphs; `words` is what to letter without them, and
/// is left out where the two would be the same.
fn say(text: String, words: Option<String>) -> Value {
    let mut v = json!({ "text": text.trim_end() });
    if let Some(words) = words.filter(|w| *w != v["text"].as_str().unwrap_or("")) {
        v["words"] = json!(words);
    }
    v
}

/// `say`, with the label the title block letters over it and whatever else the cell carries.
fn cell(label: &str, text: String, words: Option<String>, action: Option<&str>) -> Value {
    let mut v = say(text, words);
    v["label"] = json!(label);
    if let Some(action) = action {
        v["action"] = json!(action);
    }
    v
}

/// The title block's twelve cells, in the order the compositor measures them
/// (compositor/src/title.rs). `theme` is the theme as `draftpaper` writes it to its cache.
pub fn title(theme: &Value, workspace: u64, now: std::time::SystemTime, r: &Value, cpu: Option<f64>, mine: &[Value]) -> Vec<Value> {
    let (date, time) = clock(now);
    let named = |which: &str| {
        r.get("temps")
            .and_then(Value::as_array)?
            .iter()
            .find(|t| t.get("label").and_then(Value::as_str) == Some(which))
            .and_then(|t| t.get("value").and_then(Value::as_str))
            .map(str::to_string)
    };
    let of = |a: &str, b: &str| theme.get(a).and_then(|x| x.get(b)).and_then(Value::as_str).unwrap_or("").to_string();
    // a processor reading given in the readings wins over one taken here: it is how a test freezes the
    // machine for two renderers at once (cli/src/readings.rs)
    let cpu = r.get("cpu").and_then(Value::as_f64).or(cpu);
    let pct = |v: Option<f64>| v.map(|v| format!("{}%", (v * 100.0).round())).unwrap_or_else(|| "-".into());

    // the clock is the one cell lettered large, and it is said here rather than worked out from the
    // height the cell was measured at: the block's proportions stretch the whole top row past the
    // threshold the shell's own table uses, so measuring it letters eleven cells at the clock's size
    let mut clock_cell = cell("TIME", time, None, None);
    clock_cell["large"] = json!(true);

    let mut battery_cell = cell("BATTERY", battery(r).0, battery(r).1, Some("graph.battery"));
    if r.get("battery_low").and_then(Value::as_bool).unwrap_or(false) {
        battery_cell["ink"] = json!("alert");
    }

    let (temps, temp_words): (Vec<String>, Vec<String>) = TEMPERATURES
        .iter()
        .filter_map(|(k, g)| named(k).map(|v| (format!("{}{v}", mark(*g)), format!("{k} {v}"))))
        .unzip();
    let mut temps_cell = cell(
        "TEMPERATURE · FAN",
        temps.join(" ").trim().to_string(),
        Some(temp_words.join(" ").trim().to_string()),
        Some("graph.temperature"),
    );
    if r.get("hot").and_then(Value::as_bool).unwrap_or(false) {
        temps_cell["ink"] = json!("alert");
    }

    let (load, memory) = (pct(cpu), pct(crate::metrics::memory()));
    let profile = r.get("profile").and_then(Value::as_str).unwrap_or("-").to_lowercase();

    vec![
        clock_cell,
        cell("DATE", date, None, None),
        cell("SHEET", workspace.to_string(), None, None),
        cell("PROJECT", of("title", "project").to_uppercase(), None, Some("schedule.equipment")),
        {
            let (text, words) = power(r);
            cell("POWER", text, words, Some("graph.power"))
        },
        battery_cell,
        {
            let (text, words) = profiled(&profile);
            cell("PROFILE", text, words, Some("profile.next"))
        },
        // behind their glyphs the two readings stand apart on their own; without them the stroke between
        // says which is which. The shell writes two spaces here and in the temperatures, but its text is
        // styled and styled text collapses them, so what the sheet carries is one.
        cell(
            "CPU / MEMORY",
            format!("{}{load} {}{memory}", mark(PROCESSOR), mark(MEMORY)),
            Some(format!("{load} / {memory}")),
            Some("graph.load"),
        ),
        {
            let (text, words) = network(r);
            cell("NETWORK", text, words, Some("graph.network"))
        },
        temps_cell,
        cell(
            "PROCESSES",
            r.get("processes").and_then(Value::as_u64).map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
            None,
            Some("schedule.processes"),
        ),
        // the last cell is the first of the user's own where there is one, as DRAWN's place has always
        // been its to take (README, docs/sheet-content.md)
        match mine.first() {
            Some(first) => {
                let mut c = first.clone();
                if c.get("label").is_none() {
                    c["label"] = json!("");
                }
                c
            }
            None => cell("DRAWN", of("title", "drawn"), None, None),
        },
    ]
}

/// What is drawn, in watts, or that nothing is because the machine is on the mains.
fn power(r: &Value) -> (String, Option<String>) {
    let words = match r.get("power").and_then(Value::as_f64) {
        Some(w) if w > 0.0 => format!("{w:.1} W"),
        _ => "mains".into(),
    };
    (format!("{}{words}", mark(BOLT)), Some(words))
}

/// The charge, behind a cell that fills as it does, or the bolt while it is charging. Without the glyph
/// the word says what the glyph would have.
fn battery(r: &Value) -> (String, Option<String>) {
    let Some(v) = r.get("battery").and_then(Value::as_f64) else { return ("-".into(), None) };
    let charging = r.get("charging").and_then(Value::as_bool).unwrap_or(false);
    let figure = format!("{}%", (v * 100.0).round());
    let ch = if charging { CHARGING } else { ramp(v, &CELLS) };
    let words = if charging { format!("{figure} charging") } else { figure.clone() };
    (format!("{}{figure}", mark(ch)), Some(words))
}

/// The power profile. The glyph takes room the cell has not got, so beside one the longer names are
/// shortened and they are lettered whole where there is no glyph, as the shell letters them.
fn profiled(p: &str) -> (String, Option<String>) {
    let ch = match p {
        "power-saver" => SAVER,
        "performance" => PERFORMANCE,
        _ => BALANCED,
    };
    let short = match p {
        "power-saver" => "saver",
        "performance" => "perf",
        p => p,
    };
    (format!("{}{short}", mark(ch)), Some(p.to_string()))
}

/// The network behind the arc its signal fills, the plug where it is wired, or the struck-out arc where
/// it is neither.
fn network(r: &Value) -> (String, Option<String>) {
    if let Some(wifi) = r.get("wifi").filter(|w| !w.is_null()) {
        let ssid = wifi.get("ssid").and_then(Value::as_str).unwrap_or("").to_string();
        let signal = wifi.get("signal").and_then(Value::as_f64).unwrap_or(0.0) / 100.0;
        return (format!("{}{ssid}", mark(ramp(signal, &ARCS))), Some(ssid));
    }
    match r.get("wired").and_then(Value::as_str) {
        Some(address) => (format!("{}{address}", mark(WIRED)), Some(format!("wired {address}"))),
        None => (format!("{}offline", mark(OFFLINE)), Some("offline".into())),
    }
}

/// The clock, as the title block letters it: "Mon 19 Oct 2026" and "18:45", in the machine's own time
/// zone (the system's, through localtime_r, so a change of zone or of summer time is followed).
fn clock(now: std::time::SystemTime) -> (String, String) {
    // the same reason as DRAFTPAPER_READINGS: a test that compares two renderers wants the same clock
    // on both, and they are not drawn in the same second
    let now = match std::env::var("DRAFTPAPER_NOW").ok().and_then(|s| s.parse::<u64>().ok()) {
        Some(secs) => std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs),
        None => now,
    };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const WEEK: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let secs = now.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: localtime_r writes into `tm` and reads only `secs`; it is the thread-safe form
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return (String::from("-"), String::from("-"));
    }
    let month = MONTHS.get(tm.tm_mon.clamp(0, 11) as usize).copied().unwrap_or("");
    let weekday = WEEK.get(tm.tm_wday.clamp(0, 6) as usize).copied().unwrap_or("");
    (
        format!("{weekday} {:02} {month} {}", tm.tm_mday, tm.tm_year + 1900),
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min),
    )
}

/// The glyph a window is marked with along the top margin, by what it calls itself. The first entry
/// whose name is part of the id wins, and anything unrecognised is a window like any other
/// (qml/shell.qml's `appGlyphs`).
fn app_mark(app_id: &str) -> char {
    const APPS: [(&[&str], char); 21] = [
        (&["firefox", "librewolf", "zen"], '\u{f0239}'),
        (&["chromium", "chrome", "brave"], '\u{f02af}'),
        (&["alacritty", "foot", "kitty", "xterm", "wezterm", "terminal", "konsole", "ghostty"], '\u{f120}'),
        (&["code", "codium"], '\u{f0a1e}'),
        (&["claude"], '\u{f06a9}'),
        (&["thunderbird", "evolution", "geary"], '\u{f01f0}'),
        (&["signal"], '\u{f0b79}'),
        (&["telegram"], '\u{f2c6}'),
        (&["teams"], '\u{f02bb}'),
        (&["slack"], '\u{f04b1}'),
        (&["discord", "vesktop"], '\u{f066f}'),
        (&["spotify"], '\u{f04c7}'),
        (&["mpv", "vlc", "totem", "celluloid"], '\u{f057c}'),
        (&["gimp", "inkscape", "krita"], '\u{f0e3e}'),
        (&["libreoffice", "soffice"], '\u{f0219}'),
        (&["nautilus", "thunar", "pcmanfm", "nemo", "dolphin"], '\u{f024b}'),
        (&["evince", "zathura", "okular", "papers"], '\u{f0226}'),
        (&["keepassxc", "bitwarden"], '\u{f030b}'),
        (&["obs"], '\u{f044b}'),
        (&["steam"], '\u{f1b6}'),
        (&["emacs"], '\u{e632}'),
    ];
    let id = app_id.to_lowercase();
    APPS.iter().find(|(names, _)| names.iter().any(|n| id.contains(n))).map(|(_, g)| *g).unwrap_or('\u{f05af}')
}

/// The workspaces this display shows, in the first zone of its top margin: the number, and after it the
/// windows on it, up to three, each by its own mark and no mark twice. A parked window is not on the
/// sheet and is not counted (qml/shell.qml's `spaceMarkup`).
fn workspaces(spaces: &[Value], windows: &[Value], display: &str) -> Vec<Value> {
    spaces
        .iter()
        .filter(|w| w.get("output").and_then(Value::as_str) == Some(display))
        .map(|w| {
            let number = w.get("number").and_then(Value::as_u64).unwrap_or(0);
            let mut marks: Vec<char> = Vec::new();
            for win in windows {
                if win.get("workspace").and_then(Value::as_u64) != Some(number) || win.get("parked").and_then(Value::as_bool).unwrap_or(false) {
                    continue;
                }
                let g = app_mark(win.get("app_id").and_then(Value::as_str).unwrap_or(""));
                if !marks.contains(&g) {
                    marks.push(g);
                }
            }
            let marked: String = marks.iter().take(3).map(|g| format!(" {g}")).collect();
            let mut v = say(format!("{number}{marked}"), Some(number.to_string()));
            v["number"] = json!(number);
            v["focused"] = json!(w.get("focused").and_then(Value::as_bool).unwrap_or(false));
            v
        })
        .collect()
}

/// The windows parked down the side margins, each by the cell it is parked in.
fn parked(windows: &[Value], workspace: u64) -> Vec<Value> {
    windows
        .iter()
        .filter(|w| w.get("parked").and_then(Value::as_bool).unwrap_or(false))
        .filter(|w| w.get("workspace").and_then(Value::as_u64) == Some(workspace))
        .filter_map(|w| {
            let cell = w.get("cell").and_then(Value::as_u64)?;
            let title = w.get("title").and_then(Value::as_str).filter(|t| !t.is_empty());
            let name = title.or_else(|| w.get("app_id").and_then(Value::as_str)).unwrap_or("");
            let id = w.get("id").and_then(Value::as_u64)?;
            Some(json!({ "cell": cell, "text": name, "action": format!("window.{id}") }))
        })
        .collect()
}

/// What is connected over Bluetooth, in the first zone of the foot. The glyph says whether anything is,
/// so without it the reading says so in words.
fn bluetooth(r: &Value) -> Value {
    let names: Vec<&str> = r.get("bt").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let joined = names.join(", ");
    let connected = !names.is_empty();
    let mut v = say(
        format!("{}{joined}", mark(if connected { BT_ON } else { BT_OFF })),
        Some(if connected { format!("BT {joined}") } else { "BT -".into() }),
    );
    v["action"] = json!("run.bluetooth");
    v
}

/// What wants saying now, in the last zone of the top margin: a reading past the alarm the theme sets
/// for it. The volume and the brightness are lettered there too, for a few seconds after they move, but
/// they move faster than this document is asked for and stay with whoever letters the sheet.
fn status(r: &Value) -> Vec<Value> {
    let mut row = Vec::new();
    if r.get("hot").and_then(Value::as_bool).unwrap_or(false) {
        if let Some(v) = r.get("temps").and_then(Value::as_array).and_then(|a| a.iter().find(|t| t["label"] == "CPU")).and_then(|t| t["value"].as_str()) {
            let mut e = say(format!("{}{v}", mark(TEMPERATURES[0].1)), Some(format!("CPU {v}")));
            e["ink"] = json!("alert");
            row.push(e);
        }
    }
    if r.get("battery_low").and_then(Value::as_bool).unwrap_or(false) {
        if let Some(b) = r.get("battery").and_then(Value::as_f64) {
            let figure = format!("{}%", (b * 100.0).round());
            let mut e = say(format!("{}{figure}", mark(BATTERY_LOW)), Some(format!("BAT {figure}")));
            e["ink"] = json!("alert");
            row.push(e);
        }
    }
    row
}

/// What a cell or an extension last printed, without asking it again.
fn kept(p: &crate::util::Paths, key: &str) -> Option<String> {
    let note = p.cache.join("extensions").join(format!("{key}.json"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(note).ok()?).ok()?;
    v.get("out").and_then(Value::as_str).map(str::to_string)
}

/// Whether what it printed is a schedule rather than a cell's line.
fn is_schedule(out: &str) -> bool {
    serde_json::from_str::<Value>(out.trim()).is_ok_and(|v| v.get("rows").is_some())
}

/// One cell or extension run, no oftener than it asks for. What it printed last is kept beside the
/// theme with the second it was printed in and given back until its time is up; one that hangs is
/// killed after a second and the last answer stands.
fn ran(p: &crate::util::Paths, key: &str, d: &Value) -> Option<String> {
    let command: Vec<String> = d.get("command").and_then(Value::as_array)?.iter().filter_map(Value::as_str).map(str::to_string).collect();
    if command.is_empty() {
        return None;
    }
    let every = d.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1);
    let kept = p.cache.join("extensions");
    let note = kept.join(format!("{key}.json"));
    let before: Value = std::fs::read_to_string(&note).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let stamp = before.get("stamp").and_then(Value::as_u64).unwrap_or(0);
    let out = before.get("out").and_then(Value::as_str).unwrap_or("").to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if now.saturating_sub(stamp) >= every {
        // asked again, and not waited for. A sheet is drawn from what the last asking left behind, so
        // whatever this one has to say is lettered the next time round — a second late on a reading
        // taken every five, and none at all on one taken every minute. Waiting instead would put the
        // sheet at the mercy of whatever the user put on it: an extension that asks the network, which
        // is most of the interesting ones, takes longer than a sheet should ever be held up for.
        //
        // The stamp is written now rather than when the answer comes, so that it is not asked again
        // before its interval is up, however long it takes to answer.
        let _ = std::fs::create_dir_all(&kept);
        crate::util::write(&note, &crate::util::dumps(&json!({ "stamp": now, "out": out })));
        if let Ok(me) = std::env::current_exe() {
            let _ = std::process::Command::new(me)
                .args(["cell-run", key])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
    (!out.is_empty()).then_some(out)
}

/// What an extension declared about itself, as its command is told it.
///
/// An extension is two things in one file: a declaration and a command. Until now only the sheet could
/// read the declaration, so anything the command needed to know — which repository, which airport,
/// which feed — had to be written into the command itself, and every one of those examples opened with
/// a line telling the reader to go and edit a shell variable in the middle of a script. The settings
/// are data; the command is told them.
///
/// Two ways, because there are two kinds of setting. Every scalar at the top of the file is an
/// environment variable of its own, `repo = "owner/name"` as `$DRAFTPAPER_REPO`, which is what a shell
/// wants. Anything with a shape to it — a table, a list of tables, the holdings of a position — has
/// nowhere to go in an environment variable, so the whole declaration is there as JSON in
/// `$DRAFTPAPER_EXTENSION` for `jq` to read. The command itself is not in it: a script does not need
/// to be handed its own text.
fn settings(d: &Value) -> Vec<(String, String)> {
    let mut env = Vec::new();
    let Some(table) = d.as_object() else { return env };
    for (k, v) in table {
        let value = match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        let name: String = k.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
        env.push((format!("DRAFTPAPER_{name}"), value));
    }
    let mut whole = d.clone();
    if let Some(t) = whole.as_object_mut() {
        t.remove("command");
    }
    env.push(("DRAFTPAPER_EXTENSION".to_string(), crate::util::dumps(&whole)));
    env
}

/// Ask one cell or extension what it has to say and keep the answer, for the asking that comes after.
/// This is what `ran` spawns and does not wait for; it is given twenty seconds, which is long for a
/// sheet and short for a network, and nothing on the sheet waits on it either way.
pub fn run_one(p: &crate::util::Paths, key: &str) {
    let d = match declared(p).into_iter().find(|(k, _)| k == key) {
        Some((_, d)) => d,
        None => return,
    };
    let command: Vec<String> = d.get("command").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    if command.is_empty() {
        return;
    }
    let argv: Vec<&str> = command.iter().map(String::as_str).collect();
    let Some(out) = crate::util::run_with(&argv, 20, &settings(&d)) else { return };
    let kept = p.cache.join("extensions");
    let note = kept.join(format!("{key}.json"));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = std::fs::create_dir_all(&kept);
    crate::util::write(&note, &crate::util::dumps(&json!({ "stamp": now, "out": out })));
}

/// The sheet carries what the user puts on it as well: the `[[cell]]`s of the config, and the
/// extensions beside it in `extensions/`. Both are the same thing — a program that prints what it has
/// to say — and both end up as entries of this document like any other, so an extension cannot letter
/// in a face of its own, an ink off the theme's palette, or anywhere but the slot it was given. It
/// supplies words. The sheet sets them.
///
/// What a program prints is its say. A line of text is the whole of it; a JSON object with a `text` in
/// it is an entry of this document (docs/sheet-content.md), so an extension that wants a glyph, the
/// alert ink or words to fall back on can have them.
///
/// Each says how often it wants running and is run no oftener: what it printed last is kept beside the
/// theme with the second it was printed in, and given back until its time is up. One that hangs is
/// killed after a second and the last answer stands, and once a second and a half has gone on them
/// altogether the rest are left for the next time the sheet is read. A sheet is drawn every few
/// seconds; nothing the user puts on it is allowed to be the reason it is not.
/// Everything the user has put on the sheet, in the order it goes there: the config's `[[cell]]`s
/// first, then the extensions, one to a file and in the order their names fall.
fn declared(p: &crate::util::Paths) -> Vec<(String, Value)> {
    let cfg = crate::config::load(p);
    let mut all: Vec<(String, Value)> = cfg
        .get("cell")
        .and_then(Value::as_array)
        .map(|a| a.iter().enumerate().map(|(i, c)| (format!("cell-{i}"), c.clone())).collect())
        .unwrap_or_default();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(p.config_dir.join("draftpaper/extensions"))
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().is_some_and(|x| x == "toml")).collect())
        .unwrap_or_default();
    files.sort();
    for f in files {
        let Some(stem) = f.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        if let Ok(v) = toml::from_str::<Value>(&text) {
            all.push((format!("ext-{stem}"), v));
        }
    }
    all
}

pub fn extensions(p: &crate::util::Paths) -> Vec<Value> {
    let mut said = Vec::new();
    for (key, d) in declared(p) {
        // one that printed a schedule is not a cell, and is not asked again here: it is read when it is
        // called up by name (`mine_schedule`), not every time the sheet is drawn. Otherwise one that
        // asks the network would go on asking for ever with nothing of it anywhere on the sheet.
        if kept(p, &key).is_some_and(|o| is_schedule(&o)) {
            continue;
        }
        let Some(out) = ran(p, &key, &d) else { continue };
        if is_schedule(&out) {
            continue;
        }
        let line = out.lines().next().unwrap_or("").trim().to_string();
        if line.is_empty() {
            continue;
        }
        // its own say, where it printed one; else the line it printed
        let mut v = match serde_json::from_str::<Value>(&line) {
            Ok(v) if v.get("text").is_some() => v,
            _ => say(line, None),
        };
        if let Some(label) = d.get("label").and_then(Value::as_str) {
            v["label"] = json!(label);
        }
        for (side, name) in [("left", "action"), ("right", "action_right")] {
            if d.get(side).and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                v[name] = json!(format!("{key}.{side}"));
            }
        }
        said.push(v);
    }
    said
}

/// The schedules Mod+i goes through: those below, less any extension whose file says `cycle = false`.
/// One left out of the turn is still picked from the register and still answers to its own name — it is
/// only kept out of the way of a key that has to go past everything to get anywhere.
pub fn cycled_names(p: &crate::util::Paths) -> Vec<String> {
    let skipped: Vec<String> = declared(p)
        .iter()
        .filter(|(_, d)| d.get("cycle") == Some(&json!(false)))
        .filter_map(|(k, _)| k.strip_prefix("ext-").map(str::to_string))
        .collect();
    schedule_names(p).into_iter().filter(|n| !skipped.contains(n)).collect()
}

/// Every schedule that can be called up: the three the sheet comes with, then the user's own. An
/// extension counts as one where what it last printed was a schedule, or where it has not been asked
/// yet — a new one is pickable at once rather than after its first answer.
pub fn schedule_names(p: &crate::util::Paths) -> Vec<String> {
    let mut names: Vec<String> = ["equipment", "processes", "revisions"].map(str::to_string).to_vec();
    for (key, _) in declared(p) {
        let Some(name) = key.strip_prefix("ext-") else { continue };
        // one that has not been asked yet counts, so a new extension is pickable the moment its file
        // appears rather than after its first answer
        if kept(p, &key).is_none_or(|o| o.trim().is_empty() || is_schedule(&o)) {
            names.push(name.to_string());
        }
    }
    names
}

/// A schedule of the user's own, where an extension of that name printed one. It is read the same way
/// a cell of theirs is — no oftener than its interval asks, killed if it hangs — and what it printed is
/// taken as a schedule of this document (docs/sheet-content.md): a heading, columns and rows.
///
/// The three the sheet comes with are not these. They are read in process, they want things an
/// extension has no way to ask for — the displays as the compositor publishes them, the rows last read
/// when every window is parked — and one of them samples the processes over half a second, which is a
/// long time to hold a sheet up for. They are what the sheet is; this is what you add to it.
fn mine_schedule(p: &crate::util::Paths, name: &str) -> Option<Value> {
    let f = p.config_dir.join("draftpaper/extensions").join(format!("{name}.toml"));
    let d: Value = toml::from_str(&std::fs::read_to_string(&f).ok()?).ok()?;
    as_schedule(name, &ran(p, &format!("ext-{name}"), &d)?)
}

/// What a program printed, taken as a schedule of this document, or None where it did not print one.
fn as_schedule(name: &str, out: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(out.trim()).ok()?;
    v.get("rows")?;
    let columns: Vec<Value> = v
        .get("columns")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|c| {
                    column(
                        c.get("name").and_then(Value::as_str).unwrap_or(""),
                        c.get("mm").and_then(Value::as_f64).unwrap_or(0.0),
                        c.get("right").and_then(Value::as_bool).unwrap_or(false),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    Some(json!({
        "title": v.get("title").and_then(Value::as_str).unwrap_or(&name.to_uppercase()),
        "meta": v.get("meta").and_then(Value::as_str).unwrap_or(""),
        "columns": columns,
        "rows": v.get("rows").cloned().unwrap_or(json!([])),
        "note": v.get("note").and_then(Value::as_i64).unwrap_or(-1),
        "bar": v.get("bar").and_then(Value::as_i64).unwrap_or(-1),
    }))
}

/// What a `{key}.left` or `{key}.right` runs: whatever the cell or the extension it names says to, and
/// the settings it is told (`settings`).
pub fn runs(p: &crate::util::Paths, action: &str) -> Option<(Vec<String>, Vec<(String, String)>)> {
    let (key, side) = action.rsplit_once('.')?;
    let cfg = crate::config::load(p);
    let d = if let Some(i) = key.strip_prefix("cell-") {
        cfg.get("cell")?.as_array()?.get(i.parse::<usize>().ok()?)?.clone()
    } else {
        let stem = key.strip_prefix("ext-")?;
        let text = std::fs::read_to_string(p.config_dir.join("draftpaper/extensions").join(format!("{stem}.toml"))).ok()?;
        toml::from_str::<Value>(&text).ok()?
    };
    let cmd: Vec<String> = d.get(side)?.as_array()?.iter().filter_map(Value::as_str).map(str::to_string).collect();
    // what a click runs is told what the extension declared, as the extension's own command is
    (!cmd.is_empty()).then(|| (cmd, settings(&d)))
}

/// The volume, and whether it is muted. The sheet letters this for a few seconds after it moves, so
/// the level itself is whoever letters the sheet's to watch — but how it reads is said here, so that
/// two renderers cannot come to letter it differently (docs/sheet-content.md).
pub fn volume(level: f64, muted: bool) -> Value {
    const STEPS: [char; 3] = ['\u{f057f}', '\u{f0580}', '\u{f057e}'];
    let figure = if muted { "muted".to_string() } else { format!("{}%", (level * 100.0).round()) };
    let ch = if muted { '\u{f075f}' } else { ramp(level, &STEPS) };
    say(format!("{}{figure}", mark(ch)), Some(format!("VOL {figure}")))
}

/// The brightness, the same way.
pub fn brightness(level: f64) -> Value {
    const STEPS: [char; 3] = ['\u{f00de}', '\u{f00df}', '\u{f00e0}'];
    let figure = format!("{}%", (level * 100.0).round());
    say(format!("{}{figure}", mark(ramp(level, &STEPS))), Some(format!("BRT {figure}")))
}

/// The last zone of the foot: the screenshot, where the config names one to take, and the theme the
/// sheet is drawn in.
fn foot(theme: &Value) -> Vec<Value> {
    let mut end = Vec::new();
    if theme.get("commands").and_then(|c| c.get("screenshot")).and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
        let mut shot = say(SHOT.to_string(), Some("SHOT".into()));
        shot["action"] = json!("run.screenshot");
        end.push(shot);
    }
    let mut name = say(theme.get("label").and_then(Value::as_str).unwrap_or("").to_uppercase(), None);
    name["action"] = json!("settings.toggle");
    name["action_right"] = json!("theme.next");
    end.push(name);
    end
}

/// What the margins of one display carry.
pub fn margins(theme: &Value, r: &Value, spaces: &[Value], windows: &[Value], display: &str, workspace: u64, mine: &[Value]) -> Value {
    json!({
        "top": { "workspaces": workspaces(spaces, windows, display), "status": status(r) },
        // the user's own after the first, a zone each along the foot; the first took the title block's
        // last cell (`title`)
        "bottom": { "start": bluetooth(r), "end": foot(theme), "cells": mine.iter().skip(1).cloned().collect::<Vec<Value>>() },
        "sides": parked(windows, workspace),
    })
}

/// One row of a schedule: what it says across the columns, the note under it, whether a heavier rule
/// runs above it, and the share of the column that is ruled as a bar.
fn row(cells: Vec<String>, note: &str, share: Option<f64>) -> Value {
    let mut v = json!({ "cells": cells });
    if !note.is_empty() {
        v["note"] = json!(note);
    }
    if let Some(share) = share {
        v["share"] = json!(share);
    }
    v
}

fn column(name: &str, mm: f64, right: bool) -> Value {
    json!({ "name": name, "mm": mm, "right": right })
}

/// How long the machine has been up, as a drawing notes it.
fn up(seconds: f64) -> String {
    let s = seconds as u64;
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    if d > 0 {
        format!("up {d} d {h} h")
    } else if h > 0 {
        format!("up {h} h {m} min")
    } else {
        format!("up {m} min")
    }
}

/// The machine itself, as a drawing schedules its equipment: a reference, the item, what it is, and a
/// note under it. The references are lettered in groups and a heavier rule runs where the letter changes.
fn equipment(p: &crate::util::Paths, displays: &[Value], sheet: &str) -> Value {
    let m = crate::machine::json(p);
    let mut rows: Vec<Value> = Vec::new();
    let mut refs: Vec<String> = Vec::new();
    let mut add = |reference: String, item: String, text: String, note: String, action: Option<&str>| {
        if !text.is_empty() {
            refs.push(reference.clone());
            let mut r = row(vec![reference, item, text], &note, None);
            if let Some(action) = action {
                r["action"] = json!(action);
            }
            rows.push(r);
        }
    };
    let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    add("M1".into(), "MACHINE".into(), s("machine"), String::new(), None);
    if let Some(cpu) = m.get("cpu") {
        let cores = cpu.get("cores").and_then(Value::as_u64).unwrap_or(0);
        let threads = cpu.get("threads").and_then(Value::as_u64).unwrap_or(0);
        add("M2".into(), "PROCESSOR".into(), cpu.get("model").and_then(Value::as_str).unwrap_or("").to_string(), format!("{cores} cores, {threads} threads"), None);
    }
    let gpus: Vec<&str> = m.get("gpus").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    for (i, g) in gpus.iter().enumerate() {
        add(format!("M{}", 3 + i), "GRAPHICS".into(), g.to_string(), String::new(), None);
    }
    let n = 3 + gpus.len().max(1);
    if let Some(gib) = m.get("memory_gib").and_then(Value::as_f64) {
        add(format!("M{n}"), "MEMORY".into(), format!("{} GiB", gib.round()), String::new(), None);
    }
    if let Some(st) = m.get("storage") {
        let used = st.get("used_gb").and_then(Value::as_f64);
        let size = st.get("size_gb").and_then(Value::as_f64);
        let note = [
            st.get("fs").and_then(Value::as_str).unwrap_or("").to_string(),
            match (used, size) {
                (Some(u), Some(t)) => format!("{} of {} GB used", u.round(), t.round()),
                _ => String::new(),
            },
        ]
        .iter()
        .filter(|t| !t.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
        add(format!("M{}", n + 1), "STORAGE".into(), st.get("disk").and_then(Value::as_str).unwrap_or("").to_string(), note, Some("graph.disk"));
    }
    let system = [s("system"), s("kernel")].iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
    let uptime = m.get("uptime_s").and_then(Value::as_f64).map(up).unwrap_or_default();
    add("S1".into(), "SYSTEM".into(), system, uptime, None);
    let desktop = format!("Draftpaper {}{}", s("draftpaper"), if s("quickshell").is_empty() { String::new() } else { format!(" · Quickshell {}", s("quickshell")) });
    add("S2".into(), "DESKTOP".into(), desktop, String::new(), None);
    for (i, d) in displays.iter().enumerate() {
        let name = d.get("name").and_then(Value::as_str).unwrap_or("");
        let what = d.get("what").and_then(Value::as_str).unwrap_or("").to_string();
        let note = d.get("note").and_then(Value::as_str).unwrap_or("").to_string();
        add(format!("D{}", i + 1), format!("DISPLAY {name}"), what, note, None);
    }
    // the sheet itself is equipment too: the theme it is drawn in and how it is ruled
    add("P1".into(), "SHEET".into(), sheet.to_string(), String::new(), None);
    // a heavier rule where the references change letter, as a drawing groups its items
    for i in 1..rows.len() {
        if refs[i - 1].chars().next() != refs[i].chars().next() {
            rows[i]["heavy"] = json!(true);
        }
    }
    json!({
        "title": "SCHEDULE OF EQUIPMENT",
        "meta": "",
        "columns": [column("REF", 9.0, false), column("ITEM", 27.0, false), column("DESCRIPTION", 0.0, false)],
        "rows": rows,
        "note": 2,
        "bar": -1,
    })
}

/// What the machine is doing: the busiest programs first, each one's share of a processor ruled as a bar.
fn processes() -> Value {
    let p = crate::processes::json(40);
    let load = p.get("load").and_then(Value::as_str).unwrap_or("").to_string();
    let count = p.get("processes").and_then(Value::as_u64).unwrap_or(0);
    let rows: Vec<Value> = p
        .get("rows")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .enumerate()
                .map(|(i, r)| {
                    let cpu = r.get("cpu").and_then(Value::as_f64).unwrap_or(0.0);
                    let pid = r
                        .get("pid")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| r.get("pid").and_then(Value::as_u64).map(|v| v.to_string()))
                        .unwrap_or_default();
                    row(
                        vec![
                            format!("{:02}", i + 1),
                            r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                            pid,
                            format!("{cpu:.1}"),
                            r.get("memory").and_then(Value::as_str).unwrap_or("").to_string(),
                            r.get("time").and_then(Value::as_str).unwrap_or("").to_string(),
                        ],
                        "",
                        Some(cpu / 100.0),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "title": "PROCESS SCHEDULE",
        "meta": format!("BY CPU · LOAD {load} · {count} PROCESSES"),
        "columns": [
            column("NO.", 8.0, false), column("PROCESS", 0.0, false), column("PID", 15.0, true),
            column("CPU %", 24.0, true), column("MEMORY", 17.0, true), column("TIME", 16.0, true)
        ],
        "rows": rows,
        "note": -1,
        "bar": 3,
    })
}

/// What has been drawn lately, from the repository the focused window sits in. Over a window that is in
/// no repository it has no rows, and stands on the sheet empty rather than not at all.
fn revisions(sheet: u64) -> Value {
    let p = crate::util::Paths::new();
    let r = crate::revisions::json(&p);
    // nothing focused, because every window is parked: the rows last read stay, so the bare sheet goes
    // on showing the work just left. The shell held them in memory; a command that runs once and ends
    // keeps them beside the theme, as the head they were read at is kept.
    let kept = p.cache.join("revisions-said.json");
    let r = if r.get("focused") == Some(&json!(false)) {
        std::fs::read_to_string(&kept).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(r)
    } else {
        if r.get("rows").is_some() {
            let _ = std::fs::create_dir_all(&p.cache);
            crate::util::write(&kept, &crate::util::dumps(&r));
        }
        r
    };
    let repo = r.get("repo").and_then(Value::as_str).unwrap_or("");
    let rows: Vec<Value> = r
        .get("rows")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|v| {
                    let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    row(vec![g("rev"), g("description"), g("date"), g("by")], "", None)
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "title": if repo.is_empty() { "REVISIONS".to_string() } else { format!("REVISIONS · {}", repo.to_uppercase()) },
        "meta": format!("SHEET {sheet}"),
        "columns": [column("REV", 16.0, false), column("DESCRIPTION", 0.0, false), column("DATE", 21.0, true), column("BY", 22.0, true)],
        "rows": rows,
        "note": -1,
        "bar": -1,
    })
}

/// The displays as the equipment schedule lists them: what each one is, and how finely it is ruled.
fn listed(outputs: &[Value]) -> Vec<Value> {
    outputs
        .iter()
        .map(|o| {
            let f = |k: &str| o.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            let (w, h, scale, mm) = (f("width"), f("height"), f("scale"), f("px_per_mm"));
            json!({
                "name": o.get("name").and_then(Value::as_str).unwrap_or(""),
                "what": format!("{} × {} · scale {scale}", (w * scale).round(), (h * scale).round()),
                "note": format!("{:.1} px/mm · {:.1} logical px a mm", mm * scale, mm),
            })
        })
        .collect()
}

/// The sheet itself, as the equipment schedule lists it: the theme it is drawn in and how wide its margin is.
fn ruled(theme: &Value, outputs: &[Value]) -> String {
    let Some(o) = outputs.first() else { return String::new() };
    let mm = o.get("px_per_mm").and_then(Value::as_f64).unwrap_or(1.0);
    let margin = o.get("margin").and_then(Value::as_f64).unwrap_or(0.0);
    format!("{} · margin {} mm", theme.get("label").and_then(Value::as_str).unwrap_or(""), (margin / mm).round())
}

/// Whichever schedule the compositor says is up, or nothing where none is.
pub fn schedule(p: &crate::util::Paths, which: &str, sheet: u64, theme: &Value, outputs: &[Value]) -> Option<Value> {
    match which {
        "equipment" => Some(equipment(p, &listed(outputs), &ruled(theme, outputs))),
        "processes" => Some(processes()),
        "revisions" => Some(revisions(sheet)),
        // not one the sheet comes with: an extension of that name, where it printed a schedule
        "" => None,
        name => mine_schedule(p, name),
    }
}

/// The whole document: every display the compositor has, and what the sheet says on it.
///
/// What is live and held by whoever letters the sheet is not in here and cannot be: the notifications,
/// which belong to whichever process took the name on the bus, and the volume and brightness lettered
/// for a few seconds after they move. Nor are the config's own `[[cell]]`s, which are commands with
/// intervals of their own. Everything that is a reading of the machine is.
/// `want` is the schedule whoever is lettering the sheet has out, and on which display: the shell holds
/// that itself, where the sheet's own client leaves it with the compositor. Without it the compositor is
/// asked, which is what `draftpaper sheet` on its own reports.
pub fn document(p: &crate::util::Paths, want: Option<(&str, &str)>) -> Value {
    let theme: Value = std::fs::read_to_string(p.theme_json()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let readings = crate::readings::json(p);
    let now = std::time::SystemTime::now();
    let reply = |request: &str| {
        crate::util::ask(request)
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.get("reply").and_then(Value::as_array).cloned())
            .unwrap_or_default()
    };
    let outputs = reply("{\"request\": \"outputs\"}");
    let spaces = reply("{\"request\": \"workspaces\"}");
    let windows = reply("{\"request\": \"windows\"}");

    // the processor is a reading over time, so a command that runs once has to take two: a tenth of a
    // second apart is enough to say what it is doing, and the sheet is asked for this every five
    let mut cpu = crate::metrics::Cpu::new();
    std::thread::sleep(std::time::Duration::from_millis(120));
    let busy = cpu.busy();

    // the user's own, run once for the whole sheet rather than once a display
    let mine = extensions(p);

    let mut displays_out = serde_json::Map::new();
    for o in &outputs {
        let Some(name) = o.get("name").and_then(Value::as_str) else { continue };
        let workspace = o.get("workspace").and_then(Value::as_u64).unwrap_or(1);
        let mut d = margins(&theme, &readings, &spaces, &windows, name, workspace, &mine);
        d["title"] = json!(title(&theme, workspace, now, &readings, busy, &mine));
        // whichever the compositor says is out — or the revisions by themselves, where the config asks
        // for them and nothing else is chosen, on the display in use (qml/shell.qml)
        let named = match want {
            Some((which, on)) => if on == name { which } else { "" },
            None => o.get("schedule").and_then(Value::as_str).unwrap_or(""),
        };
        let by_itself = theme.get("title").and_then(|t| t.get("revisions")) == Some(&json!(true)) && o.get("focused").and_then(Value::as_bool).unwrap_or(false);
        let which = if named.is_empty() && by_itself { "revisions" } else { named };
        if let Some(s) = schedule(p, which, workspace, &theme, &outputs) {
            d["schedule"] = s;
        }
        displays_out.insert(name.to_string(), d);
    }
    json!({ "version": 1, "displays": displays_out, "legend": legend() })
}

/// The legend's measurements (docs/keyboard.md): the millimetres both renderers set it out on.
///
/// Not its contents — the keys are the compositor's and whoever letters the sheet asks it for them, the
/// one account of a keymap being the table that acts on it. What is here is the arrangement, and it is
/// here for the reason everything else on the sheet is: two renderers that each kept their own copy of
/// these seven numbers would letter two different legends the day one of them was edited.
pub fn legend() -> Value {
    json!({
        // what it is lettered with. The compositor keeps whether the legend is up and whether the
        // leader is armed; what those are *called* is the sheet's lettering and belongs here, so that
        // neither renderer carries a word of its own to drift (docs/keyboard.md).
        "title": "LEGEND",
        "held": "MOD HELD",     // the heading's right, with Mod held down
        "leader": "LEADER",     // and with Mod tapped: the next key composes
        "more": "MORE",         // "... · 12 MORE": the bindings the sheet had no room for
        // the passing word (docs/keyboard.md): the heading's right says this instead, for a moment,
        // when a key is answered rather than run. The compositor names the case ("unbound") and the
        // key; the words are here, with the rest of the legend's lettering.
        "unbound": "NOTHING ON",
        "gap": 6,       // in from the frame's corner, and clear of the title block
        "pad": 4,       // the paper under the table, out past it on every side
        "head": 10,     // the heading and its rule
        "row": 6,       // a key and what it does
        "key": 28,      // the key's column
        "inner": 2,     // between a key and what it does
        "act": 35,      // what it does
        "gutter": 3,    // between one pair of columns and the next
    })
}

pub fn main(p: &crate::util::Paths, args: &[&str]) {
    let want = (args.len() >= 2).then(|| (args[0], args[1]));
    println!("{}", crate::util::dumps(&document(p, want)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Value {
        json!({ "title": { "project": "signal", "drawn": "dann" } })
    }

    fn by<'a>(cells: &'a [Value], label: &str) -> &'a Value {
        cells.iter().find(|c| c["label"] == label).expect("a cell with that label")
    }

    #[test]
    fn twelve_cells_in_the_order_the_compositor_measures_them() {
        let cells = title(&theme(), 3, std::time::SystemTime::now(), &Value::Null, None, &[]);
        let labels: Vec<&str> = cells.iter().map(|c| c["label"].as_str().unwrap()).collect();
        assert_eq!(
            labels,
            ["TIME", "DATE", "SHEET", "PROJECT", "POWER", "BATTERY", "PROFILE", "CPU / MEMORY", "NETWORK", "TEMPERATURE · FAN", "PROCESSES", "DRAWN"]
        );
        assert_eq!(by(&cells, "SHEET")["text"], "3");
        assert_eq!(by(&cells, "PROJECT")["text"], "SIGNAL");
        assert_eq!(by(&cells, "DRAWN")["text"], "dann");
    }

    #[test]
    fn only_the_clock_is_lettered_large() {
        // said, not worked out from the cell: the block stretches the top row past the height the
        // shell's own table uses, so a renderer that measures letters eleven cells at the clock's size
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &Value::Null, None, &[]);
        let large: Vec<&str> = cells.iter().filter(|c| c["large"] == true).map(|c| c["label"].as_str().unwrap()).collect();
        assert_eq!(large, ["TIME"]);
    }

    #[test]
    fn every_reading_carries_its_glyphs_and_the_words_to_do_without_them() {
        let r = json!({
            "battery": 0.78, "charging": false, "power": 12.345, "profile": "power-saver", "processes": 287,
            "wifi": { "ssid": "a network", "signal": 62 },
            "temps": [{ "label": "CPU", "value": "57°" }, { "label": "FAN", "value": "2322" }],
            "hot": true
        });
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &r, Some(0.08), &[]);
        let c = |l| by(&cells, l).clone();
        assert_eq!(c("POWER")["text"], format!("{BOLT} 12.3 W"));
        assert_eq!(c("POWER")["words"], "12.3 W");
        // 78% is the fourth of five cells
        assert_eq!(c("BATTERY")["text"], format!("{} 78%", CELLS[3]));
        assert_eq!(c("BATTERY")["words"], "78%", "the figure alone, where the cell cannot be drawn");
        // the glyph takes room, so the name is shortened beside it and whole without it
        assert_eq!(c("PROFILE")["text"], format!("{SAVER} saver"));
        assert_eq!(c("PROFILE")["words"], "power-saver");
        assert_eq!(c("NETWORK")["text"], format!("{} a network", ARCS[3]));
        assert_eq!(c("NETWORK")["words"], "a network".to_string());
        assert_eq!(c("TEMPERATURE · FAN")["text"], format!("{} 57° {} 2322", TEMPERATURES[0].1, TEMPERATURES[2].1));
        assert_eq!(c("TEMPERATURE · FAN")["words"], "CPU 57° FAN 2322");
        assert_eq!(c("TEMPERATURE · FAN")["ink"], "alert", "read beyond its alarm");
        assert_eq!(c("CPU / MEMORY")["words"].as_str().unwrap().matches('/').count(), 1);
        assert_eq!(c("PROCESSES")["text"], "287");
    }

    #[test]
    fn what_is_not_there_is_said_rather_than_left_out() {
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &Value::Null, None, &[]);
        let c = |l| by(&cells, l).clone();
        assert_eq!(c("BATTERY")["text"], "-");
        assert_eq!(c("PROCESSES")["text"], "-");
        // nothing drawn, and no network at all: the sheet says so in words, not with a dash
        assert_eq!(c("POWER")["words"], "mains");
        assert_eq!(c("NETWORK")["words"], "offline");
    }

    #[test]
    fn a_low_battery_is_lettered_in_the_alert_ink() {
        let low = json!({ "battery": 0.08, "battery_low": true });
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &low, None, &[]);
        assert_eq!(by(&cells, "BATTERY")["ink"], "alert");
        // and the ink is a role, never a colour: the theme resolves it
        assert!(by(&cells, "BATTERY")["ink"].as_str().unwrap().chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn a_cell_that_does_something_says_what_by_name() {
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &Value::Null, None, &[]);
        assert_eq!(by(&cells, "POWER")["action"], "graph.power");
        assert_eq!(by(&cells, "PROFILE")["action"], "profile.next");
        assert_eq!(by(&cells, "PROCESSES")["action"], "schedule.processes");
        // and one that does nothing says nothing
        assert_eq!(by(&cells, "DATE")["action"], Value::Null);
    }

    #[test]
    fn the_clock_reads_as_the_title_block_letters_it() {
        let (date, time) = clock(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_792_400_000));
        let parts: Vec<&str> = date.split(' ').collect();
        assert_eq!(parts.len(), 4, "{date}");
        assert_eq!(parts[1].len(), 2, "{date}");
        assert_eq!(parts[3].len(), 4, "{date}");
        assert!(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].contains(&parts[0]), "{date}");
        assert_eq!(time.len(), 5, "{time}");
        assert_eq!(&time[2..3], ":");
    }

    #[test]
    fn an_extension_that_prints_a_schedule_gives_one() {
        let out = r#"{"title": "DEPARTURES", "meta": "PLATFORM 4",
            "columns": [{"name": "TIME", "mm": 18}, {"name": "FOR", "mm": 0}, {"name": "DUE", "mm": 24, "right": true}],
            "rows": [{"cells": ["09:14", "Cluj-Napoca", "on time"]}, {"cells": ["09:52", "Brasov", "late"], "note": "no platform yet"}]}"#;
        let s = as_schedule("trains", out).expect("a schedule");
        assert_eq!(s["title"], "DEPARTURES");
        assert_eq!(s["meta"], "PLATFORM 4");
        assert_eq!(s["columns"].as_array().unwrap().len(), 3);
        // the columns are taken as this document gives them, filled in where the extension left them out
        assert_eq!(s["columns"][1]["right"], false);
        assert_eq!(s["columns"][2]["right"], true);
        assert_eq!(s["rows"].as_array().unwrap().len(), 2);
        assert_eq!(s["rows"][1]["note"], "no platform yet");
        // nothing said about the note and bar columns is no note and no bar, not a panic
        assert_eq!(s["note"], -1);
        assert_eq!(s["bar"], -1);
        // its name does for a heading where it gives none
        assert_eq!(as_schedule("trains", r#"{"rows": []}"#).unwrap()["title"], "TRAINS");
        // and what is not a schedule is not taken for one
        assert!(as_schedule("trains", "DRAFT-42").is_none());
        assert!(as_schedule("trains", r#"{"text": "DRAFT-42"}"#).is_none());
    }

    #[test]
    fn a_workspace_is_marked_with_the_windows_on_it() {
        let spaces = json!([{ "number": 2, "focused": true, "output": "eDP-1" }, { "number": 9, "focused": false, "output": "DP-3" }]);
        let windows = json!([
            { "workspace": 2, "app_id": "firefox", "parked": false },
            { "workspace": 2, "app_id": "Alacritty", "parked": false },
            { "workspace": 2, "app_id": "foot", "parked": false },
            { "workspace": 2, "app_id": "keepassxc", "parked": true },
        ]);
        let w = workspaces(spaces.as_array().unwrap(), windows.as_array().unwrap(), "eDP-1");
        assert_eq!(w.len(), 1, "only the workspaces this display shows");
        assert_eq!(w[0]["number"], 2);
        assert_eq!(w[0]["focused"], true);
        // the number, then one mark a window, no mark twice, and nothing for a window that is parked
        assert_eq!(w[0]["text"], format!("2 {} {}", app_mark("firefox"), app_mark("foot")));
        assert_eq!(w[0]["words"], "2", "without the marks it is the number alone");
        assert_ne!(app_mark("firefox"), app_mark("foot"));
        assert_eq!(app_mark("Alacritty"), app_mark("foot"), "a terminal is a terminal");
        assert_eq!(app_mark("something-else"), app_mark("another"), "anything else is a window");
    }

    #[test]
    fn a_window_parked_is_lettered_by_the_cell_it_is_in() {
        let windows = json!([
            { "workspace": 1, "parked": true, "cell": 3, "id": 7, "title": "a file", "app_id": "nvim" },
            { "workspace": 1, "parked": false, "cell": 4, "id": 8, "title": "not parked" },
            { "workspace": 2, "parked": true, "cell": 5, "id": 9, "title": "another sheet" },
            { "workspace": 1, "parked": true, "cell": 6, "id": 10, "title": "", "app_id": "foot" },
        ]);
        let p = parked(windows.as_array().unwrap(), 1);
        assert_eq!(p.len(), 2, "parked, and on this sheet");
        assert_eq!(p[0]["cell"], 3);
        assert_eq!(p[0]["text"], "a file");
        assert_eq!(p[0]["action"], "window.7");
        assert_eq!(p[1]["text"], "foot", "a window with no title is lettered by what it calls itself");
    }

    #[test]
    fn the_foot_says_what_is_connected_and_what_the_sheet_is_drawn_in() {
        let theme = json!({ "label": "Night Panel", "commands": { "screenshot": ["grim"] } });
        let none = bluetooth(&Value::Null);
        assert_eq!(none["words"], "BT -", "nothing connected, said in words");
        let some = bluetooth(&json!({ "bt": ["a keyboard", "a mouse"] }));
        assert_eq!(some["words"], "BT a keyboard, a mouse");
        assert!(some["text"].as_str().unwrap().ends_with("a keyboard, a mouse"));
        let end = foot(&theme);
        assert_eq!(end.len(), 2);
        assert_eq!(end[0]["words"], "SHOT");
        assert_eq!(end[1]["text"], "NIGHT PANEL");
        // no screenshot command in the config: nothing to letter for it
        assert_eq!(foot(&json!({ "label": "x" })).len(), 1);
    }

    #[test]
    fn only_a_reading_past_its_alarm_is_said_along_the_top() {
        assert!(status(&Value::Null).is_empty(), "a quiet machine says nothing");
        let hot = json!({ "hot": true, "temps": [{ "label": "CPU", "value": "95°" }], "battery_low": true, "battery": 0.08 });
        let row = status(&hot);
        assert_eq!(row.len(), 2);
        assert_eq!(row[0]["words"], "CPU 95°");
        assert_eq!(row[0]["ink"], "alert");
        assert_eq!(row[1]["words"], "BAT 8%");
        assert_eq!(row[1]["ink"], "alert");
    }

    #[test]
    fn the_uptime_reads_as_a_drawing_notes_it() {
        assert_eq!(up(90.0), "up 1 min");
        assert_eq!(up(3700.0), "up 1 h 1 min");
        assert_eq!(up(200_000.0), "up 2 d 7 h");
    }

    #[test]
    fn a_ramp_runs_from_the_first_to_the_last() {
        let gs = ['a', 'b', 'c', 'd', 'e'];
        assert_eq!(ramp(0.0, &gs), 'a');
        assert_eq!(ramp(0.5, &gs), 'c');
        assert_eq!(ramp(1.0, &gs), 'e', "full is the last, not past the end");
        assert_eq!(ramp(1.5, &gs), 'e');
        assert_eq!(ramp(-1.0, &gs), 'a');
    }
}
