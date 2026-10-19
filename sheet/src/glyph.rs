//! The glyphs the sheet sets before a reading — the bolt before the power, the cell before the charge,
//! the arc before the Wi-Fi — as the shell set them (qml/shell.qml's `g` and `ramp`).
//!
//! These are Nerd Font characters in the private use area. The sheet letters in one face and falls back
//! through the others for whatever that face does not have (font.rs), so a glyph is drawn by asking for
//! the character like any other — there is no second face to place or size. Where no face installed has
//! it, or the config has turned glyphs off, the sheet letters the word instead and nothing is missing:
//! that is the whole reason every call here takes the word as well.

use crate::font::Face;

/// What goes before a reading: the glyph and the space after it where the sheet can letter it, else
/// `word` (which carries its own trailing space where it needs one).
pub fn before(face: &mut Face, on: bool, ch: char, word: &str) -> String {
    if on && face.has(ch) {
        format!("{ch} ")
    } else {
        word.to_string()
    }
}

/// One of `gs` by a level in 0..1: the first for empty, the last for full (qml/shell.qml's `ramp`).
pub fn ramp(v: f64, gs: &[char]) -> char {
    let i = (v * gs.len() as f64).floor().max(0.0) as usize;
    gs[i.min(gs.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ramp_runs_from_the_first_to_the_last() {
        let gs = ['a', 'b', 'c', 'd', 'e'];
        assert_eq!(ramp(0.0, &gs), 'a');
        assert_eq!(ramp(0.5, &gs), 'c');
        assert_eq!(ramp(1.0, &gs), 'e', "full is the last, not past the end");
        // a reading past its scale, or one that makes no sense, still picks a glyph
        assert_eq!(ramp(1.5, &gs), 'e');
        assert_eq!(ramp(-1.0, &gs), 'a');
    }

    #[test]
    fn with_glyphs_off_the_word_stands_in() {
        let Some(mut face) = Face::load(None) else { return };
        assert_eq!(before(&mut face, false, '\u{f140b}', "PWR "), "PWR ");
        // and a character no face has is the word too, however the config is set
        assert_eq!(before(&mut face, true, '\u{10fffd}', "PWR "), "PWR ");
    }
}
