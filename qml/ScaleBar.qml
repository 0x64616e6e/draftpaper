// The scale bar's lettering in the bottom Margin (shell.qml): the compositor draws the bar itself, true to
// size and on whole pixels (title.rs's scale_bar); this letters its centimetres over their rules, "mm"
// after the last, in the rows the compositor leaves above the bar. On a display that reports no size,
// nothing on the sheet is to scale: the zone says so, NTS, as a drawing does.
import QtQuick

Item {
    id: graphic
    required property var band
    required property var edge
    // the compositor's scale_bar (ipc.rs): zone, measured, marks, text; in the output's logical pixels
    required property var bar
    anchors.fill: parent
    // the band's top, in the output's logical pixels: it runs along the bottom, a whole number of pixels high
    readonly property real bandTop: edge.height - Math.round(band.frame)
    readonly property color ink: Qt.rgba(band.ink.r, band.ink.g, band.ink.b, 0.85)
    readonly property real size: bar.text ? Math.min(edge.fontPx * 0.8, bar.text.height * 1.3) : edge.fontPx * 0.8

    Text {
        visible: !graphic.bar.measured
        x: graphic.bar.zone.x; width: graphic.bar.zone.width
        y: 0; height: graphic.band.frame
        horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
        text: "NTS"
        font.family: Fonts.mono; font.pixelSize: graphic.edge.fontPx * 0.8; font.letterSpacing: 0.1 * graphic.edge.mm
        color: graphic.ink
    }

    Repeater {
        model: graphic.bar.measured ? graphic.bar.marks : []
        Text {
            required property var modelData
            required property int index
            readonly property bool last: index === graphic.bar.marks.length - 1
            // the number centred on its rule; the last one's unit runs on after it
            TextMetrics { id: number; text: String(modelData.mm); font.family: Fonts.mono; font.pixelSize: graphic.size }
            x: modelData.x - number.advanceWidth / 2
            y: graphic.bar && graphic.bar.text ? graphic.bar.text.y - graphic.bandTop : 0
            height: graphic.bar && graphic.bar.text ? graphic.bar.text.height : graphic.band.frame
            verticalAlignment: Text.AlignBottom
            text: last ? modelData.mm + " mm" : String(modelData.mm)
            font.family: Fonts.mono; font.pixelSize: graphic.size
            color: graphic.ink
        }
    }
}
