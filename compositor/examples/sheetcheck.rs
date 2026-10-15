//! Checks a screenshot of the sheet, pixel by pixel, against the geometry the compositor draws it from
//! (src/title.rs): the frame's line one colour along the title block, nothing of the block past the
//! frame, its outer rule whole and exactly as heavy as the frame's, the corner ticks on the frame's line.
//! `make pixels` takes the screenshots (a nested session at each scale) and runs this on them.
//!
//!     sheetcheck SCREENSHOT.png SCALE PX_PER_MM MARGIN_MM [ambient]
//!
//! PX_PER_MM is physical, as the compositor logs it; `ambient` when the margins take a wallpaper's colours
//! (the frame's line is drawn over the picture then, and changes along it).

#[allow(dead_code)]
#[path = "../src/title.rs"]
mod title;

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
    let ambient = args.get(5).is_some_and(|a| a == "ambient");
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
    // the outer rule: two rows and two columns of one colour, paper (or picture) either side of them
    let rule = px(left + 4, top);
    for x in left..right - 2 {
        for y in [top, top + 1] {
            if diff(px(x, y), rule) > 6 {
                fault(format!("top rule at ({x}, {y}) is {:?}, not {:?}", px(x, y), rule));
            }
        }
        if diff(px(x, top - 1), rule) <= 6 && !lone(x, top - 1, 1, 0) {
            fault(format!("({x}, {}) above the top rule is as dark as the rule", top - 1));
        }
    }
    for y in top..bottom - 2 {
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

    if faults.is_empty() {
        println!("{}: exact ({w}x{h}, scale {scale}, margin {mg} px, title block {}x{} at {left},{top})", args[1], right - left, bottom - top);
    } else {
        println!("{}: {} faults", args[1], faults.len());
        std::process::exit(1);
    }
}
