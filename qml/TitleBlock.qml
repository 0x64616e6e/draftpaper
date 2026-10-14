// The sheet's title block, live (one per screen): the readings laid out as title-block cells at the
// sheet's bottom-right corner, in the theme's ink. It is an unmanaged window pushed under every i3 window
// (draftpaper lower), so it reads as part of the paper: seen on an empty workspace or once its windows
// are parked, never on top of work. Sizes are millimetres of this screen.
import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: block
    required property var shell           // the ShellRoot of shell.qml: theme and readings
    required property var edge            // this screen's dock strip, which the window is placed from
    property string spaceName: ""

    readonly property var theme: shell.theme
    readonly property real mm: edge.mm
    readonly property real bw: 130 * mm
    readonly property real bh: 36 * mm
    readonly property color ink: theme.ink
    readonly property color line: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.85 * theme.inkAlpha + 0.1))
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property var custom: theme.cells.length > 0 ? theme.cells[0] : null

    // [x, y, w, h] in mm, label, value (StyledText), click (left), click (right), alert
    readonly property var cells: [
        [0, 0, 34, 24, "TIME", Qt.formatDateTime(shell.now, "HH:mm"), null, null, false],
        [34, 0, 32, 12, "DATE", Qt.formatDateTime(shell.now, "ddd dd MMM yyyy"), null, null, false],
        [66, 0, 22, 12, "SHEET", spaceName || "-", null, null, false],
        [88, 0, 42, 12, "PROJECT", theme.title.project || "", null, null, false],
        [34, 12, 24, 12, "POWER", shell.powerText, null, null, false],
        [58, 12, 22, 12, "BATTERY", shell.batteryText, null, null, shell.batteryLow],
        [80, 12, 22, 12, "PROFILE", shell.profileText || "-", () => shell.profileNext(), null, false],
        [102, 12, 28, 12, "CPU / MEMORY", shell.loadText, null, null, false],
        [0, 24, 34, 12, "NETWORK", shell.networkText, null, null, false],
        [34, 24, 52, 12, "TEMPERATURE · FAN", shell.tempText || "-", null, null, shell.tempHot],
        custom ? [86, 24, 44, 12, custom.label || "", shell.customText || "-", custom.left ? () => shell.run(custom.left) : null,
                  custom.right ? () => shell.run(custom.right) : null, false]
               : [86, 24, 44, 12, "DRAWN", theme.title.drawn || "", null, null, false],
    ]

    PopupWindow {
        id: win
        anchor.window: block.edge
        anchor.rect.x: block.edge.width - block.edge.margin - block.bw
        anchor.rect.y: block.edge.screen.height - block.edge.margin - block.bh
        implicitWidth: Math.round(block.bw)
        implicitHeight: Math.round(block.bh)
        visible: block.edge.width > 0
        color: "transparent"
        onBackingWindowVisibleChanged: if (backingWindowVisible) { lower.running = true; again.restart() }

        Rectangle {                       // the block's outer rule, as heavy as the frame
            anchors.fill: parent
            color: "transparent"
            border.color: block.line
            border.width: Math.max(1, Math.round(2 / block.edge.dpr))
        }
        Repeater {
            model: block.cells
            Item {
                required property var modelData
                x: modelData[0] * block.mm; y: modelData[1] * block.mm
                width: modelData[2] * block.mm; height: modelData[3] * block.mm
                Rectangle {
                    anchors.fill: parent
                    color: "transparent"
                    border.color: block.faint
                    border.width: 1 / block.edge.dpr
                }
                Text {
                    x: 2 * block.mm; y: 1.2 * block.mm
                    text: modelData[4]
                    font.family: Fonts.mono; font.pixelSize: 2.2 * block.mm; font.letterSpacing: 0.3 * block.mm
                    color: block.faint
                }
                Text {
                    x: 2 * block.mm; width: parent.width - 4 * block.mm
                    anchors.bottom: parent.bottom; anchors.bottomMargin: 1.6 * block.mm
                    text: modelData[5]
                    textFormat: Text.StyledText
                    elide: Text.ElideRight
                    fontSizeMode: Text.HorizontalFit      // shrink a long reading before cutting it
                    minimumPixelSize: 2.4 * block.mm
                    font.family: Fonts.mono
                    font.pixelSize: (modelData[3] > 12 ? 9 : 3.6) * block.mm
                    color: modelData[8] ? block.theme.alert : block.ink
                }
                MouseArea {
                    anchors.fill: parent
                    enabled: modelData[6] !== null || modelData[7] !== null
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    onClicked: e => { const f = e.button === Qt.RightButton ? modelData[7] : modelData[6]; if (f) f() }
                }
            }
        }
    }

    // under every i3 window; again a moment later in case the window was mapped after the first try
    Process { id: lower; command: [Quickshell.env("DRAFTPAPER_BIN"), "lower", String(Math.round(win.implicitWidth * block.edge.dpr)), String(Math.round(win.implicitHeight * block.edge.dpr))] }
    Timer { id: again; interval: 800; onTriggered: lower.running = true }
}
