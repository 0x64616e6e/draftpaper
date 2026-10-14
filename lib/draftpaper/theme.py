"""Themes and applying them.

A theme (themes/<name>.toml) names the paper, the ink and the window colours. The compositor reads the
theme named in config.toml itself (the sheet and the window borders, again whenever the file changes);
`apply` writes what the shell reads:

  ~/.cache/draftpaper/theme.json    the ink, alert and accent colours and the shell's settings (title
                                    block, cells, alerts, commands); shell.qml reads it live

The sheet's geometry (margin, px/mm, zones per output) is the compositor's, over its IPC.
"""
import json, tomllib
from . import config, paths, render

SHEET_DEFAULTS = {"angle": 54.0, "midpoint": 0.45, "blend": 1.0, "shading": 1.0,
                  "glow_strength": 1.0, "glow_size": 1.0, "pattern_strength": 0.5}
WINDOW_KEYS = ("focused", "focused_inactive", "unfocused", "urgent", "text", "text_inactive", "background")


def available():
    names = {}
    for d in reversed(paths.THEMES):           # the user's own themes shadow the shipped ones
        for p in sorted(d.glob("*.toml")) if d.is_dir() else []:
            names[p.stem] = p
    return names


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
        t["ink"] = "#" + "".join(f"{round(v * 0.7):02X}" for v in render.rgb(t["paper_shade"]))
    t.setdefault("alert", "#E2231A")
    for k in WINDOW_KEYS:
        if k not in t.get("windows", {}):
            raise SystemExit(f"draftpaper: theme {name} lacks windows.{k}")
    return t


def apply(name=None):
    cfg = config.load()
    t = load(name or cfg["theme"])
    paths.ensure()
    shell = {
        "name": t["name"], "label": t["label"],
        "paper": t["paper"], "ink": t["ink"], "inkAlpha": t["sheet"]["pattern_strength"], "alert": t["alert"],
        "strong": t.get("strong", "#000000"),
        "accent": t["windows"]["focused"],
        "title": cfg["title"], "cells": cfg["cell"], "alerts": cfg["alerts"], "commands": cfg["commands"],
        "themes": sorted(available()),
    }
    tmp = paths.THEME_JSON.with_suffix(".tmp")
    tmp.write_text(json.dumps(shell, indent=1))
    tmp.replace(paths.THEME_JSON)
    return t
