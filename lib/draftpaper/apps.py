"""The applications in the theme's colours: `draftpaper apply` renders apps/<template> with the theme's
[apps] palette into files Draftpaper owns, and adds a line to the application's own config, once, where
the application needs telling to read them. The user's configs stay theirs.

  alacritty  ~/.config/alacritty/draftpaper.toml     imported by alacritty.toml (last, so it wins)
  foot       ~/.config/foot/draftpaper.ini           include= in foot.ini
  tmux       ~/.config/tmux/draftpaper.conf          source-file in tmux.conf; running servers reload it
  Neovim     colors/draftpaper.lua, plugin/draftpaper.lua (sets it at start); running instances switch
  Emacs      ~/.config/draftpaper/emacs/draftpaper-theme.el, loaded by a line added to init.el; a
             running Emacs (emacsclient) loads it
  GTK 3      ~/.local/share/themes/Draftpaper-<theme>/ (on adw-gtk3 when installed, else Adwaita), set
             with gsettings with the colour scheme; running applications follow
  GTK 4      a marked block at the end of ~/.config/gtk-4.0/gtk.css (libadwaita's named colours)
  Qt         ~/.config/qt{5,6}ct/colors/draftpaper.conf, chosen in qt5ct.conf / qt6ct.conf
  Firefox    a marked block at the end of chrome/userChrome.css, and one in user.js (the prefs that let it
             read userChrome.css and follow the colour scheme; [apps] firefox_text)
  (GTK 4 and Firefox CSS have no import that comes after the file's own rules: the block at the end
  holds the theme even when another tool rewrites the rest of the file)
  X11        ~/.config/draftpaper/Xresources, merged into Xwayland (xrdb) when there is one

config.toml [apps]: `theme = false` leaves the applications alone; `skip = ["emacs", "firefox"]` leaves
some; `firefox_text` (percent) and `firefox_ui_pt` size Firefox's text and interface.
"""
import glob, os, re, shutil, subprocess
from pathlib import Path
from . import paths

HOME = Path.home()
CONFIG = Path(os.environ.get("XDG_CONFIG_HOME", HOME / ".config"))
TEMPLATES = paths.ROOT / "apps"
PLACEHOLDER = re.compile(r"\{\{([a-z_0-9]+)(?:\.(raw|rgb|argb))?\}\}")
APPS = ("alacritty", "foot", "tmux", "neovim", "emacs", "gtk", "qt", "firefox", "x11")


def run(*cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=10)
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None


def render(name, pal):
    def sub(m):
        key, flt = m.group(1), m.group(2)
        if key not in pal:
            raise SystemExit(f"draftpaper: apps/{name} uses {{{{{key}}}}}, which the theme's [apps] lacks")
        v = str(pal[key])
        if flt is None:
            return v
        h = v.lstrip("#")
        if flt == "raw":
            return h.lower()
        if flt == "rgb":
            return ", ".join(str(int(h[i:i + 2], 16)) for i in (0, 2, 4))
        return "#ff" + h.lower()                       # argb
    return PLACEHOLDER.sub(sub, (TEMPLATES / name).read_text())


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists() or path.read_text() != text:
        path.write_text(text)


def ensure_line(path, line, marker, first=False):
    """Add `line` to the user's config at `path` (created when missing) unless `marker` is in it already."""
    text = path.read_text() if path.exists() else ""
    if marker in text:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    if first:
        text = line + "\n" + text
    else:
        text = text.rstrip("\n") + ("\n" if text.strip() else "") + line + "\n"
    path.write_text(text)


def ensure_block(path, text, begin="/* draftpaper begin: written by `draftpaper apply` */", end="/* draftpaper end */"):
    """Put `text` in a marked block at the end of the user's file (replacing the block it had): later
    rules win, so the theme holds even when another tool writes the rest of the file."""
    old = path.read_text() if path.exists() else ""
    rest = re.sub(re.escape(begin) + r".*?" + re.escape(end) + r"\n?", "", old, flags=re.S).rstrip("\n")
    new = (rest + "\n\n" if rest else "") + begin + "\n" + text.rstrip("\n") + "\n" + end + "\n"
    if new != old:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(new)


def set_ini(path, section, values):
    """Set keys in one [section] of an INI file, leaving the rest as it is."""
    lines = path.read_text().splitlines() if path.exists() else []
    out, inside, seen, found = [], False, set(), False
    for line in lines:
        m = re.match(r"^\[(.+)\]\s*$", line)
        if m:
            if inside:
                out += [f"{k}={v}" for k, v in values.items() if k not in seen]
            inside = m.group(1) == section
            found |= inside
        elif inside and "=" in line and line.split("=", 1)[0].strip() in values:
            k = line.split("=", 1)[0].strip()
            out.append(f"{k}={values[k]}")
            seen.add(k)
            continue
        out.append(line)
    if inside:
        out += [f"{k}={v}" for k, v in values.items() if k not in seen]
    if not found:
        out += ["", f"[{section}]"] + [f"{k}={v}" for k, v in values.items()]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(out).lstrip("\n") + "\n")


def gtk_base(dark):
    """The GTK 3 theme Draftpaper's recolours: adw-gtk3 (it styles every widget through libadwaita's named
    colours) when installed, else GTK's own Adwaita."""
    name = "adw-gtk3-dark" if dark else "adw-gtk3"
    for d in (HOME / ".local/share/themes", Path("/usr/share/themes")):
        if (d / name / "gtk-3.0/gtk.css").exists():
            return str(d / name / "gtk-3.0/gtk.css"), name
    return ("resource:///org/gtk/libgtk/theme/Adwaita/gtk-contained-dark.css" if dark
            else "resource:///org/gtk/libgtk/theme/Adwaita/gtk-contained.css"), "Adwaita"


def apply(t, cfg):
    """Theme the applications with theme `t` (its [apps] palette), as config.toml's [apps] allows."""
    settings = cfg.get("apps") or {}
    pal = dict(t.get("apps") or {})
    if settings.get("theme") is False or not pal:
        return
    skip = set(settings.get("skip") or [])
    dark = pal.get("variant") == "dark"
    pal.update(label=t["label"], name=t["name"], gtk_theme=f"Draftpaper-{t['name']}")
    pal["gtk_base"], pal["gtk_base_name"] = gtk_base(dark)
    ui_pt = settings.get("firefox_ui_pt")
    pal["firefox_ui_css"] = "" if not ui_pt else (
        ":root, .tab-label, #urlbar-input, .urlbar-input, #searchbar, menupopup, menu, menuitem,\n"
        f"toolbarbutton, .toolbarbutton-text, #PersonalToolbar {{ font-size: {ui_pt:g}pt !important; }}")
    display = bool(os.environ.get("DISPLAY"))

    if "alacritty" not in skip:
        write(CONFIG / "alacritty/draftpaper.toml", render("alacritty.toml", pal))
        main = CONFIG / "alacritty/alacritty.toml"
        text = main.read_text() if main.exists() else ""
        if "draftpaper.toml" not in text:
            # into [general] import = [...], last (so it wins over earlier imports)
            m = re.search(r'^import\s*=\s*\[(.*?)\]', text, flags=re.M | re.S)
            if m:
                inner = m.group(1).rstrip().rstrip(",")
                text = text[:m.start(1)] + (inner + ", " if inner.strip() else "") + '"~/.config/alacritty/draftpaper.toml"' + text[m.end(1):]
            elif re.search(r"^\[general\]", text, flags=re.M):
                text = re.sub(r"^\[general\]\n", '[general]\nimport = ["~/.config/alacritty/draftpaper.toml"]\n', text, count=1, flags=re.M)
            else:
                text = '[general]\nimport = ["~/.config/alacritty/draftpaper.toml"]\n\n' + text
            main.parent.mkdir(parents=True, exist_ok=True)
            main.write_text(text)
        if main.exists():
            os.utime(main)                              # alacritty reloads on a change of its config

    if "foot" not in skip:
        write(CONFIG / "foot/draftpaper.ini", render("foot.ini", pal))
        ensure_line(CONFIG / "foot/foot.ini", "include=~/.config/foot/draftpaper.ini", "draftpaper.ini", first=True)

    if "tmux" not in skip:
        conf = CONFIG / "tmux/draftpaper.conf"
        write(conf, render("tmux.conf", pal))
        main = CONFIG / "tmux/tmux.conf" if (CONFIG / "tmux/tmux.conf").exists() or not (HOME / ".tmux.conf").exists() else HOME / ".tmux.conf"
        ensure_line(main, "source-file -q ~/.config/tmux/draftpaper.conf   # the theme's colours (draftpaper apply)", "draftpaper.conf")
        if shutil.which("tmux"):
            run("tmux", "source-file", str(conf))

    if "neovim" not in skip:
        write(CONFIG / "nvim/colors/draftpaper.lua", render("nvim.lua", pal))
        write(CONFIG / "nvim/plugin/draftpaper.lua",
              "-- generated by `draftpaper apply`: the theme's colorscheme, set after init.lua\n"
              "pcall(vim.cmd.colorscheme, \"draftpaper\")\n")
        runtime = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
        for sock in glob.glob(f"{runtime}/nvim.*.0"):
            run("nvim", "--server", sock, "--remote-send", "<Cmd>colorscheme draftpaper<CR>")

    if "emacs" not in skip:
        theme_dir = CONFIG / "draftpaper/emacs"
        write(theme_dir / "draftpaper-theme.el", render("emacs-theme.el", pal))
        init = next((p for p in (CONFIG / "emacs/init.el", HOME / ".emacs.d/init.el") if p.exists()), None)
        if init:
            ensure_line(init, f";; the theme's colours (draftpaper apply)\n(add-to-list 'custom-theme-load-path \"{theme_dir}\")\n"
                              "(load-theme 'draftpaper t)", "custom-theme-load-path \"" + str(theme_dir))
        if shutil.which("emacsclient"):
            run("emacsclient", "-q", "-e", f"(progn (add-to-list 'custom-theme-load-path \"{theme_dir}\") "
                "(load-theme 'draftpaper t))")

    if "gtk" not in skip:
        theme = HOME / ".local/share/themes" / pal["gtk_theme"]
        write(theme / "gtk-3.0/gtk.css", render("gtk3-theme.css", pal))
        write(theme / "gtk-3.0/gtk-dark.css", render("gtk3-theme-dark.css", pal))
        write(theme / "index.theme", render("gtk3-theme-index", pal))
        # the themes of the other Draftpaper themes go (one at a time is enough)
        for old in (HOME / ".local/share/themes").glob("Draftpaper-*"):
            if old.name != pal["gtk_theme"]:
                shutil.rmtree(old, ignore_errors=True)
        ensure_block(CONFIG / "gtk-4.0/gtk.css", render("gtk4.css", pal))
        if shutil.which("gsettings"):
            iface = "org.gnome.desktop.interface"
            run("gsettings", "set", iface, "gtk-theme", pal["gtk_theme"])
            run("gsettings", "set", iface, "color-scheme", "prefer-dark" if dark else "prefer-light")

    if "qt" not in skip:
        for qt in ("qt6ct", "qt5ct"):
            colours = CONFIG / qt / "colors/draftpaper.conf"
            write(colours, render("qtct-colors.conf", pal))
            set_ini(CONFIG / qt / f"{qt}.conf", "Appearance", {"custom_palette": "true", "color_scheme_path": str(colours)})

    if "firefox" not in skip:
        for profile in glob.glob(str(HOME / ".mozilla/firefox/*.default*")):
            ensure_block(Path(profile) / "chrome/userChrome.css", render("firefox.css", pal))
            prefs = ['user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);',
                     'user_pref("browser.theme.toolbar-theme", 2);',
                     'user_pref("browser.theme.content-theme", 2);',
                     'user_pref("layout.css.prefers-color-scheme.content-override", 2);']
            if settings.get("firefox_text"):
                prefs.append(f'user_pref("ui.textScaleFactor", {int(settings["firefox_text"])});')
            block = "// draftpaper begin: written by `draftpaper apply`\n" + "\n".join(prefs) + "\n// draftpaper end\n"
            user_js = Path(profile) / "user.js"
            text = user_js.read_text() if user_js.exists() else ""
            text = re.sub(r"// draftpaper begin.*?// draftpaper end\n", "", text, flags=re.S)
            user_js.write_text(text.rstrip("\n") + ("\n" if text.strip() else "") + block)

    if "x11" not in skip:
        res = CONFIG / "draftpaper/Xresources"
        write(res, render("Xresources", pal))
        if display and shutil.which("xrdb"):
            run("xrdb", "-merge", str(res))
