// The columns' numbers along a top or bottom Margin (shell.qml), as a drawing's frame numbers its zones
// (1 from the left): one in the middle of every zone that nothing else takes (`taken`, a zone's index from
// 0), lettered as the side margins letter their rows (SideCells.qml).
import QtQuick

Item {
    id: numbers
    required property var band
    required property var edge
    property var taken: k => false
    anchors.fill: parent

    Repeater {
        model: numbers.band.zones
        Item {              // the delegate holds the Zone (whose own `index` is its place in the band)
            id: cell
            required property int index
            Zone {
                band: numbers.band; index: cell.index
                visible: !numbers.taken(cell.index)
                Text {
                    anchors.centerIn: parent
                    text: cell.index + 1
                    font.family: Fonts.mono; font.pixelSize: numbers.edge.fontPx * 0.8
                    color: Qt.rgba(numbers.band.ink.r, numbers.band.ink.g, numbers.band.ink.b, 0.8)
                }
            }
        }
    }
}
