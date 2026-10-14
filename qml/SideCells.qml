// The drafting sheet's side margins (shell.qml, one per screen): a cell per zone, down the left margin
// first, then the right, each lettered with its zone (A, B, ...). The windows parked on the workspace
// shown on this screen (Super+Escape, draftpaper park) fill the cells in order, titles running up the margin
// as a drawing letters its side margins; a click brings that window back.
import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: cells
    required property var shell
    required property var edge
    property string space: ""             // the workspace shown on this screen

    readonly property var theme: shell.theme
    readonly property color ink: theme.ink
    property var parked: ({})             // `draftpaper park`'s state file: {workspace: [{id, cls, title}]}
    readonly property var windows: parked[space] || []

    FileView {
        path: Quickshell.env("DRAFTPAPER_RUNTIME") + "/parked.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onTextChanged: { try { cells.parked = JSON.parse(text()) } catch (e) { cells.parked = {} } }
    }

    function act(w) { shell.run([Quickshell.env("DRAFTPAPER_BIN"), "park", "show", String(w.id)]) }

    Repeater {
        model: ["left", "right"]
        Margin {
            id: sideBand
            required property string modelData
            required property int index
            edge: cells.edge; ink: cells.ink; inkAlpha: cells.theme.inkAlpha
            side: modelData
            Repeater {
                model: sideBand.zones
                Item {              // the delegate holds the Zone (whose own `index` is its place in the band)
                    id: cell
                    required property int index
                    Zone {
                        id: zone
                        band: sideBand; index: cell.index
                        readonly property int slot: sideBand.index * sideBand.zones + cell.index
                        readonly property var win: slot < cells.windows.length ? cells.windows[slot] : null
                        Text {          // the zone letter, at the cell's head
                            anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 1.5 * cells.edge.mm }
                            text: "ABCDEFGHJKLMNPQRSTUVWXYZ"[zone.index % 24]
                            font.family: "IBM Plex Mono"; font.pixelSize: cells.edge.fontPx * 0.8
                            color: Qt.rgba(cells.ink.r, cells.ink.g, cells.ink.b, 0.8)
                        }
                        Text {          // the window, lettered up the margin
                            visible: zone.win !== null
                            anchors.centerIn: parent
                            anchors.verticalCenterOffset: 2 * cells.edge.mm
                            width: parent.height - 9 * cells.edge.mm
                            rotation: -90
                            horizontalAlignment: Text.AlignHCenter
                            elide: Text.ElideRight
                            text: zone.win ? zone.win.title || zone.win.cls : ""
                            font.family: "IBM Plex Mono"; font.pixelSize: cells.edge.fontPx
                            color: cells.ink
                        }
                        Rectangle {     // a tick between filled cells is the wallpaper's; a hover mark along the inner edge
                            visible: hover.hovered && zone.win !== null
                            anchors { top: parent.top; bottom: parent.bottom; topMargin: 6 * cells.edge.mm; bottomMargin: 2 * cells.edge.mm }
                            x: sideBand.side === "left" ? parent.width - width : 0
                            width: Math.max(1, 2 / cells.edge.dpr)
                            color: cells.theme.accent
                        }
                        HoverHandler { id: hover }
                        MouseArea {
                            anchors.fill: parent
                            enabled: zone.win !== null
                            cursorShape: Qt.PointingHandCursor
                            onClicked: cells.act(zone.win)
                        }
                    }
                }
            }
        }
    }
}
