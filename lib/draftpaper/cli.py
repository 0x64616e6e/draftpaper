"""draftpaper — the desktop as a drafting sheet.

  draftpaper start            apply the theme and run the shell (draftpaper-shell.service, in the session)
  draftpaper stop             stop the shell
  draftpaper apply            write the theme's files again (the shell, launcher, lock screen, idle,
                              applications)
  draftpaper theme [list]     list the themes, the current one marked
  draftpaper theme <name> [--preview]
                              switch theme: the sheet and the applications; a --preview (the picker
                              while browsing) switches the sheet only; config.toml's [hooks] theme
                              runs after a full switch
  draftpaper theme next       the next theme (the shell's theme cell, right click)
  draftpaper theme json       every theme's values, for the theme picker
  draftpaper theme check [<name> ...]
                              the themes against the contrast rules (words 4.5:1, dimmed text 3:1)
  draftpaper scheme options|json|preview|keep [choice=value ...]
                              a colour scheme built from choices (the settings tool): paper, tone,
                              accent, windows, shade, terminal, terminal_shade, text, contrast; keep
                              writes [scheme] and the
                              "custom" theme
  draftpaper theme revert <name>
                              a shipped theme as shipped again: your copy of it removed
  draftpaper settings         open or close the settings (Mod+t, the theme cell): the colour scheme
  draftpaper audio save|restore
                              the default output's volume and mute, kept from one session to the next
                              (draftpaper-audio.service: the switch to the login screen mutes it)
  draftpaper wallpaper [FILE | none]
                              a picture in the frame, or none (the compositor picks it up at once)
  draftpaper margins paper|ambient
                              with a wallpaper: the margins the theme's paper or the picture's colours
  draftpaper readings         the readings the shell polls, as JSON
  draftpaper lock-on-sleep [on|off|toggle]
                              lock before every suspend (the lid) until turned off or the next login
  draftpaper version

Settings: ~/.config/draftpaper/config.toml (see /usr/share/doc/draftpaper/README.md).
"""
import os, shutil, subprocess, sys
from . import __version__, config, paths, theme


def start():
    """The shell, in the foreground: the session's service keeps it running."""
    qs = shutil.which("qs")
    if not qs:
        sys.exit("draftpaper: quickshell (qs) is not installed")
    if not os.environ.get("DRAFTPAPER_SOCKET"):
        sys.exit("draftpaper: DRAFTPAPER_SOCKET is not set (start the shell inside a Draftpaper session)")
    theme.apply()
    env = dict(os.environ, DRAFTPAPER_BIN=os.path.realpath(sys.argv[0]), DRAFTPAPER_CACHE=str(paths.CACHE),
               DRAFTPAPER_RUNTIME=str(paths.RUNTIME))
    os.execve(qs, ["qs", "-p", str(paths.QML), "-n"], env)


def audio(how):
    """Keep the default output's volume and mute across sessions. Logging out hands the sound card to the
    login screen, and the handover mutes it; the session saves the state as it ends (while it still has
    the card) and puts it back as the next one starts."""
    import json, re
    state = paths.STATE / "audio.json"
    if how == "save":
        r = subprocess.run(["wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@"], capture_output=True, text=True)
        m = re.search(r"Volume:\s*([0-9.]+)(\s*\[MUTED\])?", r.stdout)
        if m:
            state.parent.mkdir(parents=True, exist_ok=True)
            state.write_text(json.dumps({"volume": float(m.group(1)), "muted": bool(m.group(2))}))
    elif how == "restore":
        try:
            s = json.loads(state.read_text())
        except (OSError, ValueError):
            return
        subprocess.run(["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", str(s["volume"])], capture_output=True)
        subprocess.run(["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "1" if s["muted"] else "0"], capture_output=True)
    else:
        sys.exit("draftpaper audio save|restore")


def lock_on_sleep(how):
    """A flag in the runtime directory that the idle service's before-sleep hook looks for."""
    paths.ensure()
    flag = paths.RUNTIME / "lock-on-sleep"
    on = {"on": True, "off": False}.get(how, not flag.exists())
    if on:
        flag.touch()
    else:
        flag.unlink(missing_ok=True)
    title = "Lock on lid close: ON" if on else "Lock on lid close: OFF"
    body = "The screen locks before every suspend" if on else "Suspend and resume without a lock screen"
    if shutil.which("notify-send"):
        subprocess.run(["notify-send", "-a", "draftpaper", title, body], capture_output=True)
    print(title)


def stop():
    subprocess.run(["qs", "kill", "-p", str(paths.QML)], capture_output=True)


def themes(args):
    import json
    current = config.load()["theme"]
    names = theme.listed()
    if args and args[0] == "json":
        from . import scheme
        print(json.dumps([dict(t, current=(n == current), own=theme.own(n).is_file(), shipped=theme.shipped(n),
                               figures=scheme.figures(t)) for n in names for t in [theme.load(n)]]))
        return
    if not args or args[0] == "list":
        for n in names:
            print(("* " if n == current else "  ") + n)
        return
    if args[0] == "check":          # the contrast rules (scheme.py), for the named themes or all of them
        from . import scheme
        failed = False
        for n in args[1:] or names:
            for what, r, m, hard in scheme.check(theme.load(n)):
                print(f"{n}: {what} {r:.2f}:1, {'needs' if hard else 'advised'} {m:g}:1")
                failed |= hard
        sys.exit(1 if failed else 0)
    if args[0] == "revert" and len(args) == 2:      # the shipped theme again, the user's copy gone
        name = args[1]
        if not theme.shipped(name):
            sys.exit(f"draftpaper: {name} is not a shipped theme")
        theme.own(name).unlink(missing_ok=True)
        switch(name, preview=False)
        return
    preview = "--preview" in args
    args = [a for a in args if a != "--preview"]
    name = args[0]
    if name == "next":
        name = names[(names.index(current) + 1) % len(names)] if current in names else names[0]
    switch(name, preview)


def schemes(args):
    """draftpaper scheme options | json | preview | keep [CHOICE=VALUE ...]: a colour scheme built from
    choices (scheme.py), for the settings tool. The choices given override config.toml's [scheme]."""
    import json
    from . import scheme
    have = dict(config.load().get("scheme") or {})
    sub, pairs = (args[0] if args else "json"), args[1:]
    choices = {**scheme.DEFAULT, **have, **dict(p.split("=", 1) for p in pairs if "=" in p)}
    if sub == "options":
        # each choice's values, with a colour that stands for each (the scheme with that value swapped in)
        out = {}
        for k, values in scheme.CHOICES.items():
            show = {"paper": "paper", "tone": "paper"}.get(k)
            opts = []
            for v in values:
                t = scheme.build({**choices, k: v})
                colour = (t[show] if show else t["windows"]["focused"] if k == "accent"
                          else t["apps"]["bg"] if k in ("windows", "shade") else t["apps"]["term_fg"])  # the terminal's: Aa on it
                f = scheme.figures(t)
                ratio = f["ink"] if k in ("paper", "tone") else f["terminal"] if k in ("text", "terminal", "terminal_shade", "contrast") else \
                    round(scheme.contrast(t["windows"]["focused"], t["apps"]["bg"]), 1) if k == "accent" else f["text"]
                # a shade is called by its light-side name when the windows are light
                light = t["choices"]["windows"] == "light"
                label = scheme.LIGHT_NAMES.get(v, v) if light and k in ("shade", "terminal_shade") else v
                opts.append({"value": v, "label": label, "colour": colour, "ratio": ratio,
                             "on": t["apps"]["term_bg"] if k in ("text", "terminal", "terminal_shade") else None})
            out[k] = opts
        print(json.dumps({"choices": scheme.build(choices)["choices"], "options": out}))
        return
    t = scheme.build(choices)
    if sub == "json":
        print(json.dumps(dict(t, name="custom", figures=scheme.figures(t))))
        return
    draft = theme.own("custom" + theme.DRAFT)
    if sub == "preview":                # the sheet only, from a draft (theme.py)
        draft.parent.mkdir(parents=True, exist_ok=True)
        draft.write_text(scheme.toml(t))
        switch("custom" + theme.DRAFT, preview=True)
    elif sub == "keep":                 # the choices kept, the theme written, switched to whole
        config.set_table("scheme", t["choices"])
        theme.own("custom").parent.mkdir(parents=True, exist_ok=True)
        theme.own("custom").write_text(scheme.toml(t))
        switch("custom", preview=False)
        draft.unlink(missing_ok=True)
    else:
        sys.exit("draftpaper scheme options | json | preview | keep [CHOICE=VALUE ...]")


def switch(name, preview):
    """Switch to theme `name`: the sheet; and unless a preview, the applications and [hooks] theme."""
    t = theme.apply(name, apps_too=not preview)     # a preview (the picker browsing) leaves the applications
    config.set_theme(name)
    compositor("reload")          # the sheet at once, not at the compositor's next look at the file
    hook = (config.load().get("hooks") or {}).get("theme")
    if hook and not preview:
        # the applications too (detached: it may take a while, the sheet has switched already)
        subprocess.Popen([part.replace("{name}", name) for part in hook], stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL, start_new_session=True)
    print(f"{t['label']} applied.")


def compositor(action):
    """Run an action in the compositor over its IPC, when there is one (in a Draftpaper session)."""
    import json, socket
    path = os.environ.get("DRAFTPAPER_SOCKET")
    if not path:
        return
    try:
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(2)
            s.connect(path)
            s.sendall((json.dumps({"request": "action", "action": action}) + "\n").encode())
            s.recv(4096)
    except OSError:
        pass


def main(argv=None):
    a = (argv if argv is not None else sys.argv)[1:]
    if not a or a[0] in ("-h", "--help", "help"):
        print(__doc__)
        return
    c, rest = a[0], a[1:]
    if c == "start":
        start()
    elif c == "stop":
        stop()
    elif c == "apply":
        t = theme.apply()
        print(f"{t['label']} applied.")
    elif c == "theme":
        themes(rest)
    elif c == "wallpaper":
        if not rest:
            print(config.load().get("wallpaper") or "none")
        elif rest[0] == "none":
            config.set_value("wallpaper", None)
        else:
            path = os.path.abspath(os.path.expanduser(rest[0]))
            if not os.path.isfile(path):
                sys.exit(f"draftpaper: no such file {path}")
            config.set_value("wallpaper", path)
    elif c == "margins":
        if not rest or rest[0] not in ("paper", "ambient"):
            sys.exit("draftpaper margins paper|ambient")
        config.set_value("margins", rest[0])
    elif c == "audio":
        audio(rest[0] if rest else "")
    elif c == "scheme":
        schemes(rest)
    elif c in ("settings", "picker"):
        # the shell's IpcHandler "settings" (qml/shell.qml); `picker` is the name it had
        sys.exit(subprocess.run(["qs", "ipc", "-p", str(paths.QML), "call", "settings", "toggle"]).returncode)
    elif c == "lock-on-sleep":
        lock_on_sleep(rest[0] if rest else "toggle")
    elif c == "readings":
        from . import readings
        readings.main(rest)
    elif c == "version":
        print(__version__)
    else:
        sys.exit(__doc__)
