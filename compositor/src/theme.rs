//! The theme and settings, from the same files as the i3 edition: the theme name and the margin from
//! ~/.config/draftpaper/config.toml, the theme from ~/.config/draftpaper/themes/<name>.toml, the
//! repository's themes/ (when run from a checkout) or /usr/share/draftpaper/themes.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Sheet {
    pub angle: f32,
    pub midpoint: f32,
    pub blend: f32,
    pub shading: f32,
    pub glow_strength: f32,
    pub glow_size: f32,
    pub pattern_strength: f32,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub paper: [f32; 3],
    pub paper_light: [f32; 3],
    pub paper_shade: [f32; 3],
    pub glow: [f32; 3],
    pub ink: [f32; 3],
    pub sheet: Sheet,
    /// millimetres between the screen edge and the frame
    pub margin_mm: f32,
    /// window borders: the focused window, the others ([windows] focused / unfocused)
    pub border_focused: [f32; 3],
    pub border: [f32; 3],
    /// a picture in the frame's interior (config.toml `wallpaper`), with the margins the theme's paper or
    /// the picture's ambient colours (`margins = "paper" | "ambient"`) and the title block frosted, its
    /// blur tinted with the paper by `title_tint` (wallpaper.rs)
    pub wallpaper: Option<PathBuf>,
    pub ambient: bool,
    pub title_tint: f32,
}

fn rgb(hex: &str) -> Option<[f32; 3]> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

fn read(path: &Path) -> Option<toml::Table> {
    std::fs::read_to_string(path).ok()?.parse::<toml::Table>().ok()
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![home().join(".config/draftpaper/themes")];
    // a checkout: compositor/ sits next to themes/
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../themes"));
    dirs.push(PathBuf::from("/usr/share/draftpaper/themes"));
    dirs
}

impl Theme {
    /// The configured theme, or Soft Glow's values when nothing can be read.
    pub fn load() -> Theme {
        let config = read(&home().join(".config/draftpaper/config.toml")).unwrap_or_default();
        let name = config.get("theme").and_then(|v| v.as_str()).unwrap_or("softglow").to_string();
        let margin_mm = config
            .get("margin")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .unwrap_or(6.0)
            .clamp(3.0, 20.0) as f32;
        let table = theme_dirs().into_iter().find_map(|d| read(&d.join(format!("{name}.toml")))).unwrap_or_default();
        let colour = |key: &str, fallback: &str| {
            table.get(key).and_then(|v| v.as_str()).and_then(rgb).unwrap_or_else(|| rgb(fallback).unwrap())
        };
        let s = table.get("sheet").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let num = |key: &str, fallback: f32| {
            s.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(|f| f as f32).unwrap_or(fallback)
        };
        let paper_shade = colour("paper_shade", "#8C7C6C");
        Theme {
            name,
            paper: colour("paper", "#C9BDAE"),
            paper_light: colour("paper_light", "#DAD0C5"),
            paper_shade,
            glow: colour("glow", "#F6CFA0"),
            // as the draftpaper command does: without an ink, a darker paper_shade
            ink: table.get("ink").and_then(|v| v.as_str()).and_then(rgb).unwrap_or([paper_shade[0] * 0.7, paper_shade[1] * 0.7, paper_shade[2] * 0.7]),
            sheet: Sheet {
                angle: num("angle", 54.0),
                midpoint: num("midpoint", 0.45),
                blend: num("blend", 1.0),
                shading: num("shading", 1.0),
                glow_strength: num("glow_strength", 1.0),
                glow_size: num("glow_size", 1.0),
                pattern_strength: num("pattern_strength", 0.5),
            },
            margin_mm,
            border_focused: table.get("windows").and_then(|w| w.get("focused")).and_then(|v| v.as_str()).and_then(rgb).unwrap_or_else(|| rgb("#E8B374").unwrap()),
            border: table.get("windows").and_then(|w| w.get("unfocused")).and_then(|v| v.as_str()).and_then(rgb).unwrap_or_else(|| rgb("#454543").unwrap()),
            wallpaper: config.get("wallpaper").and_then(|v| v.as_str()).filter(|p| !p.is_empty()).map(|p| {
                p.strip_prefix("~/").map(|rest| home().join(rest)).unwrap_or_else(|| PathBuf::from(p))
            }),
            ambient: config.get("margins").and_then(|v| v.as_str()) == Some("ambient"),
            title_tint: config.get("title_tint").and_then(|v| v.as_float()).unwrap_or(0.7).clamp(0.0, 1.0) as f32,
        }
    }
}

/// Per-output settings from config.toml, `[outputs.<name>]`: `scale` (1, 1.5, 2...), `position` (logical
/// x and y, `[x, y]`), and `mm` (physical pixels per millimetre, for panels that report a wrong size).
#[derive(Debug, Clone, Default)]
pub struct OutputSettings {
    pub scale: Option<f64>,
    pub position: Option<(i32, i32)>,
    pub mm: Option<f32>,
}

pub fn output_settings(name: &str) -> OutputSettings {
    let config = read(&home().join(".config/draftpaper/config.toml")).unwrap_or_default();
    let Some(o) = config.get("outputs").and_then(|v| v.get(name)).and_then(|v| v.as_table()) else {
        return OutputSettings::default();
    };
    let num = |v: &toml::Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
    OutputSettings {
        scale: o.get("scale").and_then(num).filter(|s| *s >= 0.5 && *s <= 4.0),
        position: o.get("position").and_then(|v| v.as_array()).and_then(|a| {
            Some((a.first().and_then(num)? as i32, a.get(1).and_then(num)? as i32))
        }),
        mm: o.get("mm").and_then(num).map(|m| m as f32),
    }
}
