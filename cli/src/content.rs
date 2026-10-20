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
    let mut v = json!({ "text": text });
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
pub fn title(theme: &Value, workspace: u64, now: std::time::SystemTime, r: &Value, cpu: Option<f64>) -> Vec<Value> {
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
        cell("DRAWN", of("title", "drawn"), None, None),
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

/// The whole document: every display the compositor has, and what the sheet says on it. The margins and
/// the schedules are stage 1 (docs/sheet-content.md); this is the title block.
pub fn document(p: &crate::util::Paths) -> Value {
    let theme: Value = std::fs::read_to_string(p.theme_json()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let readings = crate::readings::json(p);
    let outputs = crate::util::ask("{\"request\": \"outputs\"}")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("reply").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    let now = std::time::SystemTime::now();
    let mut displays = serde_json::Map::new();
    for o in &outputs {
        let Some(name) = o.get("name").and_then(Value::as_str) else { continue };
        let workspace = o.get("workspace").and_then(Value::as_u64).unwrap_or(1);
        displays.insert(name.to_string(), json!({ "title": title(&theme, workspace, now, &readings, None) }));
    }
    json!({ "version": 1, "displays": displays })
}

pub fn main(p: &crate::util::Paths) {
    println!("{}", crate::util::dumps(&document(p)));
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
        let cells = title(&theme(), 3, std::time::SystemTime::now(), &Value::Null, None);
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
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &Value::Null, None);
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
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &r, Some(0.08));
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
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &Value::Null, None);
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
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &low, None);
        assert_eq!(by(&cells, "BATTERY")["ink"], "alert");
        // and the ink is a role, never a colour: the theme resolves it
        assert!(by(&cells, "BATTERY")["ink"].as_str().unwrap().chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn a_cell_that_does_something_says_what_by_name() {
        let cells = title(&theme(), 1, std::time::SystemTime::now(), &Value::Null, None);
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
    fn a_ramp_runs_from_the_first_to_the_last() {
        let gs = ['a', 'b', 'c', 'd', 'e'];
        assert_eq!(ramp(0.0, &gs), 'a');
        assert_eq!(ramp(0.5, &gs), 'c');
        assert_eq!(ramp(1.0, &gs), 'e', "full is the last, not past the end");
        assert_eq!(ramp(1.5, &gs), 'e');
        assert_eq!(ramp(-1.0, &gs), 'a');
    }
}
