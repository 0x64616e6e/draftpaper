//! `draftpaper processes`: the process schedule's rows (qml/ProcessSchedule.qml), as JSON: what top
//! tells, the programs most busy first. /proc read twice, half a second apart: each program's share of a
//! processor in that time (top's way: 100 is one processor busy), its memory (resident) and its processor
//! time since it started; the processes of one executable counted as one (Firefox's dozen, one row), its pid
//! when there is one, else how many. Kernel threads left out. With the load average and how many
//! processes there are, for the schedule's heading.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::util;

struct Sample {
    name: String,
    jiffies: u64,
    rss_pages: u64,
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Every user process: pid → its name (comm), processor time (utime + stime) and resident pages. A
/// kernel thread (no command line) is left out.
fn sample() -> HashMap<u32, Sample> {
    let mut out = HashMap::new();
    for e in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        let stat = read(&format!("/proc/{pid}/stat"));
        // comm is in parentheses and may hold spaces: the fields after the last ')'
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else { continue };
        // the program: its executable's name (all of Firefox's processes are firefox-esr, whatever each calls
        // itself), else its own name (comm, 15 characters at most; another user's process hides its executable)
        let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().trim_end_matches(" (deleted)").to_string()));
        let name = exe.filter(|n| !n.is_empty()).unwrap_or_else(|| stat[open + 1..close].to_string());
        let f: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        // after ')': state(0) ppid(1) ... utime(11) stime(12)
        let (Some(ut), Some(st)) = (f.get(11).and_then(|v| v.parse::<u64>().ok()), f.get(12).and_then(|v| v.parse::<u64>().ok())) else { continue };
        if read(&format!("/proc/{pid}/cmdline")).is_empty() {
            continue;
        }
        let rss_pages = read(&format!("/proc/{pid}/statm")).split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        out.insert(pid, Sample { name, jiffies: ut + st, rss_pages });
    }
    out
}

/// "0:41", "1:52:07": processor time, from clock ticks.
fn time(ticks: u64, hz: u64) -> String {
    let s = ticks / hz.max(1);
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// "212 MB", "4.1 GB".
fn memory(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 { format!("{:.1} GB", b / 1e9) } else { format!("{:.0} MB", b / 1e6) }
}

pub fn main(rows: usize) {
    let hz = 100;
    let page = 4096;
    let first = sample();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let second = sample();
    let seconds = 0.5;
    // by program: share of a processor, memory, processor time, how many and the pid of one
    struct Program {
        busy: f64,
        bytes: u64,
        ticks: u64,
        count: usize,
        pid: u32,
    }
    let mut programs: HashMap<String, Program> = HashMap::new();
    for (pid, s) in &second {
        let before = first.get(pid).filter(|b| b.name == s.name).map(|b| b.jiffies).unwrap_or(s.jiffies);
        let busy = (s.jiffies.saturating_sub(before)) as f64 / hz as f64 / seconds * 100.0;
        let p = programs.entry(s.name.clone()).or_insert(Program { busy: 0.0, bytes: 0, ticks: 0, count: 0, pid: *pid });
        p.busy += busy;
        p.bytes += s.rss_pages * page;
        p.ticks += s.jiffies;
        p.count += 1;
        p.pid = p.pid.min(*pid);
    }
    let mut list: Vec<(String, Program)> = programs.into_iter().collect();
    list.sort_by(|a, b| b.1.busy.partial_cmp(&a.1.busy).unwrap_or(std::cmp::Ordering::Equal).then(b.1.bytes.cmp(&a.1.bytes)));
    let rows: Vec<Value> = list
        .iter()
        .take(rows)
        .map(|(name, p)| {
            json!({
                "name": name,
                "pid": if p.count == 1 { p.pid.to_string() } else { format!("×{}", p.count) },
                "cpu": (p.busy * 10.0).round() / 10.0,
                "memory": memory(p.bytes),
                "time": time(p.ticks, hz),
            })
        })
        .collect();
    let load = read("/proc/loadavg").split_whitespace().next().unwrap_or("").to_string();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    println!("{}", util::dumps(&json!({ "rows": rows, "load": load, "processes": second.len(), "threads": threads })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures() {
        assert_eq!(time(4100, 100), "0:41");
        assert_eq!(time(672_700, 100), "1:52:07");
        assert_eq!(memory(212_000_000), "212 MB");
        assert_eq!(memory(4_100_000_000), "4.1 GB");
    }
}

/// How many processes there are, as the schedule counts them (kernel threads left out): the title block's
/// PROCESSES cell (`draftpaper readings`).
pub fn count() -> usize {
    std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit())))
        .filter(|e| std::fs::read(e.path().join("cmdline")).is_ok_and(|c| !c.is_empty()))
        .count()
}
