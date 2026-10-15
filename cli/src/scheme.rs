//! Colour schemes built from a few choices, and the contrast rules every theme is checked against.
//!
//! Display perception research (Legge 1990 and after): luminance contrast between text and its
//! background carries legibility; hue adds nothing. So a scheme is not painted value by value. It is
//! built from a few discrete choices, the hues taste and the lightnesses solved: every colour used for
//! words reaches at least 4.5:1 against what it sits on (7:1 or 10:1 for the main text, by the contrast
//! choice), dimmed text 3:1, the focused window's border 3:1 against the window it frames. The colours
//! are worked out in OKLCH, so a step in lightness looks like the same step whatever the hue.
//!
//!     paper     light | dark            the sheet
//!     tone      greige sand clay sage graphite frost slate teal indigo blueprint (warm to cold)
//!     accent    amber orange red rose violet blue teal green
//!     windows   dark | light            the applications: terminals, GTK, Qt, Firefox
//!     shade     deep dark charcoal slate    the windows' lightness on their side: from the deepest to a
//!                                       mid grey (light windows: white paper mist grey)
//!     terminal  windows charcoal graphite slate petrol moss olive umber wine plum    the terminal's
//!                                       background: as the windows, or a mid tone of its own on the
//!                                       windows' side (dark or light)
//!     text      neutral cream lime amber ice    the terminal's text
//!     contrast  standard | high         main text at about 7:1 (softer), or 10:1 and more
//!
//! config.toml keeps the choices ([scheme]); `draftpaper scheme keep` writes the theme they make to
//! ~/.config/draftpaper/themes/custom.toml, a theme like any other ("Custom").

use serde_json::{json, Map, Value};

use crate::util::{self, round_even};

// ---- choices ----
/// (name, hue in degrees, chroma), from warm to cold
pub const TONES: [(&str, f64, f64); 10] = [
    ("greige", 70.0, 0.020), ("sand", 80.0, 0.045), ("clay", 45.0, 0.040), ("sage", 145.0, 0.030), ("graphite", 0.0, 0.0),
    ("frost", 225.0, 0.014), ("slate", 245.0, 0.025), ("teal", 205.0, 0.040), ("indigo", 275.0, 0.045), ("blueprint", 250.0, 0.075),
];
pub const ACCENTS: [(&str, f64, f64); 8] = [
    ("amber", 70.0, 0.13), ("orange", 50.0, 0.15), ("red", 27.0, 0.15), ("rose", 0.0, 0.12),
    ("violet", 300.0, 0.12), ("blue", 255.0, 0.13), ("teal", 195.0, 0.10), ("green", 145.0, 0.12),
];
/// the terminal's text: None, neutral (the windows' hue); else (hue, chroma)
pub const TEXTS: [(&str, Option<(f64, f64)>); 5] =
    [("neutral", None), ("cream", Some((85.0, 0.03))), ("lime", Some((125.0, 0.08))), ("amber", Some((75.0, 0.10))), ("ice", Some((230.0, 0.03)))];
/// the terminal's background: None, as the windows; else (hue, chroma, lightness on dark windows, on light
/// ones): mid tones, neither near black nor near white, each a hue of its own (the chroma at 70% on light)
pub const TERMINALS: [(&str, Option<(f64, f64, f64, f64)>); 10] = [
    ("windows", None),
    ("charcoal", Some((0.0, 0.0, 0.32, 0.86))), ("graphite", Some((250.0, 0.012, 0.40, 0.80))), ("slate", Some((250.0, 0.045, 0.35, 0.86))),
    ("petrol", Some((210.0, 0.05, 0.34, 0.87))), ("moss", Some((145.0, 0.05, 0.35, 0.87))), ("olive", Some((105.0, 0.05, 0.37, 0.88))),
    ("umber", Some((55.0, 0.045, 0.34, 0.87))), ("wine", Some((12.0, 0.05, 0.32, 0.87))), ("plum", Some((320.0, 0.045, 0.33, 0.87))),
];
/// the lightness (OKLCH L) of each shade, on the dark side and on the light; "dark" is the windows' as
/// they were before shades were a choice
pub const SHADES: [(&str, f64, f64); 4] = [("deep", 0.16, 0.995), ("dark", 0.22, 0.975), ("charcoal", 0.30, 0.92), ("slate", 0.38, 0.86)];
/// what each shade is called on the light side
pub fn light_name(shade: &str) -> &str {
    match shade {
        "deep" => "white",
        "dark" => "paper",
        "charcoal" => "mist",
        "slate" => "grey",
        s => s,
    }
}

/// The choices and their values, in order.
pub fn choices() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("paper", vec!["light", "dark"]),
        ("tone", TONES.iter().map(|t| t.0).collect()),
        ("accent", ACCENTS.iter().map(|t| t.0).collect()),
        ("windows", vec!["dark", "light"]),
        ("shade", SHADES.iter().map(|t| t.0).collect()),
        ("terminal", TERMINALS.iter().map(|t| t.0).collect()),
        ("text", TEXTS.iter().map(|t| t.0).collect()),
        ("contrast", vec!["standard", "high"]),
    ]
}
pub const DEFAULT: [(&str, &str); 8] = [
    ("paper", "light"), ("tone", "greige"), ("accent", "amber"), ("windows", "dark"), ("shade", "dark"),
    ("terminal", "windows"), ("text", "cream"), ("contrast", "standard"),
];
const WORDS: f64 = 4.5;
const DIM: f64 = 3.0;
const BORDER: f64 = 3.0;

// ---- colour ----
fn lin(x: f64) -> f64 {
    if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
}
fn gam(x: f64) -> f64 {
    if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}
fn rgb(hx: &str) -> [f64; 3] {
    let h = hx.trim_start_matches('#');
    let c = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;
    [c(0), c(2), c(4)]
}
fn hexa(c: [f64; 3]) -> String {
    let mut s = String::from("#");
    for x in c {
        s += &format!("{:02X}", round_even(x.clamp(0.0, 1.0) * 255.0) as u8);
    }
    s
}
pub fn luminance(hx: &str) -> f64 {
    let [r, g, b] = rgb(hx).map(lin);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
pub fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
fn oklch_to_rgb(l: f64, c: f64, h: f64) -> [f64; 3] {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l, m, s) = (l_.powf(3.0), m_.powf(3.0), s_.powf(3.0));
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}
/// (L, C, h) of a colour.
fn to_oklch(hx: &str) -> (f64, f64, f64) {
    let [r, g, b] = rgb(hx).map(lin);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).powf(1.0 / 3.0);
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).powf(1.0 / 3.0);
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).powf(1.0 / 3.0);
    let ll = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    let aa = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let bb = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    let deg = bb.atan2(aa).to_degrees();
    // Python's modulo: the sign of the divisor
    let mut h = deg % 360.0;
    if h != 0.0 && h < 0.0 {
        h += 360.0;
    }
    (ll, aa.hypot(bb), h)
}
fn in_gamut(c: [f64; 3]) -> bool {
    c.iter().all(|x| (-1e-4..=1.0 + 1e-4).contains(x))
}
/// A colour from OKLCH, its chroma reduced until it fits in sRGB.
pub fn oklch(l: f64, c: f64, h: f64) -> String {
    let l = l.clamp(0.0, 1.0);
    let lin = oklch_to_rgb(l, c, h);
    let out = |x: [f64; 3]| hexa(x.map(|v| gam(v.clamp(0.0, 1.0))));
    if in_gamut(lin) {
        return out(lin);
    }
    let (mut lo, mut hi) = (0.0, c);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if in_gamut(oklch_to_rgb(l, mid, h)) { lo = mid } else { hi = mid }
    }
    out(oklch_to_rgb(l, lo, h))
}
/// The colour of hue `h` and chroma `c` nearest lightness `prefer` that reaches every (background,
/// minimum contrast) in `against`, going away from the backgrounds (darker on light ones).
fn solve(h: f64, c: f64, prefer: f64, against: &[(&str, f64)]) -> String {
    let dark = against.iter().map(|(bg, _)| luminance(bg)).sum::<f64>() / against.len() as f64 > 0.18;
    let mut l = prefer;
    while (0.0..=1.0).contains(&l) {
        let col = oklch(l, c, h);
        if against.iter().all(|(bg, m)| contrast(&col, bg) >= *m) {
            return col;
        }
        l += if dark { -0.005 } else { 0.005 };
    }
    if dark { "#000000".into() } else { "#FFFFFF".into() }
}
/// Whether a colour is light (dark text reads on it).
fn light(hx: &str) -> bool {
    luminance(hx) > 0.18
}

// ---- the scheme ----
/// The choices made whole: the defaults, with every given choice that names a known value.
pub fn complete(given: &[(String, String)]) -> Vec<(String, String)> {
    let all = choices();
    DEFAULT
        .iter()
        .map(|(k, d)| {
            let v = given
                .iter()
                .rev()
                .find(|(gk, gv)| gk == k && all.iter().any(|(ck, vs)| ck == k && vs.contains(&gv.as_str())))
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| d.to_string());
            (k.to_string(), v)
        })
        .collect()
}

/// The theme the choices build, as a theme file's values in order (with `choices` at the end).
pub fn build(given: &[(String, String)]) -> Map<String, Value> {
    let ch = complete(given);
    let get = |k: &str| ch.iter().find(|(ck, _)| ck == k).map(|(_, v)| v.as_str()).unwrap_or("");
    let (_, th, tc) = *TONES.iter().find(|t| t.0 == get("tone")).unwrap();
    let (_, ah, ac) = *ACCENTS.iter().find(|t| t.0 == get("accent")).unwrap();
    let main = if get("contrast") == "high" { 10.0 } else { 7.0 };
    let light_paper = get("paper") == "light";
    let dark_windows = get("windows") == "dark";

    // the sheet
    let (paper, paper_light, paper_shade, glow, ink_pref, strong_pref) = if light_paper {
        (oklch(0.86, tc, th), oklch(0.92, tc * 0.8, th), oklch(0.76, tc * 1.1, th), oklch(0.93, 0.05, ah), 0.30, 0.12)
    } else {
        (oklch(0.31, tc, th), oklch(0.37, tc, th), oklch(0.24, tc, th), oklch(0.40, 0.05, ah), 0.85, 0.97)
    };
    let sheet_bgs = [(paper.as_str(), WORDS), (paper_light.as_str(), WORDS), (glow.as_str(), WORDS), (paper_shade.as_str(), DIM)];
    let ink = solve(th, tc.min(0.04), ink_pref, &sheet_bgs);
    let strong = solve(if light_paper { th } else { ah }, if light_paper { 0.02 } else { 0.12 }, strong_pref, &[(&paper, main), (&paper_light, WORDS)]);
    let alert = solve(27.0, 0.17, if light_paper { 0.50 } else { 0.72 }, &sheet_bgs);

    // the windows: the shade chosen, the surfaces stepping away from it (lighter on dark windows, darker
    // on light ones)
    let (nh, nc) = (th, tc.min(0.012));
    let shade = SHADES.iter().find(|s| s.0 == get("shade")).unwrap();
    let w_l = if dark_windows { shade.1 } else { shade.2 };
    let steps: [f64; 5] = if dark_windows { [0.0, 0.025, 0.05, 0.09, 0.20] } else { [0.0, -0.025, -0.05, -0.085, -0.215] };
    let [bg, bg_alt, surface, surface_alt, border] = steps.map(|d| if dark_windows { oklch(w_l + d, nc, nh) } else { oklch(w_l - (-d), nc, nh) });
    let (fg_pref, dim_pref, muted_pref) = if dark_windows { (0.93, 0.80, 0.68) } else { (0.22, 0.38, 0.50) };
    // standard: the main text only as far from its background as 7:1 takes it (softer); high: near the
    // extreme, at least 10:1
    let soft = get("contrast") == "standard";
    let bg_l = to_oklch(&bg).0;
    let fg = solve(nh, nc, if soft { bg_l } else { fg_pref }, &[(&bg, main), (&bg_alt, main), (&surface, WORDS)]);
    let fg_dim = solve(nh, nc, dim_pref, &[(&bg, WORDS), (&surface, WORDS)]);
    let fg_muted = solve(nh, nc, muted_pref, &[(&bg, DIM), (&surface, DIM)]);
    // the terminal's background: as the windows', or a mid tone of its own on the same side
    let tb = TERMINALS.iter().find(|t| t.0 == get("terminal")).unwrap().1;
    let (term_bg, term_bg_alt, tbh, tbc, tb_l) = match tb {
        None => (bg.clone(), surface.clone(), nh, nc, bg_l),
        Some((h, c, ld, ll)) => {
            let (c, l) = if dark_windows { (c, ld) } else { (c * 0.7, ll) };
            (oklch(l, c, h), oklch(l + if dark_windows { 0.05 } else { -0.04 }, c, h), h, c, l)
        }
    };
    let (txh, txc) = TEXTS.iter().find(|t| t.0 == get("text")).unwrap().1.unwrap_or((nh, nc));
    let term_fg = solve(txh, txc, if soft { tb_l } else { fg_pref }, &[(&term_bg, main), (&term_bg_alt, WORDS)]);

    // the accent: the focused window's border (against the window), fills, and ink for words
    let focused = solve(ah, ac, if dark_windows { 0.72 } else { 0.58 }, &[(&bg, BORDER), (&surface, BORDER)]);
    let accent = focused.clone();
    let accent_ink = solve(ah, ac, if dark_windows { 0.75 } else { 0.48 }, &[(&bg, WORDS), (&surface, WORDS)]);
    let accent_dim = oklch(if dark_windows { 0.55 } else { 0.80 }, ac * 0.8, ah);
    let on_accent = if contrast(&bg, &accent) >= WORDS { bg.clone() } else { solve(ah, 0.02, if light(&accent) { 0.15 } else { 0.97 }, &[(&accent, WORDS)]) };
    let sel_bg = accent_dim.clone();
    let sel_fg = solve(ah, 0.02, if light(&sel_bg) { 0.12 } else { 0.97 }, &[(&sel_bg, WORDS)]);
    let sh = if ah == 125.0 || ah == 145.0 || (ah - 145.0).abs() < 30.0 { 195.0 } else { 145.0 };
    let secondary = oklch(if dark_windows { 0.72 } else { 0.55 }, 0.09, sh);
    let secondary_dim = oklch(if dark_windows { 0.58 } else { 0.70 }, 0.07, sh);
    let secondary_ink = solve(sh, 0.09, if dark_windows { 0.75 } else { 0.45 }, &[(&bg, WORDS)]);
    let states = [("urgent", 25.0, 0.14), ("warn", 55.0, 0.14), ("ok", 145.0, 0.11), ("info", 235.0, 0.08)];
    let fills: Vec<(String, String)> = states.iter().map(|(k, h, c)| (k.to_string(), oklch(if dark_windows { 0.70 } else { 0.58 }, *c, *h))).collect();
    let inks: Vec<(String, String)> = states
        .iter()
        .map(|(k, h, c)| (format!("{k}_ink"), solve(*h, *c, if dark_windows { 0.72 } else { 0.45 }, &[(&bg, WORDS), (&surface, WORDS)])))
        .collect();

    // the terminal's sixteen colours, each readable on the terminal's background
    let on_term = [(term_bg.as_str(), WORDS), (term_bg_alt.as_str(), WORDS)];
    let mut ansi: Vec<(String, String)> = Vec::new();
    for (name, h) in [("red", 27.0), ("green", 140.0), ("yellow", 90.0), ("blue", 250.0), ("magenta", 330.0), ("cyan", 200.0)] {
        ansi.push((name.into(), solve(h, 0.12, if dark_windows { 0.70 } else { 0.48 }, &on_term)));
        ansi.push((format!("br_{name}"), solve(h, 0.10, if dark_windows { 0.82 } else { 0.40 }, &on_term)));
    }
    let gc = tbc.min(0.012); // the greys, a touch of the background's hue
    if dark_windows {
        ansi.push(("black".into(), oklch(tb_l + 0.09, tbc, tbh))); // a background, not text
        ansi.push(("br_black".into(), solve(tbh, gc, 0.62, &on_term))); // comments, dim text
        ansi.push(("white".into(), solve(tbh, gc, 0.86, &[(&term_bg, WORDS)])));
        ansi.push(("br_white".into(), solve(tbh, gc, 0.97, &[(&term_bg, main)])));
    } else {
        ansi.push(("black".into(), solve(tbh, gc, 0.20, &[(&term_bg, main)])));
        ansi.push(("br_black".into(), solve(tbh, gc, 0.48, &on_term)));
        ansi.push(("white".into(), oklch(tb_l - 0.085, tbc, tbh))); // backgrounds in a light terminal
        ansi.push(("br_white".into(), oklch(tb_l - 0.035, tbc, tbh)));
    }
    let cursor = accent.clone();
    let cursor_text = if contrast(&term_bg, &cursor) >= WORDS { term_bg.clone() } else { solve(ah, 0.02, if light(&cursor) { 0.12 } else { 0.97 }, &[(&cursor, WORDS)]) };

    let mut apps = Map::new();
    let mut put = |k: &str, v: &str| {
        apps.insert(k.into(), json!(v));
    };
    put("variant", if dark_windows { "dark" } else { "light" });
    for (k, v) in [("bg", &bg), ("bg_alt", &bg_alt), ("surface", &surface), ("surface_alt", &surface_alt), ("border", &border), ("fg", &fg), ("fg_dim", &fg_dim), ("fg_muted", &fg_muted), ("field", &bg_alt), ("field_fg", &fg), ("term_bg", &term_bg), ("term_fg", &term_fg), ("term_bg_alt", &term_bg_alt), ("accent", &accent), ("accent_dim", &accent_dim), ("on_accent", &on_accent), ("secondary", &secondary), ("secondary_dim", &secondary_dim), ("sel_bg", &sel_bg), ("sel_fg", &sel_fg)] {
        put(k, v);
    }
    for (k, v) in &fills {
        put(k, v);
    }
    put("accent_ink", &accent_ink);
    put("secondary_ink", &secondary_ink);
    for (k, v) in &inks {
        put(k, v);
    }
    for (k, v) in [("titlebar_bg", &surface), ("titlebar_fg", &accent_ink), ("titlebar_inactive_bg", &bg), ("titlebar_inactive_fg", &fg_muted), ("desktop", &paper), ("desktop_grid", &paper_shade), ("cursor", &cursor), ("cursor_text", &cursor_text)] {
        put(k, v);
    }
    for (k, v) in &ansi {
        put(k, v);
    }
    let urgent_fill = fills[0].1.clone();

    let tone = get("tone");
    let label = format!("{}{} {}", tone[..1].to_uppercase(), &tone[1..], get("accent"));
    let mut choices_map = Map::new();
    for (k, v) in &ch {
        choices_map.insert(k.clone(), json!(v));
    }
    let Value::Object(t) = json!({
        "label": label, "paper": paper, "paper_light": paper_light, "paper_shade": paper_shade, "glow": glow,
        "ink": ink, "alert": alert, "strong": strong,
        "sheet": {"angle": 54, "midpoint": 0.45, "blend": 1.0, "shading": 1.0,
                  "glow_strength": if light_paper { 0.6 } else { 0.5 }, "glow_size": 1.0, "pattern_strength": 0.5},
        "windows": {"focused": focused, "focused_inactive": surface_alt, "unfocused": border, "urgent": urgent_fill,
                    "text": fg, "text_inactive": fg_muted, "background": bg},
        "apps": apps, "choices": choices_map,
    }) else { unreachable!() };
    t
}

/// The theme as a file, the choices written at the top.
pub fn toml(t: &Map<String, Value>) -> String {
    let q = |v: &Value| match v {
        Value::String(s) => format!("\"{s}\""),
        Value::Number(n) if n.is_i64() => n.to_string(),
        Value::Number(n) => util::float(n.as_f64().unwrap_or(0.0)),
        Value::Bool(b) => if *b { "True".into() } else { "False".into() },
        v => v.to_string(),
    };
    let c = t["choices"].as_object().unwrap();
    let mut lines = vec![
        format!("# Built by the settings tool from [scheme] in config.toml: {}.", c.iter().map(|(k, v)| format!("{k} {}", v.as_str().unwrap_or(""))).collect::<Vec<_>>().join(", ")),
        "# Rewritten whenever the choices change; to keep a scheme of your own, copy this file to another name.".into(),
        format!("label = \"{}\"", t["label"].as_str().unwrap_or("")),
        String::new(),
    ];
    for k in ["paper", "paper_light", "paper_shade", "glow", "ink", "alert", "strong"] {
        lines.push(format!("{k:<11} = {}", q(&t[k])));
    }
    for section in ["sheet", "windows", "apps"] {
        lines.push(String::new());
        lines.push(format!("[{section}]"));
        for (k, v) in t[section].as_object().unwrap() {
            lines.push(format!("{k:<20} = {}", q(v)));
        }
    }
    lines.join("\n") + "\n"
}

/// What the settings letter under a scheme: the main contrasts, and how many words fall below the rules.
pub fn figures(t: &Map<String, Value>) -> Value {
    let s = |v: &Value| v.as_str().unwrap_or("#000000").to_string();
    let mut out = Map::new();
    out.insert("ink".into(), json!(util::round_to(contrast(&s(&t["ink"]), &s(&t["paper"])), 1)));
    if let Some(a) = t.get("apps").and_then(Value::as_object).filter(|a| !a.is_empty()) {
        out.insert("text".into(), json!(util::round_to(contrast(&s(&a["fg"]), &s(&a["bg"])), 1)));
        out.insert("terminal".into(), json!(util::round_to(contrast(&s(&a["term_fg"]), &s(&a["term_bg"])), 1)));
    }
    out.insert("below".into(), json!(check(t).iter().filter(|c| c.3).count()));
    Value::Object(out)
}

// ---- the rules, for any theme ----
/// The ways theme `t` falls below the rules: (what, ratio, minimum, hard). The rules for words (4.5:1,
/// dimmed 3:1) are the research's and hard; the border's is advice (WCAG's 3:1 for parts of an
/// interface; the research does not cover it).
pub fn check(t: &Map<String, Value>) -> Vec<(String, f64, f64, bool)> {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("#000000").to_string();
    let w = t.get("windows");
    let mut tests: Vec<(String, String, String, f64)> = vec![
        ("sheet ink on the paper".into(), s(t.get("ink")), s(t.get("paper")), WORDS),
        ("sheet ink on the lit paper".into(), s(t.get("ink")), s(t.get("paper_light")), WORDS),
        ("sheet ink on the shaded paper".into(), s(t.get("ink")), s(t.get("paper_shade")), DIM),
        ("alert on the paper".into(), s(t.get("alert")), s(t.get("paper")), WORDS),
        ("focused border on its window".into(), s(w.and_then(|w| w.get("focused"))), s(w.and_then(|w| w.get("background"))), BORDER),
    ];
    if let Some(a) = t.get("apps").and_then(Value::as_object).filter(|a| !a.is_empty()) {
        let g = |k: &str| s(a.get(k));
        // the terminal's own background decides which end counts as background, not the variant
        let dark = luminance(&g("term_bg")) <= 0.18;
        tests.push(("text".into(), g("fg"), g("bg"), WORDS));
        tests.push(("dimmed text".into(), g("fg_muted"), g("bg"), DIM));
        tests.push(("terminal text".into(), g("term_fg"), g("term_bg"), WORDS));
        tests.push(("selection".into(), g("sel_fg"), g("sel_bg"), WORDS));
        for k in ["accent_ink", "secondary_ink", "ok_ink", "warn_ink", "urgent_ink", "info_ink"] {
            tests.push((k.into(), g(k), g("bg"), WORDS));
        }
        let exempt: &[&str] = if dark { &["black"] } else { &["white", "br_white"] };
        for p in ["", "br_"] {
            for k in ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"] {
                let key = format!("{p}{k}");
                if !exempt.contains(&key.as_str()) {
                    tests.push((format!("terminal {}", key.replace("br_", "bright ")), g(&key), g("term_bg"), WORDS));
                }
            }
        }
    }
    tests
        .into_iter()
        .filter_map(|(what, fg, bg, minimum)| {
            let r = contrast(&fg, &bg);
            (r < minimum - 0.005).then(|| {
                let hard = !what.contains("border");
                (what, r, minimum, hard)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every(axis: &str, values: &[&str], f: impl Fn(&Map<String, Value>)) {
        for v in values {
            f(&build(&[(axis.to_string(), v.to_string())]));
        }
    }

    /// Every value of every choice, on both sides (dark and light windows, light and dark paper), keeps
    /// every word readable. Bounded: one choice varied at a time over the four sides, not the product.
    #[test]
    fn every_choice_reads() {
        for (paper, windows) in [("light", "dark"), ("light", "light"), ("dark", "dark"), ("dark", "light")] {
            for contrast in ["standard", "high"] {
                for (axis, values) in choices() {
                    for v in values {
                        let given = [("paper", paper), ("windows", windows), ("contrast", contrast), (axis, v)]
                            .map(|(k, v)| (k.to_string(), v.to_string()));
                        let t = build(&given);
                        let hard: Vec<_> = check(&t).into_iter().filter(|c| c.3).collect();
                        assert!(hard.is_empty(), "{paper} paper, {windows} windows, {contrast}, {axis}={v}: {hard:?}");
                    }
                }
            }
        }
        every("terminal", &["charcoal", "plum"], |t| assert!(luminance(t["apps"]["term_bg"].as_str().unwrap()) > 0.02, "a mid tone, not near black"));
    }

    #[test]
    fn known_scheme_is_stable() {
        // the default scheme, as the Python version built it
        let t = build(&[]);
        assert_eq!(t["paper"], "#DACFC3");
        assert_eq!(t["apps"]["term_bg"], "#1E1A15");
        assert_eq!(t["label"], "Greige amber");
    }
}
