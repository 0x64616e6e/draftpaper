//! The title block's twelve cells, as the drawing's record of itself: the time and the date, which sheet
//! and which project, the power and the battery, the profile, the processor and the memory, the network,
//! the temperatures and the fan, how many processes, and who is drawing.
//!
//! The compositor measures the block and every cell in whole physical pixels (compositor/src/title.rs)
//! and publishes them; this works out none of it, only what each cell says and where in the cell it goes.
//! The lettering follows qml/TitleBlock.qml, which it replaces: the label 2 mm in and 1.2 mm down from
//! the cell's corner, the reading 2 mm in and sitting 1.6 mm above the cell's foot.

use serde_json::Value;

/// What one cell says.
pub struct Cell {
    pub label: &'static str,
    pub value: String,
    /// read beyond what the theme calls alarming: lettered in the alert ink
    pub alert: bool,
    /// the clock, and only the clock: lettered large. The shell decides this from the cell's height in
    /// the table it lays the block out from (qml/TitleBlock.qml), which is 24 mm for the clock and 12 mm
    /// for every other cell — not from the height the cell is measured at, which the block's own
    /// proportions stretch past 12 mm for the top row as well.
    pub large: bool,
}

/// The twelve cells, in the order the compositor measured them. The face is wanted because a reading
/// that would carry a glyph carries the word instead where no face installed has it (glyph.rs).
pub fn cells(theme: &crate::sheet::Theme, face: &mut crate::font::Face, workspace: u64, now: std::time::SystemTime, r: &Value, cpu: Option<f64>) -> Vec<Cell> {
    let cell = |label, value: String| Cell { label, value, alert: false, large: false };
    let (date, time) = clock(now);
    let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("-").to_string();
    let temps = r.get("temps").and_then(Value::as_array).cloned().unwrap_or_default();
    let named = |which: &str| temps.iter().find(|t| t.get("label").and_then(Value::as_str) == Some(which)).map(|t| text(t.get("value")));
    let on = theme.glyphs;
    vec![
        Cell { label: "TIME", value: time, alert: false, large: true },
        cell("DATE", date),
        cell("SHEET", workspace.to_string()),
        cell("PROJECT", theme.project.to_uppercase()),
        cell("POWER", format!("{}{}", crate::glyph::before(face, on, '\u{f140b}', ""), power(&r))),
        Cell { label: "BATTERY", value: battery(face, on, &r), alert: r.get("battery_low").and_then(Value::as_bool).unwrap_or(false), large: false },
        cell("PROFILE", profile(face, on, &text(r.get("profile")).to_lowercase())),
        cell("CPU / MEMORY", {
            let pct = |v: Option<f64>| v.map(|v| format!("{}%", (v * 100.0).round())).unwrap_or_else(|| "-".into());
            // with their glyphs the two readings stand apart on their own, without the stroke between.
            // The shell writes two spaces here and in the temperatures; its text is styled, and styled
            // text collapses them to one, which is what the sheet actually carries.
            let (load, mem) = (crate::glyph::before(face, on, '\u{f0ee0}', ""), crate::glyph::before(face, on, '\u{f035b}', ""));
            if load.is_empty() || mem.is_empty() {
                format!("{} / {}", pct(cpu), pct(draftpaper::metrics::memory()))
            } else {
                format!("{load}{} {mem}{}", pct(cpu), pct(draftpaper::metrics::memory()))
            }
        }),
        cell("NETWORK", network(face, on, &r)),
        Cell {
            label: "TEMPERATURE · FAN",
            // each reading behind its own glyph where there is one, else behind its name, as the shell
            // letters them (qml/shell.qml's `tempLabel`)
            value: [("CPU", '\u{f061a}'), ("NVME", '\u{f02ca}'), ("FAN", '\u{f0210}')]
                .iter()
                .filter_map(|(k, g)| named(k).map(|v| format!("{}{v}", crate::glyph::before(face, on, *g, &format!("{k} ")))))
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string(),
            alert: r.get("hot").and_then(Value::as_bool).unwrap_or(false),
            large: false,
        },
        cell("PROCESSES", r.get("processes").and_then(Value::as_u64).map(|n| n.to_string()).unwrap_or_else(|| "-".into())),
        cell("DRAWN", theme.drawn.clone()),
    ]
}

/// Which cell of the title block is at `x`, `y` in its surface (its own logical pixels, from the block's
/// corner), and whether a click on it does anything yet. The schedules and the graph are the shell's
/// until stage 6; the profile is this client's now.
pub fn under(block: &crate::sheet::Block, x: f64, y: f64) -> Option<(usize, bool)> {
    let i = block.cells.iter().position(|c| {
        let (cx, cy) = (c.0 - block.rect.0, c.1 - block.rect.1);
        x >= cx && y >= cy && x < cx + c.2 && y < cy + c.3
    })?;
    // the profile's cell: the sixth, counting from the clock
    Some((i, i == 6))
}

/// The next power profile after this one, as the title block's cell cycles them.
pub fn next_profile(now: &str) -> &'static str {
    const ORDER: [&str; 3] = ["power-saver", "balanced", "performance"];
    let i = ORDER.iter().position(|p| *p == now).unwrap_or(ORDER.len() - 1);
    ORDER[(i + 1) % ORDER.len()]
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

fn power(r: &Value) -> String {
    match r.get("power").and_then(Value::as_f64) {
        Some(w) if w > 0.0 => format!("{w:.1} W"),
        _ => "mains".into(),
    }
}

/// The charge, behind a cell that fills as it does — or the bolt while it is charging. Without glyphs
/// the word says what the glyph would have: "charging" after the figure, as the shell says it.
fn battery(face: &mut crate::font::Face, on: bool, r: &Value) -> String {
    let Some(v) = r.get("battery").and_then(Value::as_f64) else { return "-".into() };
    let charging = r.get("charging").and_then(Value::as_bool).unwrap_or(false);
    let cells = ['\u{f007a}', '\u{f007c}', '\u{f007e}', '\u{f0080}', '\u{f0079}'];
    let ch = if charging { '\u{f0084}' } else { crate::glyph::ramp(v, &cells) };
    let glyph = crate::glyph::before(face, on, ch, "");
    let figure = format!("{}%", (v * 100.0).round());
    if glyph.is_empty() && charging {
        format!("{figure} charging")
    } else {
        format!("{glyph}{figure}")
    }
}

/// The profile behind its glyph. The glyph takes room the cell has not got, so beside one the longer
/// names are shortened — "saver", "perf" — and lettered whole where there is no glyph, as the shell does.
fn profile(face: &mut crate::font::Face, on: bool, p: &str) -> String {
    let ch = match p {
        "power-saver" => '\u{f032a}',
        "performance" => '\u{f04c5}',
        _ => '\u{f0f85}',
    };
    let glyph = crate::glyph::before(face, on, ch, "");
    let word = match (glyph.is_empty(), p) {
        (false, "power-saver") => "saver",
        (false, "performance") => "perf",
        (_, p) => p,
    };
    format!("{glyph}{word}")
}

/// The network behind the arc its signal fills, the plug where it is wired, or the struck-out arc where
/// it is neither.
fn network(face: &mut crate::font::Face, on: bool, r: &Value) -> String {
    if let Some(wifi) = r.get("wifi").filter(|w| !w.is_null()) {
        let ssid = wifi.get("ssid").and_then(Value::as_str).unwrap_or("");
        let signal = wifi.get("signal").and_then(Value::as_f64).unwrap_or(0.0) / 100.0;
        let arcs = ['\u{f092f}', '\u{f091f}', '\u{f0922}', '\u{f0925}', '\u{f0928}'];
        return format!("{}{ssid}", crate::glyph::before(face, on, crate::glyph::ramp(signal, &arcs), ""));
    }
    match r.get("wired").and_then(Value::as_str) {
        Some(address) => format!("{}{address}", crate::glyph::before(face, on, '\u{f0201}', "wired ")),
        None => format!("{}offline", crate::glyph::before(face, on, '\u{f092e}', "")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> crate::sheet::Theme {
        crate::sheet::Theme {
            ink: crate::paint::Ink::parse("#000000", 1.0),
            alert: crate::paint::Ink::parse("#ff0000", 1.0),
            mono: None,
            project: "signal".into(),
            drawn: "dann".into(),
            label: "Soft Glow".into(),
            accent: crate::paint::Ink::parse("#b4701a", 1.0),
            strong: crate::paint::Ink::parse("#b4701a", 1.0),
            paper: crate::paint::Ink::parse("#ffffff", 1.0),
            ink_alpha: 1.0,
            glyphs: false,
        }
    }

    #[test]
    fn the_profile_cell_cycles_through_them() {
        assert_eq!(next_profile("power-saver"), "balanced");
        assert_eq!(next_profile("balanced"), "performance");
        assert_eq!(next_profile("performance"), "power-saver");
        // nothing read yet, or a profile this machine does not have: start at the beginning
        assert_eq!(next_profile(""), "power-saver");
    }

    #[test]
    fn a_click_finds_the_cell_it_is_in() {
        let block = crate::sheet::Block { rect: (100.0, 200.0, 300.0, 100.0), cells: vec![(100.0, 200.0, 150.0, 50.0), (250.0, 200.0, 150.0, 50.0)] };
        assert_eq!(under(&block, 10.0, 10.0).map(|(i, _)| i), Some(0));
        assert_eq!(under(&block, 160.0, 10.0).map(|(i, _)| i), Some(1));
        // past the block: nothing
        assert_eq!(under(&block, 400.0, 10.0), None);
    }

    #[test]
    fn the_clock_reads_as_the_title_block_letters_it() {
        let (date, time) = clock(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_792_400_000));
        // "Mon 19 Oct 2026": a weekday, a two-figure day, a month and a year
        let parts: Vec<&str> = date.split(' ').collect();
        assert_eq!(parts.len(), 4, "{date}");
        assert_eq!(parts[1].len(), 2, "{date}");
        assert_eq!(parts[3].len(), 4, "{date}");
        assert!(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].contains(&parts[0]), "{date}");
        // "18:45"
        assert_eq!(time.len(), 5, "{time}");
        assert_eq!(&time[2..3], ":");
        assert!(time[..2].parse::<u32>().unwrap() < 24);
        assert!(time[3..].parse::<u32>().unwrap() < 60);
    }

    #[test]
    fn twelve_cells_in_the_order_the_compositor_measured_them() {
        let Some(mut face) = crate::font::Face::load(None) else { return };
        let cells = cells(&theme(), &mut face, 3, std::time::SystemTime::now(), &serde_json::Value::Null, None);
        let labels: Vec<&str> = cells.iter().map(|c| c.label).collect();
        assert_eq!(
            labels,
            ["TIME", "DATE", "SHEET", "PROJECT", "POWER", "BATTERY", "PROFILE", "CPU / MEMORY", "NETWORK", "TEMPERATURE · FAN", "PROCESSES", "DRAWN"]
        );
        let by = |l: &str| cells.iter().find(|c| c.label == l).unwrap();
        assert_eq!(by("SHEET").value, "3");
        assert_eq!(by("PROJECT").value, "SIGNAL");
        assert_eq!(by("DRAWN").value, "dann");
    }

    #[test]
    fn only_the_clock_is_lettered_large() {
        // qml/TitleBlock.qml sizes the reading from the cell's height in its own table — 24 mm for the
        // clock, 12 mm for the other eleven — so the clock alone is lettered at 9 mm and the rest at
        // 3.6 mm. Reading it off the measured cell instead makes the whole top row large, because the
        // block's proportions stretch those cells past 12 mm.
        let Some(mut face) = crate::font::Face::load(None) else { return };
        let cells = cells(&theme(), &mut face, 1, std::time::SystemTime::now(), &serde_json::Value::Null, None);
        let large: Vec<&str> = cells.iter().filter(|c| c.large).map(|c| c.label).collect();
        assert_eq!(large, ["TIME"]);
    }

    #[test]
    fn a_reading_that_is_not_there_is_a_dash_rather_than_nothing() {
        let Some(mut face) = crate::font::Face::load(None) else { return };
        let cells = cells(&theme(), &mut face, 1, std::time::SystemTime::now(), &serde_json::Value::Null, None);
        let by = |l: &str| cells.iter().find(|c| c.label == l).unwrap();
        assert_eq!(by("BATTERY").value, "-");
        assert_eq!(by("PROCESSES").value, "-");
        // no network at all is a word, not a dash: the sheet says what is the case (qml/shell.qml)
        assert_eq!(by("NETWORK").value, "offline");
        // on mains there is nothing drawn, which the sheet says in words
        assert_eq!(by("POWER").value, "mains");
        assert_eq!(by("CPU / MEMORY").value.matches('/').count(), 1);
    }

    #[test]
    fn the_readings_are_lettered_as_the_sheet_letters_them() {
        let r = serde_json::json!({
            "battery": 0.78, "power": 12.345, "profile": "Performance", "processes": 287,
            "wifi": { "ssid": "a network", "signal": 62 },
            "temps": [{ "label": "CPU", "value": "57°" }, { "label": "FAN", "value": "2322" }],
            "hot": true
        });
        let Some(mut face) = crate::font::Face::load(None) else { return };
        let cells = cells(&theme(), &mut face, 1, std::time::SystemTime::now(), &r, Some(0.08));
        let by = |l: &str| cells.iter().find(|c| c.label == l).unwrap();
        assert_eq!(by("BATTERY").value, "78%");
        assert_eq!(by("POWER").value, "12.3 W");
        assert_eq!(by("PROFILE").value, "performance");
        assert_eq!(by("PROCESSES").value, "287");
        assert_eq!(by("NETWORK").value, "a network");
        assert_eq!(by("TEMPERATURE · FAN").value, "CPU 57° FAN 2322");
        assert!(by("TEMPERATURE · FAN").alert, "read beyond the alarm: the alert ink");
        assert!(by("CPU / MEMORY").value.starts_with("8% / "));
    }

    #[test]
    fn with_glyphs_on_each_reading_comes_behind_its_own() {
        let Some(mut face) = crate::font::Face::load(None) else { return };
        // no Nerd Font installed: the sheet letters the words, which the test above already holds it to
        if !face.has('\u{f140b}') {
            return;
        }
        let theme = crate::sheet::Theme { glyphs: true, ..theme() };
        let r = serde_json::json!({
            "battery": 0.78, "charging": false, "power": 12.345, "profile": "power-saver",
            "wifi": { "ssid": "a network", "signal": 62 },
            "temps": [{ "label": "CPU", "value": "57°" }],
        });
        let cells = cells(&theme, &mut face, 1, std::time::SystemTime::now(), &r, Some(0.08));
        let by = |l: &str| cells.iter().find(|c| c.label == l).unwrap().value.clone();
        assert_eq!(by("POWER"), "\u{f140b} 12.3 W");
        // the cell fills as the charge does: 78% is the fourth of five
        assert_eq!(by("BATTERY"), "\u{f0080} 78%");
        // the glyph takes room from the cell, so the longer names are shortened beside it
        assert_eq!(by("PROFILE"), "\u{f032a} saver");
        assert_eq!(by("NETWORK"), "\u{f0925} a network");
        assert_eq!(by("TEMPERATURE · FAN"), "\u{f061a} 57°");
        // the two readings stand apart behind their own glyphs, with no stroke between them
        assert_eq!(by("CPU / MEMORY").matches('/').count(), 0);
        // what never carried a glyph is lettered as before
        assert_eq!(by("PROCESSES"), "-");
    }
}
