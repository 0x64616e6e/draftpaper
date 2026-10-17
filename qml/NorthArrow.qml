// The key plan's north arrow (issue #47), in the bottom Margin (shell.qml): as a drawing's north arrow, a
// needle in a circle, half of it filled; here it points to the other screen (with several, from an
// unfocused one to the focused one, from the focused one to the nearest). The compositor works out its
// square and its bearing (title.rs's key_plan, the output's "key_plan"); the shell draws it, in the ink.
import QtQuick
import QtQuick.Shapes

Item {
    id: north
    required property var band
    required property var edge
    // the compositor's key_plan: arrow (x, y, width, height in the output's logical pixels), bearing
    required property var plan
    anchors.fill: parent
    // the band's top, in the output's logical pixels: it runs along the bottom, a whole number of pixels high
    readonly property real bandTop: edge.height - Math.round(band.frame)
    // one colour, the ink; how strongly by the opacity (Qt 6.8's curve renderer keeps a stroke colour once drawn)
    readonly property color ink: band.ink
    readonly property real stroke: Math.max(1, 1 / edge.dpr)

    Item {
        id: dial
        visible: !!north.plan && !!north.plan.arrow
        x: north.plan && north.plan.arrow ? north.plan.arrow.x : 0
        y: north.plan && north.plan.arrow ? north.plan.arrow.y - north.bandTop : 0
        width: north.plan && north.plan.arrow ? north.plan.arrow.width : 0
        height: width
        Shape {         // the circle
            anchors.fill: parent
            opacity: 0.7
            ShapePath {
                strokeColor: north.ink; strokeWidth: north.stroke; fillColor: "transparent"
                PathAngleArc {
                    centerX: dial.width / 2; centerY: dial.height / 2
                    radiusX: dial.width / 2 - north.stroke; radiusY: radiusX
                    startAngle: 0; sweepAngle: 360
                }
            }
        }
        Item {          // the needle, turned to the bearing: its tip up before turning
            id: needle
            anchors.fill: parent
            rotation: north.plan ? north.plan.bearing : 0
            readonly property real r: dial.width / 2
            Shape {     // the left half, filled
                anchors.fill: parent
                ShapePath {
                    strokeColor: north.ink; strokeWidth: north.stroke; fillColor: north.ink
                    startX: needle.r; startY: needle.r * 0.1
                    PathLine { x: needle.r * 0.62; y: needle.r * 1.55 }
                    PathLine { x: needle.r; y: needle.r * 1.25 }
                    PathLine { x: needle.r; y: needle.r * 0.1 }
                }
            }
            Shape {     // the right half, open
                anchors.fill: parent
                ShapePath {
                    strokeColor: north.ink; strokeWidth: north.stroke; fillColor: "transparent"
                    startX: needle.r; startY: needle.r * 0.1
                    PathLine { x: needle.r * 1.38; y: needle.r * 1.55 }
                    PathLine { x: needle.r; y: needle.r * 1.25 }
                }
            }
        }
    }
}
