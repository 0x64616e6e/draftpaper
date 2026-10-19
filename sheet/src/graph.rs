//! A year of readings, plotted on the sheet: now at the right, then the last five minutes, the hour, the
//! day, the week, the month and the year to the left, each period's end ticked and named. The time axis
//! is logarithmic, so the last few minutes take as much of the width as the last few months.
//!
//! This is qml/LoadGraph.qml, lettered by the sheet's own client. The readings come from the store
//! `draftpaper metrics keep` writes (cli/src/metrics.rs), read in place: each period keeps its slots'
//! count, sum, least and most, so one line runs through them all with the bold reading's least to most
//! a band beneath it, broken where nothing was kept.
//!
//! Where: level with the title block, from the frame's left line across to it, six millimetres clear of
//! either — the same place and the same rule as the title block's own, worked out for each display.

use serde_json::Value;

use crate::font::Face;
use crate::paint::{Canvas, Ink};
use crate::sheet::{Screen, Theme};

/// How far the graph keeps from the frame's lines and the title block.
const GAP: f64 = 6.0;
/// The time it spans: from five seconds ago to a year.
const YOUNGEST: f64 = 5.0;
const OLDEST: f64 = 31_536_000.0;
/// Each period's end, ticked and named.
const PERIODS: [(f64, &str); 6] = [(300.0, "5 MIN"), (3600.0, "1 H"), (86_400.0, "24 H"), (604_800.0, "7 D"), (2_592_000.0, "30 D"), (31_536_000.0, "1 Y")];

/// Which readings are plotted: a click on their cell in the title block, or on the equipment schedule's
/// storage row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Domain {
    #[default]
    None,
    Load,
    Temperature,
    Power,
    Battery,
    Network,
    Disk,
}

impl Domain {
    pub fn of(name: &str) -> Domain {
        match name {
            "load" => Domain::Load,
            "temperature" => Domain::Temperature,
            "power" => Domain::Power,
            "battery" => Domain::Battery,
            "network" => Domain::Network,
            "disk" => Domain::Disk,
            _ => Domain::None,
        }
    }

}

/// One reading's line: which it is in the store, what to call it, the scale it is drawn against, how
/// heavy it is, whether it is dashed, and whether its least-to-most is drawn as a band.
struct Line {
    key: &'static str,
    name: &'static str,
    least: f64,
    most: f64,
    /// a scale that climbs by powers of ten: throughput runs from bytes to gigabytes a second
    log: bool,
    weight: f64,
    dashed: bool,
    bold: bool,
    alert: bool,
}

/// One point of a line: how long ago, and what was read.
struct Point {
    age: f64,
    mean: f64,
    least: f64,
    most: f64,
    slot: f64,
}

/// The store as the keeper leaves it, with when it was read.
pub struct Readings {
    store: Value,
    at: f64,
}

impl Readings {
    /// From the runtime directory, where the keeper writes it every five seconds.
    pub fn read() -> Option<Readings> {
        let runtime = std::env::var_os("DRAFTPAPER_RUNTIME")
            .map(std::path::PathBuf::from)
            .or_else(|| Some(std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("draftpaper")))?;
        let text = std::fs::read_to_string(runtime.join("metrics.json")).ok()?;
        let store: Value = serde_json::from_str(&text).ok()?;
        store.get("tiers")?;
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs_f64();
        Some(Readings { store, at })
    }

    /// One reading's points, newest first. Each period gives the slots between the last period's end and
    /// its own, so the finer ones are drawn where they are kept and the line goes on through the coarser.
    fn points(&self, key: &str) -> Vec<Point> {
        let names: Vec<&str> = self.store.get("metrics").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let Some(m) = names.iter().position(|n| *n == key) else { return Vec::new() };
        let now = self.store.get("now").and_then(Value::as_f64).unwrap_or(self.at);
        let mut out: Vec<Point> = Vec::new();
        let mut from = 0.0;
        for tier in self.store.get("tiers").and_then(Value::as_array).into_iter().flatten() {
            let slot = tier.get("slot").and_then(Value::as_f64).unwrap_or(0.0);
            let upto = tier.get("upto").and_then(Value::as_f64).unwrap_or(0.0);
            for s in tier.get("slots").and_then(Value::as_array).into_iter().flatten() {
                let (Some(start), Some(acc)) = (s.get(0).and_then(Value::as_f64), s.get(1).and_then(Value::as_array)) else { continue };
                let Some(a) = acc.get(m).and_then(Value::as_array) else { continue };
                let n = a.first().and_then(Value::as_f64).unwrap_or(0.0);
                if n <= 0.0 {
                    continue;
                }
                let age = self.at - (start + slot / 2.0).min((start + now) / 2.0);
                if (age > from && age <= upto) || (from == 0.0 && age <= 0.0) {
                    out.push(Point {
                        age: age.max(0.0),
                        mean: a.get(1).and_then(Value::as_f64).unwrap_or(0.0) / n,
                        least: a.get(2).and_then(Value::as_f64).unwrap_or(0.0),
                        most: a.get(3).and_then(Value::as_f64).unwrap_or(0.0),
                        slot,
                    });
                }
            }
            from = upto;
        }
        out.sort_by(|a, b| a.age.partial_cmp(&b.age).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    /// The most any of these readings reached, for a scale that fits them.
    fn highest(&self, keys: &[&str]) -> f64 {
        keys.iter().flat_map(|k| self.points(k)).map(|p| p.most).fold(0.0, f64::max)
    }
}

/// Where the graph goes: level with the title block, across to it from the frame's left line.
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn place(s: &Screen) -> Option<Placed> {
    let block = s.title.as_ref()?;
    let x = s.frame.0 + GAP * s.mm;
    let y = block.rect.1;
    let width = ((block.rect.0 - GAP * s.mm - x) / s.mm).floor();
    let height = ((s.frame.1 + s.frame.3 - y) / s.mm).floor();
    (width >= 60.0 && height >= 20.0).then_some(Placed { x, y, width, height })
}

/// What a domain plots.
fn lines(domain: Domain, r: &Readings) -> Vec<Line> {
    let decade = |keys: &[&str]| 10f64.powf((r.highest(keys).max(1.0).log10() - 1e-9).ceil().max(6.0));
    match domain {
        Domain::Load => vec![
            Line { key: "cpu", name: "CPU", least: 0.0, most: 1.0, log: false, weight: 0.4, dashed: false, bold: true, alert: false },
            Line { key: "mem", name: "MEMORY", least: 0.0, most: 1.0, log: false, weight: 0.3, dashed: true, bold: false, alert: false },
        ],
        Domain::Temperature => vec![
            Line { key: "temp", name: "CPU", least: 30.0, most: 100.0, log: false, weight: 0.4, dashed: false, bold: true, alert: true },
            Line { key: "nvme", name: "NVME", least: 30.0, most: 100.0, log: false, weight: 0.3, dashed: true, bold: false, alert: false },
            Line { key: "fan", name: "FAN", least: 0.0, most: (r.highest(&["fan"]) / 1000.0).ceil().max(6.0) * 1000.0, log: false, weight: 0.3, dashed: true, bold: false, alert: false },
        ],
        Domain::Power => vec![Line {
            key: "power",
            name: "POWER",
            least: 0.0,
            most: (r.highest(&["power"]) / 10.0).ceil().max(2.0) * 10.0,
            log: false,
            weight: 0.4,
            dashed: false,
            bold: true,
            alert: false,
        }],
        Domain::Battery => vec![Line { key: "battery", name: "BATTERY", least: 0.0, most: 1.0, log: false, weight: 0.4, dashed: false, bold: true, alert: false }],
        Domain::Network => {
            let most = decade(&["net_down", "net_up"]);
            vec![
                Line { key: "net_down", name: "DOWN", least: 1e3, most, log: true, weight: 0.4, dashed: false, bold: true, alert: false },
                Line { key: "net_up", name: "UP", least: 1e3, most, log: true, weight: 0.3, dashed: true, bold: false, alert: false },
            ]
        }
        Domain::Disk => {
            let most = decade(&["disk_read", "disk_write"]);
            vec![
                Line { key: "disk_read", name: "READ", least: 1e3, most, log: true, weight: 0.4, dashed: false, bold: true, alert: false },
                Line { key: "disk_write", name: "WRITE", least: 1e3, most, log: true, weight: 0.3, dashed: true, bold: false, alert: false },
            ]
        }
        Domain::None => Vec::new(),
    }
}

/// How a reading is lettered at the end of its line.
fn lettered(domain: Domain, line: &Line, v: f64) -> String {
    match domain {
        Domain::Load | Domain::Battery => format!("{} {}%", line.name, (v * 100.0).round()),
        Domain::Temperature if line.key == "fan" => format!("FAN {} RPM", v.round()),
        Domain::Temperature => format!("{} {}°", line.name, v.round()),
        Domain::Power => format!("POWER {v:.1} W"),
        Domain::Network | Domain::Disk => format!("{} {}", line.name, rate(v)),
        Domain::None => String::new(),
    }
}

/// Bytes a second, as a meter reads them.
fn rate(mut v: f64) -> String {
    const UNITS: [&str; 4] = ["B/s", "kB/s", "MB/s", "GB/s"];
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if v >= 100.0 || i == 0 {
        format!("{} {}", v.round(), UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// An age in seconds to its share of the width from the right, logarithmic.
fn share(age: f64) -> f64 {
    (age.max(YOUNGEST).ln() - YOUNGEST.ln()) / (OLDEST.ln() - YOUNGEST.ln())
}

/// Draw it.
pub fn draw(canvas: &mut Canvas, face: &mut Face, domain: Domain, r: &Readings, at: &Placed, s: &Screen, theme: &Theme, scale: f64) {
    let mm = (s.mm * scale) as f32;
    let px = |v: f64| (v * s.mm * scale) as f32;
    let (wide, high) = (px(at.width), px(at.height));
    let ink = theme.ink;
    // the period ticks, as the shell dotted them
    let faint = Ink { a: 0.55, ..ink };
    let hair = 1.0f32.max((scale as f32).round());

    // the squares: a millimetre each, every tenth heavier, counted from the frame's corner as the
    // sheet's own dots are, fading out before every edge so the graph has no border of its own
    let from_frame = |v: f64| ((v - s.frame.0) / s.mm).round() as i64;
    let (x0, y0) = (from_frame(at.x), ((at.y - s.frame.1) / s.mm).round() as i64);
    let fade = |v: f32, len: f32| {
        let edge = (len * 0.25).min(30.0 * mm);
        (v / edge).min(1.0).min((len - v) / edge).max(0.0)
    };
    for i in 0..=at.width as i64 {
        let x = px(i as f64);
        let heavy = (x0 + i) % 10 == 0;
        let a = fade(x, wide) * if heavy { 0.32 } else { 0.13 };
        if a > 0.01 {
            canvas.fill(x as i32, 0, hair as i32, high as i32, Ink { a: a * ink.a as f32, ..ink }.premultiplied());
        }
    }
    for j in 0..=at.height as i64 {
        let y = px(j as f64);
        let heavy = (y0 + j) % 10 == 0;
        let a = fade(y, high) * if heavy { 0.32 } else { 0.13 };
        if a > 0.01 {
            canvas.fill(0, y as i32, wide as i32, hair as i32, Ink { a: a * ink.a as f32, ..ink }.premultiplied());
        }
    }

    // each period's end, dotted, on the whole millimetre nearest it, and named beside it
    let tick = |age: f64| -> f32 {
        let at_mm = (at.width * (1.0 - share(age))).round().clamp(1.0, at.width - 1.0);
        px(at_mm)
    };
    for (age, name) in PERIODS {
        let x = tick(age);
        let mut y = 0.0;
        while y < high {
            canvas.fill(x as i32, y as i32, hair as i32, (0.3 * mm) as i32, faint.premultiplied());
            y += 1.2 * mm;
        }
        let label = px(at.height - 5.0) + face.ascent(1.8 * mm);
        if x + face.width(name, 1.8 * mm, 0.0) + 0.4 * mm < wide {
            face.draw(canvas, x + 0.4 * mm, label, name, 1.8 * mm, 0.0, faint.premultiplied());
        }
    }

    // the readings: two millimetres inside the top and the foot, so a full or an idle machine stays
    // clear of the edge
    let plot = |v: f64, line: &Line| -> f32 {
        let share = if line.log {
            (v.max(line.least).ln() - line.least.ln()) / (line.most.ln() - line.least.ln())
        } else {
            (v - line.least) / (line.most - line.least)
        };
        px(at.height - 2.0) - share.clamp(0.0, 1.0) as f32 * (px(at.height - 2.0) - px(2.0))
    };
    let x_of = |age: f64| wide - (wide * share(age).min(1.0) as f32);
    let mut ends: Vec<(f32, String, Ink, bool)> = Vec::new();
    for line in lines(domain, r) {
        let points = r.points(line.key);
        if points.is_empty() {
            continue;
        }
        let colour = if line.alert { theme.alert } else { ink };
        // broken where nothing was kept: further apart than half as much again as their slots
        let mut runs: Vec<Vec<&Point>> = Vec::new();
        let mut run: Vec<&Point> = Vec::new();
        for p in &points {
            if let Some(last) = run.last() {
                if p.age - last.age > 1.5 * p.slot.max(last.slot) + 5.0 {
                    runs.push(std::mem::take(&mut run));
                }
            }
            run.push(p);
        }
        if !run.is_empty() {
            runs.push(run);
        }
        for run in &runs {
            // the bold reading's least to most, a band under its line
            if line.bold && run.len() > 1 {
                let band = Ink { a: (ink.a * 0.12).min(1.0), ..colour };
                for pair in run.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    let (xa, xb) = (x_of(a.age), x_of(b.age));
                    let step = (xb - xa).abs().ceil().max(1.0);
                    for i in 0..=step as i32 {
                        let t = i as f32 / step;
                        let x = xa + (xb - xa) * t;
                        let (top, foot) = (plot(a.most, &line) + (plot(b.most, &line) - plot(a.most, &line)) * t, plot(a.least, &line) + (plot(b.least, &line) - plot(a.least, &line)) * t);
                        canvas.fill(x as i32, top.min(foot) as i32, 1, (foot - top).abs().max(1.0) as i32, band.premultiplied());
                    }
                }
            }
            let weight = (line.weight * s.mm * scale) as f32;
            for (i, pair) in run.windows(2).enumerate() {
                if line.dashed && i % 3 == 2 {
                    continue;
                }
                canvas.line((x_of(pair[0].age), plot(pair[0].mean, &line)), (x_of(pair[1].age), plot(pair[1].mean, &line)), weight, colour.premultiplied());
            }
        }
        if let Some(newest) = points.first() {
            let text = if newest.age < 30.0 { lettered(domain, &line, newest.mean) } else { line.name.to_string() };
            ends.push((plot(newest.mean, &line), text, colour, line.bold));
        }
    }

    // each line's name and what it reads now, just above its end, on the paper as a drawing breaks a
    // line for its lettering; kept apart where two lines meet
    ends.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let gap = 3.2 * mm;
    for i in 1..ends.len() {
        let least = ends[i - 1].0 + gap;
        if ends[i].0 < least {
            ends[i].0 = least;
        }
    }
    for (y, text, colour, _) in &ends {
        let size = 2.1 * mm;
        let w = face.width(text, size, 0.0);
        let x = px(at.width - 1.0) - w - 0.6 * mm;
        let top = (y - size - 0.6 * mm).clamp(0.0, high - size);
        canvas.fill((x - 0.6 * mm) as i32, top as i32, (w + 1.2 * mm) as i32, (size * 1.25) as i32, Ink { a: 0.9, ..theme.paper }.premultiplied());
        face.draw(canvas, x, top + face.ascent(size), text, size, 0.0, colour.premultiplied());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reading_plotted_is_whichever_the_compositor_names() {
        assert_eq!(Domain::of("load"), Domain::Load);
        assert_eq!(Domain::of("disk"), Domain::Disk);
        assert_eq!(Domain::of(""), Domain::None);
        assert_eq!(Domain::of("nothing of the sort"), Domain::None);
    }

    #[test]
    fn the_time_axis_gives_the_recent_as_much_room_as_the_old() {
        // five seconds ago is at the right, a year at the left
        assert!(share(5.0) < 0.001);
        assert!((share(OLDEST) - 1.0).abs() < 0.001);
        // the last five minutes take about as much width as the five months before the year
        let minutes = share(300.0) - share(5.0);
        let months = share(OLDEST) - share(OLDEST / 5.0);
        assert!(minutes > months, "the recent is not given its room: {minutes} against {months}");
        // and nothing younger than five seconds runs off the end
        assert_eq!(share(0.0), share(5.0));
    }

    #[test]
    fn throughput_reads_as_a_meter_reads_it() {
        assert_eq!(rate(0.0), "0 B/s");
        assert_eq!(rate(999.0), "999 B/s");
        assert_eq!(rate(1500.0), "1.5 kB/s");
        assert_eq!(rate(12_300_000.0), "12.3 MB/s");
        assert_eq!(rate(2_500_000_000.0), "2.5 GB/s");
    }

    #[test]
    fn it_sits_level_with_the_title_block() {
        let s = crate::schedule::tests::tests_screen();
        let at = place(&s).expect("there is room");
        let block = s.title.as_ref().unwrap();
        assert_eq!(at.y, block.rect.1, "its top is the title block's");
        assert_eq!(at.x, s.frame.0 + GAP * s.mm, "it starts GAP in from the frame's line");
        // and it stops short of the block
        assert!(at.x + at.width * s.mm <= block.rect.0, "it ran into the title block");
    }
}
