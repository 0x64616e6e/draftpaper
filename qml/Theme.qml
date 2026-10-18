// The theme and settings the shell reads, from ~/.cache/draftpaper/theme.json (written by `draftpaper
// apply`); watched, so a theme switch restyles the shell live.
import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: theme
    property var data: ({})
    readonly property string name: data.name || ""
    readonly property string label: data.label || ""
    readonly property color ink: data.ink || "#62574C"
    readonly property real inkAlpha: data.inkAlpha !== undefined ? data.inkAlpha : 0.8
    readonly property color alert: data.alert || "#E2231A"
    readonly property color strong: data.strong || "#000000"     // the focused workspace
    readonly property color accent: data.accent || "#E8B374"
    readonly property var title: data.title || ({})
    readonly property var cells: data.cells || []
    readonly property var alerts: data.alerts || ({ battery: 15 })
    readonly property var commands: data.commands || ({})
    readonly property var rulers: data.rulers || ({})          // config.toml's [rulers] (qml/Rulers.qml)
    readonly property var sheet: data.sheet || ({})            // config.toml's [sheet]: machine, load (the notes on the sheet)
    readonly property bool pictured: data.pictured === true     // a wallpaper in the frame
    readonly property real titleTint: data.titleTint !== undefined ? data.titleTint : 0.7
    readonly property color paper: data.paper || "#C9BDAE"
    readonly property bool glyphs: data.glyphs !== false        // config.toml: glyphs = false for words only
    readonly property bool animations: data.animations !== false // [animations] enabled = false: no motion

    FileView {
        path: Quickshell.env("DRAFTPAPER_CACHE") + "/theme.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onTextChanged: { try { theme.data = JSON.parse(text()) } catch (e) {} }
    }
}
