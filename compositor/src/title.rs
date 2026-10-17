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

/// The key plan (issue #47): with several screens, a small plan of them all in the bottom margin's zone
/// left of the scale bar, as a drawing set marks which part of the whole a sheet shows. Each screen an
/// outline in their arrangement, scaled to fit the zone and centred in it, a pixel apart from its
/// neighbours; the focused screen's outline twice as heavy; this screen filled.
#[derive(Debug, Clone)]
pub struct KeyPlan {
    /// the zone's room in the margin, between its ticks
    pub zone: Rectangle<i32, Physical>,
    /// the outlines, none over another
    pub rules: Vec<Rule>,
    /// this screen, inside its outline
    pub fill: Rectangle<i32, Physical>,
    /// the north mark's room, left of the plan
    pub arrow: Rectangle<i32, Physical>,
    /// the arms thickened, up, right, down, left: toward each other screen (its centre's bearing from this
    /// one's, to the nearest quarter)
    pub towards: [bool; 4],
    /// the north mark, as a drawing's north arrow drawn square: a square, heavy, a thin cross through it a
    /// little longer, and the arm toward each other screen a solid bar from the centre to the square's inner
    /// edge; its pixels in rows, none twice, none over the plan
    pub mark: Vec<Rule>,
}

/// The north mark in a square of `side` pixels (made odd: a centre pixel) at `at`: which pixels are drawn,
/// and heavy (the square, the thick arms) or light (the cross), as one-pixel rows.
fn north_mark(at: (i32, i32), side: i32, towards: [bool; 4]) -> Vec<Rule> {
    let s = if side % 2 == 0 { side - 1 } else { side };
    let c = s / 2;
    // the cross runs past the square by `e` on every side; the square's outline `t` thick
    let e = ((s as f64 * 0.14).round() as i32).max(2);
    let (q0, q1) = (e, s - 1 - e);
    let t = if q1 - q0 >= 12 { 2 } else { 1 };
    let half = if q1 - q0 >= 10 { 1 } else { 0 };
    // 0: nothing, 1: light, 2: heavy
    let mut px = vec![vec![0u8; s as usize]; s as usize];
    for i in 0..s {
        px[c as usize][i as usize] = 1;
        px[i as usize][c as usize] = 1;
    }
    for y in q0..=q1 {
        for x in q0..=q1 {
            if y < q0 + t || y > q1 - t || x < q0 + t || x > q1 - t {
                px[y as usize][x as usize] = 2;
            }
        }
    }
    // the thick arms: from the centre to the square's inner edge, 2 * half + 1 wide
    let (inner0, inner1) = (q0 + t, q1 - t);
    for (k, &on) in towards.iter().enumerate() {
        if !on {
            continue;
        }
        for d in -half..=half {
            let span: Box<dyn Iterator<Item = i32>> = match k {
                0 => Box::new(inner0..=c),
                1 => Box::new(c..=inner1),
                2 => Box::new(c..=inner1),
                _ => Box::new(inner0..=c),
            };
            for v in span {
                let (x, y) = if k % 2 == 0 { (c + d, v) } else { (v, c + d) };
                px[y as usize][x as usize] = 2;
            }
        }
    }
    let mut rules = Vec::new();
    for (y, row) in px.iter().enumerate() {
        let mut x = 0;
        while x < row.len() {
            let w = row[x];
            let start = x;
            while x < row.len() && row[x] == w {
                x += 1;
            }
            if w > 0 {
                let r = Rectangle::<i32, Physical>::new((at.0 + start as i32, at.1 + y as i32).into(), ((x - start) as i32, 1).into());
                rules.push(Rule { rect: r, heavy: w == 2 });
            }
        }
    }
    rules
}

/// The key plan of an output of `size` physical pixels, `margin` from margin(), `mm` physical pixels per
/// millimetre, cut into `zones`, among the screens `layout` (each x, y, width, height, in the layout's
/// logical pixels): `this` one and the `focused` one. None with one screen, fewer than four zones across,
/// or a screen too small to draw.
pub fn key_plan(size: (i32, i32), margin: i32, mm: f64, zones: (u32, u32), layout: &[(i32, i32, i32, i32)], this: usize, focused: usize) -> Option<KeyPlan> {
    let n = zones.0;
    if layout.len() < 2 || n < 4 || this >= layout.len() {
        return None;
    }
    let (a, b) = (tick(size.0, margin, n, n - 3).0 + 1, tick(size.0, margin, n, n - 2).0);
    let rect = |x: i32, y: i32, w: i32, h: i32| Rectangle::<i32, Physical>::new((x, y).into(), (w, h).into());
    let zone = rect(a, size.1 - margin, b - a, margin);
    let pad = (1.5 * mm).round() as i32;
    let (y0, y1) = (size.1 - margin + (0.2 * margin as f64).round() as i32, size.1 - (0.2 * margin as f64).round() as i32);
    // the north mark's room on the left, as high as the plan may be; the plan in what is left
    let side = y1 - y0;
    if side < 7 {
        return None;
    }
    let arrow = rect(a + pad, y0, side, side);
    let a = a + side + pad;
    let (aw, ah) = ((b - a - 2 * pad) as f64, (y1 - y0) as f64);
    let (bx, by) = (layout.iter().map(|l| l.0).min()?, layout.iter().map(|l| l.1).min()?);
    let (bw, bh) = ((layout.iter().map(|l| l.0 + l.2).max()? - bx) as f64, (layout.iter().map(|l| l.1 + l.3).max()? - by) as f64);
    if aw <= 0.0 || ah <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        return None;
    }
    let s = (aw / bw).min(ah / bh);
    let (ox, oy) = (a + pad + ((aw - bw * s) / 2.0).round() as i32, y0 + ((ah - bh * s) / 2.0).round() as i32);
    let mut rules = Vec::new();
    let mut fill = None;
    for (i, &(x, y, w, h)) in layout.iter().enumerate() {
        // a pixel short on the right and at the bottom: apart from a neighbour that starts there
        let (x0, y0) = (ox + ((x - bx) as f64 * s).round() as i32, oy + ((y - by) as f64 * s).round() as i32);
        let (x1, y1) = (ox + ((x + w - bx) as f64 * s).round() as i32 - 1, oy + ((y + h - by) as f64 * s).round() as i32 - 1);
        let (w, h) = (x1 - x0, y1 - y0);
        if w < 4 || h < 4 {
            return None;
        }
        let t = if i == focused && w >= 6 && h >= 6 { 2 } else { 1 };
        rules.push(Rule { rect: rect(x0, y0, w, t), heavy: i == focused });
        rules.push(Rule { rect: rect(x0, y1 - t, w, t), heavy: i == focused });
        rules.push(Rule { rect: rect(x0, y0 + t, t, h - 2 * t), heavy: i == focused });
        rules.push(Rule { rect: rect(x1 - t, y0 + t, t, h - 2 * t), heavy: i == focused });
        if i == this {
            fill = Some(rect(x0 + t, y0 + t, w - 2 * t, h - 2 * t));
        }
    }
    rules.retain(|r| r.rect.size.w > 0 && r.rect.size.h > 0);
    // toward each other screen: its centre's bearing from this one's, clockwise from up (the layout's y
    // runs down), to the nearest quarter
    let centre = |l: &(i32, i32, i32, i32)| (l.0 as f64 + l.2 as f64 / 2.0, l.1 as f64 + l.3 as f64 / 2.0);
    let here = centre(&layout[this]);
    let mut towards = [false; 4];
    for (_, l) in layout.iter().enumerate().filter(|(i, _)| *i != this) {
        let there = centre(l);
        let bearing = (there.0 - here.0).atan2(-(there.1 - here.1)).to_degrees().rem_euclid(360.0);
        towards[((bearing / 90.0).round() as usize) % 4] = true;
    }
    let mark = north_mark((arrow.loc.x, arrow.loc.y), side, towards);
    Some(KeyPlan { zone, rules, fill: fill?, arrow, towards, mark })
}

/// A parking cell's room in the side margins (issue #46): `cell` counts down the left margin, then the
/// right (parking.rs), one per zone; between its zone ticks (whose pixels it leaves out), from the screen's
/// edge to the frame's line. None past the last cell.
pub fn side_cell(size: (i32, i32), margin: i32, zones: (u32, u32), cell: usize) -> Option<Rectangle<i32, Physical>> {
    let n = zones.1 as usize;
    if n == 0 || cell >= 2 * n {
        return None;
    }
    let k = (cell % n) as u32;
    let y0 = if k == 0 { margin + LINE } else { tick(size.1, margin, zones.1, k).0 + 1 };
    let y1 = tick(size.1, margin, zones.1, k + 1).0;
    let x0 = if cell < n { 0 } else { size.0 - margin };
    Some(Rectangle::new((x0, y0).into(), (margin, (y1 - y0).max(0)).into()))
}

/// The hatching's step in pixels for lines `spacing_mm` apart (0: none), three pixels at least.
pub fn hatch_step(spacing_mm: f64, mm: f64) -> i32 {
    if spacing_mm <= 0.0 {
        0
    } else {
        ((spacing_mm * mm).round() as i32).max(3)
    }
}

/// Whether the output's pixel (x, y) lies on the hatching: one-pixel lines at 45°, rising to the right,
/// every `step` pixels counted from the output's corner, so they run on unbroken from cell to cell.
pub fn hatched(x: i32, y: i32, step: i32) -> bool {
    step > 0 && (x + y).rem_euclid(step) == 0
}

/// The revision block's columns (issue #45), in millimetres of the title block's 130 across: REV,
/// DESCRIPTION, DATE, BY.
pub const REV_COLUMNS: [f32; 5] = [0.0, 16.0, 86.0, 110.0, 130.0];
/// Its rows under the header.
pub const REV_ROWS: usize = 4;

/// The revision block: in the zone right above the title block, as wide, its outer rule as heavy as the
/// frame's along the top and the left, the title block's own top rule under it and the frame's line at
/// its right. A header row (a sixth of its height) and REV_ROWS rows, each cut into REV_COLUMNS. `cells`
/// are the header's four, then each row's four, left to right.
#[derive(Debug, Clone)]
pub struct RevisionBlock {
    pub rect: Rectangle<i32, Physical>,
    pub inner: Rectangle<i32, Physical>,
    pub cells: Vec<Rectangle<i32, Physical>>,
    pub rules: Vec<Rule>,
}

/// The revision block of an output (as title_block()): None when the title block leaves no whole zone
/// above it short of the frame's top row.
pub fn revision_block(size: (i32, i32), margin: i32, mm: f32, zones: (u32, u32)) -> Option<RevisionBlock> {
    let block = title_block(size, margin, mm, zones);
    let below = block.rect.loc.y;
    let k = (1..zones.1).find(|&k| tick(size.1, margin, zones.1, k).0 == below)?;
    if k < 2 {
        return None;
    }
    let (left, top, right) = (block.rect.loc.x, tick(size.1, margin, zones.1, k - 1).0, size.0 - margin);
    let (ix0, iy0, ix1, iy1) = (left + LINE, top + LINE, right - LINE, below);
    if iy1 - iy0 < 2 * (REV_ROWS as i32 + 1) {
        return None;
    }
    let rect = |x0: i32, y0: i32, x1: i32, y1: i32| Rectangle::<i32, Physical>::new((x0, y0).into(), ((x1 - x0).max(0), (y1 - y0).max(0)).into());
    let mx = (right - left) as f32 / SIZE_MM.0;
    let xs: Vec<i32> = REV_COLUMNS.iter().map(|&v| if v >= SIZE_MM.0 { ix1 } else if v <= 0.0 { ix0 } else { left + (v * mx).round() as i32 }).collect();
    // the rows' top rules: the header's bottom, then between the rows
    let header = ((iy1 - iy0) as f32 / 6.0).round() as i32;
    let ys: Vec<i32> = (0..REV_ROWS).map(|r| iy0 + header + (r as f32 * (iy1 - iy0 - header) as f32 / REV_ROWS as f32).round() as i32).collect();

    let mut rules = vec![Rule { rect: rect(left, top, ix1, iy0), heavy: true }, Rule { rect: rect(left, iy0, ix0, iy1), heavy: true }];
    // the columns' rules whole, the rows' broken where they cross
    for &x in &xs[1..xs.len() - 1] {
        rules.push(Rule { rect: rect(x, iy0, x + 1, iy1), heavy: false });
    }
    for &y in &ys {
        for c in 0..xs.len() - 1 {
            let from = if c == 0 { ix0 } else { xs[c] + 1 };
            rules.push(Rule { rect: rect(from, y, xs[c + 1], y + 1), heavy: false });
        }
    }
    rules.retain(|r| r.rect.size.w > 0 && r.rect.size.h > 0);
    // the cells: from after their left and top rules to the next ones
    let mut cells = Vec::new();
    let bands: Vec<(i32, i32)> = std::iter::once((iy0, ys[0])).chain((0..REV_ROWS).map(|r| (ys[r] + 1, if r + 1 < REV_ROWS { ys[r + 1] } else { iy1 }))).collect();
    for (y0, y1) in bands {
        for c in 0..xs.len() - 1 {
            let x0 = if c == 0 { ix0 } else { xs[c] + 1 };
            cells.push(rect(x0, y0, xs[c + 1], y1));
        }
    }
    Some(RevisionBlock { rect: rect(left, top, right, below), inner: rect(ix0, iy0, ix1, iy1), cells, rules })
}

/// The scale bar (issue #44): in the bottom margin's zone next to the last one (under the title block, where
/// a drawing keeps its scale), a bar of whole centimetres true to the screen's size: 0 to 50 mm (less in a
/// short zone), the first centimetre ticked by the millimetre, then alternately filled and open, outlined
/// in one-pixel rules. The shell letters the numbers over the boundaries (`marks`) in the band `text`.
#[derive(Debug, Clone)]
pub struct ScaleBar {
    /// the zone's room in the margin: between its ticks, the margin's height
    pub zone: Rectangle<i32, Physical>,
    /// None: the display reports no size, so nothing on it is to scale (the shell letters NTS)
    pub bar: Option<Bar>,
}

#[derive(Debug, Clone)]
pub struct Bar {
    /// the bar's outline, outer edges
    pub rect: Rectangle<i32, Physical>,
    /// every pixel drawn in ink: rules, ticks and fills, none twice
    pub ink: Vec<Rectangle<i32, Physical>>,
    /// each centimetre's boundary: the column of its rule, and its millimetres from 0
    pub marks: Vec<(i32, u32)>,
    /// the rows the numbers are lettered in, above the bar: (top, bottom)
    pub text: (i32, i32),
}

/// The scale bar of an output of `size` physical pixels, `margin` from margin(), `mm` physical pixels per
/// millimetre (`measured`: from the display's own size, or config.toml's), cut into `zones`. None without
/// a zone to spare (fewer than three across) or room for a centimetre.
pub fn scale_bar(size: (i32, i32), margin: i32, mm: f64, zones: (u32, u32), measured: bool) -> Option<ScaleBar> {
    let n = zones.0;
    if n < 3 {
        return None;
    }
    let k = n - 2;
    let (a, b) = (tick(size.0, margin, n, k).0 + 1, tick(size.0, margin, n, k + 1).0);
    let (top, bottom) = (size.1 - margin, size.1);
    let rect = |x: i32, y: i32, w: i32, h: i32| Rectangle::<i32, Physical>::new((x, y).into(), (w, h).into());
    let zone = rect(a, top, b - a, bottom - top);
    if !measured {
        return Some(ScaleBar { zone, bar: None });
    }
    // room for half of "0" on the left; for half of the last number and " mm" on the right
    let (left, right) = ((1.5 * mm).round() as i32, (5.5 * mm).round() as i32);
    let cm = (((b - a - left - right) as f64 / (10.0 * mm)).floor() as u32).min(5);
    // a bar a third of the margin's height at most, two rules and a row between them at least
    let height = ((1.2 * mm).round() as i32).min(margin / 3).max(3);
    let y = top + (0.58 * margin as f64).round() as i32;
    if cm == 0 || y + height > bottom - 1 {
        return None;
    }
    let x0 = a + left;
    let at = |d: u32| x0 + (d as f64 * mm).round() as i32;
    let end = at(10 * cm);
    let inner = height - 2;
    let mut ink = vec![rect(x0, y, end + 1 - x0, 1), rect(x0, y + height - 1, end + 1 - x0, 1)];
    for c in 0..=cm {
        ink.push(rect(at(10 * c), y + 1, 1, inner));
    }
    // the first centimetre by the millimetre: ticks half the bar's height, hanging from its top
    for d in 1..10 {
        ink.push(rect(at(d), y + 1, 1, (inner / 2).max(1)));
    }
    // the second and fourth centimetres filled
    for c in (1..cm).step_by(2) {
        ink.push(rect(at(10 * c) + 1, y + 1, at(10 * (c + 1)) - at(10 * c) - 1, inner));
    }
    let marks = (0..=cm).map(|c| (at(10 * c), 10 * c)).collect();
    let text = (top + (0.12 * margin as f64).round() as i32, y - (0.06 * margin as f64).round().max(1.0) as i32);
    Some(ScaleBar { zone, bar: Some(Bar { rect: rect(x0, y, end + 1 - x0, height), ink, marks, text }) })
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
    fn the_key_plan_draws_every_screen() {
        // the laptop's panel with a 27" monitor to its right, a little lower; and three side by side; and two stacked
        let layouts: [&[(i32, i32, i32, i32)]; 3] = [&[(0, 240, 1280, 800), (1280, 0, 2560, 1440)], &[(0, 0, 1920, 1080), (1920, 0, 1920, 1080), (3840, 0, 1920, 1080)], &[(0, 0, 2560, 1440), (0, 1440, 2560, 1440)]];
        for ((w, h), phys_mm, scale, margin_mm) in cases() {
            let mg = margin(margin_mm, phys_mm / scale, scale);
            let zones = zones((w, h), mg, phys_mm);
            let case = format!("{w}x{h} {phys_mm} px/mm, scale {scale}, margin {margin_mm} mm");
            assert!(key_plan((w, h), mg, phys_mm, zones, &layouts[0][..1], 0, 0).is_none(), "{case}: one screen, no plan");
            for layout in layouts {
                for this in 0..layout.len() {
                    let Some(p) = key_plan((w, h), mg, phys_mm, zones, layout, this, 0) else {
                        assert!(zones.0 < 4 || margin_mm < 4.0 || layout.len() > 2, "{case}: no plan for {layout:?}");
                        continue;
                    };
                    assert_eq!(p.zone.loc.x, tick(w, mg, zones.0, zones.0 - 3).0 + 1, "{case}: left of the scale bar's zone");
                    let mut seen = std::collections::HashSet::new();
                    for r in &p.rules {
                        for px in pixels(&r.rect) {
                            assert!(seen.insert(px), "{case}: {px:?} drawn twice");
                            assert!(p.zone.contains(px), "{case}: {px:?} off the zone");
                        }
                    }
                    assert_eq!(p.rules.len(), 4 * layout.len(), "{case}: four sides a screen");
                    for px in pixels(&p.fill) {
                        assert!(!seen.contains(&px), "{case}: the fill over an outline");
                    }
                    // the focused one heavier, when there is room
                    assert!(p.rules[0].heavy && !p.rules[4].heavy, "{case}");
                    // in their arrangement: the second screen to the right of, or under, the first
                    let (first, second) = (p.rules[0].rect, p.rules[4].rect);
                    assert!(second.loc.x > first.loc.x || second.loc.y > first.loc.y, "{case}: out of order");
                    // the north mark: in its room, left of the plan, no pixel twice nor on the plan
                    assert!(p.zone.contains_rect(p.arrow), "{case}: the mark's room leaves the zone");
                    assert!(p.rules.iter().all(|r| r.rect.loc.x > p.arrow.loc.x + p.arrow.size.w), "{case}: the plan over the mark");
                    let mut mark = std::collections::HashMap::new();
                    for r in &p.mark {
                        for px in pixels(&r.rect) {
                            assert!(mark.insert(px, r.heavy).is_none(), "{case}: {px:?} marked twice");
                            assert!(p.arrow.contains(px) && !seen.contains(&px), "{case}: {px:?} off the mark's room");
                        }
                    }
                    // the heavy part (the square, the thick arms) inside the square; the thin cross past it on
                    // every side
                    let heavy: Vec<_> = mark.iter().filter(|(_, h)| **h).map(|(p, _)| *p).collect();
                    let (hx0, hx1) = (heavy.iter().map(|p| p.0).min().unwrap(), heavy.iter().map(|p| p.0).max().unwrap());
                    let (hy0, hy1) = (heavy.iter().map(|p| p.1).min().unwrap(), heavy.iter().map(|p| p.1).max().unwrap());
                    assert_eq!(hx1 - hx0, hy1 - hy0, "{case}: the square not square");
                    let (lx0, lx1) = (mark.keys().map(|p| p.0).min().unwrap(), mark.keys().map(|p| p.0).max().unwrap());
                    let (ly0, ly1) = (mark.keys().map(|p| p.1).min().unwrap(), mark.keys().map(|p| p.1).max().unwrap());
                    assert!(lx0 < hx0 && lx1 > hx1 && ly0 < hy0 && ly1 > hy1, "{case}: the cross not past the square");
                }
            }
            // the arms thickened: the laptop's panel toward the monitor at its right, the monitor toward the
            // panel at its left (each a little off level: the nearest quarter); stacked, down and up; three in
            // a row, the middle one both ways
            let arms = |layout: &[(i32, i32, i32, i32)], this| key_plan((w, h), mg, phys_mm, zones, layout, this, 0).map(|p| p.towards);
            for (layout, this, want) in [
                (layouts[0], 0, [false, true, false, false]),
                (layouts[0], 1, [false, false, false, true]),
                (layouts[2], 0, [false, false, true, false]),
                (layouts[2], 1, [true, false, false, false]),
                (layouts[1], 1, [false, true, false, true]),
            ] {
                if let Some(t) = arms(layout, this) {
                    assert_eq!(t, want, "{case}: screen {this} of {layout:?}");
                }
            }
        }
    }

    #[test]
    fn the_side_cells_fill_the_side_margins() {
        for ((w, h), phys_mm, scale, margin_mm) in cases() {
            let mg = margin(margin_mm, phys_mm / scale, scale);
            let zones = zones((w, h), mg, phys_mm);
            let case = format!("{w}x{h} {phys_mm} px/mm, scale {scale}, margin {margin_mm} mm");
            let n = zones.1 as usize;
            assert!(side_cell((w, h), mg, zones, 2 * n).is_none(), "{case}");
            for side in 0..2 {
                // down the margin: the cells and the ticks between them, from the frame's top line to its bottom
                let mut y = mg + LINE;
                for k in 0..n {
                    let c = side_cell((w, h), mg, zones, side * n + k).unwrap();
                    assert_eq!(c.loc.x, if side == 0 { 0 } else { w - mg }, "{case}");
                    assert_eq!(c.size.w, mg, "{case}: from the edge to the frame's line");
                    assert_eq!(c.loc.y, y, "{case}: cell {k} starts after the tick");
                    y = c.loc.y + c.size.h;
                    if k + 1 < n {
                        assert_eq!(tick(h, mg, zones.1, k as u32 + 1), (y, 1), "{case}: a tick ends cell {k}");
                        y += 1;
                    }
                }
                assert_eq!(y, h - mg - LINE, "{case}: the last cell ends on the frame's line");
            }
        }
        // the hatching: one pixel in `step` along a row, continuous along the diagonal
        assert_eq!(hatch_step(1.5, 6.4), 10);
        assert_eq!(hatch_step(0.0, 6.4), 0);
        assert_eq!(hatch_step(0.2, 3.6), 3);
        assert_eq!((0..40).filter(|&x| hatched(x, 7, 10)).count(), 4);
        assert!(hatched(3, 7, 10) && hatched(4, 6, 10) && !hatched(4, 7, 10));
        assert!(!(0..100).any(|x| hatched(x, 0, 0)), "no step, no lines");
    }

    #[test]
    fn the_revision_block_is_exact() {
        let mut drawn = 0;
        for ((w, h), phys_mm, scale, margin_mm) in cases() {
            let mg = margin(margin_mm, phys_mm / scale, scale);
            let zones = zones((w, h), mg, phys_mm);
            let case = format!("{w}x{h} {phys_mm} px/mm, scale {scale}, margin {margin_mm} mm");
            let t = title_block((w, h), mg, phys_mm as f32, zones);
            let Some(r) = revision_block((w, h), mg, phys_mm as f32, zones) else { continue };
            drawn += 1;
            // on the title block, as wide, up to the frame's outer edge; its top on a tick
            assert_eq!((r.rect.loc.x, r.rect.size.w), (t.rect.loc.x, t.rect.size.w), "{case}");
            assert_eq!(r.rect.loc.y + r.rect.size.h, t.rect.loc.y, "{case}");
            assert!((1..zones.1).any(|k| tick(h, mg, zones.1, k).0 == r.rect.loc.y), "{case}: top off the ticks");
            assert_eq!(r.cells.len(), 4 * (REV_ROWS + 1), "{case}");
            // no pixel twice, none off the block or on the frame's line; every inner pixel a rule's or a cell's
            let mut seen = std::collections::HashSet::new();
            for rule in &r.rules {
                for p in pixels(&rule.rect) {
                    assert!(seen.insert(p), "{case}: {p:?} ruled twice");
                    assert!(r.rect.contains(p) && p.0 < w - mg - LINE, "{case}: {p:?} off the block");
                }
            }
            for x in r.rect.loc.x..w - mg - LINE {
                for d in 0..LINE {
                    assert!(seen.contains(&(x, r.rect.loc.y + d)), "{case}: top rule gap at {x}");
                }
            }
            for c in &r.cells {
                assert!(c.size.w > 0 && c.size.h > 0, "{case}: an empty cell");
                for p in pixels(c) {
                    assert!(r.inner.contains(p), "{case}: cell {c:?} leaves the block");
                    assert!(seen.insert(p), "{case}: {p:?} in a cell and a rule, or two cells");
                }
            }
            for p in pixels(&r.inner) {
                assert!(seen.contains(&p), "{case}: {p:?} neither a rule nor a cell");
            }
            // none of it over the title block
            assert!(!r.rect.overlaps(t.rect), "{case}: over the title block");
        }
        assert!(drawn > 50, "a revision block on most displays ({drawn})");
    }

    #[test]
    fn the_scale_bar_is_true_to_size() {
        for ((w, h), phys_mm, scale, margin_mm) in cases() {
            let mg = margin(margin_mm, phys_mm / scale, scale);
            let zones = zones((w, h), mg, phys_mm);
            let case = format!("{w}x{h} {phys_mm} px/mm, scale {scale}, margin {margin_mm} mm");
            let Some(s) = scale_bar((w, h), mg, phys_mm, zones, true) else {
                assert!(zones.0 < 3 || margin_mm < 4.0, "{case}: no scale bar");
                continue;
            };
            let bar = s.bar.expect("measured: a bar");
            // in its zone: between the ticks, in the margin, under the title block's last zones
            assert!(s.zone.contains_rect(bar.rect), "{case}: the bar leaves its zone");
            assert_eq!(s.zone.loc.x, tick(w, mg, zones.0, zones.0 - 2).0 + 1, "{case}");
            assert!(bar.text.0 >= h - mg && bar.text.0 < bar.text.1 && bar.text.1 <= bar.rect.loc.y, "{case}: the numbers' rows");
            // true to size: each centimetre within half a pixel of 10 mm from the first boundary
            assert!(bar.marks.len() >= 2, "{case}: one centimetre at least");
            for &(x, d) in &bar.marks {
                let want = bar.marks[0].0 as f64 + d as f64 * phys_mm;
                assert!((x as f64 - want).abs() <= 0.5, "{case}: {d} mm at {x}, not {want}");
            }
            // no pixel twice, every pixel in the outline
            let mut seen = std::collections::HashSet::new();
            for r in &bar.ink {
                assert!(r.size.w > 0 && r.size.h > 0, "{case}: an empty rule");
                for p in pixels(r) {
                    assert!(seen.insert(p), "{case}: {p:?} drawn twice");
                    assert!(bar.rect.contains(p), "{case}: {p:?} off the bar");
                }
            }
            // the outline closed: top and bottom rows whole, a rule at each end
            for x in bar.rect.loc.x..bar.rect.loc.x + bar.rect.size.w {
                assert!(seen.contains(&(x, bar.rect.loc.y)) && seen.contains(&(x, bar.rect.loc.y + bar.rect.size.h - 1)), "{case}: gap at {x}");
            }
            // not measured: no bar, the zone kept for NTS
            let nts = scale_bar((w, h), mg, phys_mm, zones, false).expect("a zone");
            assert!(nts.bar.is_none() && nts.zone == s.zone, "{case}");
        }
        // the T14's panel: 1920x1200 at 6.4 px/mm (scale 1.5), margin 6 mm: zones of 48 mm, four centimetres
        let mg = margin(6.0, 6.4 / 1.5, 1.5);
        let z = zones((1920, 1200), mg, 6.4);
        let bar = scale_bar((1920, 1200), mg, 6.4, z, true).unwrap().bar.unwrap();
        assert_eq!(bar.marks.iter().map(|m| m.1).collect::<Vec<_>>(), [0, 10, 20, 30, 40], "zones {z:?}, zone {:?}", bar.rect);
        assert_eq!(bar.marks[1].0 - bar.marks[0].0, 64);
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
