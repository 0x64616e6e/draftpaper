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
    readonly property color accent: data.accent || "#E8B374"
    readonly property real marginPx: data.marginPx || 37
    readonly property var title: data.title || ({})
    readonly property var cells: data.cells || []
    readonly property var alerts: data.alerts || ({ battery: 15 })
    readonly property var commands: data.commands || ({})
    function mmFor(screenName) { return (data.mm && data.mm[screenName]) || data.mmDefault || 6.2 }

    FileView {
        path: Quickshell.env("DRAFTPAPER_CACHE") + "/theme.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onTextChanged: { try { theme.data = JSON.parse(text()) } catch (e) {} }
    }
}
