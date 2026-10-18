//! `draftpaper machine`: the machine schedule's slow facts (issue #49, qml/MachineSchedule.qml), as JSON:
//! the machine, its processor, graphics, memory and storage, the system, the desktop's versions and the
//! uptime. Read from /sys, /proc and /etc/os-release (lspci for the graphics, when installed); what cannot
//! be read is left out. The displays come from the compositor, live, in the shell.

use serde_json::{json, Map, Value};

use crate::util;

fn read(path: &str) -> String {
    std::fs::read_to_string(path).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// A vendor as people write it: LENOVO → Lenovo; already mixed case stays.
fn vendor(s: &str) -> String {
    let s = s.trim();
    if s.chars().any(|c| c.is_lowercase()) || s.len() <= 3 {
        return s.to_string();
    }
    let mut c = s.chars();
    c.next().map(|f| f.to_string() + &c.as_str().to_lowercase()).unwrap_or_default()
}

/// The machine: its vendor and the model's name (Lenovo keeps the name in product_version, the number in
/// product_name).
fn machine() -> Option<String> {
    let bad = |s: &str| s.is_empty() || ["none", "default string", "to be filled by o.e.m.", "system product name"].contains(&s.to_lowercase().as_str());
    let v = read("/sys/class/dmi/id/sys_vendor");
    let version = read("/sys/class/dmi/id/product_version");
    let name = read("/sys/class/dmi/id/product_name");
    let model = if !bad(&version) && version.chars().any(|c| c.is_alphabetic()) && version.contains(' ') { version } else { name };
    if bad(&model) {
        return None;
    }
    let v = vendor(&v);
    Some(if bad(&v) || model.to_lowercase().starts_with(&v.to_lowercase()) { model } else { format!("{v} {model}") })
}

/// "Intel(R) Core(TM) Ultra 7 255U" → "Intel Core Ultra 7 255U"; "AMD Ryzen 7 7840U w/ Radeon 780M
/// Graphics" stays; a clock speed goes.
fn cpu_name(s: &str) -> String {
    let s = s.replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    let s = s.split(" @ ").next().unwrap_or("").replace(" CPU", "");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cpu() -> Option<Value> {
    let info = read("/proc/cpuinfo");
    let field = |line: &str, k: &str| line.strip_prefix(k).and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string());
    let model = info.lines().find_map(|l| field(l, "model name"))?;
    let threads = info.lines().filter(|l| l.starts_with("processor")).count();
    // cores: each (package, core id) once
    let mut cores = std::collections::BTreeSet::new();
    let (mut pkg, mut core) = (String::new(), None);
    for l in info.lines() {
        if let Some(v) = field(l, "physical id") {
            pkg = v;
        } else if let Some(v) = field(l, "core id") {
            core = Some(v);
        } else if l.trim().is_empty() {
            if let Some(c) = core.take() {
                cores.insert((pkg.clone(), c));
            }
        }
    }
    if let Some(c) = core {
        cores.insert((pkg, c));
    }
    Some(json!({ "model": cpu_name(&model), "cores": if cores.is_empty() { threads } else { cores.len() }, "threads": threads }))
}

/// lspci's "Intel Corporation Arrow Lake-U [Intel Graphics]" → "Intel Graphics (Arrow Lake-U)".
fn gpu_name(s: &str) -> String {
    let short = [("Intel Corporation", "Intel"), ("NVIDIA Corporation", "NVIDIA"), ("Advanced Micro Devices, Inc. [AMD/ATI]", "AMD")];
    let mut s = s.trim().to_string();
    let mut maker = "";
    for (long, sh) in short {
        if let Some(rest) = s.strip_prefix(long) {
            maker = sh;
            s = rest.trim().to_string();
        }
    }
    let name = match (s.rfind('['), s.ends_with(']')) {
        (Some(i), true) => {
            let inner = s[i + 1..s.len() - 1].trim().to_string();
            let chip = s[..i].trim();
            let inner = if maker.is_empty() || inner.starts_with(maker) { inner } else { format!("{maker} {inner}") };
            if chip.is_empty() { inner } else { format!("{inner} ({chip})") }
        }
        _ => if maker.is_empty() { s } else { format!("{maker} {s}") },
    };
    name.replace(" (rev ", " (").trim().to_string()
}

fn gpus() -> Vec<String> {
    let out = util::run(&["lspci"], 10).unwrap_or_default();
    out.lines()
        .filter(|l| ["VGA compatible controller", "3D controller", "Display controller"].iter().any(|k| l.contains(k)))
        .filter_map(|l| l.splitn(3, ": ").nth(1).or_else(|| l.split(": ").nth(1)))
        .map(|d| gpu_name(d.split(" (rev").next().unwrap_or(d)))
        .collect()
}

/// A size in decimal units, as disks are sold: 2 TB, 512 GB.
fn disk_size(bytes: f64) -> String {
    if bytes >= 1e12 { format!("{} TB", util::round_to(bytes / 1e12, 1).to_string().trim_end_matches(".0")) } else { format!("{:.0} GB", bytes / 1e9) }
}

/// The largest disk (not a loop, a zvol, zram or a mapper): its maker, kind and size; and the root file
/// system's type and how full it is.
fn storage() -> Option<Value> {
    let mut best: Option<(f64, String, String)> = None;
    for e in std::fs::read_dir("/sys/block").ok()?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if ["loop", "zd", "zram", "dm-", "sr", "ram", "md"].iter().any(|p| n.starts_with(p)) {
            continue;
        }
        let sectors: f64 = read(&format!("/sys/block/{n}/size")).parse().unwrap_or(0.0);
        let model = read(&format!("/sys/block/{n}/device/model"));
        if best.as_ref().is_none_or(|b| sectors * 512.0 > b.0) && sectors > 0.0 {
            best = Some((sectors * 512.0, n, model));
        }
    }
    let (bytes, name, model) = best?;
    let maker = model.split_whitespace().next().map(vendor).unwrap_or_default();
    let kind = if name.starts_with("nvme") { "NVMe" } else if read(&format!("/sys/block/{name}/queue/rotational")) == "1" { "disk" } else { "SSD" };
    let df = util::run(&["df", "-B1", "--output=fstype,used,size", "/"], 10).unwrap_or_default();
    let root: Vec<&str> = df.lines().nth(1).map(|l| l.split_whitespace().collect()).unwrap_or_default();
    let n = |i: usize| root.get(i).and_then(|v| v.parse::<f64>().ok());
    Some(json!({
        "disk": format!("{} {kind} {}", maker, disk_size(bytes)).trim().to_string(),
        "fs": root.first().map(|f| if *f == "zfs" { "ZFS".to_string() } else { f.to_string() }),
        "used_gb": n(1).map(|v| (v / 1e9).round()),
        "size_gb": n(2).map(|v| (v / 1e9).round()),
    }))
}

/// "Debian 13 trixie", else the system's own name for itself.
fn system() -> Option<String> {
    let text = read("/etc/os-release");
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').to_string());
    if get("ID").as_deref() == Some("debian") {
        if let Some(v) = get("VERSION_ID") {
            return Some(format!("Debian {v}{}", get("VERSION_CODENAME").map(|c| format!(" {c}")).unwrap_or_default()));
        }
    }
    get("PRETTY_NAME")
}

/// "6.12.113+deb13-amd64" → "Linux 6.12".
fn kernel() -> Option<String> {
    let r = read("/proc/sys/kernel/osrelease");
    let mut parts = r.split(['.', '-', '+']);
    Some(format!("Linux {}.{}", parts.next()?, parts.next()?))
}

pub fn main() {
    let mut m = Map::new();
    let mut put = |k: &str, v: Option<Value>| {
        if let Some(v) = v {
            m.insert(k.into(), v);
        }
    };
    put("machine", machine().map(Value::from));
    put("cpu", cpu());
    let g = gpus();
    put("gpus", (!g.is_empty()).then(|| json!(g)));
    let mem = read("/proc/meminfo").lines().find_map(|l| l.strip_prefix("MemTotal:").and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok()));
    put("memory_gib", mem.map(|kb| json!((kb / 1024.0 / 1024.0).round())));
    put("storage", storage());
    put("system", system().map(Value::from));
    put("kernel", kernel().map(Value::from));
    put("uptime_s", read("/proc/uptime").split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).map(|v| json!(v.round())));
    put("draftpaper", Some(json!(env!("CARGO_PKG_VERSION"))));
    let qs = util::run(&["qs", "--version"], 10).unwrap_or_default();
    put("quickshell", qs.split_whitespace().nth(1).map(|v| json!(v)));
    println!("{}", util::dumps(&Value::Object(m)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(cpu_name("Intel(R) Core(TM) Ultra 7 255U"), "Intel Core Ultra 7 255U");
        assert_eq!(cpu_name("Intel(R) Core(TM) i7-8565U CPU @ 1.80GHz"), "Intel Core i7-8565U");
        assert_eq!(gpu_name("Intel Corporation Arrow Lake-U [Intel Graphics]"), "Intel Graphics (Arrow Lake-U)");
        assert_eq!(gpu_name("NVIDIA Corporation AD107M [GeForce RTX 4060 Max-Q / Mobile]"), "NVIDIA GeForce RTX 4060 Max-Q / Mobile (AD107M)");
        assert_eq!(gpu_name("Advanced Micro Devices, Inc. [AMD/ATI] Phoenix1"), "AMD Phoenix1");
        assert_eq!(vendor("LENOVO"), "Lenovo");
        assert_eq!(vendor("Dell Inc."), "Dell Inc.");
        assert_eq!(disk_size(2.048e12), "2 TB");
        assert_eq!(disk_size(5.12e11), "512 GB");
    }
}
