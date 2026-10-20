//! What the sheet's four margins carry, as a drawing letters the edges of its frame: the workspaces along
//! the top, the columns numbered from the left, a letter at the head of every row down the sides with the
//! title of whatever window is parked there running up the margin, and along the foot the scale bar's
//! figures and the name of the theme the sheet is drawn in.
//!
//! The zones are the compositor's: it ticks the margins into them and publishes how many there are, so
//! each is `length / zones` along the band, the top and bottom beginning a margin in from the display's
//! corner and the sides running the whole way down. This replaces qml/Zone.qml, qml/ZoneNumbers.qml and
//! qml/SideCells.qml, and letters what they lettered, where they lettered it.
//!
//! The notifications, the volume and brightness, the tray and Bluetooth are not here: they are what the
//! daemons say, and they come with stage 4 (docs/sheet-client.md).

use serde_json::Value;

use crate::font::Face;
use crate::paint::Canvas;
use crate::sheet::{Screen, Theme};
use crate::Side;

/// The letters a drawing numbers its rows with: I and O are left out, being too like 1 and 0.
const ROWS: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";

/// One margin's band: where its zones begin along it, and how long each is, in logical pixels.
struct Band {
    /// along the band from the display's corner
    offset: f64,
    length: f64,
    zones: u32,
    /// across the band: the margin's own thickness
    thick: f64,
    vertical: bool,
}

impl Band {
    fn of(s: &Screen, side: Side) -> Band {
        let thick = s.margin;
        if side.vertical() {
            Band { offset: thick, length: s.height - 2.0 * thick, zones: s.down, thick, vertical: true }
        } else {
            Band { offset: thick, length: s.width - 2.0 * thick, zones: s.across, thick, vertical: false }
        }
    }

    /// A place along the band, in the surface's own coordinates. The top and bottom bands run the whole
    /// width of the display, so a place on the display is a place in the surface; the sides begin a
    /// margin down, where the top band ends.
    fn here(&self, v: f64) -> f64 {
        if self.vertical {
            v - self.offset
        } else {
            v
        }
    }

    /// Where zone `k` begins and ends along the band.
    fn zone(&self, k: u32) -> (f64, f64) {
        let each = self.length / self.zones.max(1) as f64;
        let start = self.offset + k as f64 * each;
        (start, start + each)
    }
}

/// Letter one margin. `along` turns a place along the band and across it into the buffer's own physical
/// pixels, the band having been drawn from its own corner.
/// Letter one margin from the document (docs/sheet-content.md). `doc` is what the sheet says on this
/// display; `told` is the little that cannot be in it — the notifications, which belong to whoever took
/// the name on the bus, and the levels lettered for a few seconds after they move.
pub fn letter(canvas: &mut Canvas, face: &mut Face, s: &Screen, theme: &Theme, side: Side, scale: f64, doc: &Value, parked: &[(usize, String, u64)], hover: Option<(f64, f64)>, told: &Told) {
    let band = Band::of(s, side);
    let mm = (s.mm * scale) as f32;
    let px = ((s.margin * 0.5).min(3.2 * s.mm) * scale) as f32;
    let ink = theme.ink;
    let soft = crate::paint::Ink { a: (ink.a * 0.8).min(1.0), ..ink };
    let empty = Vec::new();
    let pick = |face: &mut Face, e: &Value| crate::glyph::pick(face, theme.glyphs, e);
    match side {
        Side::Top => {
            // the workspaces share the first zone, each a slot of it, the one in use in the strong ink
            let (start, end) = band.zone(0);
            let shown = doc["top"]["workspaces"].as_array().unwrap_or(&empty);
            let slot = (end - start) / shown.len().max(1) as f64;
            for (i, space) in shown.iter().enumerate() {
                let at = band.here(start + (i as f64 + 0.5) * slot);
                let text = pick(face, space);
                let w = face.width(&text, px, 0.0);
                // the one in use is lit in whatever the theme lights things with, as the shell lights it
                let colour = if space["focused"] == true { theme.strong } else { soft };
                face.draw(canvas, (at * scale) as f32 - w / 2.0, middle(&band, scale, face, px), &text, px, 0.0, colour.premultiplied());
            }
            // the newest notification, in the zone before the last, with how many wait behind it
            if let Some(text) = told.notices.last() {
                let (start, end) = band.zone(band.zones.saturating_sub(2));
                let pad = 3.0 * mm;
                let waiting = if told.notices.len() > 1 { format!("+{}", told.notices.len() - 1) } else { String::new() };
                let counted = if waiting.is_empty() { 0.0 } else { face.width(&waiting, px * 0.8, 0.0) + pad };
                let room = ((end - start) * scale) as f32 - 2.0 * pad - counted;
                let line = fit(face, text, px, room);
                let at = (band.here(start) * scale) as f32 + pad;
                face.draw(canvas, at, middle(&band, scale, face, px), &line, px, 0.0, ink.premultiplied());
                if !waiting.is_empty() {
                    let right = (band.here(end) * scale) as f32 - pad - face.width(&waiting, px * 0.8, 0.0);
                    face.draw(canvas, right, middle(&band, scale, face, px * 0.8), &waiting, px * 0.8, 0.0, soft.premultiplied());
                }
            }
            // the last zone: what wants saying now — a reading past its alarm, the volume or the
            // brightness just moved. The tray is the shell's still (docs/sheet-client.md).
            let mut row: Vec<(String, crate::paint::Ink)> = Vec::new();
            for e in doc["top"]["status"].as_array().unwrap_or(&empty) {
                let colour = if e["ink"] == "alert" { theme.alert } else { ink };
                row.push((pick(face, e), colour));
            }
            // the levels move faster than the document is asked for, so the sheet watches them itself;
            // how they read is still the document's to say (draftpaper::content)
            if let Some(v) = told.volume.filter(|_| told.volume_lit || told.muted) {
                row.push((pick(face, &draftpaper::content::volume(v, told.muted)), ink));
            }
            if let Some(b) = told.brightness.filter(|_| told.brightness_lit) {
                row.push((pick(face, &draftpaper::content::brightness(b)), ink));
            }
            if !row.is_empty() {
                let gap = 4.0 * mm;
                let total: f32 = row.iter().map(|(t, _)| face.width(t, px, 0.0)).sum::<f32>() + gap * (row.len() - 1) as f32;
                let (start, end) = band.zone(band.zones.saturating_sub(1));
                let mut at = (band.here((start + end) / 2.0) * scale) as f32 - total / 2.0;
                for (text, colour) in &row {
                    at = face.draw(canvas, at, middle(&band, scale, face, px), text, px, 0.0, colour.premultiplied()) + gap;
                }
            }
            // the columns' numbers, in every zone past the workspaces' that nothing else has taken
            // (qml/ZoneNumbers.qml): a sheet with nothing to say carries them to the frame's corner
            let notice = told.notices.is_empty().then_some(band.zones).unwrap_or(band.zones.saturating_sub(2));
            let status = if row.is_empty() { band.zones } else { band.zones.saturating_sub(1) };
            numbers(canvas, face, &band, scale, px, soft, 1, band.zones, |k| k == notice || k == status);
        }
        Side::Bottom => {
            // what is connected over Bluetooth, in the first zone
            let (start, _) = band.zone(0);
            let bt = pick(face, &doc["bottom"]["start"]);
            face.draw(canvas, (band.here(start + 2.0 * s.mm) * scale) as f32, middle(&band, scale, face, px), &bt, px, 0.0, soft.premultiplied());
            // the figures over the scale bar's rules, and the theme's name in the last zone
            numbers(canvas, face, &band, scale, px, soft, 1, band.zones.saturating_sub(2), |_| false);
            figures(canvas, face, s, scale, px, soft);
            let (start, _) = band.zone(band.zones.saturating_sub(1));
            // the screenshot is the shell's to run still, so only the theme's name is lettered here
            if let Some(name) = doc["bottom"]["end"].as_array().and_then(|a| a.last()) {
                let name = pick(face, name);
                face.draw(canvas, (band.here(start + 2.0 * s.mm) * scale) as f32, middle(&band, scale, face, px), &name, px, 0.2 * mm, soft.premultiplied());
            }
        }
        Side::Left | Side::Right => {
            // a letter at the head of every row, and the window parked there lettered up the margin
            let first = if side == Side::Left { 0 } else { band.zones as usize };
            // the row under the pointer: a mark along the inner edge, as the shell marks it
            // a side margin: `under` reads the workspaces only along the top, so none are wanted here
            if let Some(Spot::Cell { cell, .. }) = hover.and_then(|(x, y)| under(s, side, x, y, &[], parked)) {
                let k = (cell - first.min(cell)) as u32;
                let (start, end) = band.zone(k.min(band.zones.saturating_sub(1)));
                let top = (band.here(start + 6.0 * s.mm) * scale) as i32;
                let foot = (band.here(end - 2.0 * s.mm) * scale) as i32;
                let wide = (2.0 * scale).round().max(1.0) as i32;
                let x = if side == Side::Left { (band.thick * scale) as i32 - wide } else { 0 };
                canvas.fill(x, top, wide, (foot - top).max(0), theme.accent.premultiplied());
            }
            for k in 0..band.zones {
                let (start, end) = band.zone(k);
                let head = band.here(start + 1.5 * s.mm) * scale;
                let letter = ROWS.chars().nth(k as usize % ROWS.len()).unwrap_or('A').to_string();
                let w = face.width(&letter, px * 0.8, 0.0);
                let across = (band.thick * scale) as f32 / 2.0 - w / 2.0;
                face.draw(canvas, across, head as f32 + face.ascent(px * 0.8), &letter, px * 0.8, 0.0, soft.premultiplied());
                // the title runs up the middle of the cell, clear of the letter at its head
                let Some((_, title, _)) = parked.iter().find(|(c, _, _)| *c == first + k as usize) else { continue };
                let room = ((end - start) - 9.0 * s.mm) * scale;
                let text = fit(face, title, px, room as f32);
                let length = face.width(&text, px, 0.0);
                let middle_of_cell = (band.here(start + (end - start) / 2.0 + 2.0 * s.mm) * scale) as f32;
                let foot = middle_of_cell + length / 2.0;
                face.draw_up(canvas, (band.thick * scale) as f32 / 2.0 + px * 0.4, foot, &text, px, 0.0, ink.premultiplied());
            }
        }
        Side::Title | Side::Schedule | Side::Graph => {}
    }
}

/// What the margins say that the document cannot: the notifications, which belong to whichever process
/// took the name on the session bus, and the levels lettered for a few seconds after they move.
#[derive(Debug, Clone, Default)]
pub struct Told {
    pub volume: Option<f64>,
    pub muted: bool,
    pub volume_lit: bool,
    pub brightness: Option<f64>,
    pub brightness_lit: bool,
    /// what has come in and not yet gone, newest last
    pub notices: Vec<String>,
}

/// What sits under the pointer on a margin, and what a click on it does.
#[derive(Debug, Clone, PartialEq)]
pub enum Spot {
    /// a workspace along the top: shown when it is clicked
    Workspace(u64),
    /// a row of a side margin: the window parked there comes back, or the focused one goes in
    Cell { cell: usize, parked: Option<u64> },
}

/// What is at `x`, `y` in a margin's surface (its own logical pixels, from its corner).
pub fn under(s: &Screen, side: Side, x: f64, y: f64, spaces: &[(u64, bool)], parked: &[(usize, String, u64)]) -> Option<Spot> {
    let band = Band::of(s, side);
    match side {
        Side::Top => {
            // the workspaces share the first zone, a slot each
            let (start, end) = band.zone(0);
            let along = x;
            if along < start || along >= end || spaces.is_empty() {
                return None;
            }
            let slot = (end - start) / spaces.len() as f64;
            let i = ((along - start) / slot).floor() as usize;
            spaces.get(i.min(spaces.len() - 1)).map(|(n, _)| Spot::Workspace(*n))
        }
        Side::Left | Side::Right => {
            let along = y + band.offset;
            let each = band.length / band.zones.max(1) as f64;
            if along < band.offset || along >= band.offset + band.length {
                return None;
            }
            let k = ((along - band.offset) / each).floor() as usize;
            let cell = if side == Side::Left { k } else { band.zones as usize + k };
            Some(Spot::Cell { cell, parked: parked.iter().find(|(c, _, _)| *c == cell).map(|(_, _, id)| *id) })
        }
        _ => None,
    }
}

/// The stretches of the side margins this sheet letters: the zone letter at each cell's head, and the
/// title of a window parked there. The compositor breaks its hatching around them (sheet::set_clear).
pub fn clear_spans(s: &Screen, face: &mut Face, parked: &[(usize, String, u64)]) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let band = Band::of(s, Side::Left);
    let px = ((s.margin * 0.5).min(3.2 * s.mm)) as f32;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (cell, title, _) in parked {
        let k = (*cell as u32) % band.zones.max(1);
        let (start, end) = band.zone(k);
        let side = if *cell < band.zones as usize { &mut left } else { &mut right };
        // the letter at the head
        side.push((start, start + 2.5 * s.mm + 1.25 * 0.8 * px as f64));
        // and the title, up the middle of the cell
        let room = (end - start) - 9.0 * s.mm;
        let length = (face.width(title, px, 0.0) as f64).min(room);
        let centre = start + (end - start) / 2.0 + 2.0 * s.mm;
        side.push((centre - length / 2.0 - s.mm, centre + length / 2.0 + s.mm));
    }
    (left, right)
}

/// The middle of a band, for lettering that sits across it.
fn middle(band: &Band, scale: f64, face: &Face, px: f32) -> f32 {
    let height = (band.thick * scale) as f32;
    (height - (face.ascent(px) + px * 0.25)) / 2.0 + face.ascent(px)
}

/// The columns numbered from the left, one in the middle of every zone between `from` and `to`.
fn numbers(canvas: &mut Canvas, face: &mut Face, band: &Band, scale: f64, px: f32, ink: crate::paint::Ink, from: u32, to: u32, taken: impl Fn(u32) -> bool) {
    for k in (from..to).filter(|k| !taken(*k)) {
        let (start, end) = band.zone(k);
        let text = (k + 1).to_string();
        let w = face.width(&text, px * 0.8, 0.0);
        let at = band.here((start + end) / 2.0) * scale;
        face.draw(canvas, at as f32 - w / 2.0, middle(band, scale, face, px * 0.8), &text, px * 0.8, 0.0, ink.premultiplied());
    }
}

/// The scale bar's centimetres, lettered over its rules with "mm" after the last, in the rows the
/// compositor leaves above the bar. A display that reports no size says so instead: nothing is to scale.
fn figures(canvas: &mut Canvas, face: &mut Face, s: &Screen, scale: f64, px: f32, ink: crate::paint::Ink) {
    let Some(bar) = s.bar.as_ref() else { return };
    let small = px * 0.8;
    // the band is drawn from the margin's own corner, and the bar's rows are given from the display's
    let baseline = ((bar.text_top - (s.height - s.margin)) * scale) as f32 + face.ascent(small);
    if !bar.measured {
        let at = (bar.zone.0 * scale) as f32;
        face.draw(canvas, at, baseline, "NTS", small, 0.2 * (s.mm * scale) as f32, ink.premultiplied());
        return;
    }
    for (i, (x, mm)) in bar.marks.iter().enumerate() {
        let text = mm.to_string();
        let w = face.width(&text, small, 0.0);
        let at = (x * scale) as f32 - w / 2.0;
        face.draw(canvas, at, baseline, &text, small, 0.0, ink.premultiplied());
        if i + 1 == bar.marks.len() {
            face.draw(canvas, at + w + small * 0.6, baseline, "mm", small, 0.0, ink.premultiplied());
        }
    }
}

/// A title cut to the room it has, with an ellipsis where it is cut, as the shell elided it.
pub fn fit(face: &mut Face, text: &str, px: f32, room: f32) -> String {
    if face.width(text, px, 0.0) <= room {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let mut next = out.clone();
        next.push(ch);
        if face.width(&next, px, 0.0) + face.width("…", px, 0.0) > room {
            break;
        }
        out = next;
    }
    out.push('…');
    out
}
