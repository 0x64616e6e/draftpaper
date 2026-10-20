//! The keyboard's legend (docs/keyboard.md): the keymap lettered on the sheet's free side while Mod is
//! held. The compositor keeps the gesture and says so in the outputs it publishes (`legend`); the keys
//! themselves are its own, asked for once, since the table that acts on them is the only honest account
//! of what they are.
//!
//! The millimetres are `draftpaper::content::legend()`'s — the same numbers the document hands the
//! shell, so the two renderers set the legend out identically and neither keeps a copy to drift
//! (compositor/tests/lettering.sh compares them).

use serde_json::Value;

use crate::font::Face;
use crate::paint::{Canvas, Ink};
use crate::sheet::{Screen, Theme};

/// A key and what it does, as the compositor writes them.
#[derive(Debug, Clone, Default)]
pub struct Key {
    pub keys: String,
    pub action: String,
}

/// The keymap, asked of the compositor. It does not change while the session lasts, so it is asked for
/// once and kept.
pub fn keys() -> Vec<Key> {
    crate::sheet::raw("keys")
        .iter()
        .map(|k| Key {
            keys: k.get("keys").and_then(Value::as_str).unwrap_or("").to_string(),
            action: k.get("action").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .filter(|k| !k.keys.is_empty())
        .collect()
}

/// The legend's measurements, in millimetres (`draftpaper::content::legend()`).
#[derive(Debug, Clone, Copy)]
pub struct Mm {
    pub gap: f64,
    pub head: f64,
    pub row: f64,
    pub key: f64,
    pub inner: f64,
    pub act: f64,
    pub gutter: f64,
}

impl Mm {
    pub fn read() -> Mm {
        let v = draftpaper::content::legend();
        let n = |k: &str, d: f64| v.get(k).and_then(Value::as_f64).unwrap_or(d);
        Mm {
            gap: n("gap", 6.0),
            head: n("head", 10.0),
            row: n("row", 6.0),
            key: n("key", 28.0),
            inner: n("inner", 2.0),
            act: n("act", 35.0),
            gutter: n("gutter", 3.0),
        }
    }
    pub fn pair(&self) -> f64 {
        self.key + self.inner + self.act + self.gutter
    }
}

/// Where the legend goes and what of it fits: as many columns of the table as the sheet's free side
/// takes, down to the title block.
pub struct Placed {
    /// the display's logical pixels
    pub x: f64,
    pub y: f64,
    /// millimetres
    pub width: f64,
    pub height: f64,
    pub cols: usize,
    pub rows_per_col: usize,
    /// how many the sheet had no room for; counted, not dropped in silence
    pub over: usize,
    pub mm: Mm,
}

/// The legend's place on this sheet, or None where it has no room. `avoid` is a schedule's left edge in
/// millimetres from the frame's corner, where one is out: the schedules keep to the title block's column
/// and the legend would otherwise letter over them.
pub fn place(s: &Screen, n: usize, avoid: Option<f64>) -> Option<Placed> {
    if n == 0 {
        return None;
    }
    let mm = Mm::read();
    let (fx, fy, fw, fh) = s.frame;
    let frame_w = (fw / s.mm + 1e-6).floor();
    let frame_h = (fh / s.mm + 1e-6).floor();
    // across: the sheet's free width, stopping short of a schedule where one is out
    let mut right = frame_w - mm.gap;
    if let Some(at) = avoid {
        right = right.min(at.floor() - 4.0);
    }
    let width = (right - mm.gap).max(0.0);
    // down: never into the title block. It is bounded here, so it needs no bound across as well.
    let bottom = match &s.title {
        Some(b) => ((b.rect.1 - fy) / s.mm).floor() - mm.gap,
        None => frame_h - mm.gap,
    };
    let room = (bottom - mm.gap).max(0.0);

    let cols = (width / mm.pair()).floor().max(1.0) as usize;
    let rows_per_col = ((room - mm.head) / mm.row).floor().max(0.0) as usize;
    let fits = cols * rows_per_col;
    if fits == 0 || width < 60.0 {
        return None;
    }
    let shown = n.min(fits);
    let down = shown.div_ceil(cols.max(1)).min(rows_per_col);
    Some(Placed {
        x: fx + mm.gap * s.mm,
        y: fy + mm.gap * s.mm,
        width: (cols as f64) * mm.pair(),
        height: mm.head + (down as f64) * mm.row,
        cols,
        rows_per_col,
        over: n - shown,
        mm,
    })
}

/// The legend lettered: a heading, a rule, and the keymap in columns of a key and what it does.
pub fn draw(canvas: &mut Canvas, face: &mut Face, keys: &[Key], at: &Placed, s: &Screen, theme: &Theme, scale: f64) {
    let mm = (s.mm * scale) as f32;
    let px = |v: f64| (v * s.mm * scale) as f32;
    let ink = theme.ink;
    let faint = theme.faint();
    let heavy = (0.35 * mm).round().max(2.0);
    let wide = px(at.width);

    // the paper under it, always: unlike a schedule the legend stands *over* the windows, and a keymap
    // lettered straight onto whatever is on the screen is not a keymap anyone can read
    let tint = Ink { a: theme.title_tint.max(0.92), ..theme.paper };
    canvas.fill(0, 0, wide as i32, px(at.height) as i32, tint.premultiplied());

    face.draw(canvas, 0.0, px(1.0) + face.ascent(2.8 * mm), "LEGEND", 2.8 * mm, 0.25 * mm, ink.premultiplied());
    let meta = if at.over > 0 { format!("MOD HELD · {} MORE", at.over) } else { "MOD HELD".to_string() };
    let w = face.width(&meta, 2.0 * mm, 0.3 * mm);
    face.draw(canvas, wide - w, px(1.6) + face.ascent(2.0 * mm), &meta, 2.0 * mm, 0.3 * mm, faint.premultiplied());
    canvas.fill(0, px(at.mm.head - 2.0) as i32, wide as i32, heavy as i32, ink.premultiplied());

    let shown = keys.len().min(at.cols * at.rows_per_col);
    for (i, k) in keys.iter().take(shown).enumerate() {
        let col = i / at.rows_per_col;
        let row = i % at.rows_per_col;
        let x = (col as f64) * at.mm.pair();
        let y = at.mm.head + (row as f64) * at.mm.row;
        // the key, clipped to its column: XF86AudioRaiseVolume and its like would otherwise run into
        // what it does
        let text = fit(face, &k.keys, 2.2 * mm, px(at.mm.key));
        face.draw(canvas, px(x), px(y) + face.ascent(2.2 * mm) + px(1.0), &text, 2.2 * mm, 0.0, ink.premultiplied());
        let said = fit(face, &k.action, 2.2 * mm, px(at.mm.act));
        face.draw(canvas, px(x + at.mm.key + at.mm.inner), px(y) + face.ascent(2.2 * mm) + px(1.0), &said, 2.2 * mm, 0.0, faint.premultiplied());
    }
}

/// `text` cut to `room` with an ellipsis where it does not fit, as the shell's `Text` elides it. A key
/// like XF86MonBrightnessDown is wider than its column and would otherwise run into what it does.
fn fit(face: &mut Face, text: &str, px: f32, room: f32) -> String {
    if face.width(text, px, 0.0) <= room {
        return text.to_string();
    }
    let mut cut = String::new();
    for ch in text.chars() {
        let mut next = cut.clone();
        next.push(ch);
        next.push('…');
        if face.width(&next, px, 0.0) > room {
            break;
        }
        cut.push(ch);
    }
    cut.push('…');
    cut
}
