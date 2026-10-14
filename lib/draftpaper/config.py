"""The user's settings (~/.config/draftpaper/config.toml), with defaults for everything.

    theme = "softglow"            # a file in ~/.config/draftpaper/themes or /usr/share/draftpaper/themes
    margin = 6                    # millimetres between the screen edge and the frame (3 to 20)

    [title]                       # the title block's fixed cells
    project = "my-host"           # default: the host name
    drawn = "me"                  # default: the user name

    [[cell]]                      # optional: cells of your own; the first goes in the title block's last
                                  # row, the others in the bottom margin's free zones, one per zone
    label = "TICKET"
    command = ["jt-bar"]          # its first line of output is the value; empty hides it
    interval = 5                  # seconds
    left = ["jt-bar", "toggle"]   # optional click commands
    right = ["jt-bar", "pick"]

    [alerts]                      # thresholds, in degrees Celsius and RPM
    cpu = 85
    nvme = 60
    fan = 5000
    battery = 15                  # percent

    [commands]
    screenshot = ["flameshot", "gui"]
"""
import getpass, socket, tomllib
from . import paths

DEFAULTS = {
    "theme": "softglow",
    "margin": 6.0,
    "title": {},
    "cell": [],
    "alerts": {"cpu": 85, "nvme": 60, "fan": 5000, "battery": 15},
    "commands": {"screenshot": ["flameshot", "gui"]},
}


def load():
    cfg = {k: (dict(v) if isinstance(v, dict) else list(v) if isinstance(v, list) else v) for k, v in DEFAULTS.items()}
    if paths.CONFIG.exists():
        with paths.CONFIG.open("rb") as f:
            user = tomllib.load(f)
        for k, v in user.items():
            if isinstance(v, dict) and isinstance(cfg.get(k), dict):
                cfg[k].update(v)
            else:
                cfg[k] = v
    cfg["margin"] = min(20.0, max(3.0, float(cfg["margin"])))
    cfg["title"].setdefault("project", socket.gethostname().upper())
    cfg["title"].setdefault("drawn", getpass.getuser())
    return cfg


def set_theme(name):
    """Write theme = name into config.toml, keeping everything else in the file."""
    import re
    paths.CONFIG.parent.mkdir(parents=True, exist_ok=True)
    text = paths.CONFIG.read_text() if paths.CONFIG.exists() else ""
    line = f'theme = "{name}"'
    if re.search(r'^theme\s*=.*$', text, flags=re.M):
        text = re.sub(r'^theme\s*=.*$', line, text, count=1, flags=re.M)
    else:
        text = line + "\n" + text
    paths.CONFIG.write_text(text)
