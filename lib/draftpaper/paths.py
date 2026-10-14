"""Where Draftpaper finds its files and keeps its state.

Installed, the shared files live in /usr/share/draftpaper; from a checkout, in the repository itself
(the launcher sets DRAFTPAPER_ROOT). Per user: settings in ~/.config/draftpaper/config.toml, themes of
their own in ~/.config/draftpaper/themes, generated files (wallpapers, theme.json, i3.conf) in
~/.cache/draftpaper, runtime state (parked windows) in $XDG_RUNTIME_DIR/draftpaper.
"""
import os
from pathlib import Path

HOME = Path.home()
ROOT = Path(os.environ.get("DRAFTPAPER_ROOT", "/usr/share/draftpaper"))
QML = ROOT / "qml" / "shell.qml"
THEMES = [HOME / ".config/draftpaper/themes", ROOT / "themes"]
CONFIG = Path(os.environ.get("XDG_CONFIG_HOME", HOME / ".config")) / "draftpaper" / "config.toml"
CACHE = Path(os.environ.get("XDG_CACHE_HOME", HOME / ".cache")) / "draftpaper"
RUNTIME = Path(os.environ.get("XDG_RUNTIME_DIR", "/tmp")) / "draftpaper"
THEME_JSON = CACHE / "theme.json"
I3_CONF = CACHE / "i3.conf"
PARKED = RUNTIME / "parked.json"


def ensure():
    for d in (CACHE, RUNTIME):
        d.mkdir(parents=True, exist_ok=True)
