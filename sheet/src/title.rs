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
}

/// The twelve cells, in the order the compositor measured them.
pub fn cells(theme: &crate::sheet::Theme, workspace: u64, now: std::time::SystemTime, r: &Value, cpu: Option<f64>) -> Vec<Cell> {
    let cell = |label, value: String| Cell { label, value, alert: false };
    let (date, time) = clock(now);
    let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("-").to_string();
    let temps = r.get("temps").and_then(Value::as_array).cloned().unwrap_or_default();
    let named = |which: &str| temps.iter().find(|t| t.get("label").and_then(Value::as_str) == Some(which)).map(|t| text(t.get("value")));
    vec![
        cell("TIME", time),
        cell("DATE", date),
        cell("SHEET", workspace.to_string()),
        cell("PROJECT", theme.project.to_uppercase()),
        cell("POWER", power(&r)),
        Cell { label: "BATTERY", value: battery(&r), alert: r.get("battery_low").and_then(Value::as_bool).unwrap_or(false) },
        cell("PROFILE", text(r.get("profile")).to_lowercase()),
        cell("CPU / MEMORY", {
            let pct = |v: Option<f64>| v.map(|v| format!("{}%", (v * 100.0).round())).unwrap_or_else(|| "-".into());
            format!("{} / {}", pct(cpu), pct(draftpaper::metrics::memory()))
        }),
        cell("NETWORK", network(&r)),
        Cell {
            label: "TEMPERATURE · FAN",
            value: ["CPU", "NVME", "FAN"].iter().filter_map(|k| named(k).map(|v| format!("{k} {v}"))).collect::<Vec<_>>().join(" ").trim().to_string(),
            alert: r.get("hot").and_then(Value::as_bool).unwrap_or(false),
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

fn battery(r: &Value) -> String {
    match r.get("battery").and_then(Value::as_f64) {
        Some(v) => format!("{}%", (v * 100.0).round()),
        None => "-".into(),
    }
}

fn network(r: &Value) -> String {
    if let Some(ssid) = r.get("wifi").and_then(|w| w.get("ssid")).and_then(Value::as_str) {
        return ssid.to_string();
    }
    r.get("wired").and_then(Value::as_str).unwrap_or("-").to_string()
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
        let cells = cells(&theme(), 3, std::time::SystemTime::now(), &serde_json::Value::Null, None);
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
    fn a_reading_that_is_not_there_is_a_dash_rather_than_nothing() {
        let cells = cells(&theme(), 1, std::time::SystemTime::now(), &serde_json::Value::Null, None);
        let by = |l: &str| cells.iter().find(|c| c.label == l).unwrap();
        assert_eq!(by("BATTERY").value, "-");
        assert_eq!(by("NETWORK").value, "-");
        assert_eq!(by("PROCESSES").value, "-");
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
        let cells = cells(&theme(), 1, std::time::SystemTime::now(), &r, Some(0.08));
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
}
