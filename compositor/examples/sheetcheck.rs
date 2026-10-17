//! Checks a screenshot of the sheet, pixel by pixel, against the geometry the compositor draws it from
//! (src/title.rs): the frame's line one colour along the title block, nothing of the block past the
//! frame, its outer rule whole and exactly as heavy as the frame's, the corner ticks on the frame's line.
//! `make pixels` takes the screenshots (a nested session at each scale) and runs this on them.
//!
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM [ambient] [windows]
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM lens BEFORE.png X Y
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM scale bar|nts|none
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM revisions shown|none
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM hatch CELL SPACING_MM|none
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM keyplan THIS FOCUSED X,Y,W,H X,Y,W,H...
//!
//! PX_PER_MM is physical, as the compositor logs it; `ambient` when the margins take a wallpaper's colours
//! (the frame's line is drawn over the picture then, and changes along it); `windows` when two windows are
//! open side by side (layout.rs): their borders a logical pixel's worth wide, whole, against the frame's
//! line and either side of the tick's pixel between them.
//! With `lens`, the magnifier (src/lens.rs) on the pointer at X, Y (logical), at its first level and size:
//! each pixel of BEFORE (the same screen without it) a block of exactly its colour, the grid between them,
//! the hotspot's pixel outlined.

#[allow(dead_code)]
#[path = "../src/title.rs"]
mod title;
#[allow(dead_code)]
#[path = "../src/layout.rs"]
mod layout;
#[allow(dead_code)]
#[path = "../src/lens.rs"]
mod lens;

use image::{Rgba, RgbaImage};

fn diff(a: Rgba<u8>, b: Rgba<u8>) -> i32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).abs()).max().unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM [ambient]");
        std::process::exit(2);
    }
    let img: RgbaImage = image::open(&args[1]).expect("a screenshot").to_rgba8();
    let (scale, phys_mm, margin_mm): (f64, f64, f64) = (args[2].parse().unwrap(), args[3].parse().unwrap(), args[4].parse().unwrap());
    let ambient = args[5..].iter().any(|a| a == "ambient");
    let windows = args[5..].iter().any(|a| a == "windows");
    if args.get(5).is_some_and(|a| a == "keyplan") {
        let n = |i: usize| args.get(i).and_then(|v| v.parse::<usize>().ok()).expect("keyplan THIS FOCUSED X,Y,W,H...");
        let layout: Vec<(i32, i32, i32, i32)> = args[8..]
            .iter()
            .map(|l| {
                let v: Vec<i32> = l.split(',').filter_map(|x| x.parse().ok()).collect();
                (v[0], v[1], v[2], v[3])
            })
            .collect();
        keyplan_check(&img, scale, phys_mm, margin_mm, &layout, n(6), n(7), &args[1]);
        return;
    }
    if args.get(5).is_some_and(|a| a == "hatch") {
        let cell: usize = args.get(6).and_then(|c| c.parse().ok()).expect("hatch CELL SPACING_MM|none");
        let spacing = args.get(7).and_then(|s| s.parse::<f64>().ok());
        hatch_check(&img, scale, phys_mm, margin_mm, cell, spacing, &args[1]);
        return;
    }
    if args.get(5).is_some_and(|a| a == "revisions") {
        revisions_check(&img, scale, phys_mm, margin_mm, args.get(6).map(String::as_str) != Some("none"), &args[1]);
        return;
    }
    if args.get(5).is_some_and(|a| a == "scale") {
        scale_check(&img, scale, phys_mm, margin_mm, args.get(6).map(String::as_str).unwrap_or("bar"), &args[1]);
        return;
    }
    if args.get(5).is_some_and(|a| a == "lens") {
        let before: RgbaImage = image::open(&args[6]).expect("the screenshot before the lens").to_rgba8();
        let (x, y): (f64, f64) = (args[7].parse().unwrap(), args[8].parse().unwrap());
        lens_check(&img, &before, scale, phys_mm, (x, y), &args[1]);
        return;
    }
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mg = title::margin(margin_mm, phys_mm / scale, scale);
    let block = title::title_block((w, h), mg, phys_mm as f32, title::zones((w, h), mg, phys_mm));
    let (left, top, right, bottom) = (block.rect.loc.x, block.rect.loc.y, w - mg, h - mg);
    let px = |x: i32, y: i32| *img.get_pixel(x as u32, y as u32);
    // a dot of the 5 mm pattern, or a letter: unlike both neighbours across the line being checked
    let lone = |x: i32, y: i32, dx: i32, dy: i32| diff(px(x, y), px(x - dx, y - dy)) > 8 && diff(px(x, y), px(x + dx, y + dy)) > 8;
    let mut faults = Vec::new();
    let mut fault = |what: String| {
        if faults.len() < 20 {
            eprintln!("  {what}");
        }
        faults.push(what);
    };

    // the frame's line along the block, as it is just above the block (both pixels of its width)
    if !ambient {
        for x in [right - 2, right - 1] {
            let above = px(x, top - 4);
            for y in top..bottom - 2 {
                if diff(px(x, y), above) > 4 {
                    fault(format!("frame line at ({x}, {y}) is {:?}, above the block {:?}", px(x, y), above));
                }
            }
        }
        for y in [bottom - 2, bottom - 1] {
            let before = px(left - 4, y);
            for x in left..right - 2 {
                if diff(px(x, y), before) > 4 {
                    fault(format!("frame line at ({x}, {y}) is {:?}, left of the block {:?}", px(x, y), before));
                }
            }
        }
    }
    // nothing of the block past the frame: the margin's first pixels as the margin further out
    for y in top..bottom {
        for x in [right, right + 1] {
            if diff(px(x, y), px(x + 3, y)) > 6 && !lone(x, y, 0, 1) && !lone(x + 3, y, 0, 1) {
                fault(format!("margin at ({x}, {y}) is {:?}, further out {:?}: the block crosses the frame", px(x, y), px(x + 3, y)));
            }
        }
    }
    // the scale bar's zone left out: its numbers are lettered there (a block crossing the frame would show
    // along the rest of its width)
    let scale_zone = title::scale_bar((w, h), mg, phys_mm, title::zones((w, h), mg, phys_mm), true).map(|s| s.zone);
    for x in (left..right).filter(|&x| !scale_zone.is_some_and(|z| x >= z.loc.x && x < z.loc.x + z.size.w)) {
        for y in [bottom, bottom + 1] {
            if diff(px(x, y), px(x, y + 3)) > 6 && !lone(x, y, 1, 0) && !lone(x, y + 3, 1, 0) {
                fault(format!("margin at ({x}, {y}) is {:?}, further out {:?}: the block crosses the frame", px(x, y), px(x, y + 3)));
            }
        }
    }
    // the outer rule: two rows and two columns of one colour, paper (or picture) either side of them (a
    // window covers it when there are windows)
    let rule = px(left + 4, top);
    for x in (left..right - 2).filter(|_| !windows) {
        for y in [top, top + 1] {
            if diff(px(x, y), rule) > 6 {
                fault(format!("top rule at ({x}, {y}) is {:?}, not {:?}", px(x, y), rule));
            }
        }
        if diff(px(x, top - 1), rule) <= 6 && !lone(x, top - 1, 1, 0) {
            fault(format!("({x}, {}) above the top rule is as dark as the rule", top - 1));
        }
    }
    for y in (top..bottom - 2).filter(|_| !windows) {
        for x in [left, left + 1] {
            if diff(px(x, y), rule) > 6 {
                fault(format!("left rule at ({x}, {y}) is {:?}, not {:?}", px(x, y), rule));
            }
        }
    }
    // the corner ticks: in the bottom margin under the frame's right line, in the right margin beside its
    // bottom line, the frame line's two pixels and only those
    if !ambient {
        let tick = px(right - 1, bottom + mg / 2);
        for y in bottom + 1..h - 1 {
            for (x, on) in [(right - 3, false), (right - 2, true), (right - 1, true), (right, false)] {
                let same = diff(px(x, y), tick) <= 6;
                if same != on && !lone(x, y, 0, 1) {
                    fault(format!("bottom margin at ({x}, {y}): the corner tick should {}be here", if on { "" } else { "not " }));
                }
            }
        }
        let tick = px(right + mg / 2, bottom - 1);
        for x in right + 1..w - 1 {
            for (y, on) in [(bottom - 3, false), (bottom - 2, true), (bottom - 1, true), (bottom, false)] {
                let same = diff(px(x, y), tick) <= 6;
                if same != on && !lone(x, y, 1, 0) {
                    fault(format!("right margin at ({x}, {y}): the corner tick should {}be here", if on { "" } else { "not " }));
                }
            }
        }
    }

    if windows {
        // the grid as the compositor has it (an output at 0,0), and the two columns of two windows
        let size = smithay::utils::Size::from(((w as f64 / scale).ceil() as i32, (h as f64 / scale).ceil() as i32));
        let grid = layout::Grid::new((0, 0).into(), size, phys_mm / scale, margin_mm, scale, (w, h));
        let n0 = grid.zones.0 - grid.zones.0 / 2;
        let b = grid.border();
        let tiles = [grid.tile(0, n0, 0.0, grid.zones.1 as f64).pixels, grid.tile(n0, grid.zones.0, 0.0, grid.zones.1 as f64).pixels];
        for (i, p) in tiles.iter().enumerate() {
            let (x0, y0, x1, y1) = (p.loc.x, p.loc.y, p.loc.x + p.size.w, p.loc.y + p.size.h);
            let colour = px(x0 - 1, (y0 + y1) / 2);
            // the four bands, b pixels each, one colour; just outside and inside them, something else
            let mut band = |x: i32, y: i32, outside: (i32, i32), inside: (i32, i32)| {
                if diff(px(x, y), colour) > 3 {
                    fault(format!("window {i}: border at ({x}, {y}) is {:?}, not {:?}", px(x, y), colour));
                }
                for (q, what) in [(outside, "outside"), (inside, "inside")] {
                    if diff(px(q.0, q.1), colour) <= 3 {
                        fault(format!("window {i}: ({}, {}) {what} its border is the border's colour", q.0, q.1));
                    }
                }
            };
            for y in [y0 + 5, (y0 + y1) / 2, y1 - 5] {
                for d in 1..=b {
                    band(x0 - d, y, (x0 - b - 1, y), (x0, y));
                    band(x1 - 1 + d, y, (x1 + b, y), (x1 - 1, y));
                }
            }
            for x in [x0 + 5, (x0 + x1) / 2, x1 - 5] {
                for d in 1..=b {
                    band(x, y0 - d, (x, y0 - b - 1), (x, y0));
                    band(x, y1 - 1 + d, (x, y1 + b), (x, y1 - 1));
                }
            }
        }
        // between them the tick's pixel, under the tick in the top margin
        let t = tiles[0].loc.x + tiles[0].size.w + b;
        if tiles[1].loc.x - b != t + 1 {
            fault(format!("the gutter between the windows is {} pixels, not one", tiles[1].loc.x - b - t));
        }
        if diff(px(t, mg / 2), px(t - 3, mg / 2)) <= 6 {
            fault(format!("no tick in the top margin at x = {t}, above the gutter"));
        }
    }

    if faults.is_empty() {
        println!("{}: exact ({w}x{h}, scale {scale}, margin {mg} px, title block {}x{} at {left},{top})", args[1], right - left, bottom - top);
    } else {
        println!("{}: {} faults", args[1], faults.len());
        std::process::exit(1);
    }
}

/// The magnifier's check: `img` with the lens on the pointer at `at` (logical), `before` without it.
/// The key plan (title.rs's key_plan) of screen `this` among `layout`, `focused` the active one: every
/// pixel of the outlines drawn, this screen's inside filled, the others' insides paper. Drawn: apart from
/// the paper (the median of the zone's pixels) by more than 25 for the outlines, 12 for the fill.
fn keyplan_check(img: &RgbaImage, scale: f64, phys_mm: f64, margin_mm: f64, layout: &[(i32, i32, i32, i32)], this: usize, focused: usize, name: &str) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mg = title::margin(margin_mm, phys_mm / scale, scale);
    let Some(p) = title::key_plan((w, h), mg, phys_mm, title::zones((w, h), mg, phys_mm), layout, this, focused) else {
        println!("{name}: no key plan");
        std::process::exit(1);
    };
    let lum = |x: i32, y: i32| {
        let q = img.get_pixel(x as u32, y as u32);
        0.2126 * q[0] as f64 + 0.7152 * q[1] as f64 + 0.0722 * q[2] as f64
    };
    let mut all: Vec<f64> = pixels(&p.zone).map(|(x, y)| lum(x, y)).collect();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let paper = all[all.len() / 2];
    let mut faults = Vec::new();
    for r in &p.rules {
        for (x, y) in pixels(&r.rect) {
            if (lum(x, y) - paper).abs() <= 25.0 {
                faults.push(format!("outline pixel ({x}, {y}) not drawn"));
            }
        }
    }
    // the north arrow in its square (the shell's: drawn there, its way the IPC's bearing)
    let needle = pixels(&p.arrow).filter(|&(x, y)| (lum(x, y) - paper).abs() > 25.0).count();
    if needle < 15 {
        faults.push(format!("no north arrow in its square ({needle} pixels)"));
    }
    let filled = pixels(&p.fill).filter(|&(x, y)| (lum(x, y) - paper).abs() > 12.0).count();
    if filled * 10 < p.fill.size.w as usize * p.fill.size.h as usize * 9 {
        faults.push(format!("this screen not filled ({filled} pixels)"));
    }
    // the other screens' insides: inside their outlines, not this one's fill, not an outline
    let outline: std::collections::HashSet<(i32, i32)> = p.rules.iter().flat_map(|r| pixels(&r.rect).collect::<Vec<_>>()).collect();
    let (mut inside, mut drawn) = (0, 0);
    for (i, r) in p.rules.chunks(4).enumerate() {
        if i == this {
            continue;
        }
        let (top, bottom) = (r[0].rect, r[1].rect);
        for y in top.loc.y + top.size.h..bottom.loc.y {
            for x in top.loc.x..top.loc.x + top.size.w {
                if !outline.contains(&(x, y)) {
                    inside += 1;
                    if (lum(x, y) - paper).abs() > 12.0 {
                        drawn += 1;
                    }
                }
            }
        }
    }
    if drawn * 10 > inside {
        faults.push(format!("another screen filled ({drawn} of {inside})"));
    }
    for f in faults.iter().take(10) {
        eprintln!("  {f}");
    }
    if faults.is_empty() {
        println!("{name}: key plan, screen {this} of {}, focused {focused}: exact ({}x{} filled at {},{})", layout.len(), p.fill.size.w, p.fill.size.h, p.fill.loc.x, p.fill.loc.y);
    } else {
        println!("{name}: key plan: {} faults", faults.len());
        std::process::exit(1);
    }
}

fn pixels(r: &smithay::utils::Rectangle<i32, smithay::utils::Physical>) -> impl Iterator<Item = (i32, i32)> + '_ {
    (r.loc.y..r.loc.y + r.size.h).flat_map(move |y| (r.loc.x..r.loc.x + r.size.w).map(move |x| (x, y)))
}

/// A parking cell's hatching (title.rs's side_cell and hatched): with `spacing` (mm), the lines drawn on
/// their pixels (each standing apart from its right-hand neighbour, which is off the lines) along a quarter
/// of the cell's lines at least (the rest is where the shell letters it, left clear: in a short cell, more
/// than half). With none, a tenth at most (the lettering's own strokes), counted at 1.5 mm.
fn hatch_check(img: &RgbaImage, scale: f64, phys_mm: f64, margin_mm: f64, cell: usize, spacing: Option<f64>, name: &str) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mg = title::margin(margin_mm, phys_mm / scale, scale);
    let r = title::side_cell((w, h), mg, title::zones((w, h), mg, phys_mm), cell).expect("a cell");
    let step = title::hatch_step(spacing.unwrap_or(1.5), phys_mm);
    let lum = |x: i32, y: i32| {
        let p = img.get_pixel(x as u32, y as u32);
        0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64
    };
    let (mut on, mut lines) = (0, 0);
    for y in r.loc.y..r.loc.y + r.size.h {
        for x in r.loc.x..r.loc.x + r.size.w - 1 {
            if title::hatched(x, y, step) {
                lines += 1;
                if (lum(x, y) - lum(x + 1, y)).abs() > 12.0 {
                    on += 1;
                }
            }
        }
    }
    let share = on as f64 / lines.max(1) as f64;
    let fine = match spacing {
        Some(_) => share >= 0.25,
        None => share <= 0.10,
    };
    let what = match spacing {
        Some(s) => format!("hatched every {step} px ({s} mm)"),
        None => "not hatched".into(),
    };
    if fine {
        println!("{name}: cell {cell} {what}: exact ({on} of the lines' {lines} pixels drawn)");
    } else {
        println!("{name}: cell {cell} {what}: {on} of the lines' {lines} pixels drawn");
        std::process::exit(1);
    }
}

/// The revision block (title.rs's revision_block) above the title block: when `shown`, every pixel of its
/// rules drawn, and the first row's description lettered; else none of its rules drawn. A pixel is drawn
/// when it stands apart from the paper (the median of the block's inner pixels).
fn revisions_check(img: &RgbaImage, scale: f64, phys_mm: f64, margin_mm: f64, shown: bool, name: &str) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mg = title::margin(margin_mm, phys_mm / scale, scale);
    let Some(b) = title::revision_block((w, h), mg, phys_mm as f32, title::zones((w, h), mg, phys_mm)) else {
        println!("{name}: no room for a revision block");
        std::process::exit(if shown { 1 } else { 0 });
    };
    let lum = |x: i32, y: i32| {
        let p = img.get_pixel(x as u32, y as u32);
        0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64
    };
    let mut inner: Vec<f64> = (b.inner.loc.y..b.inner.loc.y + b.inner.size.h)
        .step_by(3)
        .flat_map(|y| (b.inner.loc.x..b.inner.loc.x + b.inner.size.w).step_by(3).map(move |x| (x, y)))
        .map(|(x, y)| lum(x, y))
        .collect();
    inner.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let paper = inner[inner.len() / 2];
    let drawn = |x: i32, y: i32| (lum(x, y) - paper).abs() > 25.0;
    let mut faults = Vec::new();
    let (mut on, mut all) = (0, 0);
    for r in &b.rules {
        for y in r.rect.loc.y..r.rect.loc.y + r.rect.size.h {
            for x in r.rect.loc.x..r.rect.loc.x + r.rect.size.w {
                all += 1;
                if drawn(x, y) {
                    on += 1;
                } else if shown {
                    faults.push(format!("rule pixel ({x}, {y}) not drawn"));
                }
            }
        }
    }
    if shown {
        let d = b.cells[5];
        let n = (d.loc.y..d.loc.y + d.size.h).flat_map(|y| (d.loc.x..d.loc.x + d.size.w).map(move |x| (x, y))).filter(|&(x, y)| drawn(x, y)).count();
        if n < 20 {
            faults.push("the first row's description not lettered".into());
        }
    } else if on * 2 > all {
        faults.push(format!("{on} of its {all} rule pixels drawn"));
    }
    for f in faults.iter().take(10) {
        eprintln!("  {f}");
    }
    if faults.is_empty() {
        println!("{name}: revision block {}: exact ({}x{} at {},{}, {} rule pixels)", if shown { "shown" } else { "none" }, b.rect.size.w, b.rect.size.h, b.rect.loc.x, b.rect.loc.y, all);
    } else {
        println!("{name}: revision block: {} faults", faults.len());
        std::process::exit(1);
    }
}

/// The scale bar (title.rs's scale_bar) in the bottom margin, as `expect` says it should be: `bar`, every
/// pixel of its rules and fills drawn and each centimetre's number lettered above its rule; `nts`, no bar
/// and the zone lettered (NTS); `none`, no bar. A pixel is drawn when it stands apart from the paper
/// right under the bar (the median of five, so a dot of the pattern there does not count).
fn scale_check(img: &RgbaImage, scale: f64, phys_mm: f64, margin_mm: f64, expect: &str, name: &str) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mg = title::margin(margin_mm, phys_mm / scale, scale);
    let zones = title::zones((w, h), mg, phys_mm);
    let Some(s) = title::scale_bar((w, h), mg, phys_mm, zones, true) else {
        println!("{name}: no zone for a scale bar ({} zones across)", zones.0);
        std::process::exit(if expect == "none" { 0 } else { 1 });
    };
    let bar = s.bar.expect("measured: a bar");
    let lum = |p: Rgba<u8>| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    let px = |x: i32, y: i32| *img.get_pixel(x as u32, y as u32);
    let under = bar.rect.loc.y + bar.rect.size.h;
    let paper = |x: i32| {
        let mut v: Vec<f64> = (x - 2..=x + 2).map(|x| lum(px(x, under))).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[2]
    };
    let drawn = |x: i32, y: i32| (lum(px(x, y)) - paper(x)).abs() > 30.0;
    let mut faults = Vec::new();
    match expect {
        "bar" => {
            for r in &bar.ink {
                for y in r.loc.y..r.loc.y + r.size.h {
                    for x in r.loc.x..r.loc.x + r.size.w {
                        if !drawn(x, y) {
                            faults.push(format!("({x}, {y}) of the bar not drawn"));
                        }
                    }
                }
            }
            let half = (1.5 * phys_mm).round() as i32;
            for &(x, mm) in &bar.marks {
                let n = (bar.text.0..bar.text.1).flat_map(|y| (x - half..=x + half).map(move |x| (x, y))).filter(|&(x, y)| drawn(x, y)).count();
                if n < 4 {
                    faults.push(format!("{mm} not lettered over its rule at {x}"));
                }
            }
        }
        _ => {
            let row = (bar.rect.loc.x..bar.rect.loc.x + bar.rect.size.w).filter(|&x| drawn(x, bar.rect.loc.y)).count();
            if row * 2 > bar.rect.size.w as usize {
                faults.push(format!("a bar drawn ({row} of its top row's {} pixels)", bar.rect.size.w));
            }
            if expect == "nts" {
                let n = (s.zone.loc.y..s.zone.loc.y + s.zone.size.h)
                    .flat_map(|y| (s.zone.loc.x..s.zone.loc.x + s.zone.size.w).map(move |x| (x, y)))
                    .filter(|&(x, y)| drawn(x, y))
                    .count();
                if n < 10 {
                    faults.push("the zone not lettered NTS".into());
                }
            }
        }
    }
    for f in faults.iter().take(10) {
        eprintln!("  {f}");
    }
    if faults.is_empty() {
        let cm = bar.marks.len() - 1;
        println!("{name}: scale bar {expect}: exact ({cm} cm of {} px at {},{}, 10 mm = {} px)", bar.rect.size.w, bar.rect.loc.x, bar.rect.loc.y, bar.marks[1].0 - bar.marks[0].0);
    } else {
        println!("{name}: scale bar {expect}: {} faults", faults.len());
        std::process::exit(1);
    }
}

fn lens_check(img: &RgbaImage, before: &RgbaImage, scale: f64, phys_mm: f64, at: (f64, f64), name: &str) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let lens = lens::Lens::default();
    let level = lens.level();
    let hotspot = smithay::utils::Point::from((at.0 * scale, at.1 * scale));
    let (src, dst, c) = lens::place((w, h), hotspot, level, (lens.size_mm * phys_mm).round());
    let l = level as i32;
    let outline = if level >= 12.0 { 2 } else { 1 };
    let mut faults = 0;
    let mut fault = |what: String| {
        if faults < 20 {
            eprintln!("  {what}");
        }
        faults += 1;
    };
    for j in 0..src.size.h {
        for i in 0..src.size.w {
            let (sx, sy) = (src.loc.x + i, src.loc.y + j);
            let orig = *before.get_pixel(sx as u32, sy as u32);
            let grey = |v: u8| (v as f32 * 0.7 + 0.5 * 255.0 * 0.3).round() as i32;
            for py in 0..l {
                for px in 0..l {
                    let (x, y) = (dst.loc.x + i * l + px, dst.loc.y + j * l + py);
                    if x < 0 || y < 0 || x >= w || y >= h {
                        continue;
                    }
                    let got = *img.get_pixel(x as u32, y as u32);
                    let edge = px < outline || py < outline || px >= l - outline || py >= l - outline;
                    if (sx, sy) == c && edge {
                        // the hotspot's outline: black or white
                        if !(got[0] == got[1] && got[1] == got[2] && (got[0] < 3 || got[0] > 252)) {
                            fault(format!("({x}, {y}): the hotspot's outline is {got:?}"));
                        }
                        continue;
                    }
                    let want: [i32; 3] = if level >= 6.0 && (px < 1 || py < 1) {
                        [grey(orig[0]), grey(orig[1]), grey(orig[2])]
                    } else {
                        [orig[0] as i32, orig[1] as i32, orig[2] as i32]
                    };
                    if (0..3).any(|k| (got[k] as i32 - want[k]).abs() > 2) {
                        fault(format!("({x}, {y}) in the block of ({sx}, {sy}) is {got:?}, its pixel {orig:?}"));
                    }
                }
            }
        }
    }
    // the frame: a dark pixel outside a light one, all round
    for (x, y) in [(dst.loc.x - 2, dst.loc.y + 5), (dst.loc.x + 5, dst.loc.y - 2), (dst.loc.x + dst.size.w + 1, dst.loc.y + 5), (dst.loc.x + 5, dst.loc.y + dst.size.h + 1)] {
        let (dx, dy) = ((x - dst.loc.x).signum().min(0) + (x >= dst.loc.x + dst.size.w) as i32, (y - dst.loc.y).signum().min(0) + (y >= dst.loc.y + dst.size.h) as i32);
        let (outer, inner) = (*img.get_pixel(x as u32, y as u32), *img.get_pixel((x - dx) as u32, (y - dy) as u32));
        if !(outer[0] < inner[0] && outer[1] < inner[1] && outer[2] < inner[2]) {
            fault(format!("the lens's frame at ({x}, {y}): {outer:?} outside {inner:?}"));
        }
    }
    if faults == 0 {
        println!("{name}: exact (the lens {}x{} at {},{}, {level} times, the hotspot {c:?})", dst.size.w, dst.size.h, dst.loc.x, dst.loc.y);
    } else {
        println!("{name}: {faults} faults");
        std::process::exit(1);
    }
}
