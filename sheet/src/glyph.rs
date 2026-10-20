//! The glyphs the sheet sets before a reading — the bolt before the power, the cell before the charge,
//! the arc before the Wi-Fi — as the shell set them (qml/shell.qml's `g` and `ramp`).
//!
//! These are Nerd Font characters in the private use area. The sheet letters in one face and falls back
//! through the others for whatever that face does not have (font.rs), so a glyph is drawn by asking for
//! the character like any other — there is no second face to place or size. Where no face installed has
//! it, or the config has turned glyphs off, the sheet letters the word instead and nothing is missing:
//! that is the whole reason every call here takes the word as well.

use crate::font::Face;

/// Every glyph the sheet sets, in one place. A Nerd Font is read for a glyph and let go of again
/// (font.rs), so the reading draws all of these at once rather than once for each: the sheet wants a
/// dozen of them and would otherwise pay ninety milliseconds a glyph for the privilege.
pub const ALL: &[char] = &[
    '\u{f140b}',                                                             // the bolt: power
    '\u{f0084}', '\u{f007a}', '\u{f007c}', '\u{f007e}', '\u{f0080}', '\u{f0079}', // the battery, charging and filling
    '\u{f0083}',                                                             // the battery, low
    '\u{f032a}', '\u{f0f85}', '\u{f04c5}',                                   // the power profiles
    '\u{f0ee0}', '\u{f035b}',                                                // the processor and the memory
    '\u{f092f}', '\u{f091f}', '\u{f0922}', '\u{f0925}', '\u{f0928}',         // the Wi-Fi, filling
    '\u{f0201}', '\u{f092e}',                                                // wired, and nothing
    '\u{f061a}', '\u{f02ca}', '\u{f0210}',                                   // the processor, the drive, the fan
    '\u{f00af}', '\u{f00b1}',                                                // Bluetooth, alone and connected
    '\u{f075f}', '\u{f057f}', '\u{f0580}', '\u{f057e}',                      // the volume, muted and filling
    '\u{f00de}', '\u{f00df}', '\u{f00e0}',                                   // the brightness, filling
];

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
