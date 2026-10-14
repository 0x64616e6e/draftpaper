// One margin of the drafting sheet (shell.qml): a layer-shell surface over the margin on `side` (top and
// bottom run the full width, left and right the height between them), transparent, on the bottom layer:
// above the paper the compositor draws, under every window, so it reads as lettering on the paper and a
// fullscreen window covers it. Children are laid out inside the band, usually in Zone cells.
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: band
    required property var edge            // this screen (shell.qml): its sheet's geometry from the compositor
    required property color ink
    property real inkAlpha: 1             // the pattern strength, as the sheet's lines use it
    property string side: "top"           // top, bottom, left, right
    default property alias content: area.data
    readonly property alias window: win
    readonly property bool vertical: side === "left" || side === "right"
    // the zones as the compositor ticks them: along the top and bottom between the frame's corners
    // (`offset` from the band's start), down the sides the whole band
    readonly property real frame: edge.margin
    readonly property real offset: vertical ? 0 : frame
    readonly property real length: vertical ? edge.height - 2 * frame : edge.width - 2 * frame
    readonly property int zones: vertical ? edge.zonesDown : edge.zonesAcross
    readonly property real zoneLength: length / zones
    readonly property real zoneWidth: zoneLength       // the name the top and bottom bands' users know
    readonly property color tick: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * inkAlpha + 0.1))

    PanelWindow {
        id: win
        screen: band.edge.screen
        anchors {
            top: band.side !== "bottom"
            bottom: band.side !== "top"
            left: band.side !== "right"
            right: band.side !== "left"
        }
        margins { top: band.vertical ? band.frame : 0; bottom: band.vertical ? band.frame : 0 }
        implicitWidth: band.vertical ? Math.round(band.frame) : 0
        implicitHeight: band.vertical ? 0 : Math.round(band.frame)
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        WlrLayershell.namespace: "draftpaper-margin"
        Item { id: area; anchors.fill: parent }
    }
}
