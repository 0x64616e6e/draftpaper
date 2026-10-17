//! The compositor's settings in ~/.config/draftpaper/config.toml, next to the theme and the margin:
//!
//!     [input]
//!     keyboard_layout = "us,ro"        # xkb layout(s), variant, options
//!     keyboard_variant = ""
//!     keyboard_options = "grp:alt_shift_toggle,ctrl:nocaps"
//!     repeat_delay = 200               # ms
//!     repeat_rate = 25                 # per second
//!     tap = true                       # touchpad: tap to click
//!     natural_scroll = true            # touchpad: content follows the fingers
//!     focus_follows_mouse = true       # the window, and the screen, the pointer enters take the focus
//!
//!     [animations]                     # parking, unparking, switching workspaces
//!     enabled = true                   # false: reduced motion, every change at once
//!     duration_ms = 220
//!
//!     [session]                        # at login, the applications open at the last logout, their
//!     restore = true                   # windows back in their places (session.rs)
//!     apps = true                      # their own state too (draftpaper apply: cli/src/restore.rs)
//!     skip = ["org.gnome.Calculator"]  # app ids left out
//!
//!     [[rule]]                         # window rules (rules.rs): app_id/title patterns, pip; float,
//!     title = "^Picture"               # sticky, place = "centre"|"corner", focus
//!     float = true
//!
//!     [bindings]                       # key = action; "none" removes a default
//!     "Mod+Return" = "spawn foot"      # Mod is Super on the TTY, Alt when nested
//!     "Mod+d" = "spawn fuzzel"
//!
//! Actions: focus|move|resize left|right|up|down, workspace N, send N, fullscreen, close, quit,
//! spawn COMMAND (run by sh -c). The file is read again when it changes (see main.rs); a file that does
//! not parse is reported and the previous settings stay.

use std::path::PathBuf;

use smithay::input::keyboard::{keysyms, xkb, Keysym, ModifiersState};

use crate::layout::Direction;

#[derive(Debug, Clone)]
pub enum Action {
    Focus(Direction),
    Move(Direction),
    Resize(Direction),
    Workspace(u32),
    SendTo(u32),
    Fullscreen,
    Close,
    Quit,
    /// write the session (session.rs) now, as it is
    SessionSave,
    Spawn(String),
    /// every output to ~/Pictures (TTY only)
    Screenshot,
    /// the magnifier on the pointer, or away (lens.rs)
    Zoom,
    /// the pointer to this place (logical pixels of the whole layout), for scripts and tests
    Pointer(f64, f64),
    /// park the focused window in the first free cell, or, with nothing showing, bring every parked
    /// window back (parking.rs)
    Park,
    /// park the focused window in cell n, if free (and, given a workspace, only if it is on that one)
    ParkAt(usize, Option<u32>),
    /// bring back the parked window with this IPC id
    Unpark(u64),
    /// cycle through the parked windows (+1 next, -1 previous); with none parked, move the focus
    ParkCycle(i32),
    /// the displays' power: off (DPMS, for swayidle) or on; any input turns them on again
    Power(bool),
    /// read config.toml again now (not at the next check): `draftpaper theme` after switching
    Reload,
    /// switch to virtual terminal n (TTY only; Ctrl+Alt+F1..F12, not configurable)
    Vt(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    /// Super on the TTY, Alt when nested
    pub main: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub mods: Mods,
    pub key: Keysym,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub struct Input {
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
    pub repeat_delay: i32,
    pub repeat_rate: i32,
    pub tap: bool,
    /// the window (and screen) the pointer enters takes the focus, as i3's focus_follows_mouse
    pub focus_follows_mouse: bool,
    pub natural_scroll: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub input: Input,
    pub bindings: Vec<Binding>,
    /// how long parking, unparking and switching workspaces take; None: no animations
    pub animations: Option<std::time::Duration>,
    /// window rules: the built-in ones, then config.toml's [[rule]]s (rules.rs)
    pub rules: Vec<crate::rules::Rule>,
    pub session: Session,
}

/// [session] (session.rs): the windows put back at login, but those of the applications in `skip`.
#[derive(Debug, Clone)]
pub struct Session {
    pub restore: bool,
    pub skip: Vec<String>,
}

pub fn path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/draftpaper/config.toml")
}

/// The bindings when the file names none: the same keys as the i3 edition and the prototype so far.
const DEFAULT_BINDINGS: &[(&str, &str)] = &[
    // as in the i3 edition: Mod+Left/Right cycle the parked windows, and move the focus when none are
    ("Mod+Left", "park_prev"), ("Mod+Right", "park_next"), ("Mod+Up", "focus up"), ("Mod+Down", "focus down"),
    ("Mod+h", "focus left"), ("Mod+l", "focus right"), ("Mod+k", "focus up"), ("Mod+j", "focus down"),
    ("Mod+Shift+Left", "move left"), ("Mod+Shift+Right", "move right"), ("Mod+Shift+Up", "move up"), ("Mod+Shift+Down", "move down"),
    ("Mod+Shift+h", "move left"), ("Mod+Shift+l", "move right"), ("Mod+Shift+k", "move up"), ("Mod+Shift+j", "move down"),
    ("Mod+Ctrl+Left", "resize left"), ("Mod+Ctrl+Right", "resize right"), ("Mod+Ctrl+Up", "resize up"), ("Mod+Ctrl+Down", "resize down"),
    ("Mod+Ctrl+h", "resize left"), ("Mod+Ctrl+l", "resize right"), ("Mod+Ctrl+k", "resize up"), ("Mod+Ctrl+j", "resize down"),
    ("Mod+1", "workspace 1"), ("Mod+2", "workspace 2"), ("Mod+3", "workspace 3"), ("Mod+4", "workspace 4"), ("Mod+5", "workspace 5"),
    ("Mod+6", "workspace 6"), ("Mod+7", "workspace 7"), ("Mod+8", "workspace 8"), ("Mod+9", "workspace 9"), ("Mod+0", "workspace 10"),
    ("Mod+Shift+1", "send 1"), ("Mod+Shift+2", "send 2"), ("Mod+Shift+3", "send 3"), ("Mod+Shift+4", "send 4"), ("Mod+Shift+5", "send 5"),
    ("Mod+Shift+6", "send 6"), ("Mod+Shift+7", "send 7"), ("Mod+Shift+8", "send 8"), ("Mod+Shift+9", "send 9"), ("Mod+Shift+0", "send 10"),
    ("Mod+f", "fullscreen"),
    ("Mod+Escape", "park"),
    // lock now (the lock screen, qml/lock.qml); lock before suspend for the day, or not
    ("Mod+Shift+x", "spawn draftpaper lock"),
    ("Mod+Shift+p", "spawn draftpaper lock-on-sleep toggle"),
    // the settings: the colour scheme (the shell's)
    ("Mod+t", "spawn draftpaper settings"),
    ("Mod+Return", "spawn alacritty"),
    // the launcher, themed like the sheet by `draftpaper apply`
    ("Mod+d", "spawn draftpaper launcher"),
    ("Mod+Shift+q", "close"),
    ("Mod+Shift+e", "quit"),
    // screenshots through screencopy: a region picked with slurp into swappy (annotate, save, copy), the
    // screen you are on into ~/Pictures; Mod+Print is the compositor's own, needing neither
    ("Print", r#"spawn g=$(slurp) && grim -g "$g" - | swappy -f -"#),
    ("Shift+Print", r#"spawn mkdir -p ~/Pictures && grim -o "$(draftpaper-compositor msg focused-output)" ~/Pictures/screenshot-$(date +%Y%m%d-%H%M%S).png"#),
    ("Mod+Print", "screenshot"),
    // the magnifier on the pointer, to check a line down to the pixel (lens.rs): scroll zooms, Shift and
    // scroll sizes it, Escape or Mod+z again puts it away
    ("Mod+z", "zoom"),
    // the laptop's media keys: PipeWire's wpctl (up to 150%, as the shell's volume cell), brightnessctl
    ("XF86AudioRaiseVolume", "spawn wpctl set-volume -l 1.5 @DEFAULT_AUDIO_SINK@ 5%+"),
    ("XF86AudioLowerVolume", "spawn wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"),
    ("XF86AudioMute", "spawn wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"),
    ("XF86AudioMicMute", "spawn wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"),
    ("XF86MonBrightnessUp", "spawn brightnessctl -q set 5%+"),
    ("XF86MonBrightnessDown", "spawn brightnessctl -q set 5%-"),
];

pub(crate) fn parse_action(s: &str) -> Result<Option<Action>, String> {
    let s = s.trim();
    if s == "none" {
        return Ok(None);
    }
    let (verb, arg) = s.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((s, ""));
    let dir = || match arg {
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        _ => Err(format!("\"{s}\": the direction is left, right, up or down")),
    };
    let number = || arg.parse::<u32>().map_err(|_| format!("\"{s}\": the workspace is a number"));
    Ok(Some(match verb {
        "focus" => Action::Focus(dir()?),
        "move" => Action::Move(dir()?),
        "resize" => Action::Resize(dir()?),
        "workspace" => Action::Workspace(number()?),
        "send" => Action::SendTo(number()?),
        "fullscreen" => Action::Fullscreen,
        "close" => Action::Close,
        "quit" => Action::Quit,
        "session_save" => Action::SessionSave,
        "screenshot" => Action::Screenshot,
        "zoom" => Action::Zoom,
        "pointer" => {
            let mut xy = arg.split_whitespace().map(|v| v.parse::<f64>());
            match (xy.next(), xy.next()) {
                (Some(Ok(x)), Some(Ok(y))) => Action::Pointer(x, y),
                _ => return Err(format!("\"{s}\": pointer X Y")),
            }
        }
        "reload" => Action::Reload,
        "park" => Action::Park,
        "park_at" => {
            let mut parts = arg.split_whitespace();
            let cell = parts.next().and_then(|c| c.parse().ok()).ok_or_else(|| format!("\"{s}\": park_at CELL [WORKSPACE]"))?;
            let ws = match parts.next() {
                Some(w) => Some(w.parse().map_err(|_| format!("\"{s}\": the workspace is a number"))?),
                None => None,
            };
            Action::ParkAt(cell, ws)
        }
        "unpark" => Action::Unpark(arg.parse().map_err(|_| format!("\"{s}\": unpark WINDOW-ID"))?),
        "power" => match arg {
            "off" => Action::Power(false),
            "on" => Action::Power(true),
            _ => return Err(format!("\"{s}\": power on or power off")),
        },
        "park_next" => Action::ParkCycle(1),
        "park_prev" => Action::ParkCycle(-1),
        "spawn" if !arg.is_empty() => Action::Spawn(arg.to_string()),
        _ => return Err(format!("\"{s}\": not an action")),
    }))
}

/// "Mod+Shift+Return" -> modifiers and the key (letters in lower case, as the unshifted key reads)
fn parse_keys(s: &str) -> Result<(Mods, Keysym), String> {
    let mut mods = Mods::default();
    let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let key = parts.pop().filter(|k| !k.is_empty()).ok_or_else(|| format!("\"{s}\": no key"))?;
    for m in parts {
        match m.to_ascii_lowercase().as_str() {
            "mod" => mods.main = true,
            "shift" => mods.shift = true,
            "ctrl" | "control" => mods.ctrl = true,
            "alt" => mods.alt = true,
            other => return Err(format!("\"{s}\": unknown modifier {other}")),
        }
    }
    let name = if key.chars().count() == 1 { key.to_lowercase() } else { key.to_string() };
    let sym = xkb::keysym_from_name(&name, xkb::KEYSYM_CASE_INSENSITIVE);
    if sym.raw() == keysyms::KEY_NoSymbol {
        return Err(format!("\"{s}\": unknown key {key}"));
    }
    Ok((mods, sym))
}

impl Config {
    pub fn default_config() -> Config {
        Config::from_table(&toml::Table::new()).expect("the defaults parse")
    }

    /// Read the file; Err with a message when it does not parse (the caller keeps what it had).
    pub fn load() -> Result<Config, String> {
        let text = std::fs::read_to_string(path()).unwrap_or_default();
        let table = text.parse::<toml::Table>().map_err(|e| format!("{}: {e}", path().display()))?;
        Config::from_table(&table)
    }

    fn from_table(table: &toml::Table) -> Result<Config, String> {
        let input = table.get("input").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let s = |k: &str| input.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let i = |k: &str, d: i64| input.get(k).and_then(|v| v.as_integer()).unwrap_or(d) as i32;
        let b = |k: &str, d: bool| input.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
        let input = Input {
            layout: s("keyboard_layout").unwrap_or_default(),
            variant: s("keyboard_variant").unwrap_or_default(),
            options: s("keyboard_options"),
            repeat_delay: i("repeat_delay", 200),
            repeat_rate: i("repeat_rate", 25),
            tap: b("tap", true),
            natural_scroll: b("natural_scroll", false),
            focus_follows_mouse: b("focus_follows_mouse", true),
        };

        let mut bindings: Vec<Binding> = Vec::new();
        let user = table.get("bindings").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let entries = DEFAULT_BINDINGS
            .iter()
            .map(|(k, a)| (k.to_string(), a.to_string()))
            .chain(user.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())));
        for (keys, action) in entries {
            let (mods, key) = parse_keys(&keys)?;
            bindings.retain(|b| !(b.mods == mods && b.key == key));     // later entries replace earlier
            if let Some(action) = parse_action(&action)? {
                bindings.push(Binding { mods, key, action });
            }
        }
        let anim = table.get("animations").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let enabled = anim.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        let ms = anim.get("duration_ms").and_then(|v| v.as_integer()).unwrap_or(220).clamp(0, 2000) as u64;
        let animations = (enabled && ms > 0).then(|| std::time::Duration::from_millis(ms));
        let mut rules = crate::rules::defaults();
        rules.extend(crate::rules::parse(table)?);
        // [session]: restore = true|false, skip = ["app id", ...]
        let session = table.get("session").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let session = Session {
            restore: session.get("restore").and_then(|v| v.as_bool()).unwrap_or(true),
            skip: session.get("skip").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        };
        Ok(Config { input, bindings, animations, rules, session })
    }

    /// The action for a key press. `sym` is the key as typed, `raw` the unshifted key (so Shift+1
    /// reads as 1); `tty` decides whether Mod is Super or Alt.
    pub fn action(&self, mods: &ModifiersState, sym: Keysym, raw: Option<Keysym>, tty: bool) -> Option<Action> {
        if tty && (keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12).contains(&sym.raw()) {
            return Some(Action::Vt((sym.raw() - keysyms::KEY_XF86Switch_VT_1 + 1) as i32));
        }
        let pressed = Mods {
            main: if tty { mods.logo } else { mods.alt },
            shift: mods.shift,
            ctrl: mods.ctrl,
            alt: tty && mods.alt,          // nested, Alt is Mod itself
        };
        let lower = |k: Keysym| xkb::keysym_from_name(&xkb::keysym_get_name(k).to_lowercase(), xkb::KEYSYM_CASE_INSENSITIVE);
        let keys = [Some(sym), raw, raw.map(lower), Some(lower(sym))];
        self.bindings
            .iter()
            .find(|b| b.mods == pressed && keys.iter().flatten().any(|k| *k == b.key))
            .map(|b| b.action.clone())
    }
}

impl crate::state::Draftpaper {
    /// Read config.toml again when it has changed since the last look (and once at start): bindings,
    /// keyboard and touchpad, the theme (sheet and borders), the margin, and output scale and position.
    pub fn reload_config(&mut self) {
        let mtime = std::fs::metadata(path()).and_then(|m| m.modified()).ok();
        if mtime == self.config_mtime && self.config_mtime.is_some() {
            return;
        }
        let first = self.config_mtime.is_none();
        self.config_mtime = mtime;
        match Config::load() {
            Ok(config) => self.config = config,
            Err(e) => {
                eprintln!("draftpaper: config not applied, keeping the previous settings: {e}");
                return;
            }
        }
        self.apply_input();
        if first {
            return;
        }
        eprintln!("draftpaper: config reloaded");

        // theme and margin
        let theme = crate::theme::Theme::load();
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            if let Some(sheet) = output.user_data().get::<crate::render::OutputSheet>() {
                sheet.0.borrow_mut().set_theme(theme.clone());
            }
        }
        for s in &mut self.screens {
            s.margin_mm = theme.margin_mm as f64;
        }
        // output scale and position
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            let settings = crate::theme::output_settings(&output.name());
            let scale = settings.scale.map(smithay::output::Scale::Fractional);
            let position = settings.position.map(smithay::utils::Point::from);
            if scale.is_some() || position.is_some() {
                output.change_current_state(None, None, scale, position);
                if let Some(p) = position {
                    self.space.map_output(&output, p);
                }
            }
        }
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            self.update_screen(&output);
        }
        self.refresh_output_management();
    }

    /// Keyboard layout and repeat, and touchpad settings on the devices the TTY backend knows.
    pub fn apply_input(&mut self) {
        let input = self.config.input.clone();
        let keyboard = self.seat.get_keyboard().unwrap();
        let xkb = smithay::input::keyboard::XkbConfig {
            layout: &input.layout,
            variant: &input.variant,
            options: input.options.clone(),
            ..Default::default()
        };
        if let Err(e) = keyboard.set_xkb_config(self, xkb) {
            eprintln!("draftpaper: keyboard layout {:?}: {e:?}", input.layout);
        }
        keyboard.change_repeat_info(input.repeat_rate, input.repeat_delay);
        if let Some(tty) = self.tty_data.as_mut() {
            tty.configure_devices(&input);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_parse() {
        let config = Config::default_config();
        for key in ["XF86AudioRaiseVolume", "XF86AudioMute", "XF86MonBrightnessUp", "Escape", "Print", "t", "z"] {
            let sym = xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE);
            assert!(config.bindings.iter().any(|b| b.key == sym), "{key} is bound");
        }
    }

    #[test]
    fn resize_by_letters_too() {
        let config = Config::default_config();
        let mods = Mods { main: true, ctrl: true, ..Default::default() };
        for (key, dir) in [("h", Direction::Left), ("l", Direction::Right), ("k", Direction::Up), ("j", Direction::Down)] {
            let sym = xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE);
            let b = config.bindings.iter().find(|b| b.mods == mods && b.key == sym).expect("bound");
            assert!(matches!(b.action, Action::Resize(d) if d == dir), "Mod+Ctrl+{key}");
        }
    }
}
