//! The applications in the theme's colours: `draftpaper apply` renders apps/<template> with the theme's
//! [apps] palette into files Draftpaper owns, and adds a line to the application's own config, once,
//! where the application needs telling to read them. The user's configs stay theirs.
//!
//!   alacritty  ~/.config/alacritty/draftpaper.toml     imported by alacritty.toml (last, so it wins)
//!   foot       ~/.config/foot/draftpaper.ini           include= in foot.ini
//!   tmux       ~/.config/tmux/draftpaper.conf          source-file in tmux.conf; running servers reload it
//!   Neovim     colors/draftpaper.lua, plugin/draftpaper.lua (sets it at start); running instances switch
//!   Emacs      ~/.config/draftpaper/emacs/draftpaper-theme.el, loaded by a line added to init.el; a
//!              running Emacs (emacsclient) loads it
//!   GTK 3      ~/.local/share/themes/Draftpaper-<theme>/ (on adw-gtk3 when installed, else Adwaita), set
//!              with gsettings with the colour scheme; running applications follow
//!   GTK 4      a marked block at the end of ~/.config/gtk-4.0/gtk.css (libadwaita's named colours)
//!   Qt         ~/.config/qt{5,6}ct/colors/draftpaper.conf, chosen in qt5ct.conf / qt6ct.conf
//!   Firefox    a marked block at the end of chrome/userChrome.css, and one in user.js (the prefs that
//!              let it read userChrome.css and follow the colour scheme; [apps] firefox_text)
//!   X11        ~/.config/draftpaper/Xresources, merged into Xwayland (xrdb) when there is one
//!
//! config.toml [apps]: `theme = false` leaves the applications alone; `skip = ["emacs", "firefox"]`
//! leaves some; `firefox_text` (percent) and `firefox_ui_pt` size Firefox's text and interface.

use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{Map, Value};

use crate::util::{self, Paths};

fn render(p: &Paths, name: &str, pal: &Map<String, Value>) -> String {
    let text = util::read(&p.root.join("apps").join(name));
    let re = Regex::new(r"\{\{([a-z_0-9]+)(?:\.(raw|rgb|argb))?\}\}").unwrap();
    re.replace_all(&text, |c: &regex::Captures| {
        let key = &c[1];
        let v = match pal.get(key) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => if *b { "True".into() } else { "False".into() },
            Some(v) => v.to_string(),
            None => util::die(format!("draftpaper: apps/{name} uses {{{{{key}}}}}, which the theme's [apps] lacks")),
        };
        let h = v.trim_start_matches('#');
        match c.get(2).map(|m| m.as_str()) {
            None => v.clone(),
            Some("raw") => h.to_lowercase(),
            Some("rgb") => (0..3).map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap_or(0).to_string()).collect::<Vec<_>>().join(", "),
            _ => format!("#ff{}", h.to_lowercase()), // argb
        }
    })
    .into_owned()
}

/// Add `line` to the user's config at `path` (made when missing) unless `marker` is in it already.
pub(crate) fn ensure_line(path: &Path, line: &str, marker: &str, first: bool) {
    let text = util::read(path);
    if text.contains(marker) {
        return;
    }
    let text = if first {
        format!("{line}\n{text}")
    } else {
        format!("{}{}{line}\n", text.trim_end_matches('\n'), if text.trim().is_empty() { "" } else { "\n" })
    };
    util::write(path, &text);
}

const BLOCK_BEGIN: &str = "/* draftpaper begin: written by `draftpaper apply` */";
const BLOCK_END: &str = "/* draftpaper end */";

/// Put `text` in a marked block at the end of the user's file (replacing the block it had): later
/// rules win, so the theme holds even when another tool writes the rest of the file.
fn ensure_block(path: &Path, text: &str) {
    let old = util::read(path);
    let re = Regex::new(&format!(r"(?s){}.*?{}\n?", regex::escape(BLOCK_BEGIN), regex::escape(BLOCK_END))).unwrap();
    let rest = re.replace_all(&old, "").trim_end_matches('\n').to_string();
    let new = format!("{}{BLOCK_BEGIN}\n{}\n{BLOCK_END}\n", if rest.is_empty() { String::new() } else { format!("{rest}\n\n") }, text.trim_end_matches('\n'));
    if new != old {
        util::write(path, &new);
    }
}

/// Set keys in one [section] of an INI file, leaving the rest as it is.
fn set_ini(path: &Path, section: &str, values: &[(&str, String)]) {
    let text = util::read(path);
    let mut out: Vec<String> = Vec::new();
    let (mut inside, mut found) = (false, false);
    let mut seen: Vec<&str> = Vec::new();
    let missing = |seen: &Vec<&str>| values.iter().filter(|(k, _)| !seen.contains(k)).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>();
    for line in text.lines() {
        let t = line.trim_end();
        if t.starts_with('[') && t.ends_with(']') && t.len() > 2 {
            if inside {
                out.extend(missing(&seen));
            }
            inside = &t[1..t.len() - 1] == section;
            found |= inside;
        } else if inside && line.contains('=') {
            let k = line.split('=').next().unwrap_or("").trim();
            if let Some((key, v)) = values.iter().find(|(key, _)| *key == k) {
                out.push(format!("{key}={v}"));
                seen.push(key);
                continue;
            }
        }
        out.push(line.to_string());
    }
    if inside {
        out.extend(missing(&seen));
    }
    if !found {
        out.push(String::new());
        out.push(format!("[{section}]"));
        out.extend(values.iter().map(|(k, v)| format!("{k}={v}")));
    }
    util::write(path, &(out.join("\n").trim_start_matches('\n').to_string() + "\n"));
}

/// The GTK 3 theme Draftpaper's recolours: adw-gtk3 (it styles every widget through libadwaita's named
/// colours) when installed, else GTK's own Adwaita.
fn gtk_base(p: &Paths, dark: bool) -> (String, String) {
    let name = if dark { "adw-gtk3-dark" } else { "adw-gtk3" };
    for d in [p.home.join(".local/share/themes"), PathBuf::from("/usr/share/themes")] {
        let css = d.join(name).join("gtk-3.0/gtk.css");
        if css.exists() {
            return (css.display().to_string(), name.to_string());
        }
    }
    let res = if dark { "resource:///org/gtk/libgtk/theme/Adwaita/gtk-contained-dark.css" } else { "resource:///org/gtk/libgtk/theme/Adwaita/gtk-contained.css" };
    (res.to_string(), "Adwaita".to_string())
}

fn dirs(d: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(d).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Theme the applications with theme `t` (its [apps] palette), as config.toml's [apps] allows.
pub fn apply(p: &Paths, t: &Map<String, Value>, cfg: &Map<String, Value>) {
    let settings = cfg.get("apps").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut pal = t.get("apps").and_then(Value::as_object).cloned().unwrap_or_default();
    if settings.get("theme") == Some(&Value::Bool(false)) || pal.is_empty() {
        return;
    }
    let skip: Vec<String> = settings.get("skip").and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
    let on = |app: &str| !skip.iter().any(|s| s == app);
    let dark = pal.get("variant").and_then(Value::as_str) == Some("dark");
    let name = t["name"].as_str().unwrap_or("").to_string();
    pal.insert("label".into(), t["label"].clone());
    pal.insert("name".into(), Value::String(name.clone()));
    let gtk_theme = format!("Draftpaper-{name}");
    pal.insert("gtk_theme".into(), Value::String(gtk_theme.clone()));
    let (base, base_name) = gtk_base(p, dark);
    pal.insert("gtk_base".into(), Value::String(base));
    pal.insert("gtk_base_name".into(), Value::String(base_name));
    let ui_pt = settings.get("firefox_ui_pt").and_then(Value::as_f64).filter(|v| *v != 0.0);
    pal.insert(
        "firefox_ui_css".into(),
        Value::String(match ui_pt {
            None => String::new(),
            Some(pt) => format!(
                ":root, .tab-label, #urlbar-input, .urlbar-input, #searchbar, menupopup, menu, menuitem,\ntoolbarbutton, .toolbarbutton-text, #PersonalToolbar {{ font-size: {}pt !important; }}",
                if pt.fract() == 0.0 { format!("{}", pt as i64) } else { format!("{pt}") }
            ),
        }),
    );
    let conf = &p.config_dir;
    let home = &p.home;

    if on("alacritty") {
        util::write(&conf.join("alacritty/draftpaper.toml"), &render(p, "alacritty.toml", &pal));
        let main = conf.join("alacritty/alacritty.toml");
        let text = util::read(&main);
        if !text.contains("draftpaper.toml") {
            // into [general] import = [...], last (so it wins over earlier imports)
            let import = Regex::new(r"(?ms)^import\s*=\s*\[(.*?)\]").unwrap();
            let general = Regex::new(r"(?m)^\[general\]").unwrap();
            let entry = "\"~/.config/alacritty/draftpaper.toml\"";
            let text = if let Some(c) = import.captures(&text) {
                let m = c.get(1).unwrap();
                let inner = m.as_str().trim_end().trim_end_matches(',');
                format!("{}{}{entry}{}", &text[..m.start()], if inner.trim().is_empty() { String::new() } else { format!("{inner}, ") }, &text[m.end()..])
            } else if general.is_match(&text) {
                Regex::new(r"(?m)^\[general\]\n").unwrap().replacen(&text, 1, regex::NoExpand(&format!("[general]\nimport = [{entry}]\n"))).into_owned()
            } else {
                format!("[general]\nimport = [{entry}]\n\n{text}")
            };
            util::write(&main, &text);
        }
        if main.exists() {
            // alacritty reloads on a change of its config
            let _ = std::fs::File::options().append(true).open(&main).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        }
    }

    if on("foot") {
        util::write(&conf.join("foot/draftpaper.ini"), &render(p, "foot.ini", &pal));
        ensure_line(&conf.join("foot/foot.ini"), "include=~/.config/foot/draftpaper.ini", "draftpaper.ini", true);
    }

    if on("tmux") {
        let tconf = conf.join("tmux/draftpaper.conf");
        util::write(&tconf, &render(p, "tmux.conf", &pal));
        let main = if conf.join("tmux/tmux.conf").exists() || !home.join(".tmux.conf").exists() { conf.join("tmux/tmux.conf") } else { home.join(".tmux.conf") };
        ensure_line(&main, "source-file -q ~/.config/tmux/draftpaper.conf   # the theme's colours (draftpaper apply)", "draftpaper.conf", false);
        util::run(&["tmux", "source-file", &tconf.display().to_string()], 10);
    }

    if on("neovim") {
        util::write(&conf.join("nvim/colors/draftpaper.lua"), &render(p, "nvim.lua", &pal));
        util::write(
            &conf.join("nvim/plugin/draftpaper.lua"),
            "-- generated by `draftpaper apply`: the theme's colorscheme, set after init.lua\npcall(vim.cmd.colorscheme, \"draftpaper\")\n",
        );
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        for sock in dirs(Path::new(&runtime)) {
            let n = sock.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if n.starts_with("nvim.") && n.ends_with(".0") {
                util::run(&["nvim", "--server", &sock.display().to_string(), "--remote-send", "<Cmd>colorscheme draftpaper<CR>"], 10);
            }
        }
    }

    if on("emacs") {
        let theme_dir = conf.join("draftpaper/emacs");
        util::write(&theme_dir.join("draftpaper-theme.el"), &render(p, "emacs-theme.el", &pal));
        let td = theme_dir.display().to_string();
        if let Some(init) = [conf.join("emacs/init.el"), home.join(".emacs.d/init.el")].into_iter().find(|p| p.exists()) {
            ensure_line(
                &init,
                &format!(";; the theme's colours (draftpaper apply)\n(add-to-list 'custom-theme-load-path \"{td}\")\n(load-theme 'draftpaper t)"),
                &format!("custom-theme-load-path \"{td}"),
                false,
            );
        }
        util::run(&["emacsclient", "-q", "-e", &format!("(progn (add-to-list 'custom-theme-load-path \"{td}\") (load-theme 'draftpaper t))")], 10);
    }

    if on("gtk") {
        let themes = home.join(".local/share/themes");
        let theme = themes.join(&gtk_theme);
        util::write(&theme.join("gtk-3.0/gtk.css"), &render(p, "gtk3-theme.css", &pal));
        util::write(&theme.join("gtk-3.0/gtk-dark.css"), &render(p, "gtk3-theme-dark.css", &pal));
        util::write(&theme.join("index.theme"), &render(p, "gtk3-theme-index", &pal));
        // the themes of the other Draftpaper themes go (one at a time is enough)
        for old in dirs(&themes) {
            let n = old.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if n.starts_with("Draftpaper-") && n != gtk_theme {
                let _ = std::fs::remove_dir_all(&old);
            }
        }
        ensure_block(&conf.join("gtk-4.0/gtk.css"), &render(p, "gtk4.css", &pal));
        let iface = "org.gnome.desktop.interface";
        util::run(&["gsettings", "set", iface, "gtk-theme", &gtk_theme], 10);
        util::run(&["gsettings", "set", iface, "color-scheme", if dark { "prefer-dark" } else { "prefer-light" }], 10);
    }

    if on("qt") {
        for qt in ["qt6ct", "qt5ct"] {
            let colours = conf.join(qt).join("colors/draftpaper.conf");
            util::write(&colours, &render(p, "qtct-colors.conf", &pal));
            set_ini(&conf.join(qt).join(format!("{qt}.conf")), "Appearance", &[("custom_palette", "true".into()), ("color_scheme_path", colours.display().to_string())]);
        }
    }

    if on("firefox") {
        for profile in dirs(&home.join(".mozilla/firefox")) {
            if !profile.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(".default")) || !profile.is_dir() {
                continue;
            }
            ensure_block(&profile.join("chrome/userChrome.css"), &render(p, "firefox.css", &pal));
            let mut prefs = vec![
                r#"user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);"#.to_string(),
                r#"user_pref("browser.theme.toolbar-theme", 2);"#.to_string(),
                r#"user_pref("browser.theme.content-theme", 2);"#.to_string(),
                r#"user_pref("layout.css.prefers-color-scheme.content-override", 2);"#.to_string(),
            ];
            if let Some(text) = settings.get("firefox_text").and_then(Value::as_f64).filter(|v| *v != 0.0) {
                prefs.push(format!(r#"user_pref("ui.textScaleFactor", {});"#, text as i64));
            }
            let block = format!("// draftpaper begin: written by `draftpaper apply`\n{}\n// draftpaper end\n", prefs.join("\n"));
            let user_js = profile.join("user.js");
            let text = util::read(&user_js);
            let text = Regex::new(r"(?s)// draftpaper begin.*?// draftpaper end\n").unwrap().replace_all(&text, "").into_owned();
            let text = format!("{}{}{block}", text.trim_end_matches('\n'), if text.trim().is_empty() { "" } else { "\n" });
            let _ = std::fs::write(&user_js, text);
        }
    }

    if on("x11") {
        let res = conf.join("draftpaper/Xresources");
        util::write(&res, &render(p, "Xresources", &pal));
        if std::env::var("DISPLAY").is_ok_and(|d| !d.is_empty()) {
            util::run(&["xrdb", "-merge", &res.display().to_string()], 10);
        }
    }
    // the files above written again: [text]'s fonts at their ends again (text.rs)
    crate::text::apply(p, cfg);
}
