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
}

impl Band {
    fn of(s: &Screen, side: Side) -> Band {
        let thick = s.margin;
        if side.vertical() {
            Band { offset: thick, length: s.height - 2.0 * thick, zones: s.down, thick }
        } else {
            Band { offset: thick, length: s.width - 2.0 * thick, zones: s.across, thick }
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
pub fn letter(canvas: &mut Canvas, face: &mut Face, s: &Screen, theme: &Theme, side: Side, scale: f64, workspace: u64, spaces: &[(u64, bool)], parked: &[(usize, String)]) {
    let band = Band::of(s, side);
    let mm = (s.mm * scale) as f32;
    let px = ((s.margin * 0.5).min(3.2 * s.mm) * scale) as f32;
    let ink = theme.ink;
    let soft = crate::paint::Ink { a: (ink.a * 0.8).min(1.0), ..ink };
    // the band's own corner, so a place on the display becomes a place in the buffer
    let corner = if side.vertical() { band.offset } else { band.offset };
    let _ = corner;

    match side {
        Side::Top => {
            // the workspaces share the first zone, each a slot of it, the one in use in the strong ink
            let (start, end) = band.zone(0);
            let shown: Vec<&(u64, bool)> = spaces.iter().collect();
            let slot = (end - start) / shown.len().max(1) as f64;
            for (i, (number, focused)) in shown.iter().enumerate() {
                let at = start + (i as f64 + 0.5) * slot - band.offset;
                let text = number.to_string();
                let w = face.width(&text, px, 0.0);
                let colour = if *focused { ink } else { soft };
                face.draw(canvas, (at * scale) as f32 - w / 2.0, middle(&band, scale, face, px), &text, px, 0.0, colour.premultiplied());
            }
            numbers(canvas, face, &band, scale, px, soft, 1, band.zones.saturating_sub(1));
        }
        Side::Bottom => {
            // the figures over the scale bar's rules, and the theme's name in the last zone
            numbers(canvas, face, &band, scale, px, soft, 0, band.zones.saturating_sub(2));
            figures(canvas, face, s, scale, px, soft);
            let (start, _) = band.zone(band.zones.saturating_sub(1));
            let name = theme.label.to_uppercase();
            face.draw(canvas, ((start - band.offset + 2.0 * s.mm) * scale) as f32, middle(&band, scale, face, px), &name, px, 0.2 * mm, soft.premultiplied());
        }
        Side::Left | Side::Right => {
            // a letter at the head of every row, and the window parked there lettered up the margin
            let first = if side == Side::Left { 0 } else { band.zones as usize };
            for k in 0..band.zones {
                let (start, end) = band.zone(k);
                let head = (start - band.offset + 1.5 * s.mm) * scale;
                let letter = ROWS.chars().nth(k as usize % ROWS.len()).unwrap_or('A').to_string();
                let w = face.width(&letter, px * 0.8, 0.0);
                let across = (band.thick * scale) as f32 / 2.0 - w / 2.0;
                face.draw(canvas, across, head as f32 + face.ascent(px * 0.8), &letter, px * 0.8, 0.0, soft.premultiplied());
                // the title runs up the middle of the cell, clear of the letter at its head
                let Some((_, title)) = parked.iter().find(|(c, _)| *c == first + k as usize) else { continue };
                let room = ((end - start) - 9.0 * s.mm) * scale;
                let text = cut(face, title, px, room as f32);
                let length = face.width(&text, px, 0.0);
                let middle_of_cell = ((start - band.offset + (end - start) / 2.0 + 2.0 * s.mm) * scale) as f32;
                let foot = middle_of_cell + length / 2.0;
                face.draw_up(canvas, (band.thick * scale) as f32 / 2.0 + px * 0.4, foot, &text, px, 0.0, ink.premultiplied());
            }
        }
        Side::Title => {}
    }
    let _ = workspace;
}

/// The stretches of the side margins this sheet letters: the zone letter at each cell's head, and the
/// title of a window parked there. The compositor breaks its hatching around them (sheet::set_clear).
pub fn clear_spans(s: &Screen, face: &mut Face, parked: &[(usize, String)]) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let band = Band::of(s, Side::Left);
    let px = ((s.margin * 0.5).min(3.2 * s.mm)) as f32;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (cell, title) in parked {
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
fn numbers(canvas: &mut Canvas, face: &mut Face, band: &Band, scale: f64, px: f32, ink: crate::paint::Ink, from: u32, to: u32) {
    for k in from..to {
        let (start, end) = band.zone(k);
        let text = (k + 1).to_string();
        let w = face.width(&text, px * 0.8, 0.0);
        let at = ((start + end) / 2.0 - band.offset) * scale;
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
        let at = ((bar.zone.0 - s.margin) * scale) as f32;
        face.draw(canvas, at, baseline, "NTS", small, 0.2 * (s.mm * scale) as f32, ink.premultiplied());
        return;
    }
    for (i, (x, mm)) in bar.marks.iter().enumerate() {
        let text = mm.to_string();
        let w = face.width(&text, small, 0.0);
        let at = ((x - s.margin) * scale) as f32 - w / 2.0;
        face.draw(canvas, at, baseline, &text, small, 0.0, ink.premultiplied());
        if i + 1 == bar.marks.len() {
            face.draw(canvas, at + w + small * 0.6, baseline, "mm", small, 0.0, ink.premultiplied());
        }
    }
}

/// A title cut to the room it has, with an ellipsis where it is cut, as the shell elided it.
fn cut(face: &mut Face, text: &str, px: f32, room: f32) -> String {
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
