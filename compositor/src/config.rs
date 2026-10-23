//! The compositor's settings in ~/.config/draftpaper/config.toml, next to the theme and the margin:
//!
//!     [input]                          # the keyboard, and what the pointer does to the focus
//!     keyboard_layout = "us,ro"        # xkb layout(s), variant, options
//!     keyboard_variant = ""
//!     keyboard_options = "grp:alt_shift_toggle,ctrl:nocaps"
//!     repeat_delay = 200               # ms
//!     repeat_rate = 25                 # per second
//!     focus_follows_mouse = true       # the window, and the screen, the pointer enters take the focus
//!
//!     [touchpad]                       # the built-in pad. `[input] tap` and `[input] natural_scroll`
//!     tap = true                       # still set these two where this table does not
//!     tap_and_drag = true              # tap, then tap and hold: the second tap drags
//!     natural_scroll = false           # the content follows the fingers
//!     disable_while_typing = true
//!     click_method = "button-areas"    # or "clickfinger": one finger or two, anywhere on the pad
//!     scroll_method = "two-finger"     # or "edge"
//!     speed = 0.0                      # -1 (slow) to 1 (fast)
//!     middle_emulation = false         # both buttons at once is the middle one
//!     left_handed = false
//!
//!     [touchscreen]                    # the panel itself
//!     enabled = true
//!     output = ""                      # the display it is the surface of; "" the built-in panel
//!
//!     [pointer]                        # mice, and the TrackPoint
//!     natural_scroll = false
//!     speed = 0.0
//!     middle_emulation = false
//!     left_handed = false
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
//!     [title]
//!     scale = true                     # the scale bar in the bottom margin, true to size (title.rs)
//!     key_plan = true                  # with several screens, a plan of them in the bottom margin
//!
//!     [[rule]]                         # window rules (rules.rs): app_id/title patterns, pip; float,
//!     title = "^Picture"               # sticky, place = "centre"|"corner", focus
//!     float = true
//!
//!     [bindings]                       # key = action; "none" removes a default
//!     "Mod+Return" = "spawn foot"      # Mod is Super on the TTY, Alt when nested
//!     "Mod+d" = "spawn fuzzel"
//!
//!     [keys]                           # the legend (docs/keyboard.md): Mod held alone this long
//!     legend_after = 500               # letters the keymap over the sheet; 0 turns it off
//!     word_for = 1200                  # how long a passing word stands on it
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
    /// the pointer moved by this much (logical pixels), as a mouse moves it: through the rulers' steps
    PointerBy(f64, f64),
    /// a mouse button (Linux's code: left 0x110, right 0x111) pressed or let go where the pointer is, for
    /// scripts and tests (a drag: button left down, pointer ..., button left up)
    Button(u32, bool),
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
    /// the binding as it was written — "Mod+Shift+i", "spawn draftpaper schedule pick". Kept rather than
    /// formatted back from `mods`, `key` and `action`, so what the legend letters is what the table says
    /// and the two cannot drift (docs/keyboard.md).
    pub keys: String,
    pub text: String,
}

/// [input]: the keyboard, and what the pointer does to the focus. What a *device* does is its own
/// table — a touchpad, a touchscreen and a TrackPoint are three different things, and one
/// `natural_scroll` for all of them was one setting doing three jobs.
#[derive(Debug, Clone)]
pub struct Input {
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
    pub repeat_delay: i32,
    pub repeat_rate: i32,
    /// the window (and screen) the pointer enters takes the focus, as i3's focus_follows_mouse
    pub focus_follows_mouse: bool,
}

/// [touchpad]: the built-in pad, told apart from a mouse by having fingers to tap with
/// (`config_tap_finger_count`). `[input] tap` and `[input] natural_scroll` still set `tap` and
/// `natural_scroll` where this table does not, so a config written before this stays true.
#[derive(Debug, Clone)]
pub struct Touchpad {
    pub tap: bool,
    /// tap, then tap and hold: the second tap drags
    pub tap_and_drag: bool,
    pub natural_scroll: bool,
    /// the pad goes quiet while the keyboard is being used
    pub disable_while_typing: bool,
    /// "button-areas": the foot of the pad is left and right; "clickfinger": one finger or two
    pub click_method: String,
    /// "two-finger" or "edge"
    pub scroll_method: String,
    /// libinput's pointer acceleration, -1 (slow) to 1 (fast)
    pub speed: f64,
    pub middle_emulation: bool,
    pub left_handed: bool,
}

/// [touchscreen]: the panel itself. There is little to set on a touch device — it reports where it
/// was touched and that is all — so this is whether it is listened to, and which display it is the
/// surface of (input.rs's `touch_location`; "" is the built-in panel, as it always was).
#[derive(Debug, Clone)]
pub struct Touchscreen {
    pub enabled: bool,
    pub output: String,
}

/// [pointer]: mice, and the TrackPoint. Scrolling a wheel is not scrolling a pad, which is why this
/// is not the touchpad's table.
#[derive(Debug, Clone)]
pub struct Pointer {
    pub natural_scroll: bool,
    pub speed: f64,
    pub middle_emulation: bool,
    pub left_handed: bool,
}


#[derive(Debug, Clone)]
pub struct Config {
    pub input: Input,
    pub touchpad: Touchpad,
    pub touchscreen: Touchscreen,
    pub pointer: Pointer,
    pub bindings: Vec<Binding>,
    /// how long parking, unparking and switching workspaces take; None: no animations
    pub animations: Option<std::time::Duration>,
    /// window rules: the built-in ones, then config.toml's [[rule]]s (rules.rs)
    pub rules: Vec<crate::rules::Rule>,
    pub session: Session,
    /// [title] scale: the scale bar in the bottom margin (title.rs's scale_bar)
    pub scale_bar: bool,
    /// [title] key_plan: with several screens, a plan of them in the bottom margin (title.rs's key_plan)
    pub key_plan: bool,
    /// [keys] legend_after: how long Mod must be held, alone, before the legend is lettered over the
    /// sheet — the keymap shown on the things it acts on (docs/keyboard.md). None where it is turned
    /// off with 0. Any other key cancels it, so a slow chord never raises it.
    pub legend_after: Option<std::time::Duration>,
    /// [keys] word_for: how long a passing word stands on the sheet — the line the legend says in
    /// answer to a key and then takes back (docs/keyboard.md). It is the keyboard's own register, so
    /// it is timed here with the hold rather than by whoever letters it.
    pub word_for: std::time::Duration,
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
    // the rulers (qml/Rulers.qml): out or away; the pointer held to their millimetres meanwhile
    ("Mod+r", "spawn draftpaper rulers"),
    // the schedules on the sheet in turn: the equipment, the processes, neither (qml/Schedule.qml)
    ("Mod+i", "spawn draftpaper schedule next"),
    ("Mod+Shift+i", "spawn draftpaper schedule pick"),
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
    // through draftpaper rather than wpctl alone, so the sheet is told what the volume now is
    ("XF86AudioRaiseVolume", "spawn draftpaper volume up"),
    ("XF86AudioLowerVolume", "spawn draftpaper volume down"),
    ("XF86AudioMute", "spawn draftpaper volume mute"),
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
        "pointer_by" => {
            let mut xy = arg.split_whitespace().map(|v| v.parse::<f64>());
            match (xy.next(), xy.next()) {
                (Some(Ok(x)), Some(Ok(y))) => Action::PointerBy(x, y),
                _ => return Err(format!("\"{s}\": pointer_by DX DY")),
            }
        }
        "button" => {
            let mut a = arg.split_whitespace();
            let code = match a.next() {
                Some("left") => 0x110,
                Some("right") => 0x111,
                Some("middle") => 0x112,
                _ => return Err(format!("\"{s}\": button left|right|middle down|up")),
            };
            match a.next() {
                Some("down") => Action::Button(code, true),
                Some("up") => Action::Button(code, false),
                _ => return Err(format!("\"{s}\": button left|right|middle down|up")),
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
            focus_follows_mouse: b("focus_follows_mouse", true),
        };
        // [touchpad], [touchscreen], [pointer]: what each kind of device does, rather than one table
        // read into all of them. `[input] tap` and `[input] natural_scroll` are where the touchpad's
        // two settings used to live, so they are what this falls back to: a config written before
        // these tables existed goes on meaning what it meant.
        let pad = table.get("touchpad").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let pb = |k: &str, d: bool| pad.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
        let one = |t: &toml::Table, k: &str| -> f64 {
            t.get(k).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64))).unwrap_or(0.0).clamp(-1.0, 1.0)
        };
        let among = |t: &toml::Table, k: &str, ok: &[&str], d: &str| -> String {
            let said = t.get(k).and_then(|v| v.as_str()).unwrap_or(d);
            ok.iter().find(|o| **o == said).unwrap_or(&d).to_string()
        };
        let touchpad = Touchpad {
            tap: pb("tap", b("tap", true)),
            tap_and_drag: pb("tap_and_drag", true),
            natural_scroll: pb("natural_scroll", b("natural_scroll", false)),
            disable_while_typing: pb("disable_while_typing", true),
            click_method: among(&pad, "click_method", &["button-areas", "clickfinger"], "button-areas"),
            scroll_method: among(&pad, "scroll_method", &["two-finger", "edge"], "two-finger"),
            speed: one(&pad, "speed"),
            middle_emulation: pb("middle_emulation", false),
            left_handed: pb("left_handed", false),
        };
        let touch = table.get("touchscreen").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let touchscreen = Touchscreen {
            enabled: touch.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
            output: touch.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };
        let mouse = table.get("pointer").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let mb = |k: &str, d: bool| mouse.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
        let pointer = Pointer {
            natural_scroll: mb("natural_scroll", false),
            speed: one(&mouse, "speed"),
            middle_emulation: mb("middle_emulation", false),
            left_handed: mb("left_handed", false),
        };

        let mut bindings: Vec<Binding> = Vec::new();
        let user = table.get("bindings").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let entries = DEFAULT_BINDINGS
            .iter()
            .map(|(k, a)| (k.to_string(), a.to_string()))
            .chain(user.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())));
        for (keys, text) in entries {
            let (mods, key) = parse_keys(&keys)?;
            bindings.retain(|b| !(b.mods == mods && b.key == key));     // later entries replace earlier
            if let Some(action) = parse_action(&text)? {
                bindings.push(Binding { mods, key, action, keys, text });
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
        // [title]: scale = true|false (the rest of [title] is the shell's: project, drawn)
        let scale_bar = table.get("title").and_then(|v| v.get("scale")).and_then(|v| v.as_bool()).unwrap_or(true);
        let key_plan = table.get("title").and_then(|v| v.get("key_plan")).and_then(|v| v.as_bool()).unwrap_or(true);
        // [keys] legend_after: milliseconds, 0 to turn the legend off
        let after = table.get("keys").and_then(|v| v.get("legend_after")).and_then(|v| v.as_integer()).unwrap_or(500).clamp(0, 5000) as u64;
        let legend_after = (after > 0).then(|| std::time::Duration::from_millis(after));
        // [keys] word_for: milliseconds a passing word stands. Long enough to be read without being
        // looked for, short enough that the sheet is a drawing again before you have finished thinking.
        let word_for = table.get("keys").and_then(|v| v.get("word_for")).and_then(|v| v.as_integer()).unwrap_or(1200).clamp(200, 10_000) as u64;
        let word_for = std::time::Duration::from_millis(word_for);
        Ok(Config { input, touchpad, touchscreen, pointer, bindings, animations, rules, session, scale_bar, key_plan, legend_after, word_for })
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
        self.bound(pressed, sym, raw)
    }

    /// The action for a key typed after the leader: Mod tapped and let go, then this key
    /// (docs/keyboard.md). It is the chord's own table, looked up with Mod held, so `Mod` tap then `h`
    /// is `Mod+h` by construction and not by a second list that could say otherwise. Whatever else is
    /// held still counts, so the leader reaches `Mod+Shift+Left` as well.
    pub fn led(&self, mods: &ModifiersState, sym: Keysym, raw: Option<Keysym>, tty: bool) -> Option<Action> {
        self.bound(
            Mods { main: true, shift: mods.shift, ctrl: mods.ctrl, alt: tty && mods.alt },
            sym,
            raw,
        )
    }

    /// The binding for these modifiers and this key, however they came to be pressed.
    fn bound(&self, pressed: Mods, sym: Keysym, raw: Option<Keysym>) -> Option<Action> {
        let lower = |k: Keysym| xkb::keysym_from_name(&xkb::keysym_get_name(k).to_lowercase(), xkb::KEYSYM_CASE_INSENSITIVE);
        let keys = [Some(sym), raw, raw.map(lower), Some(lower(sym))];
        self.bindings
            .iter()
            .find(|b| b.mods == pressed && keys.iter().flatten().any(|k| *k == b.key))
            .map(|b| b.action.clone())
    }
}

/// Which of an output's `[outputs.<name>]` scale and position config.toml set at the last read (or when it
/// was plugged in), kept with it: only a key that goes puts the output back as plugged in.
#[derive(Debug, Clone, Copy, Default)]
pub struct LastSet {
    pub scale: bool,
    pub position: bool,
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
            let settings = crate::theme::output_settings(&output.name(), &output.physical_properties().model);
            // a key taken out since the last read (the settings' Escape): the output as it would be without
            // config.toml; one never set leaves the output as it is (wlr-randr's arrangement, say)
            let plug = output.user_data().get::<crate::theme::PlugDefaults>().copied();
            output.user_data().insert_if_missing(|| std::cell::RefCell::new(LastSet::default()));
            let last = output.user_data().get::<std::cell::RefCell<LastSet>>().unwrap();
            let was = last.replace(LastSet { scale: settings.scale.is_some(), position: settings.position.is_some() });
            let scale = settings.scale.or(plug.filter(|_| was.scale).map(|d| d.scale)).map(smithay::output::Scale::Fractional);
            let position = settings.position.or(plug.filter(|_| was.position).map(|d| d.position)).map(smithay::utils::Point::from);
            // only what differs: clients hear of a change, not of every save
            let scale = scale.filter(|s| (s.fractional_scale() - output.current_scale().fractional_scale()).abs() > 1e-6);
            let position = position.filter(|p| self.space.output_geometry(&output).map(|g| g.loc) != Some(*p));
            if scale.is_some() || position.is_some() {
                output.change_current_state(None, None, scale, position);
                if let Some(p) = position {
                    self.space.map_output(&output, p);
                }
            }
        }
        // a display that grew can now be lying under its neighbour: put the automatic ones clear
        self.space_out_outputs();
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            self.update_screen(&output);
        }
        // the gamma tables: a display's ICC profile may have changed
        crate::gamma::refresh_all(self);
        self.refresh_output_management();
    }

    /// Keyboard layout and repeat, and touchpad settings on the devices the TTY backend knows.
    /// No two displays may stand on the same logical ground.
    ///
    /// An output plugged in without a `position` of its own is placed to the right of everything
    /// there was *at that moment* (tty.rs). Change another display's scale afterwards and it is a
    /// different size, so the one placed against it can end up lying underneath — and in the overlap
    /// the pointer is inside both outputs at once, which is why the cursor was drawn on two displays.
    ///
    /// So the automatic ones are put clear again whenever anything is applied. A display that
    /// config.toml gives a `position` to is the user's arrangement and is never moved: if two of
    /// those overlap, that is what was asked for.
    pub fn space_out_outputs(&mut self) {
        use smithay::utils::{Point, Rectangle};
        let mut fixed: Vec<Rectangle<i32, smithay::utils::Logical>> = Vec::new();
        let mut loose: Vec<smithay::output::Output> = Vec::new();
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            let settings = crate::theme::output_settings(&output.name(), &output.physical_properties().model);
            let Some(geometry) = self.space.output_geometry(&output) else { continue };
            if settings.position.is_some() {
                fixed.push(geometry);
            } else {
                loose.push(output);
            }
        }
        // left to right as they stand, so the arrangement keeps the order it had
        loose.sort_by_key(|o| self.space.output_geometry(o).map(|g| g.loc.x).unwrap_or(0));
        let held: Vec<(i32, i32, i32, i32)> = fixed.iter().map(|r| (r.loc.x, r.loc.y, r.size.w, r.size.h)).collect();
        let moving: Vec<(i32, i32, i32, i32)> = loose
            .iter()
            .filter_map(|o| self.space.output_geometry(o))
            .map(|g| (g.loc.x, g.loc.y, g.size.w, g.size.h))
            .collect();
        for (output, (x, y)) in loose.iter().zip(spaced_out(&held, &moving)) {
            let Some(geometry) = self.space.output_geometry(output) else { continue };
            if (geometry.loc.x, geometry.loc.y) == (x, y) {
                continue;
            }
            eprintln!("draftpaper: output {} was under another display, moved to {:?}", output.name(), (x, y));
            let at = Point::from((x, y));
            output.change_current_state(None, None, None, Some(at));
            self.space.map_output(output, at);
        }
    }

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
        let config = self.config.clone();
        if let Some(tty) = self.tty_data.as_mut() {
            tty.configure_devices(&config);
        }
    }
}

/// Where the automatically placed displays go so that none of them lies under another.
///
/// `fixed` is every display config.toml gives a `position` to, as (x, y, w, h): that is the user's
/// arrangement and it never moves, even if two of them overlap — that was what was asked for.
/// `loose` is the rest, left to right as they stand. Each is left alone where it is clear, and put
/// to the right of everything standing where it is not, keeping the height it was placed at.
fn spaced_out(fixed: &[(i32, i32, i32, i32)], loose: &[(i32, i32, i32, i32)]) -> Vec<(i32, i32)> {
    let hits = |a: &(i32, i32, i32, i32), b: &(i32, i32, i32, i32)| {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    };
    let mut taken: Vec<(i32, i32, i32, i32)> = fixed.to_vec();
    let mut out = Vec::with_capacity(loose.len());
    for d in loose {
        if !taken.iter().any(|r| hits(r, d)) {
            taken.push(*d);
            out.push((d.0, d.1));
            continue;
        }
        let x = taken.iter().map(|r| r.0 + r.2).max().unwrap_or(0);
        taken.push((x, d.1, d.2, d.3));
        out.push((x, d.1));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defect this holds off, exactly as it happened: the 4K was plugged in while the laptop's
    /// panel was at scale 1.5, so it was placed to the right of 1280 logical pixels. The panel was
    /// then set to scale 1 and became 1920 wide, and the 4K was left lying under its right-hand 640 —
    /// where the pointer is inside two displays at once and the cursor is drawn on both.
    #[test]
    fn a_display_that_grew_does_not_leave_its_neighbour_underneath() {
        let panel = [(0, 0, 1920, 1200)];                       // eDP-1, now scale 1
        let four_k = [(1280, 0, 1920, 1080)];                   // DP-3, placed when the panel was 1280
        assert_eq!(spaced_out(&panel, &four_k), vec![(1920, 0)], "the 4K goes clear of the panel");
        // and once it is clear, nothing moves again: applying the settings twice is not two moves
        let settled = [(1920, 0, 1920, 1080)];
        assert_eq!(spaced_out(&panel, &settled), vec![(1920, 0)]);
    }

    /// An arrangement written in config.toml is the user's and is never moved, even where it overlaps:
    /// two displays mirrored onto the same ground is a thing somebody may have asked for.
    #[test]
    fn the_displays_config_toml_places_are_left_exactly_where_they_are_asked_for() {
        let both_at_origin = [(0, 0, 1920, 1200), (0, 0, 1920, 1080)];
        assert_eq!(spaced_out(&both_at_origin, &[]), Vec::<(i32, i32)>::new());
        // and a loose one goes to the right of the whole of it, not into the gap
        assert_eq!(spaced_out(&both_at_origin, &[(0, 0, 800, 600)]), vec![(1920, 0)]);
    }

    /// Displays that do not touch are left alone: this runs whenever anything is applied, and it must
    /// not shuffle an arrangement that is already good.
    #[test]
    fn displays_standing_clear_are_not_moved() {
        let loose = [(0, 0, 1280, 800), (1280, 0, 1920, 1080), (3200, 0, 1024, 768)];
        assert_eq!(spaced_out(&[], &loose), vec![(0, 0), (1280, 0), (3200, 0)]);
        // one above another is clear too: a display stacked over the laptop is a real arrangement
        let stacked = [(0, 0, 1920, 1200), (0, -1080, 1920, 1080)];
        assert_eq!(spaced_out(&[], &stacked), vec![(0, 0), (0, -1080)]);
    }

    /// A config written before the devices had tables of their own goes on meaning what it meant: the
    /// touchpad's two settings lived in `[input]`, and that is where they are still read from when
    /// `[touchpad]` does not say.
    #[test]
    fn the_touchpads_old_home_in_input_is_still_read() {
        let old: toml::Table = toml::from_str("[input]\ntap = false\nnatural_scroll = true\n").unwrap();
        let c = Config::from_table(&old).unwrap();
        assert!(!c.touchpad.tap, "[input] tap still turns tapping off");
        assert!(c.touchpad.natural_scroll, "and [input] natural_scroll still turns it on");
        // the pointer is not the touchpad: one table never meant the other
        assert!(!c.pointer.natural_scroll, "a mouse is not scrolled by the touchpad's setting");

        // and where both are written the device's own table wins
        let both: toml::Table = toml::from_str("[input]\ntap = false\n[touchpad]\ntap = true\n").unwrap();
        assert!(Config::from_table(&both).unwrap().touchpad.tap);
    }

    /// A value that is not one of the ones offered is not passed to libinput: the sheet's settings
    /// write these, but config.toml is the user's file and may say anything.
    #[test]
    fn a_choice_nothing_offers_falls_back_rather_than_reaching_libinput() {
        let odd: toml::Table = toml::from_str(
            "[touchpad]\nclick_method = \"telepathy\"\nscroll_method = \"\"\nspeed = 9.5\n",
        )
        .unwrap();
        let c = Config::from_table(&odd).unwrap();
        assert_eq!(c.touchpad.click_method, "button-areas");
        assert_eq!(c.touchpad.scroll_method, "two-finger");
        assert_eq!(c.touchpad.speed, 1.0, "libinput takes -1 to 1 and nothing else");
        let slow: toml::Table = toml::from_str("[pointer]\nspeed = -4\n").unwrap();
        assert_eq!(Config::from_table(&slow).unwrap().pointer.speed, -1.0, "an integer reads as a speed too");
    }

    /// The touchscreen is the surface of one display, and with none named it is the one it always was.
    #[test]
    fn the_touchscreen_is_on_by_default_and_belongs_to_no_display_in_particular() {
        let c = Config::from_table(&toml::Table::new()).unwrap();
        assert!(c.touchscreen.enabled);
        assert_eq!(c.touchscreen.output, "");
        let off: toml::Table = toml::from_str("[touchscreen]\nenabled = false\noutput = \"DP-3\"\n").unwrap();
        let c = Config::from_table(&off).unwrap();
        assert!(!c.touchscreen.enabled);
        assert_eq!(c.touchscreen.output, "DP-3");
    }

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
