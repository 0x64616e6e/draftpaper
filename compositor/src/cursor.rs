//! The pointer image on the TTY, where the compositor draws the cursor itself: the "left_ptr" (or
//! "default") image of the XCursor theme (XCURSOR_THEME, XCURSOR_SIZE), or a small drawn arrow when no
//! theme can be read. Not animated. Loading follows anvil's cursor.rs (Smithay v0.7.0, MIT).

use std::io::Read;

use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::{Logical, Point, Transform},
};
use xcursor::{parser::parse_xcursor, CursorTheme};

pub struct Cursor {
    pub buffer: MemoryRenderBuffer,
    pub hotspot: Point<i32, Logical>,
}

impl Cursor {
    pub fn load() -> Cursor {
        let theme = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());
        let size: u32 = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(24);
        let theme = CursorTheme::load(&theme);
        let image = ["left_ptr", "default"].iter().find_map(|name| {
            let mut data = Vec::new();
            std::fs::File::open(theme.load_icon(name)?).ok()?.read_to_end(&mut data).ok()?;
            let images = parse_xcursor(&data)?;
            images.into_iter().min_by_key(|i| (i.size as i32 - size as i32).abs())
        });
        match image {
            Some(i) => Cursor {
                buffer: MemoryRenderBuffer::from_slice(
                    &i.pixels_rgba,
                    Fourcc::Abgr8888,
                    (i.width as i32, i.height as i32),
                    1,
                    Transform::Normal,
                    None,
                ),
                hotspot: (i.xhot as i32, i.yhot as i32).into(),
            },
            None => Cursor::arrow(),
        }
    }

    /// A 12×18 black arrow with a white edge.
    fn arrow() -> Cursor {
        let (w, h) = (12usize, 18usize);
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let inside = x <= y && x + y / 3 < 12 && y < 16;
                let edge = inside && (x == 0 || x == y || x + y / 3 == 11 || y == 15);
                let i = (y * w + x) * 4;
                if inside {
                    let v = if edge { 255 } else { 0 };
                    px[i..i + 4].copy_from_slice(&[v, v, v, 255]);
                }
            }
        }
        Cursor {
            buffer: MemoryRenderBuffer::from_slice(&px, Fourcc::Abgr8888, (w as i32, h as i32), 1, Transform::Normal, None),
            hotspot: (0, 0).into(),
        }
    }
}
