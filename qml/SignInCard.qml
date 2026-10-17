// A sign-in card on the sheet, for the login screen (greeter.qml) and the lock screen (lock.qml): whole
// zones in the middle of the frame (about 130 mm across, the title block's width, and 45 mm down), its
// rules on the ticks, ruled like the title block: a heading (what it is, the machine, the date and time)
// and rows of a label and a value. The row being typed in is framed; a row with the fingerprint reader
// has its glyph, drawn, lit and breathing while the reader listens; a note row (what PAM or greetd
// says) runs the card's width, in the alert colour for an error.
//
// It fills its parent (a surface over the whole output, `screenName`) and draws only the card; the keys
// are the owner's. Its geometry (mm, font, the title block's DRAWN cell) is there for the owner too.
import QtQuick
import QtQuick.Shapes
import Quickshell

Item {
    id: card
    anchors.fill: parent
    required property var theme                 // Theme.qml
    property string screenName: ""
    property string heading: ""
    // each {label, value, here: typed in, note: a note row, alert: in the alert colour, reader: the
    // fingerprint glyph, name: the user's name (nameItem)}
    property var rows: []
    property bool finger: false                 // the reader listening
    property bool caret: true                   // a caret in the row typed in
    property bool hideName: false               // the name row's value hidden (it slides away)
    property Item nameItem: null                // the name row's value, once drawn
    readonly property alias box: box            // the card itself (to fade it)

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
    readonly property int spanX: Math.max(2, Math.min(across, Math.round(130 * mm / zoneW)))
    readonly property int spanY: Math.max(1, Math.min(down, Math.round(45 * mm / zoneH)))
    readonly property int fromX: Math.floor((across - spanX) / 2)
    readonly property int fromY: Math.floor((down - spanY) / 2)
    function px(v) { return Math.round(v * dpr) / dpr }     // on a physical pixel
    readonly property real line: 2 / dpr                    // the frame's line, as the sheet draws it
    readonly property real hair: 1 / dpr
    readonly property real cardX: px(frameX + fromX * zoneW)
    readonly property real cardY: px(frameY + fromY * zoneH)
    readonly property real cardW: px(frameX + (fromX + spanX) * zoneW) + line - cardX
    readonly property real cardH: px(frameY + (fromY + spanY) * zoneH) + line - cardY
    readonly property real headH: px(Math.min(10 * mm, cardH / 4))
    readonly property real rowH: (cardH - headH - 2 * line) / Math.max(1, rows.length)
    readonly property real font: 2.8 * mm
    readonly property real colLabel: px(Math.min(34 * mm, cardW * 0.3))
    readonly property color ink: theme.ink
    readonly property color paper: theme.data.paper || "#C9BDAE"
    function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }
    // the title block's DRAWN cell (the last; title.rs), when the shell letters the name there
    readonly property var drawnCell: out && out.title && theme.data.drawnCell !== false ? out.title.cells[out.title.cells.length - 1] : null

    SystemClock { id: time; precision: SystemClock.Minutes }

    Rectangle {
        id: box
        x: card.cardX; y: card.cardY
        width: card.cardW; height: card.cardH
        color: card.paper
        // the outer rule, as heavy as the frame's line
        border.width: card.line; border.color: card.ink
        Rectangle { y: card.headH; width: box.width; height: card.hair; color: card.ink }

        // the heading, like a title block cell
        Text {
            x: 2 * card.mm
            y: (card.headH - height) / 2
            text: card.heading
            font.family: Fonts.mono; font.pixelSize: card.font * 0.75; font.letterSpacing: 0.3 * card.mm
            color: card.inkA(0.6)
        }
        Text {
            anchors.right: parent.right; anchors.rightMargin: 2 * card.mm
            y: (card.headH - height) / 2
            text: Qt.formatDateTime(time.date, "yyyy-MM-dd · HH:mm")
            font.family: Fonts.mono; font.pixelSize: card.font * 0.75; font.letterSpacing: 0.3 * card.mm
            color: card.inkA(0.6)
        }

        Repeater {
            model: card.rows
            Item {
                id: row
                required property var modelData
                required property int index
                x: card.line; y: card.headH + card.hair + index * card.rowH
                width: box.width - 2 * card.line; height: card.rowH
                Rectangle { visible: row.index > 0; width: parent.width; height: card.hair; color: card.inkA(0.35) }
                Rectangle { visible: !row.modelData.note; x: card.colLabel; width: card.hair; height: row.height; color: card.inkA(0.35) }
                Rectangle {
                    visible: row.modelData.here === true
                    anchors.fill: parent
                    color: card.inkA(0.12)
                    border.width: Math.max(card.line, 0.5 * card.mm); border.color: card.ink
                }
                Text {
                    x: 2 * card.mm; width: card.colLabel - 4 * card.mm
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideRight
                    text: row.modelData.label || ""
                    font.family: Fonts.mono; font.pixelSize: card.font * 0.7; font.letterSpacing: 0.3 * card.mm
                    color: card.inkA(0.6)
                }
                Text {
                    id: value
                    visible: !(row.modelData.name && card.hideName)
                    x: row.modelData.note ? 2 * card.mm : card.colLabel + 2 * card.mm
                    width: row.width - x - (row.modelData.reader ? 12 : 2) * card.mm
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideLeft; textFormat: Text.PlainText
                    text: (row.modelData.value || "") + (row.modelData.here && card.caret ? "▏" : "")
                    font.family: Fonts.mono; font.pixelSize: row.modelData.note ? card.font * 0.85 : card.font
                    font.bold: row.modelData.here === true
                    color: row.modelData.alert ? card.theme.alert : row.modelData.note ? card.inkA(0.7) : card.ink
                    Component.onCompleted: if (row.modelData.name) card.nameItem = value
                }
                // the fingerprint reader: Tabler Icons' fingerprint (MIT, Paweł Kuna; debian/copyright),
                // its strokes drawn at the weight of the sheet's rules (no font needed: a lock or login
                // screen may have no Nerd Font); faint, lit (and breathing) while the reader listens
                Item {
                    visible: row.modelData.reader === true
                    anchors.right: parent.right; anchors.rightMargin: 3 * card.mm
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.round(6.5 * card.mm); height: width
                    Shape {
                        id: glyph
                        // the icon's own 24 by 24 units, scaled to the cell
                        width: 24; height: 24
                        scale: parent.width / 24
                        transformOrigin: Item.TopLeft
                        preferredRendererType: Shape.CurveRenderer
                        readonly property color colour: card.finger ? card.ink : card.inkA(0.35)
                        // the frame line's weight (two physical pixels), in the icon's units
                        readonly property real weight: card.line * 24 / parent.width
                        ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M18.9 7a8 8 0 0 1 1.1 5v1a6 6 0 0 0 .8 3" } }
                        ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M8 11a4 4 0 0 1 8 0v1a10 10 0 0 0 2 6" } }
                        ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M12 11v2a14 14 0 0 0 2.5 8" } }
                        ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M8 15a18 18 0 0 0 1.8 6" } }
                        ShapePath { strokeColor: glyph.colour; strokeWidth: glyph.weight; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: "M4.9 19a22 22 0 0 1 -.9 -7v-1a8 8 0 0 1 12 -6.95" } }
                    }
                    SequentialAnimation on opacity {
                        running: card.finger
                        loops: Animation.Infinite
                        NumberAnimation { to: 0.45; duration: 700; easing.type: Easing.InOutSine }
                        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                        onRunningChanged: if (!running) parent.opacity = 1
                    }
                }
            }
        }
    }
}
