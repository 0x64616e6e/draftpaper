// The theme picker (Mod+t, or a click on the theme cell): the themes laid out as drawings on the board
// (Board.qml: the inside of the sheet's frame cleared to the sheet's paper, or the wallpaper blurred), in
// a grid, all the same size, each a miniature desktop in that theme (Desktop.qml) drawn from its own
// values (`draftpaper theme json`) with a drawing label below; the picked one is outlined in heavier ink.
// The margins stay the sheet's, the keys are lettered in the top margin (shell.qml, the notice zone), and
// the title block shows when no window covers it. Browsing previews live: the sheet takes the picked
// theme a moment after it is picked; Enter keeps it (and runs config.toml's [hooks] theme: the
// applications), Escape goes back to the theme it was, E edits the picked one (ThemeEditor.qml, which
// comes back here on Escape). The arrows (or h j k l) and the wheel browse; a click on a drawing picks
// it, on the picked one keeps it, anywhere else goes back.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: picker
    required property var shell
    visible: false
    // the screen it opens on, fixed until it closes (not the focus's, which moves as the pointer does)
    screen: Quickshell.screens[0]
    function focusedScreen() { return Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0] }
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: out ? out.px_per_mm : 4.2
    readonly property real dpr: out ? out.scale : 1
    property var themes: []
    property int selected: 0
    property string original: ""        // the theme when the picker opened: Escape goes back to it
    property string shown: ""           // the theme applied for the preview
    property string wallpaper: ""       // config.toml's, as a file URL ("" for none)
    // the keys, lettered in the top margin while the picker is open
    readonly property string hint: visible ? "←→↑↓ browse · e edit · enter keep · esc back to "
        + (themes.find(t => t.name === original) || { label: "" }).label : ""

    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-themes"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"

    // the board's theme: the one the sheet shows
    readonly property var t: themes.find(x => x.name === shown) || themes[selected] || null
    readonly property color ink: board.ink
    readonly property real inX: board.inX
    readonly property real inY: board.inY
    readonly property real inW: board.inW
    readonly property real inH: board.inH
    readonly property bool blockCovered: board.blockCovered

    // the drawings: a grid of columns enough for a square-ish layout, clear of the title block's height
    // when it shows
    readonly property int cols: Math.max(1, Math.ceil(Math.sqrt(themes.length)))
    readonly property int rows: Math.max(1, Math.ceil(themes.length / cols))
    readonly property real gutter: 8 * mm
    readonly property real labelH: 8 * mm
    readonly property real areaH: inH - gutter - (blockCovered ? 0 : 36 * mm + gutter)
    readonly property real cardH: Math.max(10, Math.min((areaH - gutter - (rows - 1) * gutter - rows * labelH) / rows,
                                                        ((inW - 2 * gutter - (cols - 1) * gutter) / cols) * height / Math.max(1, width)))
    readonly property real cardW: cardH * width / Math.max(1, height)
    readonly property real gridX: inX + (inW - cols * cardW - (cols - 1) * gutter) / 2
    readonly property real gridY: inY + gutter + Math.max(0, (areaH - gutter - rows * (cardH + labelH) - (rows - 1) * gutter) / 2)

    function open() {
        screen = focusedScreen()
        load.running = true
        wall.running = true
        visible = true
    }
    // browsing switches the sheet only (`--preview`: no [hooks] theme); keeping switches it all
    function preview(name) {
        if (!name || name === shown) return
        shown = name
        shell.run([shell.bin, "theme", name, "--preview"])
    }
    function keep() {
        const name = themes[selected] ? themes[selected].name : ""
        if (name) shell.run([shell.bin, "theme", name])
        shown = name
        visible = false
    }
    function cancel() {
        preview(original)
        visible = false
    }
    // the editor on the picked theme; Escape there comes back here (resume)
    function edit() {
        const name = themes[selected] ? themes[selected].name : ""
        if (!name) return
        settle.stop()
        visible = false
        shell.editTheme(name, picker.shown, true, picker.screen)
    }
    function resume(name) {
        load.running = true            // the themes again: one may have been saved
        reselect = name
        visible = true
    }
    property string reselect: ""
    function pick(i) {
        selected = Math.max(0, Math.min(themes.length - 1, i))
        settle.restart()
    }
    // the sheet takes the picked theme once the picking pauses
    Timer { id: settle; interval: 350; onTriggered: picker.preview(picker.themes[picker.selected] ? picker.themes[picker.selected].name : "") }

    Process {
        id: load
        command: [picker.shell.bin, "theme", "json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    picker.themes = JSON.parse(this.text)
                    if (picker.reselect) {         // back from the editor: the same theme picked, the same one to go back to
                        picker.selected = Math.max(0, picker.themes.findIndex(t => t.name === picker.reselect))
                        picker.shown = picker.reselect
                        picker.reselect = ""
                        return
                    }
                    const i = Math.max(0, picker.themes.findIndex(t => t.current))
                    picker.original = picker.shown = picker.themes[i] ? picker.themes[i].name : ""
                    picker.selected = i
                } catch (e) {}
            }
        }
    }
    Process {
        id: wall
        command: [picker.shell.bin, "wallpaper"]
        stdout: StdioCollector {
            onStreamFinished: {
                const p = this.text.trim().replace(/^~(?=\/)/, Quickshell.env("HOME"))
                picker.wallpaper = p && p !== "none" ? "file://" + p : ""
            }
        }
    }

    Board { id: board; out: picker.out; t: picker.t; wallpaper: picker.wallpaper }

    Item {
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const k = e.key
            if (k === Qt.Key_Escape) picker.cancel()
            else if (k === Qt.Key_Left || k === Qt.Key_H) picker.pick(picker.selected - 1)
            else if (k === Qt.Key_Right || k === Qt.Key_L) picker.pick(picker.selected + 1)
            else if (k === Qt.Key_Up || k === Qt.Key_K) picker.pick(picker.selected - picker.cols)
            else if (k === Qt.Key_Down || k === Qt.Key_J) picker.pick(picker.selected + picker.cols)
            else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) picker.keep()
            else if (k === Qt.Key_E) picker.edit()
            else return
            e.accepted = true
        }
        MouseArea {
            anchors.fill: parent
            onClicked: picker.cancel()
            onWheel: w => picker.pick(picker.selected + (w.angleDelta.y < 0 || w.angleDelta.x < 0 ? 1 : -1))
        }

        Repeater {
            model: picker.themes
            Item {
                id: card
                required property var modelData
                required property int index
                readonly property bool picked: index === picker.selected
                x: picker.gridX + (index % picker.cols) * (picker.cardW + picker.gutter)
                y: picker.gridY + Math.floor(index / picker.cols) * (picker.cardH + picker.labelH + picker.gutter)
                width: picker.cardW; height: picker.cardH

                Desktop { anchors.fill: parent; t: card.modelData }
                Rectangle {          // the outline: a hairline of ink, heavier around the picked one
                    readonly property real w: card.picked ? Math.max(2, 0.7 * picker.mm) : Math.max(1 / picker.dpr, 0.2 * picker.mm)
                    anchors.fill: parent
                    anchors.margins: -w
                    color: "transparent"
                    border.width: w
                    border.color: card.picked ? picker.ink : Qt.rgba(picker.ink.r, picker.ink.g, picker.ink.b, 0.7)
                }
                Text {               // the drawing's label
                    anchors { left: parent.left; top: parent.bottom; topMargin: 2.2 * picker.mm }
                    text: "DWG " + String(card.index + 1).padStart(2, "0") + "    " + card.modelData.label.toUpperCase()
                    font.family: Fonts.mono; font.pixelSize: 3 * picker.mm; font.letterSpacing: 0.3 * picker.mm
                    font.bold: card.picked
                    color: card.picked ? picker.ink : Qt.rgba(picker.ink.r, picker.ink.g, picker.ink.b, 0.7)
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: card.picked ? picker.keep() : picker.pick(card.index)
                }
            }
        }
    }
}
