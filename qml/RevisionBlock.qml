// The revision block (issue #45, one per screen), as drawings keep one above the title block: the latest
// commits of the git repository the focused window works in, a row each of REV, DESCRIPTION, DATE and
// BY, the repository's name in the header. The compositor draws its rules on whole pixels while this
// surface is up (compositor/src/title.rs's revision_block, sent as the output's "revisions"); the shell
// letters the cells. Shown on the focused screen when [title] revisions = true and the window is in a
// repository (`draftpaper revisions`, polled by shell.qml); off by default, since commit subjects are then
// on screen and in every screenshot. On the bottom layer, under every window, like the title block.
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: block
    required property var shell           // the ShellRoot of shell.qml: theme and the polled revisions
    required property var edge            // this screen (shell.qml): its sheet's geometry from the compositor

    readonly property var theme: shell.theme
    readonly property real mm: edge.mm
    readonly property color ink: theme.ink
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property var geo: edge.output && edge.output.revisions ? edge.output.revisions : null
    readonly property var revs: shell.revisions
    readonly property bool shown: theme.title.revisions === true && geo !== null && !!edge.output && edge.output.focused
        && !!revs.rows && revs.rows.length > 0

    // the surface: whole logical pixels from the frame's right edge and from the block's top
    readonly property int marginR: Math.round(edge.margin)
    readonly property int surfTop: geo ? Math.floor(geo.rect.y) : 0
    readonly property int surfW: geo ? Math.ceil(edge.width - marginR - geo.rect.x - 1e-6) : 1
    readonly property int surfH: geo ? Math.ceil(geo.rect.y + geo.rect.height - surfTop - 1e-6) : 1
    readonly property real ox: geo ? geo.rect.x - (edge.width - marginR - surfW) : 0
    readonly property real oy: geo ? geo.rect.y - surfTop : 0

    // the cells' words, in the compositor's order: the header's four, then each row's
    readonly property var words: {
        const out = ["REV", "REVISIONS" + (revs.repo ? " · " + String(revs.repo).toUpperCase() : ""), "DATE", "BY"]
        for (let r = 0; r < (geo ? geo.cells.length / 4 - 1 : 0); r++) {
            const row = revs.rows && r < revs.rows.length ? revs.rows[r] : null
            out.push(row ? row.rev : "", row ? row.description : "", row ? row.date : "", row ? row.by : "")
        }
        return out
    }

    PanelWindow {
        screen: block.edge.screen
        visible: block.shown
        anchors { top: true; right: true }
        margins { top: block.surfTop; right: block.marginR }
        implicitWidth: block.surfW
        implicitHeight: block.surfH
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        WlrLayershell.namespace: "draftpaper-revisions"

        Item {
            x: block.ox; y: block.oy
            width: block.geo ? block.geo.rect.width : 0
            height: block.geo ? block.geo.rect.height : 0
            clip: true
            Repeater {
                model: block.geo ? block.geo.cells : []
                Item {
                    required property var modelData
                    required property int index
                    readonly property bool header: index < 4
                    x: modelData.x - block.geo.rect.x; y: modelData.y - block.geo.rect.y
                    width: modelData.width; height: modelData.height
                    clip: true
                    Text {
                        x: 1.6 * block.mm; width: parent.width - 3.2 * block.mm
                        anchors.verticalCenter: parent.verticalCenter
                        text: block.words[index] || ""
                        elide: Text.ElideRight
                        font.family: Fonts.mono
                        font.pixelSize: header ? Math.min(2.2 * block.mm, parent.height * 0.55) : Math.min(3.2 * block.mm, parent.height * 0.5)
                        font.letterSpacing: header ? 0.3 * block.mm : 0
                        color: header ? block.faint : block.ink
                    }
                }
            }
        }
    }
}
