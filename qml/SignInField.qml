// The sign-in field, for the login screen (greeter.qml) and the lock screen (lock.qml), on the sheet
// blurred inside its frame: one field in the middle of the frame and nothing else, no words. Whole zones
// wide (two, or one on an odd count), its sides on the ticks, 12 mm high on the frame's middle, its rule
// as heavy as the frame's line. In it, on the left, the fingerprint reader's glyph (Tabler Icons' (MIT,
// Paweł Kuna; debian/copyright), drawn as strokes at the rules' weight): faint, lit and breathing while
// the reader listens; then what is typed (dots for a password) and the caret. While PAM or greetd checks,
// the glyph and the letters dim; refuse() turns the rule to the alert colour and shakes the field.
//
// It fills its parent (a surface over the whole output, `screenName`) and draws only the field; the keys
// are the owner's. Its geometry (mm, font, the title block's DRAWN cell) is there for the owner too.
import QtQuick
import QtQuick.Shapes
import Quickshell

Item {
    id: field
    anchors.fill: parent
    required property var theme                 // Theme.qml
    property string screenName: ""
    property string text: ""                    // what shows typed: letters, or dots for a password
    property bool reader: true                  // the glyph shown (no reader while a name is typed)
    property bool finger: false                 // the reader listening
    property bool busy: false                   // being checked
    property bool caret: true
    readonly property alias box: box            // the field itself (to fade it)
    readonly property alias letters: typed      // what is typed in it (where a name slides from)

    // ---- the sheet's geometry (logical pixels, from the output's top-left corner) ----
    readonly property var out: Compositor.output(screenName)
    readonly property real mm: out ? out.px_per_mm : 4.2
    readonly property real dpr: out ? out.scale : 1
    readonly property real zoneW: out ? out.zones.width : 50 * mm
    readonly property real zoneH: out ? out.zones.height : 50 * mm
    readonly property real frameX: out ? out.frame.x - out.x : 6 * mm
    readonly property real frameY: out ? out.frame.y - out.y : 6 * mm
    readonly property int across: out ? out.zones.across : 4
    readonly property int down: out ? out.zones.down : 3
    readonly property int spanX: across % 2 === 0 ? 2 : 1
    readonly property int fromX: Math.floor((across - spanX) / 2)
    function px(v) { return Math.round(v * dpr) / dpr }     // on a physical pixel
    readonly property real line: 2 / dpr                    // the frame's line, as the sheet draws it
    readonly property real font: 2.8 * mm
    readonly property real fieldX: px(frameX + fromX * zoneW)
    readonly property real fieldW: px(frameX + (fromX + spanX) * zoneW) + line - fieldX
    readonly property real fieldH: px(12 * mm)
    readonly property real fieldY: px(frameY + down * zoneH / 2 - fieldH / 2)
    readonly property color ink: theme.ink
    readonly property color paper: theme.data.paper || "#C9BDAE"
    function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }
    // the title block's DRAWN cell (the last; title.rs), when the shell letters the name there
    readonly property var drawnCell: out && out.title && theme.data.drawnCell !== false ? out.title.cells[out.title.cells.length - 1] : null

    // a refusal: the rule in the alert colour, a short shake
    property bool refused: false
    function refuse() { refused = true; shake.restart(); unrefuse.restart() }
    Timer { id: unrefuse; interval: 1600; onTriggered: field.refused = false }

    Rectangle {
        id: box
        x: field.fieldX; y: field.fieldY
        width: field.fieldW; height: field.fieldH
        color: field.paper
        border.width: Math.max(field.line, 0.5 * field.mm)
        border.color: field.refused ? field.theme.alert : field.ink
        Behavior on border.color { ColorAnimation { duration: 200 } }

        SequentialAnimation {
            id: shake
            readonly property real d: 2 * field.mm
            NumberAnimation { target: box; property: "x"; to: field.fieldX - shake.d; duration: 50 }
            NumberAnimation { target: box; property: "x"; to: field.fieldX + shake.d; duration: 80 }
            NumberAnimation { target: box; property: "x"; to: field.fieldX - shake.d / 2; duration: 70 }
            NumberAnimation { target: box; property: "x"; to: field.fieldX; duration: 60 }
        }

        // the reader's glyph, on the left: a square as high as the field
        Item {
            id: glyphCell
            visible: field.reader
            x: 0; width: field.reader ? box.height : 0; height: box.height
            opacity: field.busy ? 0.4 : 1
            Item {
                anchors.centerIn: parent
                width: Math.round(6.5 * field.mm); height: width
                Shape {
                    id: glyph
                    // the icon's own 24 by 24 units, scaled to the cell
                    width: 24; height: 24
                    scale: parent.width / 24
                    transformOrigin: Item.TopLeft
                    preferredRendererType: Shape.CurveRenderer
                    readonly property color colour: field.finger ? field.ink : field.inkA(0.4)
                    // the frame line's weight (two physical pixels), in the icon's units
                    readonly property real weight: field.line * 24 / parent.width
                    ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M18.9 7a8 8 0 0 1 1.1 5v1a6 6 0 0 0 .8 3" } }
                    ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M8 11a4 4 0 0 1 8 0v1a10 10 0 0 0 2 6" } }
                    ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M12 11v2a14 14 0 0 0 2.5 8" } }
                    ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M8 15a18 18 0 0 0 1.8 6" } }
                    ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M4.9 19a22 22 0 0 1 -.9 -7v-1a8 8 0 0 1 12 -6.95" } }
                }
                SequentialAnimation on opacity {
                    running: field.finger && !field.busy
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.45; duration: 700; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                    onRunningChanged: if (!running) parent.opacity = 1
                }
            }
        }

        // what is typed, after the glyph
        Text {
            id: typed
            x: glyphCell.width + (field.reader ? 0 : 3 * field.mm)
            width: box.width - x - 3 * field.mm
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideLeft; textFormat: Text.PlainText
            text: field.text + (field.caret ? "▏" : "")
            font.family: Fonts.mono; font.pixelSize: field.font
            color: field.busy ? field.inkA(0.5) : field.ink
        }
    }
}
