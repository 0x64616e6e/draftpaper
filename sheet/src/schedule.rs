//! The schedules on the sheet, as a drawing carries them: the equipment (what the machine is), the
//! processes (what it is doing) and the revisions (what has been drawn lately). One at a time, in the
//! title block's column — the same width as the block, from the frame's top down to it.
//!
//! This is qml/Schedule.qml and the three that use it, lettered by the sheet's own client. The rules and
//! the lettering sit at whole millimetres from the schedule's corner, each edge then put on its pixel.
//!
//! A schedule with more rows than fit keeps its place: the millimetres left over go to its heading, so
//! its last row ends the same distance above the title block as the graph keeps from it.

use draftpaper::util::Paths;
use serde_json::Value;

use crate::font::Face;
use crate::paint::{Canvas, Ink};
use crate::sheet::{Screen, Theme};

/// How far a schedule keeps from the frame's lines and the title block.
pub const GAP: f64 = 6.0;

/// One column: what it is called, how wide it is (0: whatever is left), and whether it reads from the
/// right, as figures do.
pub struct Column {
    pub name: String,
    pub mm: f64,
    pub right: bool,
}

/// One row: what each column says, a note under one of them, a heavier rule above where a group starts,
/// and a share of something, drawn as a bar before the figure.
#[derive(Default, Clone)]
pub struct Row {
    pub cells: Vec<String>,
    pub note: String,
    pub heavy_above: bool,
    pub share: Option<f64>,
    /// the ink the row is lettered in, by the name the theme knows it under ("alert", "faint",
    /// "strong"); empty for the sheet's own. A schedule of the user's own says which (extensions/)
    pub ink: String,
}

pub struct Schedule {
    pub title: String,
    pub meta: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// the column a note sits under, and the one whose share is drawn as a bar
    pub note_col: isize,
    pub bar_col: isize,
}

/// A row's height in millimetres: six, nine where it carries a note.
fn row_height(row: &Row) -> f64 {
    if row.note.is_empty() {
        6.0
    } else {
        9.0
    }
}

impl Schedule {
    /// Where the schedule goes on this sheet, and the rows that fit: its left edge on the title block's
    /// left rule, its top `GAP` in from the frame's line, down to `GAP` above the title block.
    pub fn place(&self, s: &Screen) -> Option<Placed> {
        let block = s.title.as_ref()?;
        let x = block.rect.0;
        let y = s.frame.1 + GAP * s.mm;
        let width = ((block.rect.0 + block.rect.2 - GAP * s.mm - x) / s.mm).floor();
        let room = ((block.rect.1 - y) / s.mm).floor() - GAP;
        if width < 70.0 || room < 18.0 {
            return None;
        }
        // the heading, the columns' heads, then as many rows as fit
        let mut laid: Vec<(f64, f64)> = Vec::new();
        let mut at = 12.0;
        for row in &self.rows {
            let h = row_height(row);
            if at + h > room {
                break;
            }
            laid.push((at, h));
            at += h;
        }
        // a schedule with nothing in it is still a schedule: the heading and the columns' heads are
        // lettered, as the shell lettered them. Called up over a repository that is not one, or a
        // reading not yet taken, it says so by standing there empty rather than by not appearing —
        // which reads as the sheet having ignored the asking.
        // more rows than fit: the millimetres over go to the heading, so the last row ends GAP above the
        // title block, as the graph keeps from it
        let head = 7.0 + if laid.len() < self.rows.len() { (room - at).max(0.0) } else { 0.0 };
        let rows: Vec<(f64, f64)> = laid.iter().map(|(y, h)| (y + head - 7.0, *h)).collect();
        let height = rows.last().map(|(y, h)| y + h).unwrap_or(head + 5.0);
        Some(Placed { x, y, width, head, rows, height, room })
    }
}

/// A schedule worked out for one sheet: where it is, and where each row sits in it.
pub struct Placed {
    /// the display's logical pixels
    pub x: f64,
    pub y: f64,
    /// millimetres
    pub width: f64,
    pub head: f64,
    /// each row's top and height, in millimetres from the schedule's corner
    pub rows: Vec<(f64, f64)>,
    pub height: f64,
    /// the whole of the sheet the schedule has to stand in, whatever it has in it. The surface is cut
    /// to this and not to `height`: a buffer of a new size takes a new slot in a pool that never gives
    /// one back, so a schedule read again every two seconds would grow the sheet by twenty megabytes a
    /// time if its rows settled how big it was.
    pub room: f64,
}

impl Placed {
    /// The columns' left edges, in millimetres from the schedule's.
    fn columns(&self, columns: &[Column]) -> Vec<f64> {
        let fixed: f64 = columns.iter().map(|c| c.mm).sum();
        let mut x = 0.0;
        columns
            .iter()
            .map(|c| {
                let at = x;
                x += if c.mm > 0.0 { c.mm } else { (self.width - fixed).max(10.0) };
                at
            })
            .collect()
    }
}

/// Draw it. `scale` turns the display's logical pixels into the buffer's physical ones.
pub fn draw(canvas: &mut Canvas, face: &mut Face, schedule: &Schedule, at: &Placed, s: &Screen, theme: &Theme, scale: f64, hover: Option<usize>) {
    let mm = (s.mm * scale) as f32;
    let px = |v: f64| (v * s.mm * scale) as f32;
    let ink = theme.ink;
    let faint = theme.faint();
    let rule = theme.line();
    let hair = 1.0f32.max((scale as f32).round());
    let heavy = (0.35 * mm).round().max(2.0);
    let wide = px(at.width);
    let colx = at.columns(&schedule.columns);
    let col_w = |i: usize| (if i + 1 < colx.len() { colx[i + 1] } else { at.width }) - colx[i];

    // the heading: what it is, and on the right what it is of
    face.draw(canvas, 0.0, px(1.0) + face.ascent(2.8 * mm), &schedule.title, 2.8 * mm, 0.25 * mm, ink.premultiplied());
    if !schedule.meta.is_empty() {
        let w = face.width(&schedule.meta, 2.0 * mm, 0.3 * mm);
        face.draw(canvas, wide - w, px(1.6) + face.ascent(2.0 * mm), &schedule.meta, 2.0 * mm, 0.3 * mm, faint.premultiplied());
    }
    canvas.fill(0, px(at.head) as i32, wide as i32, heavy as i32, ink.premultiplied());
    // the columns' heads
    for (i, c) in schedule.columns.iter().enumerate() {
        let text = c.name.as_str();
        let w = face.width(text, 2.0 * mm, 0.3 * mm);
        let x = if c.right { px(colx[i] + col_w(i) - 1.6) - w } else { px(colx[i] + if i == 0 { 0.0 } else { 1.6 }) };
        face.draw(canvas, x, px(at.head + 0.6) + face.ascent(2.0 * mm), text, 2.0 * mm, 0.3 * mm, faint.premultiplied());
    }
    canvas.fill(0, px(at.head + 5.0) as i32, wide as i32, hair as i32, ink.premultiplied());

    for (i, (row, (top, high))) in schedule.rows.iter().zip(&at.rows).enumerate() {
        if hover == Some(i) {
            canvas.fill(0, px(*top) as i32, wide as i32, (px(top + high) - px(*top)) as i32, Ink { a: 0.07, ..ink }.premultiplied());
        }
        if row.heavy_above {
            canvas.fill(0, px(*top) as i32, wide as i32, heavy as i32, ink.premultiplied());
        }
        // the rule under it; none under the last, so the schedule has no outline of its own
        if i + 1 < at.rows.len() {
            canvas.fill(0, px(top + high) as i32, wide as i32, hair as i32, rule.premultiplied());
        }
        // the ink this row asks for, if it asks for one: a schedule of the user's own may letter a
        // row in the theme's alert, faint or strong ink (docs/sheet-content.md)
        let said = match row.ink.as_str() {
            "alert" => Some(theme.alert),
            "faint" => Some(theme.faint()),
            "strong" => Some(theme.strong),
            _ => None,
        };
        for (c, column) in schedule.columns.iter().enumerate() {
            let Some(text) = row.cells.get(c) else { continue };
            if text.is_empty() {
                continue;
            }
            let size = if c == 0 { 2.5 * mm } else { 2.6 * mm };
            let from = px(colx[c] + if c == 0 { 0.0 } else { 1.6 });
            let to = px(colx[c] + col_w(c) - 1.6);
            // the row's own ink where it names one, else the sheet's — and the bar's column turns to
            // the alert ink on its own once the share is most of the way along
            let colour = said.unwrap_or(if schedule.bar_col == c as isize && row.share.unwrap_or(0.0) >= 0.8 { theme.alert } else { ink });
            let baseline = px(top + 1.4) + face.ascent(size);
            if column.right {
                let w = face.width(text, size, 0.0);
                face.draw(canvas, to - w, baseline, text, size, 0.0, colour.premultiplied());
            } else {
                let cut = crate::margins::fit(face, text, size, to - from);
                face.draw(canvas, from, baseline, &cut, size, 0.0, colour.premultiplied());
            }
            // the share before the figure: a bar ruled to the millimetre, ten long
            if schedule.bar_col == c as isize {
                if let Some(share) = row.share {
                    let (x, y) = (px(colx[c] + if c == 0 { 0.0 } else { 1.6 }), px(top + 2.0));
                    let (w, h) = (px(colx[c] + 10.0) - px(colx[c]), px(top + 4.0) - px(top + 2.0));
                    canvas.fill(x as i32, y as i32, w as i32, hair as i32, rule.premultiplied());
                    canvas.fill(x as i32, (y + h) as i32, w as i32, hair as i32, rule.premultiplied());
                    canvas.fill(x as i32, y as i32, hair as i32, h as i32, rule.premultiplied());
                    canvas.fill((x + w) as i32, y as i32, hair as i32, h as i32, rule.premultiplied());
                    let filled = (w * share.clamp(0.0, 1.0) as f32) as i32;
                    let colour = said.unwrap_or(if share >= 0.8 { theme.alert } else { ink });
                    canvas.fill(x as i32, y as i32, filled, h as i32, colour.premultiplied());
                }
            }
        }
        // the note, under the column it belongs to
        if !row.note.is_empty() && schedule.note_col >= 0 {
            let c = schedule.note_col as usize;
            let from = px(colx.get(c).copied().unwrap_or(0.0) + if c == 0 { 0.0 } else { 1.6 });
            let cut = crate::margins::fit(face, &row.note, 2.1 * mm, wide - from);
            face.draw(canvas, from, px(top + 5.0) + face.ascent(2.1 * mm), &cut, 2.1 * mm, 0.0, faint.premultiplied());
        }
    }
    // the columns' rules, between them only, from the heads down
    for i in 1..colx.len() {
        let x = px(colx[i]) as i32;
        canvas.fill(x, px(at.head) as i32, hair as i32, (px(at.height) - px(at.head)) as i32, rule.premultiplied());
    }
}

/// The equipment: what the machine is, as the schedule of equipment on a drawing.
/// The schedule the compositor says is up, read from the one account of what the sheet says
/// (`draftpaper::content`, docs/sheet-content.md). Nothing is worked out here: what each schedule
/// holds is the document's, and this only turns it into what the sheet can place and draw.
pub fn of(which: &str, sheet: u64, theme: &Value, outputs: &[Value]) -> Option<Schedule> {
    let v = draftpaper::content::schedule(&Paths::new(), which, sheet, theme, outputs)?;
    let text = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let columns = v["columns"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| Column {
                    name: text(c, "name"),
                    mm: c.get("mm").and_then(Value::as_f64).unwrap_or(0.0),
                    right: c.get("right").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let rows = v["rows"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| Row {
                    ink: text(r, "ink"),
                    cells: r["cells"].as_array().map(|c| c.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default(),
                    note: text(r, "note"),
                    heavy_above: r.get("heavy").and_then(Value::as_bool).unwrap_or(false),
                    share: r.get("share").and_then(Value::as_f64),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Schedule {
        title: text(&v, "title"),
        meta: text(&v, "meta"),
        columns,
        rows,
        note_col: v.get("note").and_then(Value::as_i64).unwrap_or(-1) as isize,
        bar_col: v.get("bar").and_then(Value::as_i64).unwrap_or(-1) as isize,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn tests_screen() -> Screen {
        sheet()
    }

    fn sheet() -> Screen {
        Screen {
            name: "test".into(),
            width: 1920.0,
            height: 1200.0,
            scale: 1.0,
            mm: 6.4,
            margin: 38.0,
            frame: (38.0, 38.0, 1844.0, 1124.0),
            title: Some(crate::sheet::Block { rect: (1050.0, 930.0, 832.0, 230.0), cells: vec![(1050.0, 930.0, 100.0, 100.0)] }),
            workspace: 1,
            across: 6,
            down: 4,
            parking: Vec::new(),
            bar: None,
            schedule: String::new(),
            graph: String::new(),
            legend: false,
        }
    }

    fn rows(n: usize) -> Vec<Row> {
        (0..n).map(|i| Row { cells: vec![format!("{i}"), "a row".into()], ..Row::default() }).collect()
    }

    #[test]
    fn it_sits_in_the_title_blocks_column_and_stops_above_it() {
        let s = sheet();
        let schedule = Schedule {
            title: "T".into(),
            meta: String::new(),
            columns: vec![Column { name: "A".into(), mm: 10.0, right: false }, Column { name: "B".into(), mm: 0.0, right: false }],
            rows: rows(4),
            note_col: -1,
            bar_col: -1,
        };
        let at = schedule.place(&s).expect("it fits");
        assert_eq!(at.x, 1050.0, "its left edge is the title block's");
        assert_eq!(at.y, 38.0 + GAP * 6.4, "its top is GAP in from the frame");
        assert_eq!(at.rows.len(), 4);
        // the last row ends above the title block
        let foot = at.y + at.height * s.mm;
        assert!(foot <= 930.0, "it ran into the title block: {foot}");
    }

    #[test]
    fn more_rows_than_fit_push_the_heading_down() {
        let s = sheet();
        let make = |n| Schedule {
            title: "T".into(),
            meta: String::new(),
            columns: vec![Column { name: "A".into(), mm: 10.0, right: false }, Column { name: "B".into(), mm: 0.0, right: false }],
            rows: rows(n),
            note_col: -1,
            bar_col: -1,
        };
        let few = make(4).place(&s).expect("a few fit");
        let many = make(400).place(&s).expect("many do too, in part");
        assert_eq!(few.head, 7.0, "a schedule that fits keeps its heading where it is");
        assert!(many.head > 7.0, "one that does not puts the spare millimetres in its heading");
        // and its last row ends where the one that fits would have ended: GAP above the title block
        let foot = (many.y + many.height * s.mm - 930.0).abs();
        assert!(foot <= GAP * s.mm + s.mm, "the last row does not end GAP above the block: {foot}");
    }

    #[test]
    fn a_schedule_with_nothing_in_it_still_stands_on_the_sheet() {
        // called up over a window that is not in a repository, the revisions schedule has no rows; the
        // sheet letters its heading and its columns' heads all the same, or the asking looks ignored
        let s = sheet();
        let schedule = Schedule {
            title: "REVISIONS".into(),
            meta: "SHEET 1".into(),
            columns: vec![Column { name: "REV".into(), mm: 16.0, right: false }, Column { name: "DESCRIPTION".into(), mm: 0.0, right: false }],
            rows: Vec::new(),
            note_col: -1,
            bar_col: -1,
        };
        let at = schedule.place(&s).expect("an empty schedule is still placed");
        assert!(at.rows.is_empty());
        assert!(at.height > 0.0, "it takes up the heading and the columns' heads");
    }

    #[test]
    fn how_big_it_stands_does_not_depend_on_what_is_in_it() {
        // the surface is cut to `room`. If the rows settled it, a schedule read again every two
        // seconds would ask for a buffer of a new size each time, and every one of those takes a slot
        // in a pool that never hands one back: twenty megabytes a refresh on a large display.
        let s = sheet();
        let of = |n: usize| {
            Schedule { title: "T".into(), meta: String::new(), columns: vec![Column { name: "A".into(), mm: 0.0, right: false }], rows: rows(n), note_col: -1, bar_col: -1 }
                .place(&s)
                .expect("placed")
        };
        let (few, many, none) = (of(1), of(40), of(0));
        assert_eq!(few.room, many.room);
        assert_eq!(few.room, none.room);
        assert_eq!(few.width, many.width);
        // what is drawn inside it does change with the rows; it is only the surface that does not
        assert!(many.height > few.height);
    }

    #[test]
    fn a_sheet_with_no_room_carries_none() {
        let mut s = sheet();
        // a title block that reaches nearly to the frame's top leaves nothing above it
        s.title = Some(crate::sheet::Block { rect: (1050.0, 60.0, 832.0, 230.0), cells: Vec::new() });
        let schedule = Schedule { title: "T".into(), meta: String::new(), columns: vec![Column { name: "A".into(), mm: 0.0, right: false }], rows: rows(4), note_col: -1, bar_col: -1 };
        assert!(schedule.place(&s).is_none());
    }
}
