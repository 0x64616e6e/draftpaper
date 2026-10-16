//! The sheet's geometry in whole physical pixels, computed once here for everything that draws it: the
//! frame (sheet.rs), the wallpaper's frost (wallpaper.rs), the title block's rules (render.rs) and its
//! cells (sent to the shell, which letters them: TitleBlock.qml). Lines meet without gaps or overlaps
//! because they all come from these numbers, never from a rounding of their own.
//!
//! The frame's line is 2 pixels wide, its outer edge `margin` pixels in from the output's edges. The title
//! block sits in the frame's bottom-right corner: its right and bottom are the frame's line, its top and
//! left an outer rule as heavy as the frame's, its cells divided by 1 pixel rules.

use smithay::utils::{Physical, Rectangle};

/// The frame's line, in pixels.
pub const LINE: i32 = 2;

/// The title block, in millimetres: it takes the whole zones nearest this (title_block).
pub const SIZE_MM: (f32, f32) = (130.0, 36.0);

/// The title block's cells, [x, y, width, height] in millimetres from its top-left corner; they tile it.
/// The shell letters them in this order (TitleBlock.qml).
pub const CELLS: [[f32; 4]; 11] = [
    [0.0, 0.0, 34.0, 24.0],    // time
    [34.0, 0.0, 32.0, 12.0],   // date
    [66.0, 0.0, 22.0, 12.0],   // sheet
    [88.0, 0.0, 42.0, 12.0],   // project
    [34.0, 12.0, 24.0, 12.0],  // power
    [58.0, 12.0, 22.0, 12.0],  // battery
    [80.0, 12.0, 22.0, 12.0],  // profile
    [102.0, 12.0, 28.0, 12.0], // cpu / memory
    [0.0, 24.0, 34.0, 12.0],   // network
    [34.0, 24.0, 52.0, 12.0],  // temperature
    [86.0, 24.0, 44.0, 12.0],  // the custom cell, or drawn
];

/// The margin in physical pixels: `margin_mm` in whole logical pixels (as the layout takes it), scaled.
/// `mm` is logical pixels per millimetre.
pub fn margin(margin_mm: f64, mm: f64, scale: f64) -> i32 {
    ((margin_mm * mm).round() * scale).round() as i32
}

/// A rule of the title block: its pixels, and whether it is heavy (the outer rule) or a cell's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    pub rect: Rectangle<i32, Physical>,
    pub heavy: bool,
}

#[derive(Debug, Clone)]
pub struct TitleBlock {
    /// the block, to the outer edge of the frame's line on the right and bottom
    pub rect: Rectangle<i32, Physical>,
    /// inside the frame's line and the outer rule: what the frost and the lettering may cover
    pub inner: Rectangle<i32, Physical>,
    /// the cells (CELLS), each inside its rules
    pub cells: Vec<Rectangle<i32, Physical>>,
    /// none overlapping another, or the frame's line
    pub rules: Vec<Rule>,
}

/// The zones across and down of an output of `size` physical pixels, `margin` from margin(), `mm`
/// physical pixels per millimetre: about 50 mm each, two at least (the layout's Grid cuts the same).
pub fn zones(size: (i32, i32), margin: i32, mm: f64) -> (u32, u32) {
    let n = |len: i32| ((len - 2 * margin) as f64 / (50.0 * mm)).round().max(2.0) as u32;
    (n(size.0), n(size.1))
}

/// The title block of an output of `size` physical pixels, `margin` from margin(), `mm` physical pixels
/// per millimetre, cut into `zones`: the whole zones nearest SIZE_MM in the frame's bottom-right corner,
/// its outer rule starting on the ticks, and its cells in proportion.
pub fn title_block(size: (i32, i32), margin: i32, mm: f32, zones: (u32, u32)) -> TitleBlock {
    let (right, bottom) = (size.0 - margin, size.1 - margin);
    let whole = |len: i32, n: u32, want: f32| {
        let zone = (len - 2 * margin) as f32 / n.max(1) as f32;
        let k = ((want * mm / zone).round() as u32).clamp(1, n.saturating_sub(1).max(1));
        tick(len, margin, n, n - k).0
    };
    let (left, top) = (whole(size.0, zones.0, SIZE_MM.0).min(right - 2 * LINE - 1), whole(size.1, zones.1, SIZE_MM.1).min(bottom - 2 * LINE - 1));
    let (mx, my) = ((right - left) as f32 / SIZE_MM.0, (bottom - top) as f32 / SIZE_MM.1);
    let x = |v: f32| if v >= SIZE_MM.0 { right } else { left + (v * mx).round() as i32 };
    let y = |v: f32| if v >= SIZE_MM.1 { bottom } else { top + (v * my).round() as i32 };
    // what is not the frame's line nor the outer rule
    let (ix0, iy0, ix1, iy1) = (left + LINE, top + LINE, right - LINE, bottom - LINE);
    let rect = |x0: i32, y0: i32, x1: i32, y1: i32| Rectangle::<i32, Physical>::new((x0, y0).into(), ((x1 - x0).max(0), (y1 - y0).max(0)).into());

    let mut rules = vec![
        Rule { rect: rect(left, top, ix1, iy0), heavy: true },
        Rule { rect: rect(left, iy0, ix0, iy1), heavy: true },
    ];
    // a cell's left and top edges (its right and bottom are a neighbour's, or the frame's): vertical rules
    // whole, horizontal ones broken where a vertical one crosses
    let mut verticals: Vec<(i32, i32, i32)> = Vec::new();
    let mut horizontals: Vec<(i32, i32, i32)> = Vec::new();
    for c in CELLS {
        let (x0, y0, x1, y1) = (x(c[0]), y(c[1]), x(c[0] + c[2]), y(c[1] + c[3]));
        if c[0] > 0.0 {
            verticals.push((x0, y0.max(iy0), y1.min(iy1)));
        }
        if c[1] > 0.0 {
            horizontals.push((y0, x0.max(ix0), x1.min(ix1)));
        }
    }
    for &(vx, y0, y1) in &verticals {
        rules.push(Rule { rect: rect(vx, y0, vx + 1, y1), heavy: false });
    }
    for (hy, x0, x1) in horizontals {
        let mut cuts: Vec<i32> = verticals.iter().filter(|v| v.1 <= hy && hy < v.2 && x0 <= v.0 && v.0 < x1).map(|v| v.0).collect();
        cuts.sort();
        let mut from = x0;
        for c in cuts.into_iter().chain([x1]) {
            if c > from {
                rules.push(Rule { rect: rect(from, hy, c, hy + 1), heavy: false });
            }
            from = c + 1;
        }
    }
    rules.retain(|r| r.rect.size.w > 0 && r.rect.size.h > 0);

    // a cell's room: from after its left and top rules to its right and bottom ones
    let cells = CELLS
        .iter()
        .map(|c| {
            let (x0, y0, x1, y1) = (x(c[0]), y(c[1]), x(c[0] + c[2]), y(c[1] + c[3]));
            let (x0, y0) = (if c[0] > 0.0 { x0 + 1 } else { ix0 }, if c[1] > 0.0 { y0 + 1 } else { iy0 });
            let x1 = if c[0] + c[2] >= SIZE_MM.0 { ix1 } else { x1 };
            let y1 = if c[1] + c[3] >= SIZE_MM.1 { iy1 } else { y1 };
            rect(x0, y0, x1, y1)
        })
        .collect();

    TitleBlock { rect: rect(left, top, right, bottom), inner: rect(ix0, iy0, ix1, iy1), cells, rules }
}

// the twin of the sheet shader's ticked(), which cannot be called from here: its tests are the shader's; the
// title block starts on its ticks
/// The physical pixel of the tick at zone boundary `k` (of `zones`) along a side `len` pixels long: for
/// the frame's corners (k = 0, zones) the first pixel of the frame's line there, which the tick covers
/// whole (LINE pixels); otherwise one pixel.
pub fn tick(len: i32, margin: i32, zones: u32, k: u32) -> (i32, i32) {
    let inside = len - 2 * margin;
    if k == 0 {
        (margin, LINE)
    } else if k >= zones {
        (len - margin - LINE, LINE)
    } else {
        (margin + (k as f64 * inside as f64 / zones as f64).round() as i32, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(r: &Rectangle<i32, Physical>) -> impl Iterator<Item = (i32, i32)> + '_ {
        (r.loc.y..r.loc.y + r.size.h).flat_map(move |y| (r.loc.x..r.loc.x + r.size.w).map(move |x| (x, y)))
    }

    /// Every display the sheet may be drawn on: sizes, densities and scales, the laptop's and the 4K's among them.
    fn cases() -> Vec<((i32, i32), f64, f64, f64)> {
        let mut v = Vec::new();
        for &(w, h, phys_mm) in &[(1920, 1200, 6.4), (3840, 2160, 6.1935), (2560, 1440, 4.13), (1366, 768, 3.6), (2880, 1800, 9.1)] {
            for &scale in &[1.0, 1.25, 1.5, 1.75, 2.0] {
                for &margin_mm in &[3.0, 6.0, 7.5, 20.0] {
                    v.push(((w, h), phys_mm, scale, margin_mm));
                }
            }
        }
        v
    }

    #[test]
    fn the_title_block_is_exact() {
        for ((w, h), phys_mm, scale, margin_mm) in cases() {
            let mg = margin(margin_mm, phys_mm / scale, scale);
            let zones = zones((w, h), mg, phys_mm);
            let t = title_block((w, h), mg, phys_mm as f32, zones);
            let case = format!("{w}x{h} {phys_mm} px/mm, scale {scale}, margin {margin_mm} mm");
            // on the frame's outer edges, inside the frame
            assert_eq!(t.rect.loc.x + t.rect.size.w, w - mg, "{case}");
            assert_eq!(t.rect.loc.y + t.rect.size.h, h - mg, "{case}");
            assert!(t.rect.loc.x > mg && t.rect.loc.y > mg, "{case}");
            // its outer rule starts on a tick: whole zones
            assert!((1..zones.0).any(|k| tick(w, mg, zones.0, k).0 == t.rect.loc.x), "{case}: left off the ticks");
            assert!((1..zones.1).any(|k| tick(h, mg, zones.1, k).0 == t.rect.loc.y), "{case}: top off the ticks");
            // rules: no pixel twice, none on the frame's line, all in the block
            let mut seen = std::collections::HashSet::new();
            for r in &t.rules {
                for p in pixels(&r.rect) {
                    assert!(seen.insert(p), "{case}: {p:?} ruled twice");
                    assert!(p.0 >= t.rect.loc.x && p.0 < w - mg - LINE && p.1 >= t.rect.loc.y && p.1 < h - mg - LINE, "{case}: {p:?} off the block");
                }
            }
            // the outer rule: LINE pixels, the whole of the top and the left
            for x in t.rect.loc.x..w - mg - LINE {
                for d in 0..LINE {
                    assert!(seen.contains(&(x, t.rect.loc.y + d)), "{case}: top rule gap at {x}");
                }
            }
            // cells: inside the inner rect, apart, and every inner pixel a cell's or a rule's
            for c in &t.cells {
                assert!(c.size.w > 0 && c.size.h > 0, "{case}");
                for p in pixels(c) {
                    assert!(t.inner.contains(p), "{case}: cell {c:?} leaves the block");
                    assert!(seen.insert(p), "{case}: {p:?} in a cell and a rule, or two cells");
                }
            }
            for p in pixels(&t.inner) {
                assert!(seen.contains(&p), "{case}: {p:?} neither a rule nor a cell");
            }
        }
    }

    #[test]
    fn the_corner_ticks_are_the_frame_line() {
        for len in [1080, 1200, 1281, 2160] {
            for mg in [19, 26, 31, 39, 52] {
                for zones in 2..12u32 {
                    assert_eq!(tick(len, mg, zones, 0), (mg, LINE));
                    assert_eq!(tick(len, mg, zones, zones), (len - mg - LINE, LINE));
                    for k in 1..zones {
                        let (at, n) = tick(len, mg, zones, k);
                        assert_eq!(n, 1);
                        assert!(at >= mg + LINE && at < len - mg - LINE);
                        assert!(at > tick(len, mg, zones, k - 1).0);
                    }
                }
            }
        }
    }
}
