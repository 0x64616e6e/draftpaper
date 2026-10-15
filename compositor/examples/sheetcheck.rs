//! Checks a screenshot of the sheet, pixel by pixel, against the geometry the compositor draws it from
//! (src/title.rs): the frame's line one colour along the title block, nothing of the block past the
//! frame, its outer rule whole and exactly as heavy as the frame's, the corner ticks on the frame's line.
//! `make pixels` takes the screenshots (a nested session at each scale) and runs this on them.
//!
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM [ambient] [windows]
//!
//! PX_PER_MM is physical, as the compositor logs it; `ambient` when the margins take a wallpaper's colours
//! (the frame's line is drawn over the picture then, and changes along it); `windows` when two windows are
//! open side by side (layout.rs): their borders a logical pixel's worth wide, whole, against the frame's
//! line and either side of the tick's pixel between them.

#[allow(dead_code)]
#[path = "../src/title.rs"]
mod title;
#[allow(dead_code)]
#[path = "../src/layout.rs"]
mod layout;

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
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mg = title::margin(margin_mm, phys_mm / scale, scale);
    let block = title::title_block((w, h), mg, phys_mm as f32);
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
    for x in left..right {
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
