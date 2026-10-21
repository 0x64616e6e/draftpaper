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
    pub pad: f64,
    pub head: f64,
    pub row: f64,
    pub key: f64,
    pub inner: f64,
    pub act: f64,
    pub gutter: f64,
}

/// What the legend is lettered with (`draftpaper::content::legend()`). The compositor keeps whether the
/// legend is up and the leader armed; what those are called is the sheet's lettering, so it comes from
/// the document and neither renderer carries a word of its own (docs/keyboard.md).
#[derive(Debug, Clone)]
pub struct Words {
    pub title: String,
    pub held: String,
    pub leader: String,
    pub more: String,
}

impl Words {
    pub fn read() -> Words {
        let v = draftpaper::content::legend();
        let w = |k: &str, d: &str| v.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        Words { title: w("title", "LEGEND"), held: w("held", "MOD HELD"), leader: w("leader", "LEADER"), more: w("more", "MORE") }
    }
}

impl Mm {
    pub fn read() -> Mm {
        let v = draftpaper::content::legend();
        let n = |k: &str, d: f64| v.get(k).and_then(Value::as_f64).unwrap_or(d);
        Mm {
            gap: n("gap", 6.0),
            pad: n("pad", 4.0),
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
    /// the surface: the frame's whole interior, in the display's logical pixels. It is this big because
    /// the legend marks the windows as well as lettering the table, and a window may be anywhere on
    /// the sheet; the table is drawn `gap` in from this corner.
    pub x: f64,
    pub y: f64,
    /// the surface, in millimetres
    pub width: f64,
    pub height: f64,
    /// the table itself, in millimetres
    pub table_w: f64,
    pub table_h: f64,
    pub cols: usize,
    pub rows_per_col: usize,
    /// how many the sheet had no room for; counted, not dropped in silence
    pub over: usize,
    pub mm: Mm,
}

/// The legend's place on this sheet, or None where it has no room. `avoid` is a schedule's left edge in
/// millimetres from the frame's corner, where one is out: the schedules keep to the title block's column
/// and the legend would otherwise letter over them.
pub fn place(s: &Screen, n: usize, avoid: Option<f64>, marked: bool) -> Option<Placed> {
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
    // with marks on the sheet the table gives way to them: it takes half the width at most, so the
    // windows it is teaching you to reach are not buried under the reference for reaching them.
    // What will not fit is counted in the heading, as it always was.
    let mut width = (right - mm.gap).max(0.0);
    if marked {
        width = width.min((frame_w / 2.0).floor());
    }
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
    // the columns and the rows the table actually uses, which is not what it has room for: a column is
    // filled to the bottom before the next is begun (`draw`), so a table of 67 in 4 columns of 44 is two
    // full-height columns, not four of 17. Measuring it the other way left the paper short of the
    // lettering on a sheet with room to spare.
    let across = shown.div_ceil(rows_per_col.max(1)).min(cols).max(1);
    let down = shown.min(rows_per_col);
    Some(Placed {
        // the surface is the frame's interior, so a mark can go wherever its window is
        x: fx,
        y: fy,
        width: frame_w,
        height: frame_h,
        table_w: (across as f64) * mm.pair(),
        table_h: mm.head + (down as f64) * mm.row,
        cols,
        rows_per_col,
        over: n - shown,
        mm,
    })
}

/// A window the legend marks: where it is on this sheet, and the key that would go to it.
pub struct Mark {
    /// the window, in millimetres from the frame's corner
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub key: String,
    /// the window the keys act *from*: lettered as the one you are on, not as somewhere to go
    pub here: bool,
}

/// The legend lettered: the table of the keymap, and a mark on each window a focus key would reach.
pub fn draw(canvas: &mut Canvas, face: &mut Face, keys: &[Key], marks: &[Mark], at: &Placed, s: &Screen, theme: &Theme, scale: f64, words: &Words) {
    let mm = (s.mm * scale) as f32;
    let px = |v: f64| (v * s.mm * scale) as f32;
    let ink = theme.ink;
    let faint = theme.faint();
    let heavy = (0.35 * mm).round().max(2.0);
    let g = at.mm.gap;
    let wide = px(at.table_w);

    // ---- the table ----
    // the paper under it, `pad` out past the table on every side: it stands over the windows, so it is
    // the theme's paper and nothing of them shows through it. The last column's gutter is already slack
    // on the right, so the pad is measured from inside it and the margin is the same all the way round.
    let pad = at.mm.pad;
    let (p0, p1) = (g - pad, g + at.table_w - at.mm.gutter + pad);
    let (q0, q1) = (g - pad, g + at.table_h + pad);
    let paper = Ink { a: 1.0, ..theme.paper };
    canvas.fill(px(p0) as i32, px(q0) as i32, (px(p1) - px(p0)) as i32, (px(q1) - px(q0)) as i32, paper.premultiplied());

    face.draw(canvas, px(g), px(g + 1.0) + face.ascent(2.8 * mm), &words.title, 2.8 * mm, 0.25 * mm, ink.premultiplied());
    let how = if s.leader { &words.leader } else { &words.held };
    let meta = if at.over > 0 { format!("{how} · {} {}", at.over, words.more) } else { how.clone() };
    let w = face.width(&meta, 2.0 * mm, 0.3 * mm);
    face.draw(canvas, px(g) + wide - w, px(g + 1.6) + face.ascent(2.0 * mm), &meta, 2.0 * mm, 0.3 * mm, faint.premultiplied());
    canvas.fill(px(g) as i32, px(g + at.mm.head - 2.0) as i32, wide as i32, heavy as i32, ink.premultiplied());

    let shown = keys.len().min(at.cols * at.rows_per_col);
    for (i, k) in keys.iter().take(shown).enumerate() {
        let col = i / at.rows_per_col;
        let row = i % at.rows_per_col;
        let x = g + (col as f64) * at.mm.pair();
        let y = g + at.mm.head + (row as f64) * at.mm.row;
        // the key, clipped to its column: XF86AudioRaiseVolume and its like would otherwise run into
        // what it does
        let text = fit(face, &k.keys, 2.2 * mm, px(at.mm.key));
        face.draw(canvas, px(x), px(y) + face.ascent(2.2 * mm) + px(1.0), &text, 2.2 * mm, 0.0, ink.premultiplied());
        let said = fit(face, &k.action, 2.2 * mm, px(at.mm.act));
        face.draw(canvas, px(x + at.mm.key + at.mm.inner), px(y) + face.ascent(2.2 * mm) + px(1.0), &said, 2.2 * mm, 0.0, faint.premultiplied());
    }

    // ---- the marks, on the windows themselves ----
    // a key lettered on the window it would take you to, which is the whole of why the gesture is worth
    // holding: the slow path and the fast one are the same movement (docs/keyboard.md)
    for m in marks {
        let box_mm = if m.here { 10.0 } else { 14.0 };
        let bx = m.x + (m.w - box_mm) / 2.0;
        let by = m.y + (m.h - 8.0) / 2.0;
        let tint = Ink { a: 1.0, ..theme.paper };
        canvas.fill(px(bx) as i32, px(by) as i32, (px(bx + box_mm) - px(bx)) as i32, (px(by + 8.0) - px(by)) as i32, tint.premultiplied());
        let rule = if m.here { faint } else { ink };
        let (x0, y0, x1, y1) = (px(bx) as i32, px(by) as i32, px(bx + box_mm) as i32, px(by + 8.0) as i32);
        canvas.fill(x0, y0, x1 - x0, heavy as i32, rule.premultiplied());
        canvas.fill(x0, y1 - heavy as i32, x1 - x0, heavy as i32, rule.premultiplied());
        canvas.fill(x0, y0, heavy as i32, y1 - y0, rule.premultiplied());
        canvas.fill(x1 - heavy as i32, y0, heavy as i32, y1 - y0, rule.premultiplied());
        let size = 3.6 * mm;
        let w = face.width(&m.key, size, 0.0);
        let colour = if m.here { faint } else { ink };
        face.draw(canvas, px(bx + box_mm / 2.0) - w / 2.0, px(by + 1.4) + face.ascent(size), &m.key, size, 0.0, colour.premultiplied());
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

/// The windows a focus key would reach on this sheet, with the key lettered on each.
///
/// The compositor says *which* window each direction lands on (`focus_to` in the outputs, worked out by
/// the same code the key itself runs) and the keymap says *which key* that direction is bound to. Both
/// are asked rather than worked out here, so a mark cannot promise something the key would not do.
pub fn marks(s: &Screen, out: &serde_json::Value, windows: &[Value], keys: &[Key]) -> Vec<Mark> {
    let to = out.get("focus_to").cloned().unwrap_or(Value::Null);
    let ox = out.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let oy = out.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    let (fx, fy, ..) = s.frame;
    // the shortest key bound to this action: "Mod+h" rather than "Mod+Left" where both are, since the
    // letter is what the keymap is built on and the arrow is the alias
    let key_for = |action: &str| -> Option<String> {
        keys.iter()
            .filter(|k| k.action == action)
            .map(|k| k.keys.clone())
            .min_by_key(|k| (k.len(), k.clone()))
            .map(|k| k.rsplit('+').next().unwrap_or("").to_string())
            .filter(|k| !k.is_empty())
    };
    let mut out_marks = Vec::new();
    for (dir, action, here) in [
        ("focused", "", true),
        ("left", "focus left", false),
        ("right", "focus right", false),
        ("up", "focus up", false),
        ("down", "focus down", false),
    ] {
        let Some(id) = to.get(dir).and_then(Value::as_u64) else { continue };
        let Some(w) = windows.iter().find(|w| w.get("id").and_then(Value::as_u64) == Some(id)) else { continue };
        let Some(g) = w.get("geometry").filter(|g| !g.is_null()) else { continue };
        let n = |k: &str| g.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let key = match here {
            true => "·".to_string(),
            false => match key_for(action) {
                Some(k) => k,
                None => continue,
            },
        };
        out_marks.push(Mark {
            x: (n("x") - ox - fx) / s.mm,
            y: (n("y") - oy - fy) / s.mm,
            w: n("width") / s.mm,
            h: n("height") / s.mm,
            key,
            here,
        });
    }
    out_marks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sheet big enough that the table has room to spare: four columns' width and far more rows per
    /// column than there are keys to put in them.
    fn roomy() -> Screen {
        let mut s = crate::schedule::tests::tests_screen();
        // 3840x2160 at scale 2, as the 4K display stands: a frame of 610 mm by 339 mm
        s.width = 1920.0;
        s.height = 1080.0;
        s.mm = 3.096_774;
        s.frame = (15.0, 15.0, 1890.0, 1050.0);
        s.title = Some(crate::sheet::Block { rect: (1200.0, 920.0, 690.0, 130.0), cells: Vec::new() });
        s
    }

    /// A column is filled to its foot before the next is begun, so the paper must be as deep as the
    /// first column — not as deep as the keys divided by the columns there was room for. The bug this
    /// holds off showed only on a sheet with room to spare: where the table is exactly full the two
    /// ways of measuring it agree, which is why every smaller display looked right.
    #[test]
    fn the_paper_is_cut_to_the_rows_and_columns_the_table_uses() {
        let s = roomy();
        let at = place(&s, 67, None, true).expect("the legend has room on a sheet this size");
        assert!(at.rows_per_col > 44, "this sheet should have room to spare: {}", at.rows_per_col);
        assert!(at.cols >= 2, "and more than one column: {}", at.cols);
        let shown = 67usize.min(at.cols * at.rows_per_col);
        // every key is lettered at its own row, and the deepest of them must be on the paper
        let deepest = (0..shown).map(|i| i % at.rows_per_col).max().unwrap();
        let foot = at.mm.head + ((deepest + 1) as f64) * at.mm.row;
        assert!(at.table_h >= foot, "the paper stops at {} and the lettering reaches {foot}", at.table_h);
        // and no wider than the columns it fills
        let across = (0..shown).map(|i| i / at.rows_per_col).max().unwrap() + 1;
        assert_eq!(at.table_w, (across as f64) * at.mm.pair(), "the paper is wider than the columns it covers");
    }

    /// Where the table is exactly full the paper is the whole of it, which is the case the old
    /// arithmetic got right and must go on getting right.
    #[test]
    fn a_table_filled_to_the_brim_is_covered_to_its_edges() {
        let s = roomy();
        let at = place(&s, 4000, None, true).expect("the legend has room");
        assert_eq!(at.table_h, at.mm.head + (at.rows_per_col as f64) * at.mm.row);
        assert_eq!(at.table_w, (at.cols as f64) * at.mm.pair());
        assert!(at.over > 0, "more keys than fit are counted, not dropped in silence");
    }
}
