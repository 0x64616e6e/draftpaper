//! The three daemons the title block reads, asked over D-Bus instead of through a program each: BlueZ for
//! the devices connected, power-profiles-daemon for the profile, NetworkManager for the Wi-Fi and the
//! wired address. `draftpaper readings` ran `bluetoothctl`, `busctl` and `nmcli` twice every five
//! seconds — four processes started and thrown away, about 35 ms of a core each time, the largest cost of
//! an idle desktop.
//!
//! Every call here answers None where the daemon is not there or says something unexpected, and the whole
//! lot runs under a deadline its caller sets (readings.rs), so a daemon that does not answer cannot hold
//! up the sheet's lettering.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const PROPS: &str = "org.freedesktop.DBus.Properties";

/// The system bus, or None where there is none (a container, a session without one).
pub fn system() -> Option<Connection> {
    Connection::system().ok()
}

/// One property of one object, as `T`.
fn get<T: TryFrom<OwnedValue>>(conn: &Connection, dest: &str, path: &str, iface: &str, name: &str) -> Option<T> {
    let reply = conn.call_method(Some(dest), path, Some(PROPS), "Get", &(iface, name)).ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    T::try_from(value).ok()
}

/// The devices BlueZ has connected, by the name they are known under.
pub fn bluetooth(conn: &Connection) -> Vec<String> {
    type Managed = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;
    let Ok(reply) = conn.call_method(Some("org.bluez"), "/", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &()) else {
        return Vec::new();
    };
    let Ok(objects) = reply.body().deserialize::<Managed>() else {
        return Vec::new();
    };
    let mut names: Vec<String> = objects
        .values()
        .filter_map(|ifaces| {
            let device = ifaces.get("org.bluez.Device1")?;
            // connected only; its alias is what the user named it, its name what it calls itself
            if !device.get("Connected").and_then(|v| bool::try_from(v.try_clone().ok()?).ok()).unwrap_or(false) {
                return None;
            }
            ["Alias", "Name"].iter().find_map(|k| device.get(*k).and_then(|v| String::try_from(v.try_clone().ok()?).ok()))
        })
        .collect();
    names.sort();
    names
}

/// The power profile in use ("performance", "balanced", "power-saver"), empty without the daemon. It
/// answers on its old name and its new one, and which depends on the version installed.
pub fn power_profile(conn: &Connection) -> String {
    for (dest, path, iface) in [
        ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles", "net.hadess.PowerProfiles"),
        ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles", "org.freedesktop.UPower.PowerProfiles"),
    ] {
        if let Some(v) = get::<String>(conn, dest, path, iface, "ActiveProfile") {
            return v;
        }
    }
    String::new()
}

/// The Wi-Fi in use (its strength 0..100 and its name) and the address of a connected wired device, from
/// NetworkManager; both None without it.
pub fn network(conn: &Connection) -> (Option<(u32, String)>, Option<String>) {
    let Ok(reply) = conn.call_method(Some(NM), NM_PATH, Some(NM), "GetDevices", &()) else {
        return (None, None);
    };
    let Ok(devices) = reply.body().deserialize::<Vec<OwnedObjectPath>>() else {
        return (None, None);
    };
    let (mut wifi, mut wired) = (None, None);
    for device in devices {
        let path = device.as_str();
        let iface = "org.freedesktop.NetworkManager.Device";
        // 1 ethernet, 2 Wi-Fi; 100 is activated (NMDeviceState)
        let kind: u32 = get(conn, NM, path, iface, "DeviceType").unwrap_or(0);
        let state: u32 = get(conn, NM, path, iface, "State").unwrap_or(0);
        if state != 100 {
            continue;
        }
        if kind == 2 && wifi.is_none() {
            wifi = access_point(conn, path);
        } else if kind == 1 && wired.is_none() {
            wired = address(conn, path);
        }
    }
    (wifi, wired)
}

/// The access point a Wi-Fi device is on: its strength (0..100) and its name.
fn access_point(conn: &Connection, device: &str) -> Option<(u32, String)> {
    let ap: OwnedObjectPath = get(conn, NM, device, "org.freedesktop.NetworkManager.Device.Wireless", "ActiveAccessPoint")?;
    let ap = ap.as_str();
    if ap == "/" {
        return None;
    }
    let iface = "org.freedesktop.NetworkManager.AccessPoint";
    // the name is bytes, not a string: it is whatever the network calls itself
    let ssid: Vec<u8> = get(conn, NM, ap, iface, "Ssid")?;
    let ssid = String::from_utf8(ssid).ok()?;
    let strength: u8 = get(conn, NM, ap, iface, "Strength").unwrap_or(0);
    (!ssid.is_empty()).then_some((strength as u32, ssid))
}

/// The first address of a device, without its prefix length.
fn address(conn: &Connection, device: &str) -> Option<String> {
    let config: OwnedObjectPath = get(conn, NM, device, "org.freedesktop.NetworkManager.Device", "Ip4Config")?;
    let config = config.as_str();
    if config == "/" {
        return None;
    }
    let data: Vec<HashMap<String, OwnedValue>> = get(conn, NM, config, "org.freedesktop.NetworkManager.IP4Config", "AddressData")?;
    data.first()?.get("address").and_then(|v| String::try_from(v.try_clone().ok()?).ok()).filter(|a| !a.is_empty())
}
