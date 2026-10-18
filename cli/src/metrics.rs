//! The readings over time (the graph's, qml/LoadGraph.qml): `draftpaper metrics keep`, a user service of the
//! session (draftpaper-metrics.service), samples the machine every 5 s — the processor's load and the memory
//! in use (0..1), the CPU's and the NVMe's temperatures (°C), the fan (RPM), the power drawn on battery (W)
//! and the battery's charge (0..1) — into a store of six periods, each of fixed slots: the last 5 minutes
//! in 5 s slots, the hour in minutes, the day in quarters of an hour, the week in hours, the month in
//! quarter days, the year in days. Every sample goes into the current slot of every period at once (its
//! count, sum, least and most), so nothing is kept but those (some 870 slots a reading) and nothing has to
//! be rolled up later; a slot older than its period goes.
//!
//! Written whole every sample to the runtime directory (memory: $XDG_RUNTIME_DIR/draftpaper/metrics.json,
//! which the shell reads in place), and every 5 minutes to the state directory (the disk:
//! ~/.local/state/draftpaper/metrics.json), from which the next start goes on. The file:
//!
//!     {"now": EPOCH, "metrics": ["cpu", ...], "tiers": [{"slot": 5, "upto": 300,
//!       "slots": [[START, [[n, sum, min, max] | null, ...]], ...]}, ...]}

use std::collections::VecDeque;

use serde_json::{json, Value};

use crate::util::{self, Paths};

/// Each period: its slot and how far back it reaches, in seconds.
pub const TIERS: [(u64, u64); 6] = [(5, 300), (60, 3600), (900, 86_400), (3600, 604_800), (21_600, 2_592_000), (86_400, 31_536_000)];
pub const METRICS: [&str; 7] = ["cpu", "mem", "temp", "nvme", "fan", "power", "battery"];

#[derive(Debug, Clone, Copy, PartialEq)]
struct Acc {
    n: u32,
    sum: f64,
    min: f64,
    max: f64,
}

impl Acc {
    fn add(a: Option<Acc>, v: f64) -> Acc {
        match a {
            None => Acc { n: 1, sum: v, min: v, max: v },
            Some(a) => Acc { n: a.n + 1, sum: a.sum + v, min: a.min.min(v), max: a.max.max(v) },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Slot {
    start: u64,
    acc: [Option<Acc>; METRICS.len()],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Store {
    now: u64,
    tiers: [VecDeque<Slot>; TIERS.len()],
}

impl Store {
    /// A sample at `now` (epoch seconds): each reading, or None where there is none.
    pub fn add(&mut self, now: u64, values: [Option<f64>; METRICS.len()]) {
        self.now = self.now.max(now);
        for (t, &(slot, upto)) in TIERS.iter().enumerate() {
            let start = now - now % slot;
            let q = &mut self.tiers[t];
            if q.back().is_none_or(|s| s.start < start) {
                q.push_back(Slot { start, acc: [None; METRICS.len()] });
            }
            if let Some(s) = q.iter_mut().rev().find(|s| s.start == start) {
                for (m, v) in values.iter().enumerate() {
                    if let Some(v) = v.filter(|v| v.is_finite()) {
                        s.acc[m] = Some(Acc::add(s.acc[m], v));
                    }
                }
            }
            // slots older than the period go
            while q.front().is_some_and(|s| s.start + upto <= self.now) {
                q.pop_front();
            }
        }
    }

    pub fn to_json(&self) -> Value {
        let r = |v: f64| (v * 1000.0).round() / 1000.0;
        let tiers: Vec<Value> = TIERS
            .iter()
            .zip(&self.tiers)
            .map(|(&(slot, upto), q)| {
                let slots: Vec<Value> = q
                    .iter()
                    .map(|s| json!([s.start, s.acc.iter().map(|a| a.map(|a| json!([a.n, r(a.sum), r(a.min), r(a.max)])).unwrap_or(Value::Null)).collect::<Vec<_>>()]))
                    .collect();
                json!({ "slot": slot, "upto": upto, "slots": slots })
            })
            .collect();
        json!({ "now": self.now, "metrics": METRICS, "tiers": tiers })
    }

    /// A store as written; readings of names it does not know are left out, periods matched by their slot.
    pub fn from_json(v: &Value) -> Store {
        let mut store = Store { now: v["now"].as_u64().unwrap_or(0), ..Default::default() };
        let names: Vec<&str> = v["metrics"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        for tier in v["tiers"].as_array().into_iter().flatten() {
            let Some(t) = TIERS.iter().position(|&(slot, _)| Some(slot) == tier["slot"].as_u64()) else { continue };
            for s in tier["slots"].as_array().into_iter().flatten() {
                let Some(start) = s[0].as_u64() else { continue };
                let mut acc = [None; METRICS.len()];
                for (i, a) in s[1].as_array().into_iter().flatten().enumerate() {
                    let (Some(name), Some(f)) = (names.get(i), a.as_array()) else { continue };
                    let Some(m) = METRICS.iter().position(|n| n == name) else { continue };
                    if let (Some(n), Some(sum), Some(min), Some(max)) = (f.first().and_then(Value::as_u64), f.get(1).and_then(Value::as_f64), f.get(2).and_then(Value::as_f64), f.get(3).and_then(Value::as_f64)) {
                        acc[m] = Some(Acc { n: n as u32, sum, min, max });
                    }
                }
                store.tiers[t].push_back(Slot { start, acc });
            }
        }
        store
    }
}

/// The processor's busy share since the last call, from /proc/stat (None the first time).
struct Cpu(Option<(u64, u64)>);

impl Cpu {
    fn busy(&mut self) -> Option<f64> {
        let line = util::read(std::path::Path::new("/proc/stat")).lines().next()?.to_string();
        let f: Vec<u64> = line.split_whitespace().skip(1).take(8).filter_map(|v| v.parse().ok()).collect();
        if f.len() < 5 {
            return None;
        }
        let (idle, total) = (f[3] + f[4], f.iter().sum::<u64>());
        let before = self.0.replace((idle, total));
        let (i0, t0) = before?;
        (total > t0).then(|| 1.0 - (idle - i0) as f64 / (total - t0) as f64)
    }
}

fn memory() -> Option<f64> {
    let text = util::read(std::path::Path::new("/proc/meminfo"));
    let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)).and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok());
    let (total, available) = (field("MemTotal:")?, field("MemAvailable:")?);
    (total > 0.0).then(|| 1.0 - available / total)
}

fn epoch() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn runtime_file(p: &Paths) -> std::path::PathBuf {
    p.runtime.join("metrics.json")
}

fn state_file(p: &Paths) -> std::path::PathBuf {
    p.state.join("metrics.json")
}

/// `draftpaper metrics keep`: sample every 5 s (DRAFTPAPER_METRICS_EVERY seconds, for the tests) for as
/// long as it runs, going on from the newer of the runtime and the state copies.
pub fn keep(p: &Paths) {
    let every = std::env::var("DRAFTPAPER_METRICS_EVERY").ok().and_then(|v| v.parse::<u64>().ok()).filter(|v| *v > 0).unwrap_or(5);
    let load = |f: std::path::PathBuf| serde_json::from_str::<Value>(&util::read(&f)).ok().map(|v| Store::from_json(&v));
    let mut store = [load(runtime_file(p)), load(state_file(p))].into_iter().flatten().max_by_key(|s| s.now).unwrap_or_default();
    let mut cpu = Cpu(None);
    cpu.busy();
    let _ = std::fs::create_dir_all(&p.runtime);
    let mut saved = epoch();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(every));
        let [temp, nvme, fan, power, battery] = crate::readings::raw();
        let now = epoch();
        store.add(now, [cpu.busy(), memory(), temp, nvme, fan, power, battery]);
        let text = util::dumps(&store.to_json());
        util::write_atomic(&runtime_file(p), &text);
        // the disk: every 5 minutes
        if now >= saved + 300 {
            let _ = std::fs::create_dir_all(&p.state);
            util::write_atomic(&state_file(p), &text);
            saved = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(cpu: f64) -> [Option<f64>; METRICS.len()] {
        [Some(cpu), Some(0.5), Some(50.0), None, None, None, Some(0.8)]
    }

    #[test]
    fn every_period_takes_every_sample() {
        let mut s = Store::default();
        let t0 = 1_800_000_000 - 1_800_000_000 % 86_400;
        for k in 0..24 {
            s.add(t0 + k * 5, v(k as f64 / 100.0));
        }
        // 2 minutes: 24 slots of 5 s, 2 of a minute, 1 of each longer period
        assert_eq!(s.tiers.iter().map(|q| q.len()).collect::<Vec<_>>(), vec![24, 2, 1, 1, 1, 1]);
        let day = s.tiers[5][0].acc[0].unwrap();
        assert_eq!(day.n, 24);
        assert!((day.sum / 24.0 - 0.115).abs() < 1e-9 && day.min == 0.0 && (day.max - 0.23).abs() < 1e-9);
        assert!(s.tiers[0][0].acc[3].is_none(), "a reading that is not there stays out");
    }

    #[test]
    fn slots_older_than_their_period_go() {
        let mut s = Store::default();
        let t0 = 1_800_000_000 - 1_800_000_000 % 86_400;
        for k in 0..200 {
            s.add(t0 + k * 5, v(0.1));
        }
        // 1000 s in: the 5 s slots of the last 5 minutes only
        assert_eq!(s.tiers[0].len(), 60);
        assert!(s.tiers[0].front().unwrap().start + 300 > t0 + 199 * 5);
        assert_eq!(s.tiers[1].len(), 17);
    }

    #[test]
    fn written_and_read_back() {
        let mut s = Store::default();
        for k in 0..30 {
            s.add(1_800_000_000 + k * 7, v(0.25));
        }
        let back = Store::from_json(&s.to_json());
        assert_eq!(back.to_json(), s.to_json());
        assert_eq!(back.now, s.now);
    }
}
