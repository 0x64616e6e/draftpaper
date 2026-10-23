// .pragma library
// The physical board the Bindings page draws (docs/keyboard.md).
//
// Positions are here and legends are only half here. xkb's geometry is vestigial — no layout in use
// states where its keys sit — so a standard board is laid out by evdev code. What each key *says* comes
// from the compositor's live keymap (`msg keyboard`) when it offers one, so a Romanian or a Dvorak
// layout is drawn in its own letters on the same plan; a US legend is kept beside every key for when it
// does not, which is any compositor older than that request and the nested one, where there is no
// libinput to ask.
//
// The keysym name is kept the same way, because that is what a binding is written with: `[bindings]
// "Mod+h"` is parsed back by keysym_from_name (compositor/src/config.rs), so the board matches the
// keymap on names rather than on anything it draws.
//
// Widths are in key units, as a keyboard is specified and as the Window Manager Settings canvas draws
// it: every row of the main block is fifteen units across.
.pragma library

// one key: its evdev code, its width in units, the US legend, the US keysym name, a modifier's role
function key(code, w, cap, sym, mod) {
    return { code: code, w: w || 1, cap: cap, sym: sym || cap, mod: mod || "" }
}

// a run of plain letter or digit keys, from consecutive evdev codes
function run(from, caps) {
    var out = []
    for (var i = 0; i < caps.length; i++) out.push(key(from + i, 1, caps[i], caps[i].toLowerCase()))
    return out
}

// the main block: five rows, fifteen units each
function rows() {
    return [
        [key(41, 1, "`", "grave")]
            .concat(run(2, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]),
                    [key(12, 1, "-", "minus"), key(13, 1, "=", "equal"), key(14, 2, "Backspace", "BackSpace")]),
        [key(15, 1.5, "Tab", "Tab")]
            .concat(run(16, ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"]),
                    [key(26, 1, "[", "bracketleft"), key(27, 1, "]", "bracketright"),
                     key(43, 1.5, "\\", "backslash")]),
        [key(58, 1.75, "Caps", "Caps_Lock")]
            .concat(run(30, ["A", "S", "D", "F", "G", "H", "J", "K", "L"]),
                    [key(39, 1, ";", "semicolon"), key(40, 1, "'", "apostrophe"),
                     key(28, 2.25, "Enter", "Return")]),
        [key(42, 2.25, "Shift", "Shift_L", "Shift")]
            .concat(run(44, ["Z", "X", "C", "V", "B", "N", "M"]),
                    [key(51, 1, ",", "comma"), key(52, 1, ".", "period"), key(53, 1, "/", "slash"),
                     key(54, 2.75, "Shift", "Shift_R", "Shift")]),
        [key(29, 1.25, "Ctrl", "Control_L", "Ctrl"), key(125, 1.25, "Mod", "Super_L", "Mod"),
         key(56, 1.25, "Alt", "Alt_L", "Alt"), key(57, 6.25, "Space", "space"),
         key(100, 1.25, "Alt", "Alt_R", "Alt"), key(126, 1.25, "Mod", "Super_R", "Mod"),
         key(127, 1.25, "Menu", "Menu"), key(97, 1.25, "Ctrl", "Control_R", "Ctrl")]
    ]
}

// the arrows, in the inverted T they are made in. Draftpaper binds them heavily — parking is on
// Mod+Left and Mod+Right — so a board without them would be missing half of this keymap.
function arrows() {
    return [
        [null, key(103, 1, "↑", "Up"), null],
        [key(105, 1, "←", "Left"), key(108, 1, "↓", "Down"), key(106, 1, "→", "Right")]
    ]
}

var UNITS = 15          // the main block's width
var ARROW_UNITS = 3
var GAP = 0.75          // between the block and the arrows

function found(kb, code) {
    if (kb && kb.keys)
        for (var i = 0; i < kb.keys.length; i++)
            if (kb.keys[i].code === code) return kb.keys[i]
    return null
}

// What a key says: the keymap's legend where there is one, else the US legend beside it. A modifier is
// lettered with the name the chord uses — "Mod", not "Super_L" — because that is what it is here.
function legend(kb, k) {
    if (k.mod) return k.cap
    var live = found(kb, k.code)
    if (!live || !live.label) return k.cap
    return live.label.length === 1 ? live.label.toUpperCase() : k.cap
}

// The name a binding writes after its last plus. The keymap's where it has one, so a rebound layout
// binds what is on the key; the US name otherwise, which is what the defaults were written against.
function symbol(kb, k) {
    var live = found(kb, k.code)
    return live && live.name ? live.name : k.sym
}
