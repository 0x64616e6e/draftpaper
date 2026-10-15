//! Readings the shell cannot take by itself, as one JSON object (`draftpaper readings`, every 5 s):
//!
//!   temps    [{label, value}] CPU package (coretemp or k10temp), NVMe, and a fan in RPM
//!   hot      true while any of them is past its threshold (config [alerts]: cpu, nvme, fan)
//!   power    battery draw in watts while discharging, null on mains
//!   profile  the power profile (power-profiles-daemon over D-Bus, busctl), "" without it
//!   bt       names of the connected Bluetooth devices (bluetoothctl)
//!
//! hwmon devices are found by name, not index, so a reboot that renumbers them changes nothing.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::util::{self, Paths};

const HWMON: &str = "/sys/class/hwmon";

fn read_str(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}
fn read_int(p: &Path) -> Option<i64> {
    read_str(p)?.parse().ok()
}

fn hwmons() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(HWMON)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("hwmon"))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn hwmon(names: &[&str]) -> Option<PathBuf> {
    hwmons().into_iter().find(|d| read_str(&d.join("name")).is_some_and(|n| names.contains(&n.as_str())))
}

fn labelled(d: &Path, labels: &[&str]) -> Option<i64> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(d)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("temp") && n.ends_with("_label"))).collect())
        .unwrap_or_default();
    files.sort();
    for f in files {
        if read_str(&f).is_some_and(|l| labels.contains(&l.as_str())) {
            return read_int(&PathBuf::from(f.display().to_string().replace("_label", "_input")));
        }
    }
    read_int(&d.join("temp1_input"))
}

fn alert(alerts: &Value, k: &str, default: f64) -> f64 {
    alerts.get(k).and_then(Value::as_f64).unwrap_or(default)
}

fn temps(alerts: &Value) -> (Vec<Value>, bool) {
    let (mut out, mut hot) = (Vec::new(), false);
    let deg = |v: i64| format!("{:.0}°", v as f64 / 1000.0);
    if let Some(cpu) = hwmon(&["coretemp", "k10temp", "zenpower"]) {
        if let Some(v) = labelled(&cpu, &["Package id 0", "Tctl", "Tdie"]) {
            out.push(json!({"label": "CPU", "value": deg(v)}));
            hot |= v as f64 / 1000.0 >= alert(alerts, "cpu", 85.0);
        }
    }
    if let Some(nvme) = hwmon(&["nvme"]) {
        if let Some(v) = read_int(&nvme.join("temp1_input")) {
            out.push(json!({"label": "NVME", "value": deg(v)}));
            hot |= v as f64 / 1000.0 >= alert(alerts, "nvme", 60.0);
        }
    }
    // the fan: a laptop's own sensor first (acpi_fan often reads 0 whatever the fan does)
    let mut fans: Vec<PathBuf> = hwmons().into_iter().map(|d| d.join("fan1_input")).filter(|f| f.exists()).collect();
    fans.sort_by_key(|f| read_str(&f.with_file_name("name")).as_deref() == Some("acpi_fan"));
    for f in fans {
        if let Some(v) = read_int(&f) {
            out.push(json!({"label": "FAN", "value": v.to_string()}));
            hot |= v as f64 >= alert(alerts, "fan", 5000.0);
            break;
        }
    }
    (out, hot)
}

fn power() -> Value {
    let mut bats: Vec<PathBuf> = std::fs::read_dir("/sys/class/power_supply")
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("BAT"))).collect())
        .unwrap_or_default();
    bats.sort();
    for b in bats {
        if read_str(&b.join("status")).as_deref() == Some("Discharging") {
            let uw = read_int(&b.join("power_now")).map(|v| v as f64).or_else(|| {
                // some batteries report current and voltage
                match (read_int(&b.join("current_now")), read_int(&b.join("voltage_now"))) {
                    (Some(a), Some(v)) if a != 0 && v != 0 => Some(a as f64 * v as f64 / 1e6),
                    _ => None,
                }
            });
            return match uw {
                Some(w) if w != 0.0 => json!(util::round_to(w / 1e6, 1)),
                _ => Value::Null,
            };
        }
    }
    Value::Null
}

pub fn main(p: &Paths) {
    // the two daemons are asked side by side, while the sensors are read
    let bt = std::thread::spawn(|| util::run(&["bluetoothctl", "devices", "Connected"], 3));
    // straight from the daemon (powerprofilesctl is a Python script: 67 ms, against busctl's 4)
    let profile = std::thread::spawn(|| {
        util::run(&["busctl", "get-property", "net.hadess.PowerProfiles", "/net/hadess/PowerProfiles", "net.hadess.PowerProfiles", "ActiveProfile"], 3)
            .unwrap_or_default()
            .trim()
            .strip_prefix("s ")
            .map(|v| v.trim_matches('"').to_string())
            .unwrap_or_default()
    });
    let cfg = crate::config::load(p);
    let (t, hot) = temps(cfg.get("alerts").unwrap_or(&Value::Null));
    let power = power();
    let bt: Vec<Value> = bt
        .join()
        .ok()
        .flatten()
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with("Device "))
        .filter_map(|l| l.splitn(3, ' ').nth(2).map(|n| json!(n)))
        .collect();
    let profile = profile.join().unwrap_or_default();
    println!("{}", util::dumps(&json!({"temps": t, "hot": hot, "power": power, "profile": profile, "bt": bt})));
}
