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

/// Every display controller on the PCI bus, named from the system's own list of PCI ids: what `lspci`
/// prints, read from /sys instead of starting it. The classes are 0300 VGA, 0302 3D, 0380 display.
fn gpus() -> Vec<String> {
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir("/sys/bus/pci/devices").into_iter().flatten().flatten().map(|e| e.path()).collect();
    dirs.sort();
    let mut cards: Vec<(u16, u16)> = Vec::new();
    for d in dirs {
        let at = |f: &str| read(&d.join(f).to_string_lossy());
        let class = at("class");
        if !["0300", "0302", "0380"].iter().any(|k| class.trim_start_matches("0x").starts_with(k)) {
            continue;
        }
        let id = |f: &str| u16::from_str_radix(at(f).trim().trim_start_matches("0x"), 16).ok();
        if let (Some(vendor), Some(device)) = (id("vendor"), id("device")) {
            cards.push((vendor, device));
        }
    }
    let names = pci_names(&cards);
    cards
        .iter()
        .map(|k| match names.get(k) {
            Some(name) => gpu_name(name),
            // no list installed (hwdata): the ids themselves, as lspci shows them without it
            None => format!("{:04x}:{:04x}", k.0, k.1),
        })
        .collect()
}

/// The names of the PCI devices wanted, from the list `lspci` itself reads; empty where it is not
/// installed. Both the vendor's name and the device's, as one string ("Intel Corporation Arrow Lake-U").
fn pci_names(want: &[(u16, u16)]) -> std::collections::HashMap<(u16, u16), String> {
    let mut out = std::collections::HashMap::new();
    if want.is_empty() {
        return out;
    }
    let text = ["/usr/share/misc/pci.ids", "/usr/share/hwdata/pci.ids"].iter().map(|p| read(p)).find(|t| !t.is_empty()).unwrap_or_default();
    // vendors start at the left, their devices one tab in, a device's subsystems two
    let mut vendor: Option<(u16, String)> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() || line.starts_with("\t\t") {
            continue;
        }
        let (id, name) = line.trim_start().split_once("  ").unwrap_or((line.trim_start(), ""));
        let Ok(id) = u16::from_str_radix(id.trim(), 16) else { continue };
        if !line.starts_with('\t') {
            vendor = Some((id, name.trim().to_string()));
            continue;
        }
        let Some((v, maker)) = vendor.as_ref() else { continue };
        if want.contains(&(*v, id)) {
            out.insert((*v, id), format!("{maker} {}", name.trim()));
            if out.len() == want.len() {
                break;
            }
        }
    }
    out
}

/// The root file system: its kind, and the bytes used and in all, as `df` reads them — the mount table
/// for the kind, the kernel for the rest.
fn root_fs() -> (Option<String>, Option<f64>, Option<f64>) {
    // the last mount on "/" is the one in use; its kind follows the " - " that ends the optional fields
    let kind = read("/proc/self/mountinfo")
        .lines()
        .filter(|l| l.split(' ').nth(4) == Some("/"))
        .filter_map(|l| Some(l.split(" - ").nth(1)?.split_whitespace().next()?.to_string()))
        .next_back();
    let Ok(s) = rustix::fs::statvfs("/") else { return (kind, None, None) };
    let unit = s.f_frsize as f64;
    (kind, Some((s.f_blocks - s.f_bfree) as f64 * unit), Some(s.f_blocks as f64 * unit))
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
    let (fs, used, size) = root_fs();
    Some(json!({
        "disk": format!("{} {kind} {}", maker, disk_size(bytes)).trim().to_string(),
        "fs": fs.map(|f| if f == "zfs" { "ZFS".to_string() } else { f }),
        "used_gb": used.map(|v| (v / 1e9).round()),
        "size_gb": size.map(|v| (v / 1e9).round()),
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

/// Everything the equipment schedule letters.
pub fn json(p: &crate::util::Paths) -> Value {
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
    put("quickshell", quickshell(p).map(|v| json!(v)));
    Value::Object(m)
}

pub fn main(p: &crate::util::Paths) {
    println!("{}", util::dumps(&json(p)));
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

/// Quickshell's version, kept in the cache beside the theme: asking the program itself costs 31 ms of
/// Qt starting up, and the answer changes only when Quickshell does. The note is keyed by the size of
/// its binary and when it was last written, so an upgrade is picked up at once.
fn quickshell(p: &crate::util::Paths) -> Option<String> {
    let bin = util::which("qs")?;
    let stamp = std::fs::metadata(&bin)
        .ok()
        .map(|m| format!("{}-{}", m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)))
        .unwrap_or_default();
    let note = p.cache.join("quickshell-version");
    if let Some((kept, version)) = read(&note.to_string_lossy()).split_once(' ') {
        if kept == stamp && !version.is_empty() {
            return Some(version.to_string());
        }
    }
    let version = util::run(&["qs", "--version"], 10)?.split_whitespace().nth(1)?.to_string();
    let _ = std::fs::create_dir_all(&p.cache);
    util::write(&note, &format!("{stamp} {version}"));
    Some(version)
}
