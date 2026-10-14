// A piece of lettering on the sheet: text in the sheet's mono face, with optional click and scroll actions.
import QtQuick

Text {
    id: cell
    property var onLeft: null
    property var onMiddle: null
    property var onRight: null
    property var onScroll: null          // called with +1 (up) or -1 (down)
    property real size: 12               // logical px
    textFormat: Text.StyledText
    font.family: "IBM Plex Mono"
    font.pixelSize: size
    anchors.verticalCenter: parent ? parent.verticalCenter : undefined

    MouseArea {
        anchors.fill: parent
        enabled: cell.onLeft || cell.onMiddle || cell.onRight || cell.onScroll
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: e => {
            const f = e.button === Qt.LeftButton ? cell.onLeft : e.button === Qt.MiddleButton ? cell.onMiddle : cell.onRight
            if (f) f()
        }
        onWheel: w => { if (cell.onScroll) cell.onScroll(w.angleDelta.y > 0 ? 1 : -1) }
    }
}
