//! The sheet's geometry, as the compositor works it out: the client never computes any of it. One
//! request on the compositor's socket (`{"request": "outputs"}`, compositor/src/ipc.rs) gives every
//! display its frame, its margin, its zones and the pixels a millimetre comes to, all in that display's
//! logical pixels.
//!
//! Stage 1 subscribes to the socket instead, so a change arrives on its own; stage 0 asks again whenever
//! Wayland says the displays have changed, which is what moves the sheet.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use serde_json::Value;

use crate::paint::Ink;

/// One display's sheet, in that display's logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    /// the display, from the whole layout's corner
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    /// logical pixels a millimetre
    pub mm: f64,
    /// the margin, from the display's edge to the frame's line
    pub margin: f64,
    /// the frame's interior, from the display's corner
    pub frame: (f64, f64, f64, f64),
    /// the title block and its twelve cells, from the display's corner
    pub title: Option<Block>,
    /// the workspace this display shows
    pub workspace: u64,
}

/// The title block: where it is, and where each of its cells is, all from the display's corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub rect: (f64, f64, f64, f64),
    pub cells: Vec<(f64, f64, f64, f64)>,
}

impl Screen {
    /// `k` millimetres from the frame's corner, across or down: the display's logical pixels, put on the
    /// nearest physical pixel — the sheet's invariant (qml/sheet.js `at`, docs/wayland-design.md).
    pub fn at(&self, k: f64, across: bool) -> f64 {
        let from = if across { self.frame.0 } else { self.frame.1 };
        ((from + k * self.mm) * self.scale).round() / self.scale
    }
}

/// Every display's sheet, asked of the compositor; empty where it cannot be reached.
pub fn screens() -> Vec<Screen> {
    let Some(reply) = ask("{\"request\": \"outputs\"}") else { return Vec::new() };
    let Some(outputs) = reply.get("reply").and_then(Value::as_array) else { return Vec::new() };
    outputs
        .iter()
        .filter_map(|o| {
            let n = |k: &str| o.get(k)?.as_f64();
            let frame = o.get("frame")?;
            let f = |k: &str| frame.get(k)?.as_f64();
            Some(Screen {
                name: o.get("name")?.as_str()?.to_string(),
                width: n("width")?,
                height: n("height")?,
                scale: n("scale").filter(|s| *s > 0.0)?,
                mm: n("px_per_mm").filter(|m| *m > 0.0)?,
                margin: n("margin")?,
                // the frame is given from the layout's corner; the sheet is drawn from the display's
                frame: (f("x")? - n("x")?, f("y")? - n("y")?, f("width")?, f("height")?),
                title: block(o.get("title"), n("x")?, n("y")?),
                workspace: o.get("workspace").and_then(Value::as_u64).unwrap_or(1),
            })
        })
        .collect()
}

/// The title block as the compositor measured it, moved to the display's own corner.
fn block(value: Option<&Value>, ox: f64, oy: f64) -> Option<Block> {
    let v = value?;
    let rect = |r: &Value| {
        let g = |k: &str| r.get(k)?.as_f64();
        Some((g("x")? - ox, g("y")? - oy, g("width")?, g("height")?))
    };
    let cells: Vec<(f64, f64, f64, f64)> = v.get("cells")?.as_array()?.iter().filter_map(rect).collect();
    let whole = rect(v.get("rect")?)?;
    (!cells.is_empty()).then_some(Block { rect: whole, cells })
}

/// The theme the sheet is lettered in: what the shell reads from the same file (qml/Theme.qml).
#[derive(Debug, Clone)]
pub struct Theme {
    pub ink: Ink,
    pub alert: Ink,
    /// the family the lettering asks for, where the config names one
    pub mono: Option<String>,
    pub project: String,
    pub drawn: String,
}

impl Theme {
    /// From the cache the `draftpaper` command writes it to, as the shell reads it.
    pub fn load() -> Theme {
        let cache = std::env::var_os("DRAFTPAPER_CACHE").map(std::path::PathBuf::from).or_else(|| {
            let home = std::env::var_os("HOME")?;
            Some(std::path::PathBuf::from(home).join(".cache/draftpaper"))
        });
        let text = cache.map(|c| std::fs::read_to_string(c.join("theme.json")).unwrap_or_default()).unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        // the lettering is softened by the theme's own measure of how strongly ink sits on the paper
        let alpha = v.get("inkAlpha").and_then(Value::as_f64).unwrap_or(1.0) as f32;
        let s = |a: &str, b: &str| v.get(a).and_then(|x| x.get(b)).and_then(Value::as_str).unwrap_or("").to_string();
        Theme {
            ink: Ink::parse(v.get("ink").and_then(Value::as_str).unwrap_or("#000000"), 1.0),
            alert: Ink::parse(v.get("alert").and_then(Value::as_str).unwrap_or("#aa0000"), 1.0),
            mono: Some(s("text", "monospace")).filter(|m| !m.is_empty()),
            project: s("title", "project"),
            drawn: s("title", "drawn"),
        }
        .with_alpha(alpha)
    }

    fn with_alpha(mut self, alpha: f32) -> Theme {
        // as the title block mixes them (qml/TitleBlock.qml): the readings nearly full strength, their
        // labels fainter, and never quite invisible however pale the theme
        self.ink.a = (0.85 * alpha + 0.1).min(1.0);
        self.alert.a = self.ink.a;
        self
    }

    /// The ink a label is lettered in: fainter than the reading it names.
    pub fn faint(&self) -> Ink {
        Ink { a: (self.ink.a * 0.72).min(1.0), ..self.ink }
    }
}

/// One request on the compositor's socket, one reply.
fn ask(request: &str) -> Option<Value> {
    let path = std::env::var_os("DRAFTPAPER_SOCKET")?;
    let mut socket = UnixStream::connect(path).ok()?;
    let timeout = Some(std::time::Duration::from_secs(2));
    socket.set_read_timeout(timeout).ok()?;
    socket.set_write_timeout(timeout).ok()?;
    socket.write_all(format!("{request}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(scale: f64, mm: f64, frame_x: f64) -> Screen {
        Screen { name: "test".into(), width: 1920.0, height: 1200.0, scale, mm, margin: 38.0, frame: (frame_x, frame_x, 1000.0, 800.0), title: None, workspace: 1 }
    }

    #[test]
    fn a_millimetre_lands_on_a_physical_pixel() {
        // whatever the scale, what `at` gives is a whole physical pixel: the invariant the sheet rests on
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let s = screen(scale, 6.4 / scale, 30.4);
            for k in 0..200 {
                let v = s.at(k as f64, true) * scale;
                assert!((v - v.round()).abs() < 1e-9, "scale {scale}, {k} mm landed at {v}");
            }
        }
    }

    #[test]
    fn millimetres_keep_their_distance() {
        // 50 mm from the corner is 50 mm away, to within half a pixel: it is the rounding of one edge,
        // never a drift that grows with the distance
        let s = screen(1.25, 5.12, 30.4);
        for k in 0..200 {
            let want = s.frame.0 + k as f64 * s.mm;
            assert!((s.at(k as f64, true) - want).abs() <= 0.5 / s.scale + 1e-9, "{k} mm drifted");
        }
    }
}
