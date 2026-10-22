// Hatched ground, as a drawing hatches what is not built on: 45° lines, one pixel wide, every
// `pitch` pixels. The Window Manager Settings canvas draws it with
// `repeating-linear-gradient(45deg, #C6C9C2 0, #C6C9C2 1px, transparent 1px, transparent 8px)`;
// this is the same thing on a Canvas, which is what Qt has.
//
// It is the one piece of the settings that is drawn rather than laid out, so it lives on its own and
// the pages that need a plan use it (qml/Plan.qml).
import QtQuick

Canvas {
    id: hatching
    property color stroke: "#C6C9C2"
    property real pitch: 8
    property real thickness: 1

    onStrokeChanged: requestPaint()
    onPitchChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        const c = getContext("2d")
        c.reset()
        c.strokeStyle = hatching.stroke
        c.lineWidth = hatching.thickness
        // a 45° line crosses the box from its top edge to its left edge; walking the start point
        // along the top and then down the right covers the whole of it
        const step = hatching.pitch * Math.SQRT2
        c.beginPath()
        for (let d = -height; d < width + height; d += step) {
            c.moveTo(d, 0)
            c.lineTo(d + height, height)
        }
        c.stroke()
    }
}
