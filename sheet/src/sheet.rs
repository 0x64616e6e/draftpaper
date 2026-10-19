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
    /// the zones the margins are ticked into
    pub across: u32,
    pub down: u32,
    /// the parking cells, the left margin's from the top then the right's
    pub parking: Vec<(f64, f64, f64, f64)>,
    /// the scale bar's rules and the rows left above them for its figures
    pub bar: Option<Bar>,
    /// the schedule this screen carries, and the reading it plots, as the compositor keeps them
    pub schedule: String,
    pub graph: String,
}

/// The scale bar the compositor draws: each rule's place and what millimetre it stands for, and the band
/// the figures go in. All from the display's corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub marks: Vec<(f64, u32)>,
    pub text_top: f64,
    pub text_height: f64,
    pub measured: bool,
    pub zone: (f64, f64, f64, f64),
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
    let Some(reply) = request_reply("{\"request\": \"outputs\"}") else { return Vec::new() };
    parse(reply.get("reply").unwrap_or(&Value::Null))
}

/// The compositor's answer, read. The frame and the zones are given from the whole layout's corner, and
/// the title block, the scale bar and the parking cells from each display's own: a display whose corner
/// is not the layout's shows up the difference, and taking one for the other put everything placed by
/// the title block off the left edge of every display but the first.
pub fn parse(outputs: &Value) -> Vec<Screen> {
    let Some(outputs) = outputs.as_array() else { return Vec::new() };
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
                // the title block and the scale bar come already measured from this display's own corner
                // (compositor/src/ipc.rs); only the frame and the zones are given from the whole layout's
                title: block(o.get("title")),
                workspace: o.get("workspace").and_then(Value::as_u64).unwrap_or(1),
                across: o.get("zones").and_then(|z| z.get("across")).and_then(Value::as_u64).unwrap_or(6) as u32,
                down: o.get("zones").and_then(|z| z.get("down")).and_then(Value::as_u64).unwrap_or(4) as u32,
                parking: o
                    .get("parking")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|r| Some((r.get("x")?.as_f64()?, r.get("y")?.as_f64()?, r.get("width")?.as_f64()?, r.get("height")?.as_f64()?))).collect())
                    .unwrap_or_default(),
                bar: bar(o.get("scale_bar")),
                schedule: o.get("schedule").and_then(Value::as_str).unwrap_or("").to_string(),
                graph: o.get("graph").and_then(Value::as_str).unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// The title block as the compositor measured it, from this display's own corner.
fn block(value: Option<&Value>) -> Option<Block> {
    let v = value?;
    let rect = |r: &Value| {
        let g = |k: &str| r.get(k)?.as_f64();
        Some((g("x")?, g("y")?, g("width")?, g("height")?))
    };
    let cells: Vec<(f64, f64, f64, f64)> = v.get("cells")?.as_array()?.iter().filter_map(rect).collect();
    let whole = rect(v.get("rect")?)?;
    (!cells.is_empty()).then_some(Block { rect: whole, cells })
}

/// The scale bar, moved to the display's own corner; None where the display reports no size and nothing
/// on the sheet is to scale.
fn bar(value: Option<&Value>) -> Option<Bar> {
    let v = value?;
    let zone = v.get("zone")?;
    let g = |r: &Value, k: &str| r.get(k)?.as_f64();
    let marks = v
        .get("marks")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|m| Some((m.get("x")?.as_f64()?, m.get("mm")?.as_u64()? as u32))).collect())
        .unwrap_or_default();
    Some(Bar {
        marks,
        text_top: v.get("text").and_then(|t| g(t, "y")).unwrap_or(0.0),
        text_height: v.get("text").and_then(|t| g(t, "height")).unwrap_or(0.0),
        measured: v.get("measured").and_then(Value::as_bool).unwrap_or(false),
        zone: (g(zone, "x")?, g(zone, "y")?, g(zone, "width")?, g(zone, "height")?),
    })
}

/// The workspaces of one display, and the windows parked on the one it shows.
pub fn workspaces() -> Vec<(u64, bool, bool, String)> {
    let Some(reply) = request_reply("{\"request\": \"workspaces\"}") else { return Vec::new() };
    reply
        .get("reply")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    Some((
                        w.get("number")?.as_u64()?,
                        w.get("focused")?.as_bool()?,
                        w.get("shown")?.as_bool()?,
                        w.get("output")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every window parked on `workspace`: which cell it is in, what to letter up the margin, and the id to
/// call it back by.
pub fn parked(workspace: u64) -> Vec<(usize, String, u64)> {
    let Some(reply) = request_reply("{\"request\": \"windows\"}") else { return Vec::new() };
    reply
        .get("reply")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|w| w.get("parked").and_then(Value::as_bool).unwrap_or(false))
                .filter(|w| w.get("workspace").and_then(Value::as_u64) == Some(workspace))
                .filter_map(|w| {
                    let cell = w.get("cell")?.as_u64()? as usize;
                    let title = w.get("title").and_then(Value::as_str).filter(|t| !t.is_empty());
                    let name = title.or_else(|| w.get("app_id").and_then(Value::as_str)).unwrap_or("");
                    Some((cell, name.to_string(), w.get("id")?.as_u64()?))
                })
                .collect()
        })
        .unwrap_or_default()
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
    /// the theme's own name, lettered at the foot of the sheet
    pub label: String,
    /// the accent, for the mark under the pointer
    pub accent: Ink,
    /// what the theme lights: the workspace in use along the top margin (qml/shell.qml's `strong`)
    pub strong: Ink,
    /// the paper, for the lettering a drawing breaks its lines to make room for
    pub paper: Ink,
    /// whether the sheet sets a glyph before a reading where it can (config [text] glyphs)
    pub glyphs: bool,
    /// how strongly the theme sets ink on paper: what is lettered is lettered in full, and only the
    /// labels, the rules and the ticks are softened by it (qml/TitleBlock.qml)
    pub ink_alpha: f32,
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
            label: v.get("label").and_then(Value::as_str).unwrap_or("").to_string(),
            accent: Ink::parse(v.get("accent").and_then(Value::as_str).unwrap_or("#b4701a"), 1.0),
            strong: Ink::parse(v.get("strong").and_then(Value::as_str).or_else(|| v.get("accent").and_then(Value::as_str)).unwrap_or("#b4701a"), 1.0),
            paper: Ink::parse(v.get("paper").and_then(Value::as_str).unwrap_or("#ffffff"), 1.0),
            ink_alpha: alpha,
            glyphs: v.get("glyphs").and_then(Value::as_bool).unwrap_or(true),
        }
    }

    /// The ink a label is lettered in: fainter than the reading it names, and never quite invisible
    /// however pale the theme (qml/TitleBlock.qml's `faint`).
    pub fn faint(&self) -> Ink {
        Ink { a: (0.6 * self.ink_alpha + 0.1).min(1.0), ..self.ink }
    }

    /// The ink a rule is drawn in, between the two.
    pub fn line(&self) -> Ink {
        Ink { a: (0.85 * self.ink_alpha + 0.1).min(1.0), ..self.ink }
    }
}

/// Watch the compositor and say when the sheet, the workspaces or the windows change, so the margins
/// follow a workspace switch at once rather than at the next reading. Ends quietly where the socket does.
pub fn watch(say: std::sync::mpsc::Sender<()>) {
    let Some(path) = std::env::var_os("DRAFTPAPER_SOCKET") else { return };
    let Ok(mut socket) = UnixStream::connect(path) else { return };
    if socket.write_all(b"{\"request\": \"subscribe\", \"events\": [\"outputs\", \"workspaces\", \"windows\"]}\n").is_err() {
        return;
    }
    for line in BufReader::new(socket).lines().map_while(Result::ok) {
        // which of them changed does not matter: the sheet is read again whole, which is a few
        // small reads of a socket
        if line.contains("\"event\"") && say.send(()).is_err() {
            return;
        }
    }
}

/// What the sheet shows for a moment after it changes: the volume, as `draftpaper volume` last left it,
/// and the backlight, read straight from the kernel. Both are plain files, read in place.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Levels {
    pub volume: Option<f64>,
    pub muted: bool,
    /// when the volume was last moved, in seconds since the epoch
    pub volume_at: u64,
    pub brightness: Option<f64>,
}

impl Levels {
    pub fn read(backlight: Option<&str>) -> Levels {
        let runtime = std::env::var_os("DRAFTPAPER_RUNTIME")
            .map(std::path::PathBuf::from)
            .or_else(|| Some(std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("draftpaper")));
        let v: Value = runtime
            .map(|r| std::fs::read_to_string(r.join("volume.json")).unwrap_or_default())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let number = |dir: &str, file: &str| std::fs::read_to_string(format!("{dir}/{file}")).ok()?.trim().parse::<f64>().ok();
        let brightness = backlight.and_then(|d| {
            let (now, max) = (number(d, "brightness")?, number(d, "max_brightness")?);
            (max > 0.0).then_some(now / max)
        });
        Levels {
            volume: v.get("volume").and_then(Value::as_f64),
            muted: v.get("muted").and_then(Value::as_bool).unwrap_or(false),
            volume_at: v.get("at").and_then(Value::as_u64).unwrap_or(0),
            brightness,
        }
    }
}

/// Do one of the compositor's own actions, as a key binding would (compositor/src/config.rs).
pub fn action(what: &str) {
    ask(&serde_json::json!({ "request": "action", "action": what }).to_string());
}

/// Tell the compositor which stretches of the side margins carry lettering, so its hatching of a parked
/// cell breaks around the words, as a drawing's hatch does. Spans are along the margin, in the display's
/// logical pixels.
pub fn set_clear(output: &str, left: &[(f64, f64)], right: &[(f64, f64)]) {
    let list = |spans: &[(f64, f64)]| spans.iter().map(|(a, b)| serde_json::json!([a, b])).collect::<Vec<_>>();
    let request = serde_json::json!({ "request": "sheet", "output": output, "clear": { "left": list(left), "right": list(right) } });
    let _ = ask(&request.to_string());
}

/// One request on the compositor's socket, when the reply does not matter.
pub fn ask(request: &str) {
    let _ = request_reply(request);
}

/// One request on the compositor's socket, one reply.
fn request_reply(request: &str) -> Option<Value> {
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

    /// Two displays side by side, as the compositor answers: the second's corner is not the layout's.
    fn two() -> Value {
        serde_json::json!([
            {
                "name": "one", "x": 0, "y": 0, "width": 1280, "height": 800, "scale": 1.5, "px_per_mm": 4.3, "margin": 26.0,
                "frame": { "x": 26.0, "y": 26.0, "width": 1228.0, "height": 748.0 },
                "zones": { "across": 6, "down": 4 }, "workspace": 1,
                "title": { "rect": { "x": 700.0, "y": 600.0, "width": 540.0, "height": 150.0 }, "cells": [{ "x": 700.0, "y": 600.0, "width": 100.0, "height": 60.0 }] },
                "scale_bar": { "zone": { "x": 900.0, "y": 760.0, "width": 200.0, "height": 26.0 }, "marks": [{ "x": 910.0, "mm": 0 }], "text": { "y": 752.0, "height": 10.0 }, "measured": true },
                "parking": [{ "x": 0.0, "y": 26.0, "width": 26.0, "height": 187.0 }]
            },
            {
                "name": "two", "x": 1280, "y": 0, "width": 1706, "height": 960, "scale": 1.5, "px_per_mm": 3.5, "margin": 21.0,
                "frame": { "x": 1301.0, "y": 21.0, "width": 1664.0, "height": 918.0 },
                "zones": { "across": 8, "down": 5 }, "workspace": 2,
                "title": { "rect": { "x": 1150.0, "y": 780.0, "width": 535.0, "height": 150.0 }, "cells": [{ "x": 1150.0, "y": 780.0, "width": 100.0, "height": 60.0 }] },
                "scale_bar": { "zone": { "x": 1400.0, "y": 920.0, "width": 200.0, "height": 21.0 }, "marks": [{ "x": 1410.0, "mm": 0 }], "text": { "y": 912.0, "height": 10.0 }, "measured": true },
                "parking": [{ "x": 0.0, "y": 21.0, "width": 21.0, "height": 180.0 }]
            }
        ])
    }

    #[test]
    fn the_frame_is_moved_to_its_display_and_the_title_block_is_left_alone() {
        let screens = parse(&two());
        assert_eq!(screens.len(), 2);
        // the frame comes from the whole layout's corner, so it is moved to the display's
        assert_eq!(screens[0].frame.0, 26.0);
        assert_eq!(screens[1].frame.0, 21.0, "the second display's frame was not brought to its own corner");
        // the title block already comes from the display's own corner: moving it again put the schedule
        // and the graph off the left edge of every display but the first
        let block = screens[1].title.as_ref().expect("a title block");
        assert_eq!(block.rect.0, 1150.0, "the second display's title block was moved twice");
        assert_eq!(block.cells[0].0, 1150.0);
        assert!(block.rect.0 > 0.0 && block.rect.0 < screens[1].width, "its title block is off the display");
        // and so does the scale bar
        let bar = screens[1].bar.as_ref().expect("a scale bar");
        assert_eq!(bar.marks[0].0, 1410.0, "the second display's scale bar was moved twice");
        assert!(bar.zone.0 > 0.0 && bar.zone.0 < screens[1].width);
    }

    #[test]
    fn each_display_keeps_its_own_measure() {
        let screens = parse(&two());
        assert_eq!((screens[0].across, screens[0].down), (6, 4));
        assert_eq!((screens[1].across, screens[1].down), (8, 5));
        assert_eq!(screens[1].workspace, 2);
        assert_eq!(screens[1].parking.len(), 1);
    }

    fn screen(scale: f64, mm: f64, frame_x: f64) -> Screen {
        Screen { name: "test".into(), width: 1920.0, height: 1200.0, scale, mm, margin: 38.0, frame: (frame_x, frame_x, 1000.0, 800.0), title: None, workspace: 1, across: 6, down: 4, parking: Vec::new(), bar: None, schedule: String::new(), graph: String::new() }
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
