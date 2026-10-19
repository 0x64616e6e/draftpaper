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
    faces: Vec<fontdue::Font>,
    /// by character and by size, in sixty-fourths of a pixel so a size is a whole number
    glyphs: HashMap<(char, u32), Option<Glyph>>,
}

impl Face {
    /// The theme's face, with what is installed to fall back on. None where no face can be loaded at all.
    pub fn load(wanted: Option<&str>) -> Option<Face> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut faces = Vec::new();
        // the family the theme names first, then the sheet's own, then what Debian always has; a Nerd
        // Font last, for the glyphs before the readings
        let mut families: Vec<String> = wanted.into_iter().map(str::to_string).collect();
        families.extend(["IBM Plex Mono", "DejaVu Sans Mono"].map(str::to_string));
        families.extend(nerd_fonts(&db));
        for family in families {
            let query = fontdb::Query { families: &[fontdb::Family::Name(&family)], ..Default::default() };
            let Some(id) = db.query(&query) else { continue };
            // the family asked for, not whatever the database would fall back to
            if !db.face(id).is_some_and(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(&family))) {
                continue;
            }
            let loaded = db.with_face_data(id, |data, index| fontdue::Font::from_bytes(data, fontdue::FontSettings { collection_index: index, ..Default::default() }).ok());
            if let Some(Some(font)) = loaded {
                faces.push(font);
            }
        }
        (!faces.is_empty()).then_some(Face { faces, glyphs: HashMap::new() })
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
        self.faces.first().and_then(|f| f.horizontal_line_metrics(px)).map(|m| m.ascent).unwrap_or(px * 0.8)
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

    /// The glyph for a character, rasterised once and kept. The first face that has it wins; a character
    /// no face has is left out rather than drawn as a box.
    fn glyph(&mut self, ch: char, px: f32) -> Option<&Glyph> {
        let key = (ch, (px * 64.0).round() as u32);
        if !self.glyphs.contains_key(&key) {
            let made = self.faces.iter().find(|f| f.lookup_glyph_index(ch) != 0 || ch == ' ').map(|font| {
                let (metrics, coverage) = font.rasterize(ch, px);
                Glyph {
                    coverage,
                    width: metrics.width,
                    height: metrics.height,
                    left: metrics.xmin,
                    top: metrics.height as i32 + metrics.ymin,
                    advance: metrics.advance_width,
                }
            });
            self.glyphs.insert(key, made);
        }
        self.glyphs.get(&key).and_then(|g| g.as_ref())
    }
}

/// The monospaced Nerd Fonts installed, most wanted first; empty where none is (the sheet then letters
/// words where it would have put a glyph, as the shell does).
fn nerd_fonts(db: &fontdb::Database) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for face in db.faces() {
        for (name, _) in &face.families {
            if name.ends_with(" Nerd Font Mono") && !found.contains(name) {
                found.push(name.clone());
            }
        }
    }
    // the two the shell prefers, then whatever else is there
    found.sort_by_key(|n| match n.as_str() {
        "Symbols Nerd Font Mono" => 0,
        "BlexMono Nerd Font Mono" => 1,
        _ => 2,
    });
    found
}
