"""draftpaper — the i3 desktop as a drafting sheet.

  draftpaper start            apply the theme and start the shell (once; i3 runs this on every start)
  draftpaper stop             stop the shell
  draftpaper apply            re-render the sheet and the generated i3 and shell files
  draftpaper theme [list]     list the themes, the current one marked
  draftpaper theme <name>     switch theme
  draftpaper theme next       the next theme (the shell's theme cell)
  draftpaper park ...         park windows in the side margins (draftpaper park, without arguments, explains)
  draftpaper readings         the readings the shell polls, as JSON
  draftpaper lower W H [PID]  used by the shell: put its margin windows under every i3 window
  draftpaper version

Settings: ~/.config/draftpaper/config.toml (see /usr/share/doc/draftpaper/README.md).
"""
import os, shutil, subprocess, sys
from . import __version__, config, paths, theme


def start():
    if not shutil.which("qs"):
        sys.exit("draftpaper: quickshell (qs) is not installed")
    first = not paths.I3_CONF.exists()
    theme.apply()
    if first:
        subprocess.run(["i3-msg", "reload"], capture_output=True)
    env = dict(os.environ, DRAFTPAPER_BIN=os.path.realpath(sys.argv[0]), DRAFTPAPER_CACHE=str(paths.CACHE),
               DRAFTPAPER_RUNTIME=str(paths.RUNTIME))
    subprocess.run(["qs", "-p", str(paths.QML), "-n", "-d"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def stop():
    subprocess.run(["qs", "kill", "-p", str(paths.QML)], capture_output=True)


def themes(args):
    current = config.load()["theme"]
    names = sorted(theme.available())
    if not args or args[0] == "list":
        for n in names:
            print(("* " if n == current else "  ") + n)
        return
    name = args[0]
    if name == "next":
        name = names[(names.index(current) + 1) % len(names)] if current in names else names[0]
    t = theme.apply(name)
    config.set_theme(name)
    print(f"{t['label']} applied.")


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
    elif c == "park":
        from . import park
        park.main(rest)
    elif c == "readings":
        from . import readings
        readings.main(rest)
    elif c == "lower":
        from . import lower
        lower.main(rest)
    elif c == "version":
        print(__version__)
    else:
        sys.exit(__doc__)
