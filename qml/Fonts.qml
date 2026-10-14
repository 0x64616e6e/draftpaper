pragma Singleton
// The sheet's lettering face: IBM Plex Mono (fonts-ibm-plex, in Debian's contrib section) when installed,
// otherwise DejaVu Sans Mono, which every Debian has; and a Nerd Font for glyphs, when one is installed.
import QtQuick
import Quickshell

Singleton {
    readonly property string mono: Qt.fontFamilies().indexOf("IBM Plex Mono") >= 0 ? "IBM Plex Mono" : "DejaVu Sans Mono"
    // glyphs (volume, battery, Wi-Fi...): a Nerd Font's monospaced variant, the symbols-only one or Plex's
    // own (BlexMono) first; "" when none is installed, and the shell letters words instead
    readonly property string glyph: {
        const all = Qt.fontFamilies()
        for (const f of ["Symbols Nerd Font Mono", "BlexMono Nerd Font Mono"])
            if (all.indexOf(f) >= 0) return f
        return all.find(f => / Nerd Font Mono$/.test(f)) || ""
    }
}
