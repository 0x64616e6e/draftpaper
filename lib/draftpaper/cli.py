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
  draftpaper theme edit [<name>]
                              open the theme editor on a theme (the current one); it saves your own
                              copy in ~/.config/draftpaper/themes, which a shipped theme yields to
  draftpaper theme revert <name>
                              a shipped theme as shipped again: your copy of it removed
  draftpaper picker           open or close the theme picker (Mod+t, the theme cell)
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
        print(json.dumps([dict(theme.load(n), current=(n == current), own=theme.own(n).is_file(), shipped=theme.shipped(n))
                          for n in names]))
        return
    if not args or args[0] == "list":
        for n in names:
            print(("* " if n == current else "  ") + n)
        return
    # the theme editor (qml/ThemeEditor.qml)
    if args[0] == "edit":
        sys.exit(subprocess.run(["qs", "ipc", "-p", str(paths.QML), "call", "editor", "open", args[1] if len(args) > 1 else ""]).returncode)
    if args[0] in ("draft", "save") and len(args) == 3:
        name, values = args[1], json.loads(args[2])
        if args[0] == "draft":          # the sheet previews the values, from a draft beside the user's themes
            theme.write(name, values, theme.own(name + theme.DRAFT))
            switch(name + theme.DRAFT, preview=True)
        else:                           # the user's own file, then the theme as a whole
            theme.write(name, values, theme.own(name))
            switch(name, preview=False)
            theme.own(name + theme.DRAFT).unlink(missing_ok=True)
        return
    if args[0] == "discard" and len(args) == 3:     # back to a theme, the draft gone
        switch(args[2], preview=True)
        theme.own(args[1] + theme.DRAFT).unlink(missing_ok=True)
        return
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
    elif c == "picker":
        # the shell's IpcHandler "themes" (qml/shell.qml)
        sys.exit(subprocess.run(["qs", "ipc", "-p", str(paths.QML), "call", "themes", "toggle"]).returncode)
    elif c == "lock-on-sleep":
        lock_on_sleep(rest[0] if rest else "toggle")
    elif c == "readings":
        from . import readings
        readings.main(rest)
    elif c == "version":
        print(__version__)
    else:
        sys.exit(__doc__)
