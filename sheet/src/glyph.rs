//! The glyphs the sheet sets before a reading — the bolt before the power, the cell before the charge,
//! the arc before the Wi-Fi — as the shell set them (qml/shell.qml's `g` and `ramp`).
//!
//! These are Nerd Font characters in the private use area. The sheet letters in one face and falls back
//! through the others for whatever that face does not have (font.rs), so a glyph is drawn by asking for
//! the character like any other — there is no second face to place or size. Where no face installed has
//! it, or the config has turned glyphs off, the sheet letters the word instead and nothing is missing:
//! that is the whole reason every call here takes the word as well.

use crate::font::Face;
use serde_json::Value;

/// What the sheet can letter of one entry of the document (docs/sheet-content.md): its `words` where
/// the config has turned glyphs off or no face installed has every character of `text`, else `text`.
///
/// This is the whole of what a renderer decides about words, and it is a decision only a renderer can
/// make: the document is written by something that does not know which faces are on the machine.
pub fn pick(face: &mut Face, glyphs: bool, entry: &Value) -> String {
    let text = entry["text"].as_str().unwrap_or("");
    let Some(words) = entry["words"].as_str() else { return text.to_string() };
    if !glyphs || !text.chars().all(|ch| ch.is_ascii() || face.has(ch)) {
        return words.to_string();
    }
    text.to_string()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_keeps_its_glyphs_only_where_they_can_be_drawn() {
        let Some(mut face) = Face::load(None) else { return };
        let reading = serde_json::json!({ "text": "\u{f140b} mains", "words": "mains" });
        // turned off in the config, whatever is installed
        assert_eq!(pick(&mut face, false, &reading), "mains");
        // a character no face installed has would come out an empty box: the words instead
        assert_eq!(pick(&mut face, true, &serde_json::json!({ "text": "\u{f8fff} x", "words": "x" })), "x");
        // and an entry with nothing to fall back to is lettered as it stands
        assert_eq!(pick(&mut face, true, &serde_json::json!({ "text": "04:28" })), "04:28");
    }
}
