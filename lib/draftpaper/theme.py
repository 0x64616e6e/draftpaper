"""Themes and applying them.

A theme (themes/<name>.toml) names the paper, the ink and the window colours. The compositor reads the
theme named in config.toml itself (the sheet and the window borders, again whenever the file changes);
`apply` writes what the shell reads:

  ~/.cache/draftpaper/theme.json    the ink, alert and accent colours and the shell's settings (title
                                    block, cells, alerts, commands); shell.qml reads it live
  ~/.cache/draftpaper/fuzzel.ini    the launcher (Mod+d) drawn like the sheet: paper, ink, a square
                                    frame, matches in the alert colour, the selection inverted
  ~/.cache/draftpaper/swaylock.conf the lock screen (Mod+Shift+x): the paper, an ink ring, the accent while
                                    typing, the alert colour for a wrong password
  ~/.cache/draftpaper/swayidle.conf idle (config.toml [idle]): the displays off, a lock, a lock before
                                    sleep; draftpaper-idle.service runs it and is restarted on apply
  and the applications, from the theme's [apps] palette (apps.py)

The sheet's geometry (margin, px/mm, zones per output) is the compositor's, over its IPC.

The theme editor (qml/ThemeEditor.qml) writes its values into a copy of the theme's file, line by line so
the comments and the [apps] palette stay as they were (`write`): while editing, a draft beside the user's
themes (<name>.draft.toml) that the sheet previews; on saving, the user's own <name>.toml, which shadows a
shipped theme of that name until it is reverted.
"""
import json, re, shutil, subprocess, tomllib
from . import config, paths

SHEET_DEFAULTS = {"angle": 54.0, "midpoint": 0.45, "blend": 1.0, "shading": 1.0,
                  "glow_strength": 1.0, "glow_size": 1.0, "pattern_strength": 0.5}
WINDOW_KEYS = ("focused", "focused_inactive", "unfocused", "urgent", "text", "text_inactive", "background")


DRAFT = ".draft"


def available():
    names = {}
    for d in reversed(paths.THEMES):           # the user's own themes shadow the shipped ones
        for p in sorted(d.glob("*.toml")) if d.is_dir() else []:
            names[p.stem] = p
    return names


def listed():
    """The themes to offer: every one but the editor's drafts."""
    return sorted(n for n in available() if not n.endswith(DRAFT))


def own(name):
    """The user's own file for `name` (whether or not it exists)."""
    return paths.THEMES[0] / f"{name}.toml"


def shipped(name):
    return (paths.THEMES[-1] / f"{name}.toml").is_file()


# the values the editor sets: (section, key); None is the top of the file
EDITABLE = ([(None, k) for k in ("paper", "paper_light", "paper_shade", "glow", "ink", "alert", "strong")]
            + [("sheet", k) for k in SHEET_DEFAULTS] + [("windows", k) for k in WINDOW_KEYS])


def _toml_value(key, v):
    if isinstance(v, str):
        if not re.fullmatch(r"#[0-9A-Fa-f]{6}", v):
            raise SystemExit(f"draftpaper: {key} = {v!r} is not a colour (#RRGGBB)")
        return f'"{v.upper()}"'
    if key == "angle":
        return str(round(float(v)))
    return repr(round(float(v), 2))


def write(name, values, to):
    """Write theme `name`'s file to `to` with `values` ({"paper": .., "sheet": {..}, "windows": {..}})
    in place of its own, line by line: comments, alignment and the other tables stay as they were;
    a value the file lacks goes at the end of its table."""
    lines = available()[name].read_text().splitlines()
    wanted = {}
    for section, key in EDITABLE:
        v = values.get(key) if section is None else (values.get(section) or {}).get(key)
        if v is not None:
            wanted[(section, key)] = _toml_value(key, v)
    section, ends = None, {None: 0}
    for i, line in enumerate(lines):
        head = re.match(r"\s*\[([^\]]+)\]", line)
        if head:
            section = head.group(1).strip()
            ends[section] = i + 1
            continue
        m = re.match(r"(\s*([A-Za-z_]+)\s*=\s*)(\"[^\"]*\"|[^\s#]+)(.*)$", line)
        if m:
            ends[section] = i + 1
            key = (section, m.group(2))
            if key in wanted:
                old, new = m.group(3), wanted.pop(key)
                rest = m.group(4)
                # keep a trailing comment where it was: the value's width changes, the padding gives
                pad = len(rest) - len(rest.lstrip())
                if rest.strip().startswith("#"):
                    rest = " " * max(1, pad + len(old) - len(new)) + rest.lstrip()
                lines[i] = m.group(1) + new + rest
    # what the file lacked, at the end of its table (a missing table, at the end of the file)
    for (section, key), v in wanted.items():
        if section in ends:
            at = ends[section]
            lines.insert(at, f"{key} = {v}")
            for s in ends:
                if ends[s] >= at and s != section:
                    ends[s] += 1
            ends[section] += 1
        else:
            lines += ["", f"[{section}]", f"{key} = {v}"]
            ends[section] = len(lines)
    to.parent.mkdir(parents=True, exist_ok=True)
    tmp = to.with_suffix(".tmp")
    tmp.write_text("\n".join(lines) + "\n")
    tmp.replace(to)


def load(name):
    found = available()
    if name not in found:
        raise SystemExit(f"draftpaper: no theme named {name!r} (draftpaper theme list)")
    with found[name].open("rb") as f:
        t = tomllib.load(f)
    t["name"] = name
    t.setdefault("label", name)
    t["sheet"] = {**SHEET_DEFAULTS, **t.get("sheet", {})}
    if "ink" not in t:
        shade = t["paper_shade"].lstrip("#")
        t["ink"] = "#" + "".join(f"{round(int(shade[i:i + 2], 16) * 0.7):02X}" for i in (0, 2, 4))
    t.setdefault("alert", "#E2231A")
    for k in WINDOW_KEYS:
        if k not in t.get("windows", {}):
            raise SystemExit(f"draftpaper: theme {name} lacks windows.{k}")
    return t


def mono_font():
    """The sheet's lettering face, as the shell picks it (qml/Fonts.qml)."""
    r = subprocess.run(["fc-list", ":", "family"], capture_output=True, text=True) if shutil.which("fc-list") else None
    families = {f.strip() for line in (r.stdout if r else "").splitlines() for f in line.split(",")}
    return "IBM Plex Mono" if "IBM Plex Mono" in families else "DejaVu Sans Mono"


def fuzzel_ini(t):
    """fuzzel's config for theme t. Colours are RRGGBBAA."""
    c = lambda hexv, alpha="ff": hexv.lstrip("#").lower() + alpha
    terminal = next((f"{term} -e" for term in ("alacritty", "foot") if shutil.which(term)), "x-terminal-emulator -e")
    return f"""# generated by `draftpaper apply` ({t['label']}): the launcher (Mod+d) drawn like the sheet
[main]
font={mono_font()}:size=11
prompt="› "
placeholder=launch
terminal={terminal}
layer=overlay
width=44
lines=12
horizontal-pad=24
vertical-pad=18
inner-pad=10
icons-enabled=yes

[colors]
background={c(t['paper'], 'f5')}
text={c(t['ink'])}
prompt={c(t['ink'])}
input={c(t['ink'])}
placeholder={c(t['ink'], '80')}
match={c(t['alert'])}
selection={c(t['ink'])}
selection-text={c(t['paper'])}
selection-match={c(t['paper_light'])}
counter={c(t['ink'], '99')}
border={c(t['ink'])}

[border]
width=2
radius=0
"""


LOCK = 'swaylock -f -C "${XDG_CACHE_HOME:-$HOME/.cache}/draftpaper/swaylock.conf"'


def swaylock_conf(t):
    """swaylock's config for theme t (colours RRGGBB[AA])."""
    c = lambda hexv, alpha="": hexv.lstrip("#").lower() + alpha
    paper, ink, alert, accent = t["paper"], t["ink"], t["alert"], t["windows"]["focused"]
    return f"""# generated by `draftpaper apply` ({t['label']}): the lock screen (Mod+Shift+x)
color={c(paper)}
font={mono_font()}
font-size=22
indicator-idle-visible
indicator-radius=96
indicator-thickness=6
show-failed-attempts
inside-color={c(paper)}
inside-clear-color={c(paper)}
inside-caps-lock-color={c(paper)}
inside-ver-color={c(paper)}
inside-wrong-color={c(paper)}
ring-color={c(ink)}
ring-clear-color={c(ink)}
ring-caps-lock-color={c(alert)}
ring-ver-color={c(accent)}
ring-wrong-color={c(alert)}
key-hl-color={c(accent)}
caps-lock-key-hl-color={c(alert)}
bs-hl-color={c(alert)}
caps-lock-bs-hl-color={c(alert)}
line-color=00000000
line-clear-color=00000000
line-caps-lock-color=00000000
line-ver-color=00000000
line-wrong-color=00000000
separator-color=00000000
text-color={c(ink)}
text-clear-color={c(ink)}
text-caps-lock-color={c(alert)}
text-ver-color={c(ink)}
text-wrong-color={c(alert)}
"""


def swayidle_conf(cfg):
    """swayidle's config from config.toml [idle]: the displays off, a lock, a lock before sleep."""
    idle = cfg["idle"]
    lines = ["# generated by `draftpaper apply` from ~/.config/draftpaper/config.toml [idle]"]
    if idle.get("lock_after", 0) > 0:
        lines.append(f"timeout {int(idle['lock_after'] * 60)} '{LOCK}'")
    if idle.get("screens_off", 0) > 0:
        lines.append(f"timeout {int(idle['screens_off'] * 60)} 'draftpaper-compositor msg power off' resume 'draftpaper-compositor msg power on'")
    # before suspend (the lid): always when lock_on_sleep, else only while `draftpaper lock-on-sleep` is on
    flag = '"$XDG_RUNTIME_DIR/draftpaper/lock-on-sleep"'
    lines.append(f"before-sleep '{LOCK}'" if idle.get("lock_on_sleep") else f"before-sleep 'if [ -e {flag} ]; then {LOCK}; fi'")
    return "\n".join(lines) + "\n"


def apply(name=None, apps_too=True):
    cfg = config.load()
    t = load(name or cfg["theme"])
    paths.ensure()
    shell = {
        "name": t["name"], "label": t["label"],
        "paper": t["paper"], "ink": t["ink"], "inkAlpha": t["sheet"]["pattern_strength"], "alert": t["alert"],
        "strong": t.get("strong", "#000000"),
        "accent": t["windows"]["focused"],
        "title": cfg["title"], "cells": cfg["cell"], "alerts": cfg["alerts"], "commands": cfg["commands"],
        "glyphs": bool(cfg["glyphs"]),
        # motion in the shell (the theme picker) follows the compositor's [animations]
        "animations": bool((cfg.get("animations") or {}).get("enabled", True)),
        "themes": listed(),
    }
    tmp = paths.THEME_JSON.with_suffix(".tmp")
    tmp.write_text(json.dumps(shell, indent=1))
    tmp.replace(paths.THEME_JSON)
    (paths.CACHE / "fuzzel.ini").write_text(fuzzel_ini(t))
    if apps_too:
        from . import apps
        apps.apply(t, cfg)
    (paths.CACHE / "swaylock.conf").write_text(swaylock_conf(t))
    idle = paths.CACHE / "swayidle.conf"
    text = swayidle_conf(cfg)
    if not idle.exists() or idle.read_text() != text:
        idle.write_text(text)
        if shutil.which("systemctl"):          # the idle service reads its file at start
            subprocess.run(["systemctl", "--user", "try-restart", "draftpaper-idle.service"], capture_output=True)
    return t
