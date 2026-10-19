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
    // the three daemons are asked on one bus connection, in a thread, while the sensors are read here
    // (dbus.rs). A daemon that does not answer is left out: the thread goes with the process.
    let (sent, daemons) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Some(conn) = crate::dbus::system() else { return };
        let (wifi, wired) = crate::dbus::network(&conn);
        let _ = sent.send((crate::dbus::bluetooth(&conn), crate::dbus::power_profile(&conn), wifi, wired));
    });
    let cfg = crate::config::load(p);
    let (t, hot) = temps(cfg.get("alerts").unwrap_or(&Value::Null));
    let power = power();
    let (bt, profile, wifi, wired) = daemons.recv_timeout(std::time::Duration::from_millis(750)).unwrap_or_default();
    let bt: Vec<Value> = bt.into_iter().map(|n| json!(n)).collect();
    let wifi = wifi.map(|(signal, ssid)| json!({ "signal": signal, "ssid": ssid })).unwrap_or(Value::Null);
    println!(
        "{}",
        util::dumps(&json!({"temps": t, "hot": hot, "power": power, "profile": profile, "bt": bt, "processes": crate::processes::count(),
            "wifi": wifi, "wired": wired, "backlight": backlight()}))
    );
}

/// The first backlight's directory, for the shell to read itself (qml/Watch.qml); None without one.
fn backlight() -> Option<String> {
    let mut dirs: Vec<_> = std::fs::read_dir("/sys/class/backlight").ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.first().map(|d| d.display().to_string())
}

/// The sensors as numbers, for the recorder (metrics.rs): the CPU's and the NVMe's temperatures (°C), the
/// fan (RPM), the power drawn while on battery (W) and the battery's charge (0..1); None where there is none.
pub fn raw() -> [Option<f64>; 5] {
    let cpu = hwmon(&["coretemp", "k10temp", "zenpower"]).and_then(|d| labelled(&d, &["Package id 0", "Tctl", "Tdie"])).map(|v| v as f64 / 1000.0);
    let nvme = hwmon(&["nvme"]).and_then(|d| read_int(&d.join("temp1_input"))).map(|v| v as f64 / 1000.0);
    let mut fans: Vec<PathBuf> = hwmons().into_iter().map(|d| d.join("fan1_input")).filter(|f| f.exists()).collect();
    fans.sort_by_key(|f| read_str(&f.with_file_name("name")).as_deref() == Some("acpi_fan"));
    let fan = fans.iter().find_map(|f| read_int(f)).map(|v| v as f64);
    let power = power().as_f64();
    let mut bats: Vec<PathBuf> = std::fs::read_dir("/sys/class/power_supply")
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("BAT"))).collect())
        .unwrap_or_default();
    bats.sort();
    let battery = bats.first().and_then(|b| read_int(&b.join("capacity"))).map(|v| v as f64 / 100.0);
    [cpu, nvme, fan, power, battery]
}
