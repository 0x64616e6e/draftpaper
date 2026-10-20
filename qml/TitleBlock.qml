// The sheet's title block, live (one per screen): the readings lettered in the title block's cells, in
// the frame's bottom-right corner, in the theme's ink. The compositor draws the block's rules, measured
// in the display's own pixels with the frame (compositor/src/title.rs), and sends where its cells are
// (the output's "title"); the shell only letters them. A layer-shell surface on the bottom layer, under
// every window, so it reads as part of the paper: seen on an empty workspace or once its windows are
// parked, never on top of work. Sizes are millimetres of this screen.
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: block
    required property var shell           // the ShellRoot of shell.qml: theme and readings
    required property var edge            // this screen (shell.qml): its sheet's geometry from the compositor
    property string spaceName: ""

    readonly property var theme: shell.theme
    readonly property real mm: edge.mm
    readonly property color ink: theme.ink
    readonly property color line: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.85 * theme.inkAlpha + 0.1))
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property var custom: theme.cells.length > 0 ? theme.cells[0] : null

    // the block and its cells from the compositor, in logical pixels of the output; a compositor before
    // 0.3.0 sends none, and the shell draws the rules itself (from the millimetres below)
    readonly property var geo: edge.output && edge.output.title ? edge.output.title : null
    // the layer surface: whole logical pixels from the frame's corner, wide enough to hold the block;
    // the block inside it at (ox, oy)
    readonly property int marginR: Math.round(edge.margin)
    readonly property int surfW: geo ? Math.ceil(edge.width - marginR - geo.rect.x - 1e-6) : Math.round(130 * mm)
    readonly property int surfH: geo ? Math.ceil(edge.height - marginR - geo.rect.y - 1e-6) : Math.round(36 * mm)
    readonly property real ox: geo ? geo.rect.x - (edge.width - marginR - surfW) : 0
    readonly property real oy: geo ? geo.rect.y - (edge.height - marginR - surfH) : 0

    // per cell, in the compositor's order: [x, y, w, h] in mm (only without its geometry), label, value
    // (the readings' cells, NETWORK too: a click shows their year on the sheet, LoadGraph.qml, or puts it away;
    // PROJECT the schedule of equipment and PROCESSES the process schedule, Schedule.qml)
    // (StyledText), click (left), click (right), alert
    // where each cell sits in millimetres from the block's corner, for a compositor that has not
    // measured the block yet. What each one says is the document's (docs/sheet-content.md), including
    // which is lettered large — the shell used to read that off this table's heights.
    readonly property var places: [
        [0, 0, 34, 24], [34, 0, 32, 12], [66, 0, 22, 12], [88, 0, 42, 12],
        [34, 12, 24, 12], [58, 12, 22, 12], [80, 12, 22, 12], [102, 12, 28, 12],
        [0, 24, 34, 12], [34, 24, 52, 12], [86, 24, 16, 12], [102, 24, 28, 12],
    ]
    // the twelfth cell is the config's first [[cell]] where it has one: a command with an interval of
    // its own, which is why it is not in the document (docs/sheet-content.md)
    readonly property var cells: {
        const said = block.edge.doc.title || []
        if (!said.length) return block.places.map(() => ({}))
        if (!custom) return said
        const mine = said.slice()
        mine[11] = { label: custom.label || "", text: shell.customText || "-",
                     action: custom.left ? "run.custom_left" : undefined,
                     action_right: custom.right ? "run.custom_right" : undefined }
        return mine
    }

    PanelWindow {
        id: win
        screen: block.edge.screen
        anchors { bottom: true; right: true }
        margins { bottom: block.marginR; right: block.marginR }
        implicitWidth: block.surfW
        implicitHeight: block.surfH
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        WlrLayershell.namespace: "draftpaper-title-block"

        // without the compositor's geometry: the block's outer rule on its top and left (its right and
        // bottom are the frame's line), and each cell's border
        Rectangle {
            visible: !block.geo
            width: parent.width; height: Math.max(1, Math.round(2 / block.edge.dpr))
            color: block.line
        }
        Rectangle {
            visible: !block.geo
            width: Math.max(1, Math.round(2 / block.edge.dpr)); height: parent.height
            color: block.line
        }
        Item {                            // the cells, kept inside the block
            x: block.ox; y: block.oy
            width: block.geo ? block.geo.rect.width : parent.width - Math.max(1, Math.round(2 / block.edge.dpr))
            height: block.geo ? block.geo.rect.height : parent.height - Math.max(1, Math.round(2 / block.edge.dpr))
            clip: true
        Repeater {
            // a compositor from before the PROCESSES cell (until the next login) has eleven: that cell left out
            model: block.geo && block.geo.cells.length < block.cells.length ? block.cells.filter(c => c.label !== "PROCESSES") : block.cells
            Item {
                id: cell
                required property var modelData
                required property int index
                readonly property var at: block.geo ? block.geo.cells[index] : null
                readonly property var place: block.places[index] || [0, 0, 0, 0]
                x: at ? at.x - block.geo.rect.x : place[0] * block.mm
                y: at ? at.y - block.geo.rect.y : place[1] * block.mm
                width: at ? at.width : place[2] * block.mm
                height: at ? at.height : place[3] * block.mm
                Rectangle {
                    visible: !block.geo
                    anchors.fill: parent
                    color: "transparent"
                    border.color: block.faint
                    border.width: 1 / block.edge.dpr
                }
                Text {
                    x: 2 * block.mm; y: 1.2 * block.mm
                    text: modelData.label || ""
                    font.family: Fonts.mono; font.pixelSize: 2.2 * block.mm; font.letterSpacing: 0.3 * block.mm
                    color: block.faint
                }
                Text {
                    x: 2 * block.mm; width: parent.width - 4 * block.mm
                    anchors.bottom: parent.bottom; anchors.bottomMargin: 1.6 * block.mm
                    text: Fonts.lettered(modelData, shell.glyphs)
                    textFormat: Text.StyledText
                    elide: Text.ElideRight
                    fontSizeMode: Text.HorizontalFit      // shrink a long reading before cutting it
                    minimumPixelSize: 2.4 * block.mm
                    font.family: Fonts.mono
                    font.pixelSize: (modelData.large ? 9 : 3.6) * block.mm
                    color: modelData.ink === "alert" ? block.theme.alert : block.ink
                }
                // a cell that does something: the whole of it, to the middle of its rules (no dead pixel
                // between two), takes the click, and shows it under the pointer with a faint tint
                MouseArea {
                    id: hit
                    anchors.fill: parent
                    anchors.margins: -1.5 / block.edge.dpr
                    enabled: modelData.action !== undefined || modelData.action_right !== undefined
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    onClicked: e => {
                        const name = e.button === Qt.RightButton ? modelData.action_right : modelData.action
                        if (name === "run.custom_left") shell.run(block.custom.left)
                        else if (name === "run.custom_right") shell.run(block.custom.right)
                        else shell.act(name, block.edge.screen.name)
                    }
                }
                Rectangle {
                    anchors.fill: parent
                    visible: hit.enabled && hit.containsMouse
                    color: Qt.rgba(block.ink.r, block.ink.g, block.ink.b, 0.07)
                }
            }
        }
        }
    }
}
