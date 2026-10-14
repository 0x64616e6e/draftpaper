// One margin of the drafting sheet (shell.qml): an unmanaged window over the margin on `side` (top and
// bottom run the full width, left and right the height between them), transparent, pushed under every
// i3 window (draftpaper lower) so it reads as lettering on the paper and a fullscreen window covers it.
// Children are laid out inside the band, usually in Zone cells.
import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: band
    required property var edge            // this screen's dock strip, which the window is placed from
    required property color ink
    property real inkAlpha: 1             // the pattern strength, as the wallpaper's lines use it
    property string side: "top"           // top, bottom, left, right
    default property alias content: area.data
    readonly property alias window: win
    readonly property bool vertical: side === "left" || side === "right"
    readonly property real screenHeight: edge.screen.height
    // the zones as the sheet renderer ticks them, about 50 mm each and at least two: along the top and bottom
    // between the frame's corners (`offset` from the band's start), down the sides the whole band
    readonly property real frame: edge.margin
    readonly property real offset: vertical ? 0 : frame
    readonly property real length: vertical ? screenHeight - 2 * frame : edge.width - 2 * frame
    readonly property int zones: Math.max(2, Math.round(length / (50 * edge.mm)))
    readonly property real zoneLength: length / zones
    readonly property real zoneWidth: zoneLength       // the name the top and bottom bands' users know
    readonly property color tick: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * inkAlpha + 0.1))

    PopupWindow {
        id: win
        anchor.window: band.edge
        anchor.rect.x: band.side === "right" ? band.edge.width - band.frame : 0
        anchor.rect.y: band.side === "bottom" ? band.screenHeight - band.frame : band.vertical ? band.frame : 0
        implicitWidth: band.vertical ? Math.round(band.frame) : band.edge.width
        implicitHeight: band.vertical ? Math.round(band.length) : Math.round(band.frame)
        visible: band.edge.width > 0
        color: "transparent"
        onBackingWindowVisibleChanged: if (backingWindowVisible) { lower.running = true; again.restart() }
        Item { id: area; anchors.fill: parent }
    }
    Process { id: lower; command: [Quickshell.env("DRAFTPAPER_BIN"), "lower", String(Math.round(win.implicitWidth * band.edge.dpr)), String(Math.round(win.implicitHeight * band.edge.dpr))] }
    Timer { id: again; interval: 800; onTriggered: lower.running = true }
}
