// A cell of the drafting sheet's margin: the stretch of a Margin between two zone ticks (as the renderer
// draws them), `span` zones long. `index` counts from the start (left, or top on the sides) from 0, or
// from the end from -1. What goes in it is laid out by its children.
import QtQuick

Item {
    id: zone
    required property var band
    property int index: 0
    property int span: 1
    readonly property int first: index < 0 ? band.zones + index - span + 1 : index
    readonly property real start: band.offset + first * band.zoneLength
    x: band.vertical ? 0 : start
    y: band.vertical ? start : 0
    width: band.vertical ? band.frame : span * band.zoneLength
    height: band.vertical ? span * band.zoneLength : band.frame
}
