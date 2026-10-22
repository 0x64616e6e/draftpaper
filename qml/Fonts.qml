pragma Singleton
// The sheet's lettering face: config.toml's [text] monospace when it is installed (the settings' Text page,
// through theme.json), else IBM Plex Mono (fonts-ibm-plex, in Debian's contrib section) when installed,
// otherwise DejaVu Sans Mono, which every Debian has; and a Nerd Font for glyphs, when one is installed.
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: fonts
    property string chosen: ""          // [text] monospace, from theme.json
    readonly property string mono: {
        const all = Qt.fontFamilies()
        if (chosen && all.indexOf(chosen) >= 0) return chosen
        return all.indexOf("IBM Plex Mono") >= 0 ? "IBM Plex Mono" : "DejaVu Sans Mono"
    }
    // the interface face: [text] interface when it is installed, else IBM Plex Sans, else what the
    // system has. Only the settings letter in it -- the sheet itself is lettered in one face, which is
    // the mono above; a drawing is lettered, an application is set.
    property string chosenIface: ""
    readonly property string iface: {
        const all = Qt.fontFamilies()
        if (chosenIface && all.indexOf(chosenIface) >= 0) return chosenIface
        for (const f of ["IBM Plex Sans", "Noto Sans", "DejaVu Sans"])
            if (all.indexOf(f) >= 0) return f
        return "sans-serif"
    }
    // glyphs (volume, battery, Wi-Fi...): a Nerd Font's monospaced variant, the symbols-only one or Plex's
    // own (BlexMono) first; "" when none is installed, and the shell letters words instead
    readonly property string glyph: {
        const all = Qt.fontFamilies()
        for (const f of ["Symbols Nerd Font Mono", "BlexMono Nerd Font Mono"])
            if (all.indexOf(f) >= 0) return f
        return all.find(f => / Nerd Font Mono$/.test(f)) || ""
    }

    // What the sheet can letter of one entry of the document (docs/sheet-content.md): its `text` with
    // the glyphs set in the face that has them, or its plain `words` where there is no such face or the
    // config has turned glyphs off. This is the whole of what a renderer decides about words, and it is
    // a decision only a renderer can make: the document is written by something that does not know
    // which faces are on the machine.
    function lettered(entry, on) {
        if (!entry) return ""
        var text = entry.text || ""
        if (!on || fonts.glyph === "") return entry.words !== undefined ? entry.words : text
        var out = ""
        for (var i = 0; i < text.length; i++) {
            var c = text.charCodeAt(i), ch = text.charAt(i)
            if (c >= 0xD800 && c <= 0xDBFF && i + 1 < text.length) {   // one glyph, two code units
                c = (c - 0xD800) * 0x400 + text.charCodeAt(i + 1) - 0xDC00 + 0x10000
                ch = text.substr(i, 2)
                i++
            }
            var pua = (c >= 0xE000 && c <= 0xF8FF) || (c >= 0xF0000 && c <= 0xFFFFD)
            out += pua ? "<font face='" + fonts.glyph + "'>" + ch + "</font>" : ch
        }
        return out
    }

    FileView {
        path: Quickshell.env("DRAFTPAPER_CACHE") + "/theme.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onTextChanged: {
            try {
                const t = JSON.parse(text()).text || {}
                fonts.chosen = t.monospace || ""
                fonts.chosenIface = t.interface || ""
            } catch (e) {}
        }
    }
}
