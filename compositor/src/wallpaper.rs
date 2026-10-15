//! The wallpaper: a picture mounted in the sheet's frame. config.toml's `wallpaper` names it; it fills
//! the frame's interior on every output (cropped to the interior's proportions, where
//! `wallpaper_position` says), or with `wallpaper_fit` sits whole (`fit`) or at its own size (`centre`)
//! on the paper; `wallpaper_blur` (mm), `wallpaper_brightness` and `wallpaper_wash` (the paper laid over
//! it) change how it looks. The
//! margins stay the theme's paper, or with `margins = "ambient"` take the picture's colours: each margin
//! cell the average of the picture's edge beside it, neighbouring cells blending softly, and the margins'
//! lettering in whichever of the theme's inks reads on them (`margin_ink`, sent to the shell). Under the
//! title block (title.rs: 130 by 36 mm in the frame's bottom-right corner, up to its line) the picture
//! is blurred and tinted with the paper (`title_tint`), so the readings stay legible.
//!
//! Everything is prepared once per output, size and theme (decoding, cropping, averaging, blurring on
//! the CPU) and drawn as plain textures under the sheet's lines (render.rs).

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use image::{imageops, imageops::FilterType, Rgba, RgbaImage};
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::{Physical, Rectangle, Transform},
};

use crate::theme::Theme;

struct Picture {
    path: PathBuf,
    modified: Option<SystemTime>,
    image: Arc<RgbaImage>,
}

thread_local! {
    /// the decoded picture, shared by the outputs; decoded again when the file or the name changes
    static PICTURE: RefCell<Option<Picture>> = const { RefCell::new(None) };
}

fn picture(path: &Path) -> Option<(Arc<RgbaImage>, Option<SystemTime>)> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    PICTURE.with(|p| {
        let mut p = p.borrow_mut();
        if let Some(pic) = p.as_ref().filter(|pic| pic.path == path && pic.modified == modified) {
            return Some((pic.image.clone(), modified));
        }
        match image::open(path) {
            Ok(img) => {
                let image = Arc::new(img.to_rgba8());
                *p = Some(Picture { path: path.to_path_buf(), modified, image: image.clone() });
                Some((image, modified))
            }
            Err(e) => {
                if p.as_ref().is_none_or(|pic| pic.path != path || pic.modified != modified) {
                    eprintln!("draftpaper: wallpaper {}: {e}", path.display());
                }
                *p = Some(Picture { path: path.to_path_buf(), modified, image: Arc::new(RgbaImage::new(0, 0)) });
                None
            }
        }
    })
    .filter(|(img, _)| img.width() > 0)
}

/// What an output's wallpaper was prepared for: rebuilt when any of it changes.
#[derive(PartialEq, Clone)]
struct Key {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: (i32, i32),
    margin: i32,
    zones: (u32, u32),
    mm: u32,
    ambient: bool,
    tint: u32,
    paper: [u32; 3],
    ink: [u32; 3],
    mount: [u32; 6],
}

/// One output's wallpaper, in its physical pixels, relative to its corner.
pub struct OutputWallpaper {
    key: Key,
    /// the frame's interior and the picture cropped to fill it
    pub interior: Rectangle<i32, Physical>,
    pub picture: MemoryRenderBuffer,
    /// the margins' colours, at an eighth of the output's size (drawn stretched over it, smoothly)
    pub ambient: Option<(MemoryRenderBuffer, (i32, i32))>,
    /// the title block's place and the picture there, blurred and tinted
    pub title: Rectangle<i32, Physical>,
    pub frosted: MemoryRenderBuffer,
    /// the margins' lettering: the theme's ink or its counterpart, whichever reads on the ambient colours
    pub margin_ink: Option<[f32; 3]>,
}

/// The output's wallpaper cache (in its user data).
#[derive(Default)]
pub struct WallpaperCache(pub RefCell<Option<OutputWallpaper>>);

fn bits(c: [f32; 3]) -> [u32; 3] {
    c.map(f32::to_bits)
}

fn rgba(c: [f32; 3]) -> Rgba<u8> {
    Rgba([(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255])
}

fn luminance(c: [f32; 3]) -> f32 {
    let l = |x: f32| if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
    0.2126 * l(c[0]) + 0.7152 * l(c[1]) + 0.0722 * l(c[2])
}

fn contrast(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The average colour of a region of `img`.
fn average(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let (x, y) = (x.min(img.width().saturating_sub(1)), y.min(img.height().saturating_sub(1)));
    let (w, h) = (w.max(1).min(img.width() - x), h.max(1).min(img.height() - y));
    let mut sum = [0f64; 3];
    let step = ((w * h) as f64 / 4096.0).sqrt().max(1.0) as u32;     // a sample is enough
    let mut n = 0f64;
    for yy in (y..y + h).step_by(step as usize) {
        for xx in (x..x + w).step_by(step as usize) {
            let p = img.get_pixel(xx, yy);
            for i in 0..3 {
                sum[i] += p[i] as f64;
            }
            n += 1.0;
        }
    }
    sum.map(|s| (s / n.max(1.0) / 255.0) as f32)
}

fn buffer(img: &RgbaImage) -> MemoryRenderBuffer {
    MemoryRenderBuffer::from_slice(img.as_raw(), Fourcc::Abgr8888, (img.width() as i32, img.height() as i32), 1, Transform::Normal, None)
}

/// The picture as the frame's interior (`iw` by `ih` physical pixels) shows it: filled (cropped to the
/// interior's proportions, where the anchor says), fitted whole or at its own size (on the paper, where
/// the anchor says); then blurred, darkened and washed with the paper as config.toml asks. The ambient
/// margins and the title block take their colours from this, as you see it.
fn mounted(img: &RgbaImage, iw: u32, ih: u32, theme: &Theme, mm: f32) -> RgbaImage {
    use crate::theme::Fit;
    let m = &theme.mount;
    let (ax, ay) = (m.anchor.0 as f64, m.anchor.1 as f64);
    let (pw, ph) = (img.width(), img.height());
    let mut out = match m.fit {
        Fit::Fill => {
            let scale = (iw as f64 / pw as f64).max(ih as f64 / ph as f64);
            let (cw, ch) = (((iw as f64 / scale).round() as u32).min(pw), ((ih as f64 / scale).round() as u32).min(ph));
            let (x, y) = (((pw - cw) as f64 * ax).round() as u32, ((ph - ch) as f64 * ay).round() as u32);
            let crop = imageops::crop_imm(img, x, y, cw, ch).to_image();
            imageops::resize(&crop, iw, ih, FilterType::Triangle)
        }
        Fit::Fit | Fit::Centre => {
            // the paper, and the picture on it: scaled to fit whole, or at its own size (cropped where it
            // is larger than the interior)
            let mut canvas = RgbaImage::from_pixel(iw, ih, rgba(theme.paper));
            let pic = if m.fit == Fit::Fit {
                let scale = (iw as f64 / pw as f64).min(ih as f64 / ph as f64);
                imageops::resize(img, ((pw as f64 * scale).round() as u32).max(1), ((ph as f64 * scale).round() as u32).max(1), FilterType::Triangle)
            } else {
                let (cw, ch) = (pw.min(iw), ph.min(ih));
                imageops::crop_imm(img, ((pw - cw) as f64 * ax).round() as u32, ((ph - ch) as f64 * ay).round() as u32, cw, ch).to_image()
            };
            let x = ((iw - pic.width().min(iw)) as f64 * ax).round() as i64;
            let y = ((ih - pic.height().min(ih)) as f64 * ay).round() as i64;
            imageops::overlay(&mut canvas, &pic, x, y);
            canvas
        }
    };
    if m.blur_mm > 0.0 {
        // blurred small and scaled back up: a wide blur at full size would take seconds
        let r = (m.blur_mm * mm).max(1.0);
        let k = (r / 4.0).max(1.0);
        let small = imageops::resize(&out, ((iw as f32 / k) as u32).max(2), ((ih as f32 / k) as u32).max(2), FilterType::Triangle);
        let small = imageops::blur(&small, (r / k).max(0.5));
        out = imageops::resize(&small, iw, ih, FilterType::Triangle);
    }
    if m.brightness < 1.0 || m.wash > 0.0 {
        let (b, wash, paper) = (m.brightness, m.wash, rgba(theme.paper));
        for p in out.pixels_mut() {
            for i in 0..3 {
                let v = p[i] as f32 * b;
                p[i] = (v * (1.0 - wash) + paper[i] as f32 * wash).round().clamp(0.0, 255.0) as u8;
            }
            p[3] = 255;
        }
    }
    out
}

/// Prepare (or keep) the wallpaper of an output of `size` physical pixels, its margin `margin` pixels,
/// `zones` across and down, `mm` pixels per millimetre; false without a wallpaper or when it cannot be read.
pub fn prepare(cache: &WallpaperCache, theme: &Theme, size: (i32, i32), margin: i32, zones: (u32, u32), mm: f32) -> bool {
    let Some(path) = theme.wallpaper.as_ref() else {
        cache.0.borrow_mut().take();
        return false;
    };
    let Some((img, modified)) = picture(path) else {
        cache.0.borrow_mut().take();
        return false;
    };
    let key = Key {
        path: path.clone(),
        modified,
        size,
        margin,
        zones,
        mm: mm.to_bits(),
        ambient: theme.ambient,
        tint: theme.title_tint.to_bits(),
        paper: bits(theme.paper),
        ink: bits(theme.ink),
        mount: {
            let m = &theme.mount;
            [m.fit as u32, m.anchor.0.to_bits(), m.anchor.1.to_bits(), m.brightness.to_bits(), m.wash.to_bits(), m.blur_mm.to_bits()]
        },
    };
    if cache.0.borrow().as_ref().is_some_and(|w| w.key == key) {
        return true;
    }
    let (w, h) = size;
    let interior = Rectangle::<i32, Physical>::new((margin, margin).into(), ((w - 2 * margin).max(1), (h - 2 * margin).max(1)).into());
    let (iw, ih) = (interior.size.w as u32, interior.size.h as u32);
    let filled = mounted(&img, iw, ih, theme, mm);

    // ambient margins: each cell the average of the picture's edge beside it, at an eighth of the size,
    // softened so that neighbouring cells blend; corners take the picture's corners
    let (ambient, margin_ink) = if theme.ambient {
        let (sw, sh) = ((w / 8).max(4) as u32, (h / 8).max(4) as u32);
        let mut small = RgbaImage::new(sw, sh);
        let (nx, ny) = (zones.0.max(1), zones.1.max(1));
        let band = (iw.min(ih) / 6).max(1);
        let (zw, zh) = (iw as f64 / nx as f64, ih as f64 / ny as f64);
        let mut seen = Vec::new();
        let to_small = |v: f64, full: i32, small: u32| ((v / full as f64) * small as f64) as u32;
        let fill = |small: &mut RgbaImage, x0: f64, y0: f64, x1: f64, y1: f64, c: [f32; 3]| {
            let (sx0, sy0, sx1, sy1) = (to_small(x0, w, sw), to_small(y0, h, sh), to_small(x1, w, sw).min(sw), to_small(y1, h, sh).min(sh));
            for y in sy0..sy1.max(sy0 + 1).min(sh) {
                for x in sx0..sx1.max(sx0 + 1).min(sw) {
                    small.put_pixel(x, y, rgba(c));
                }
            }
        };
        let m = margin as f64;
        for i in 0..nx {
            let x = (i as f64 * zw) as u32;
            let top = average(&filled, x, 0, zw as u32, band);
            let bottom = average(&filled, x, ih - band, zw as u32, band);
            fill(&mut small, m + i as f64 * zw, 0.0, m + (i + 1) as f64 * zw, m, top);
            fill(&mut small, m + i as f64 * zw, h as f64 - m, m + (i + 1) as f64 * zw, h as f64, bottom);
            seen.extend([top, bottom]);
        }
        for j in 0..ny {
            let y = (j as f64 * zh) as u32;
            let left = average(&filled, 0, y, band, zh as u32);
            let right = average(&filled, iw - band, y, band, zh as u32);
            fill(&mut small, 0.0, m + j as f64 * zh, m, m + (j + 1) as f64 * zh, left);
            fill(&mut small, w as f64 - m, m + j as f64 * zh, w as f64, m + (j + 1) as f64 * zh, right);
            seen.extend([left, right]);
        }
        for (cx, cy, x0, y0) in [(0, 0, 0.0, 0.0), (iw - band, 0, w as f64 - m, 0.0), (0, ih - band, 0.0, h as f64 - m), (iw - band, ih - band, w as f64 - m, h as f64 - m)] {
            let c = average(&filled, cx, cy, band, band);
            fill(&mut small, x0, y0, x0 + m, y0 + m, c);
        }
        // the interior (the picture covers it) takes the nearest edge, so the blur does not darken the margins
        let inner = average(&filled, 0, 0, iw, ih);
        fill(&mut small, m, m, w as f64 - m, h as f64 - m, inner);
        // soft: the blur spans about a third of a zone, so a cell keeps its colour in its middle
        let soft = imageops::blur(&small, ((zw.min(zh) / 8.0 / 3.0) as f32).max(1.0));
        let avg = seen.iter().fold([0f32; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]]).map(|s| s / seen.len().max(1) as f32);
        // the margins' ink: the theme's, or the paper's light or shade when that reads better on them
        let other = if luminance(theme.ink) < 0.4 { theme.paper_light } else { theme.paper_shade };
        let ink = if contrast(theme.ink, avg) >= contrast(other, avg).min(4.5) { theme.ink } else { other };
        (Some((buffer(&soft), (soft.width() as i32, soft.height() as i32))), Some(ink))
    } else {
        (None, None)
    };

    // the title block's place (title.rs: bottom right of the interior, up to the frame's line, which is drawn
    // over the picture as along the rest of the frame) and the picture there, blurred and tinted
    let block = crate::title::title_block(size, margin, mm).rect;
    let title = Rectangle::<i32, Physical>::new(block.loc, (block.size.w - crate::title::LINE, block.size.h - crate::title::LINE).into());
    let (bw, bh) = (title.size.w, title.size.h);
    let (tx, ty) = ((title.loc.x - interior.loc.x).max(0) as u32, (title.loc.y - interior.loc.y).max(0) as u32);
    let under = imageops::crop_imm(&filled, tx.min(iw - 1), ty.min(ih - 1), (bw as u32).min(iw - tx.min(iw - 1)), (bh as u32).min(ih - ty.min(ih - 1))).to_image();
    // frosted glass: blurred about 1.5 mm (blurred small and scaled back, quick), so the picture's shapes
    // still show through; how much paper lies over it is `title_tint`
    let k = (mm / 2.0).max(1.0);
    let tiny = imageops::resize(&under, ((under.width() as f32 / k) as u32).max(2), ((under.height() as f32 / k) as u32).max(2), FilterType::Triangle);
    let tiny = imageops::blur(&tiny, 3.0);
    let mut frosted = imageops::resize(&tiny, bw.max(1) as u32, bh.max(1) as u32, FilterType::Triangle);
    let (t, paper) = (theme.title_tint, rgba(theme.paper));
    for p in frosted.pixels_mut() {
        for i in 0..3 {
            p[i] = (p[i] as f32 * (1.0 - t) + paper[i] as f32 * t) as u8;
        }
        p[3] = 255;
    }

    *cache.0.borrow_mut() = Some(OutputWallpaper {
        key,
        interior,
        picture: buffer(&filled),
        ambient,
        title,
        frosted: buffer(&frosted),
        margin_ink,
    });
    true
}
