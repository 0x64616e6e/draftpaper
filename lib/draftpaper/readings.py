"""Readings the shell cannot take by itself, as one JSON object (`draftpaper readings`, every 5 s):

  temps    [{label, value}] CPU package (coretemp or k10temp), NVMe, and a fan in RPM
  hot      true while any of them is past its threshold (config [alerts]: cpu, nvme, fan)
  power    battery draw in watts while discharging, null on mains
  profile  the power profile (powerprofilesctl), "" without power-profiles-daemon
  bt       names of the connected Bluetooth devices (bluetoothctl)

hwmon devices are found by name, not index, so a reboot that renumbers them changes nothing.
"""
import glob, json, shutil, subprocess
from . import config

HWMON = "/sys/class/hwmon"


def read(path, cast=int):
    try:
        with open(path) as f:
            return cast(f.read().strip())
    except (OSError, ValueError):
        return None


def hwmon(*names):
    for d in sorted(glob.glob(f"{HWMON}/hwmon*")):
        if read(f"{d}/name", str) in names:
            return d
    return None


def labelled(d, *labels):
    for f in sorted(glob.glob(f"{d}/temp*_label")):
        if read(f, str) in labels:
            return read(f.replace("_label", "_input"))
    return read(f"{d}/temp1_input")


def temps(alerts):
    out, hot = [], False
    cpu = hwmon("coretemp", "k10temp", "zenpower")
    if cpu:
        v = labelled(cpu, "Package id 0", "Tctl", "Tdie")
        if v is not None:
            out.append({"label": "CPU", "value": f"{v / 1000:.0f}°"})
            hot |= v / 1000 >= alerts["cpu"]
    nvme = hwmon("nvme")
    if nvme:
        v = read(f"{nvme}/temp1_input")
        if v is not None:
            out.append({"label": "NVME", "value": f"{v / 1000:.0f}°"})
            hot |= v / 1000 >= alerts["nvme"]
    # the fan: a laptop's own sensor first (acpi_fan often reads 0 whatever the fan does)
    fans = sorted(glob.glob(f"{HWMON}/hwmon*/fan1_input"), key=lambda f: read(f"{f.rsplit('/', 1)[0]}/name", str) == "acpi_fan")
    for f in fans:
        v = read(f)
        if v is not None:
            out.append({"label": "FAN", "value": f"{v}"})
            hot |= v >= alerts["fan"]
            break
    return out, hot


def power():
    for b in sorted(glob.glob("/sys/class/power_supply/BAT*")):
        if read(f"{b}/status", str) == "Discharging":
            uw = read(f"{b}/power_now")
            if uw is None:                                   # some batteries report current and voltage
                ua, uv = read(f"{b}/current_now"), read(f"{b}/voltage_now")
                uw = ua * uv / 1e6 if ua and uv else None
            return round(uw / 1e6, 1) if uw else None
    return None


def command(*cmd):
    if not shutil.which(cmd[0]):
        return ""
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=3).stdout.strip()
    except (subprocess.TimeoutExpired, OSError):
        return ""


def main(_args):
    alerts = config.load()["alerts"]
    t, hot = temps(alerts)
    bt = [line.split(" ", 2)[2] for line in command("bluetoothctl", "devices", "Connected").splitlines()
          if line.startswith("Device ") and len(line.split(" ", 2)) == 3]
    print(json.dumps({"temps": t, "hot": hot, "power": power(),
                      "profile": command("powerprofilesctl", "get"), "bt": bt}))
