//! The sheet's lettering face, and the glyphs it has rasterised so far.
//!
//! The sheet letters in one monospaced face, so nothing has to be shaped: a character is a glyph and
//! every glyph is the same width apart. `fontdb` finds the family the theme names, the way the shell does
//! (qml/Fonts.qml): `[text] monospace` when it is installed, else IBM Plex Mono, else DejaVu Sans Mono,
//! which every Debian has. A second face is kept for the glyphs the first does not have — a Nerd Font
//! where one is installed, and DejaVu for the rest.
//!
//! Each glyph is rasterised once at each size it is asked for and kept, since the sheet letters the same
//! few hundred characters over and over.

use std::collections::HashMap;

use crate::paint::Canvas;

/// A glyph as it is drawn: its coverage, where it sits against the pen, and how far the pen then moves.
struct Glyph {
    coverage: Vec<u8>,
    width: usize,
    height: usize,
    left: i32,
    top: i32,
    advance: f32,
}

pub struct Face {
    /// the faces of letters read so far: the first is the sheet's own, the rest are read only when a
    /// character turns up that none before them has. Reading one is not cheap — all of a face's glyphs
    /// are laid out as it is read — and a sheet of words rarely needs more than the first.
    read: Vec<fontdue::Font>,
    waiting: Vec<(std::path::PathBuf, u32)>,
    /// the Nerd Fonts, which the sheet reads for the glyphs it sets before its readings and never keeps:
    /// one holds some ten thousand glyphs and costs forty megabytes read, against ninety milliseconds to
    /// read again and a few kilobytes for the dozen glyphs the sheet actually draws from it.
    spares: Vec<(std::path::PathBuf, u32)>,
    /// by character and by size, in sixty-fourths of a pixel so a size is a whole number
    glyphs: HashMap<(char, u32), Option<Glyph>>,
    /// which characters any face has, so that asking again answers from here rather than reading a Nerd
    /// Font back in. The sheet asks before every glyph it letters, on every draw.
    covers: HashMap<char, bool>,
}

impl Face {
    /// The theme's face, with what is installed to fall back on. None where no face can be loaded at all.
    pub fn load(wanted: Option<&str>) -> Option<Face> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut found: Vec<(std::path::PathBuf, u32)> = Vec::new();
        let mut spares: Vec<(std::path::PathBuf, u32)> = Vec::new();
        // the family the theme names first, then the sheet's own, then what Debian always has. The Nerd
        // Fonts are kept apart: the sheet reads them for a glyph and never holds them.
        let mut families: Vec<String> = wanted.into_iter().map(str::to_string).collect();
        families.extend(["IBM Plex Mono", "DejaVu Sans Mono"].map(str::to_string));
        let letters = families.len();
        families.extend(nerd_fonts(&db));
        for (i, family) in families.iter().enumerate() {
            let query = fontdb::Query { families: &[fontdb::Family::Name(family)], ..Default::default() };
            let Some(id) = db.query(&query) else { continue };
            // the family asked for, not whatever the database would fall back to
            if !db.face(id).is_some_and(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(family))) {
                continue;
            }
            if let Some(fontdb::Source::File(path)) = db.face(id).map(|f| f.source.clone()) {
                let index = db.face(id).map(|f| f.index).unwrap_or(0);
                let at = (path, index);
                if found.contains(&at) || spares.contains(&at) {
                    continue;
                }
                if i < letters {
                    found.push(at);
                } else {
                    spares.push(at);
                }
            }
        }
        drop(db);
        // reading what every font on the machine calls itself costs a few megabytes, and the sheet
        // wants none of it again: given back to the allocator it would only sit there
        #[cfg(target_env = "gnu")]
        unsafe {
            libc::malloc_trim(0);
        }
        let mut face = Face { read: Vec::new(), waiting: found, spares, glyphs: HashMap::new(), covers: HashMap::new() };
        // the sheet's own face is read at once: without it there is nothing to letter with
        face.read_another();
        (!face.read.is_empty()).then_some(face)
    }

    /// Read the next face of letters waiting, if any; whether there was one.
    fn read_another(&mut self) -> bool {
        while !self.waiting.is_empty() {
            let (path, index) = self.waiting.remove(0);
            let Ok(data) = std::fs::read(&path) else { continue };
            if let Ok(font) = fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings { collection_index: index, ..Default::default() }) {
                self.read.push(font);
                return true;
            }
        }
        false
    }

    /// Draw, from the Nerd Fonts, every glyph the sheet sets before a reading, at one size, and record
    /// which of them any face has at all. The faces themselves are gone by the time this returns.
    ///
    /// It is done on a thread of its own, and that is the whole point of it. A Nerd Font read costs some
    /// forty megabytes; given back on the sheet's own thread the allocator keeps nearly a quarter of it,
    /// because the glyphs drawn meanwhile sit above it and pin it there. A thread is given an arena of
    /// its own, so what it asks for and gives back comes back whole — a tenth of a megabyte stays, not
    /// ten. The glyphs it drew are kept, so a sheet lettering the same few asks for this once a size.
    fn draw_from_spares(&mut self, ch: char, px: f32) {
        if self.spares.is_empty() {
            return;
        }
        let spares = self.spares.clone();
        let mut wanted: Vec<char> = crate::glyph::ALL.to_vec();
        if !wanted.contains(&ch) {
            wanted.push(ch);
        }
        let made = std::thread::spawn(move || {
            let mut out: Vec<(char, Option<Glyph>)> = Vec::new();
            for (path, index) in spares {
                if wanted.iter().all(|c| out.iter().any(|(o, g)| o == c && g.is_some())) {
                    break;
                }
                let Ok(data) = std::fs::read(&path) else { continue };
                let settings = fontdue::FontSettings { collection_index: index, ..Default::default() };
                let Ok(font) = fontdue::Font::from_bytes(data.as_slice(), settings) else { continue };
                for c in &wanted {
                    if out.iter().any(|(o, g)| o == c && g.is_some()) || font.lookup_glyph_index(*c) == 0 {
                        continue;
                    }
                    out.retain(|(o, _)| o != c);
                    out.push((*c, Some(drawn(&font, *c, px))));
                }
                for c in &wanted {
                    if !out.iter().any(|(o, _)| o == c) {
                        out.push((*c, None));
                    }
                }
                // the face goes here, before the next is read, so two are never held at once
            }
            out
        })
        .join()
        .unwrap_or_default();
        // copied, not taken: what the thread drew was drawn out of the thread's own arena, and one
        // glyph left in it holds the whole of it open. Copied here and let go of there, the arena has
        // nothing live in it and goes back whole.
        for (c, glyph) in &made {
            let copy = glyph.as_ref().map(|g| Glyph {
                coverage: Vec::from(&g.coverage[..]),
                width: g.width,
                height: g.height,
                left: g.left,
                top: g.top,
                advance: g.advance,
            });
            self.covers.insert(*c, copy.is_some());
            self.glyphs.insert((*c, (px * 64.0).round() as u32), copy);
        }
        drop(made);
        // the arena is given back only when the allocator is asked to hand it to the kernel
        #[cfg(target_env = "gnu")]
        unsafe {
            libc::malloc_trim(0);
        }
    }

    /// Whether any face the sheet can letter with has this character, reading the ones still waiting
    /// until one does. A glyph the sheet cannot draw would come out as an empty box, so what wants
    /// lettering in glyphs asks this first and letters the word where the answer is no.
    pub fn has(&mut self, ch: char) -> bool {
        if let Some(known) = self.covers.get(&ch) {
            return *known;
        }
        let found = |read: &Vec<fontdue::Font>| read.iter().any(|f| f.lookup_glyph_index(ch) != 0);
        while !found(&self.read) && self.read_another() {}
        if found(&self.read) {
            self.covers.insert(ch, true);
            return true;
        }
        // no face of letters has it, so it is one of the glyphs, or nothing: the Nerd Fonts are asked,
        // and answer for every glyph the sheet sets at once. Where there is no Nerd Font to ask, the
        // answer is still an answer and is remembered as one — otherwise a machine without one walks
        // every face it has before every glyph it does not letter, for as long as the session lasts.
        self.draw_from_spares(ch, 16.0);
        let answer = self.covers.get(&ch).copied().unwrap_or(false);
        self.covers.insert(ch, answer);
        answer
    }

    /// How far the pen moves for one character at this size, letter spacing aside.
    pub fn advance(&mut self, ch: char, px: f32) -> f32 {
        self.glyph(ch, px).map(|g| g.advance).unwrap_or(px * 0.6)
    }

    /// How wide a string is at this size, with `spacing` added after each character.
    pub fn width(&mut self, text: &str, px: f32, spacing: f32) -> f32 {
        text.chars().map(|c| self.advance(c, px) + spacing).sum::<f32>()
    }

    /// Where the baseline sits under the top of a line, and how tall the line is.
    pub fn ascent(&self, px: f32) -> f32 {
        self.read.first().and_then(|f| f.horizontal_line_metrics(px)).map(|m| m.ascent).unwrap_or(px * 0.8)
    }

    /// How far the line reaches below the baseline. Qt sets a block of text by its box, not its
    /// baseline, so a reading anchored to the foot of a cell stands this much above it.
    pub fn descent(&self, px: f32) -> f32 {
        self.read.first().and_then(|f| f.horizontal_line_metrics(px)).map(|m| -m.descent).unwrap_or(px * 0.2)
    }

    /// Letter `text` with its baseline at `y`, starting at `x`, in `colour` (which carries its own
    /// alpha). Returns where the pen ended.
    pub fn draw(&mut self, canvas: &mut Canvas, x: f32, y: f32, text: &str, px: f32, spacing: f32, colour: u32) -> f32 {
        let mut pen = x;
        for ch in text.chars() {
            if let Some(g) = self.glyph(ch, px) {
                let (left, top, w, h) = (g.left, g.top, g.width, g.height);
                let at = (pen.round() as i32 + left, y.round() as i32 - top);
                for row in 0..h {
                    for col in 0..w {
                        let cover = g.coverage[row * w + col];
                        if cover > 0 {
                            canvas.blend(at.0 + col as i32, at.1 + row as i32, colour, cover);
                        }
                    }
                }
                pen += g.advance + spacing;
            } else {
                pen += px * 0.6 + spacing;
            }
        }
        pen
    }

    /// The same, read upwards: the lettering turned a quarter turn against the clock, as a drawing
    /// letters the sides of its frame. `foot` is where the first character sits, and the pen climbs.
    pub fn draw_up(&mut self, canvas: &mut Canvas, x: f32, foot: f32, text: &str, px: f32, spacing: f32, colour: u32) {
        let mut pen = foot;
        for ch in text.chars() {
            if let Some(g) = self.glyph(ch, px) {
                let (left, top, w, h) = (g.left, g.top, g.width, g.height);
                for row in 0..h {
                    for col in 0..w {
                        let cover = g.coverage[row * w + col];
                        if cover > 0 {
                            // a quarter turn against the clock about the pen: what was across becomes up
                            let at = (x.round() as i32 + row as i32 - top, pen.round() as i32 - left - col as i32);
                            canvas.blend(at.0, at.1, colour, cover);
                        }
                    }
                }
                pen -= g.advance + spacing;
            } else {
                pen -= px * 0.6 + spacing;
            }
        }
    }

    /// The glyph for a character, rasterised once and kept. The first face that has it wins; a character
    /// no face has is left out rather than drawn as a box.
    fn glyph(&mut self, ch: char, px: f32) -> Option<&Glyph> {
        let key = (ch, (px * 64.0).round() as u32);
        if !self.glyphs.contains_key(&key) {
            // a character none of the faces read so far has: read the next one waiting and look again
            while !self.read.iter().any(|f| f.lookup_glyph_index(ch) != 0) && ch != ' ' && self.read_another() {}
            let made = self.read.iter().find(|f| f.lookup_glyph_index(ch) != 0 || ch == ' ').map(|font| drawn(font, ch, px));
            self.glyphs.insert(key, made);
        }
        // still nothing: it is one of the glyphs the Nerd Fonts carry, which are read for it and let go
        if self.glyphs.get(&key).is_some_and(Option::is_none) && self.covers.get(&ch) != Some(&false) {
            self.glyphs.remove(&key);
            self.draw_from_spares(ch, px);
            self.glyphs.entry(key).or_insert(None);
        }
        self.glyphs.get(&key).and_then(|g| g.as_ref())
    }
}

/// One character of one face, rasterised at one size.
fn drawn(font: &fontdue::Font, ch: char, px: f32) -> Glyph {
    let (metrics, coverage) = font.rasterize(ch, px);
    Glyph {
        coverage,
        width: metrics.width,
        height: metrics.height,
        left: metrics.xmin,
        top: metrics.height as i32 + metrics.ymin,
        advance: metrics.advance_width,
    }
}

/// The monospaced Nerd Fonts installed, most wanted first; empty where none is (the sheet then letters
/// words where it would have put a glyph, as the shell does).
/// The Nerd Fonts installed, most wanted first; empty where none is (the sheet then letters words where
/// it would have put a glyph, as the shell does).
///
/// A patched font comes in three cuts. The *Mono* one scales every glyph down into a single cell; the
/// plain one draws them at the size they were drawn at and keeps the single advance anyway, so the ink
/// stands out of its cell and nothing moves; *Propo* widens the advance as well, which would put the
/// lettering after it off the millimetre it belongs on.
///
/// The plain cut is the one to have. It is what Qt hands the shell when the shell asks for the Mono one
/// — asked for `BlexMono Nerd Font Mono`, which is a family it lists, Qt letters from `BlexMono Nerd
/// Font` — and it is the better of the two for a sheet: at the size a reading is lettered, a glyph
/// squeezed into one cell comes out as hairlines, and hairlines are what an unhinted rasteriser renders
/// faintest. Propo is never taken, whatever is installed.
fn nerd_fonts(db: &fontdb::Database) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for face in db.faces() {
        for (name, _) in &face.families {
            if (name.ends_with(" Nerd Font") || name.ends_with(" Nerd Font Mono")) && !found.contains(name) {
                found.push(name.clone());
            }
        }
    }
    // the plain cut of what the shell prefers, then its Mono cut, then whatever else is there
    found.sort_by_key(|n| match n.as_str() {
        "Symbols Nerd Font" => 0,
        "BlexMono Nerd Font" => 1,
        "Symbols Nerd Font Mono" => 2,
        "BlexMono Nerd Font Mono" => 3,
        n if n.ends_with(" Nerd Font") => 4,
        _ => 5,
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_is_drawn_without_the_face_it_came_from_being_kept() {
        let Some(mut face) = Face::load(None) else { return };
        // no Nerd Font installed: nothing here to try, and the sheet letters words instead
        if !face.has('\u{f140b}') {
            return;
        }
        let held = face.read.len();
        assert!(face.glyph('\u{f140b}', 23.0).is_some(), "the glyph is drawn");
        assert_eq!(face.read.len(), held, "and no face was taken on to draw it");
        // the whole set came back together, so the rest of the sheet's glyphs are in hand already
        assert!(face.glyph('\u{f0928}', 23.0).is_some(), "the Wi-Fi arc came with it");
        assert_eq!(face.read.len(), held);
    }

    #[test]
    fn a_character_nothing_has_is_asked_for_once() {
        let Some(mut face) = Face::load(None) else { return };
        // a private-use character no face carries: the answer is no, and it is remembered as no, so
        // the sheet does not read every Nerd Font again every time it letters
        assert!(!face.has('\u{f8fff}'));
        assert_eq!(face.covers.get(&'\u{f8fff}'), Some(&false));
        assert!(!face.has('\u{f8fff}'));
    }

    #[test]
    fn letters_come_from_the_face_the_sheet_keeps() {
        let Some(mut face) = Face::load(None) else { return };
        let before = face.read.len();
        assert!(face.glyph('A', 23.0).is_some());
        assert_eq!(face.read.len(), before, "a letter wants no face the sheet has not already read");
    }
}
