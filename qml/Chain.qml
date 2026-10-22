// A dimension chain, as a drawing dimensions a run: one baseline, a tick where every measurement
// begins and ends, and the measurement lettered over the segment it belongs to.
//
// The ticks are the drawing convention rather than an arrowhead — a short stroke at 45° across the
// baseline, which is how a plan is dimensioned and what the Window Manager Settings canvas draws.
// Taken from it: the baseline 1 px in the ink, the upright 18 px in #A7AAA3, the 45° stroke 11 px in
// the ink, the figures in the mono face at 11 px above.
//
// `marks` is the boundaries along the run, each 0..1 of the width. `labels` is one entry per segment
// between them: { text, strong }.
import QtQuick

Item {
    id: chain
    property var marks: []
    property var labels: []
    property color ink: "#16171A"
    property color tick: "#A7AAA3"
    property string face: "monospace"
    implicitHeight: 40

    Rectangle {         // the baseline
        y: chain.height - 12 - height
        width: chain.width; height: 1
        color: chain.ink
    }

    Repeater {          // a tick where a measurement begins or ends
        model: chain.marks
        Item {
            required property real modelData
            x: Math.round(modelData * chain.width)
            y: 0; width: 1; height: chain.height
            Rectangle {     // the upright, through the baseline
                y: parent.parent.height - 18
                width: 1; height: 18
                color: chain.tick
            }
            Rectangle {     // and the stroke across it, which is the dimension's own mark
                y: parent.parent.height - 12 - 11 / 2
                width: 1; height: 11
                color: chain.ink
                transform: Rotation { origin.x: 0.5; origin.y: 5.5; angle: 45 }
            }
        }
    }

    Repeater {          // what each segment measures, over the middle of it
        model: chain.labels
        Text {
            required property var modelData
            required property int index
            readonly property real a: index < chain.marks.length ? chain.marks[index] : 0
            readonly property real b: index + 1 < chain.marks.length ? chain.marks[index + 1] : 1
            x: Math.round((a + b) / 2 * chain.width) - width / 2
            y: chain.height - 12 - 7 - height
            visible: (b - a) * chain.width > width + 6
            text: modelData.text
            font.family: chain.face; font.pixelSize: 11
            font.weight: modelData.strong ? Font.Medium : Font.Normal
            color: modelData.strong ? chain.ink : "#63676C"
        }
    }
}
