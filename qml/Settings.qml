// Draftpaper's settings (Mod+t, a click on the theme cell, `draftpaper settings`), on the board
// (Board.qml: the inside of the sheet's frame cleared to the paper, or the wallpaper blurred). The
// categories run down the left like the title block's cells; for now one, the colour scheme (more to
// come, #36: displays, text, wallpaper).
//
// The colour scheme: the themes as drawings along the top (Desktop.qml), each lettered with its main
// contrasts, or with how many of its words fall below 4.5:1; and below them the builder, six rows of
// discrete choices (paper, tone, accent, windows, terminal text, contrast) from which `draftpaper scheme`
// derives a whole theme whose every word is readable (scheme.py). Building shapes the last drawing,
// Custom. Whatever is picked, a drawing or a choice, the sheet previews a moment later; Enter keeps it
// (the applications too: a theme switch, or `draftpaper scheme keep`, which writes [scheme] and the
// custom theme); Escape goes back to the theme it was. R twice on your copy of a shipped theme puts the
// shipped one back. Up and down (k, j) move between the drawings and the rows, left and right (h, l)
// along them; the wheel browses, a click picks, a click on the picked drawing keeps it.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: settings
    required property var shell
    visible: false
    // the screen it opens on, fixed until it closes (not the focus's, which moves as the pointer does)
    screen: Quickshell.screens[0]
    function focusedScreen() { return Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0] }
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: board.mm
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-settings"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"

    property var themes: []             // `draftpaper theme json`, without the custom theme
    property var custom: null           // the scheme the choices build (`draftpaper scheme json`)
    property var options: ({})         // each choice's values, with their colours and contrasts
    property var choices: ({})
    property int row: 0                 // 0: the drawings; 1 to 6: the builder's rows
    property int card: 0                // the drawing picked
    property string original: ""        // the theme when it opened: Escape goes back to it
    property string shown: ""           // the theme the sheet previews
    property string wallpaper: ""
    property bool reverting: false
    readonly property var rows: ["paper", "tone", "accent", "windows", "text", "contrast"]
    readonly property var rowNames: ({ paper: "PAPER", tone: "TONE", accent: "ACCENT", windows: "WINDOWS", text: "TERMINAL TEXT", contrast: "CONTRAST" })
    // the drawings: the themes, then the one the choices build
    readonly property var cards: custom ? themes.concat([custom]) : themes
    readonly property var picked: cards[card] || null
    readonly property bool isCustom: picked !== null && picked.name === "custom"

    readonly property string hint: !visible ? ""
        : reverting ? "r again: " + (picked ? picked.label : "") + " as shipped"
        : "↑↓ drawings and choices · ←→ pick · enter keep · esc back to " + (themes.find(t => t.name === original) || { label: original }).label
          + (picked && picked.own && picked.shipped ? " · r revert" : "")

    // ---- opening, picking, keeping ----
    function open() {
        screen = focusedScreen()
        row = 0
        reverting = false
        themes = []
        custom = null
        loadThemes.running = true
        wall.running = true
        visible = true
    }
    function args() { return rows.map(k => k + "=" + choices[k]) }
    function pickCard(i) {
        card = Math.max(0, Math.min(cards.length - 1, i))
        settle.restart()
    }
    function choose(k, v) {
        const c = Object.assign({}, choices)
        c[k] = v
        choices = c
        card = cards.length - 1                        // Custom
        reloadScheme()
        settle.restart()
    }
    function step(dir) {
        if (row === 0) pickCard(card + dir)
        else {
            const k = rows[row - 1], opts = options[k] || []
            const i = opts.findIndex(o => o.value === choices[k])
            const n = Math.max(0, Math.min(opts.length - 1, i + dir))
            if (opts[n] && n !== i) choose(k, opts[n].value)
        }
    }
    // the sheet previews what is picked once the picking pauses
    Timer {
        id: settle; interval: 350
        onTriggered: {
            if (!settings.picked) return
            if (settings.isCustom) { settings.queue(["scheme", "preview"].concat(settings.args()), true); settings.shown = "custom.draft" }
            else if (settings.picked.name !== settings.shown) { settings.queue(["theme", settings.picked.name, "--preview"], true); settings.shown = settings.picked.name }
        }
    }
    function keep() {
        settle.stop()
        if (!picked) return
        if (isCustom) queue(["scheme", "keep"].concat(args()), false)
        else queue(["theme", picked.name], false)
        visible = false
    }
    function cancel() {
        settle.stop()
        if (shown !== original) queue(["theme", original, "--preview"], false)
        visible = false
    }
    function revert() {
        if (!picked || !picked.own || !picked.shipped) return
        queue(["theme", "revert", picked.name], false)
        reverting = false
        loadThemes.running = true
    }
    Timer { id: revertWindow; interval: 2000; onTriggered: settings.reverting = false }

    // `draftpaper ...` one at a time, in order; a newer preview replaces one still waiting
    property var waiting: []
    function queue(a, isPreview) {
        const w = waiting.filter(x => !(isPreview && x.preview))
        w.push({ args: a, preview: isPreview })
        waiting = w
        pump()
    }
    function pump() {
        if (runner.running || waiting.length === 0) return
        runner.command = [shell.bin].concat(waiting[0].args)
        waiting = waiting.slice(1)
        runner.running = true
    }
    Process { id: runner; onExited: settings.pump() }

    // ---- what it shows ----
    Process {
        id: loadThemes
        command: [settings.shell.bin, "theme", "json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const all = JSON.parse(this.text)
                    const cur = all.find(t => t.current)
                    const keepName = settings.picked ? settings.picked.name : ""
                    settings.themes = all.filter(t => t.name !== "custom")
                    if (!settings.original) {
                        settings.original = settings.shown = cur ? cur.name : (all[0] ? all[0].name : "")
                    }
                    const at = settings.cards.findIndex(t => t.name === (keepName || settings.original))
                    settings.card = at >= 0 ? at : 0
                } catch (e) {}
                if (!settings.custom) loadOptions.running = true
            }
        }
    }
    function reloadScheme() {
        loadOptions.command = [shell.bin, "scheme", "options"].concat(args())
        loadOptions.running = true
    }
    Process {
        id: loadOptions
        command: [settings.shell.bin, "scheme", "options"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const d = JSON.parse(this.text)
                    settings.options = d.options
                    settings.choices = d.choices
                    loadCustom.command = [settings.shell.bin, "scheme", "json"].concat(settings.args())
                    loadCustom.running = true
                } catch (e) {}
            }
        }
    }
    Process {
        id: loadCustom
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const t = JSON.parse(this.text)
                    t.label = "Custom · " + t.label
                    const wasCustom = settings.isCustom
                    settings.custom = t
                    if (wasCustom) settings.card = settings.cards.length - 1
                    // the current theme is the custom one: pick it
                    if (settings.original === "custom" && settings.row === 0 && !wasCustom) settings.card = settings.cards.length - 1
                } catch (e) {}
            }
        }
    }
    Process {
        id: wall
        command: [settings.shell.bin, "wallpaper"]
        stdout: StdioCollector {
            onStreamFinished: {
                const p = this.text.trim().replace(/^~(?=\/)/, Quickshell.env("HOME"))
                settings.wallpaper = p && p !== "none" ? "file://" + p : ""
            }
        }
    }
    onVisibleChanged: if (visible) keys.forceActiveFocus(); else original = ""

    // the board takes the theme the sheet shows: the picked drawing's
    Board { id: board; out: settings.out; t: settings.picked; wallpaper: settings.wallpaper; keepBlock: false }

    // ---- the layout ----
    readonly property real gutter: 8 * mm
    readonly property color ink: board.ink
    function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }
    readonly property real font: 2.8 * mm
    readonly property real catX: board.inX + gutter
    readonly property real catW: 42 * mm
    readonly property real x0: catX + catW + gutter
    readonly property real cw: board.inX + board.inW - gutter - x0
    readonly property real cardGap: 5 * mm
    readonly property real cardW: Math.min(70 * mm, (cw - (cards.length - 1) * cardGap) / Math.max(1, cards.length))
    readonly property real cardH: cardW * height / Math.max(1, width)
    readonly property real rowH: 12 * mm
    readonly property real cellW: Math.min(24 * mm, (cw - 34 * mm) / 8)

    Item {
        id: keys
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const k = e.key
            if (k !== Qt.Key_R) settings.reverting = false
            if (k === Qt.Key_Escape) settings.cancel()
            else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) settings.keep()
            else if (k === Qt.Key_Up || k === Qt.Key_K) settings.row = Math.max(0, settings.row - 1)
            else if (k === Qt.Key_Down || k === Qt.Key_J) settings.row = Math.min(settings.rows.length, settings.row + 1)
            else if (k === Qt.Key_Left || k === Qt.Key_H) settings.step(-1)
            else if (k === Qt.Key_Right || k === Qt.Key_L) settings.step(1)
            else if (k === Qt.Key_R && settings.picked && settings.picked.own && settings.picked.shipped) {
                if (settings.reverting) settings.revert()
                else { settings.reverting = true; revertWindow.restart() }
            }
            else return
            e.accepted = true
        }
        MouseArea {
            anchors.fill: parent
            onClicked: settings.cancel()
            onWheel: w => settings.step(w.angleDelta.y < 0 || w.angleDelta.x < 0 ? 1 : -1)
        }

        // the categories, ruled like the title block's cells
        Rectangle {
            x: settings.catX; y: board.inY + settings.gutter
            width: settings.catW; height: 14 * settings.mm
            color: "transparent"
            border.width: Math.max(2, 0.6 * settings.mm); border.color: settings.ink
            Text {
                x: 2 * settings.mm; y: 1.4 * settings.mm
                text: "SETTINGS"
                font.family: Fonts.mono; font.pixelSize: settings.font * 0.75; font.letterSpacing: 0.3 * settings.mm
                color: settings.inkA(0.6)
            }
            Text {
                x: 2 * settings.mm; anchors.bottom: parent.bottom; anchors.bottomMargin: 2 * settings.mm
                text: "COLOUR SCHEME"
                font.family: Fonts.mono; font.pixelSize: settings.font * 1.1; font.bold: true
                color: settings.ink
            }
        }

        // the drawings
        Text {
            x: settings.x0; y: board.inY + settings.gutter
            text: "THEMES"
            font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.3 * settings.mm; font.bold: settings.row === 0
            color: settings.row === 0 ? settings.ink : settings.inkA(0.6)
        }
        Repeater {
            model: settings.cards
            Item {
                id: c
                required property var modelData
                required property int index
                readonly property bool picked: index === settings.card
                x: settings.x0 + index * (settings.cardW + settings.cardGap)
                y: board.inY + settings.gutter + 6 * settings.mm
                width: settings.cardW; height: settings.cardH
                Desktop { anchors.fill: parent; t: c.modelData }
                Rectangle {
                    readonly property real w: c.picked ? Math.max(2, 0.7 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                    anchors.fill: parent; anchors.margins: -w
                    color: "transparent"; border.width: w
                    border.color: c.picked ? settings.ink : settings.inkA(0.6)
                }
                Text {
                    id: name
                    anchors { left: parent.left; top: parent.bottom; topMargin: 2 * settings.mm }
                    width: parent.width; elide: Text.ElideRight
                    text: c.modelData.label.toUpperCase()
                    font.family: Fonts.mono; font.pixelSize: settings.font * 0.9; font.letterSpacing: 0.2 * settings.mm; font.bold: c.picked
                    color: c.picked ? settings.ink : settings.inkA(0.75)
                }
                Text {          // its contrasts, or how many words it leaves unreadable
                    anchors { left: parent.left; top: name.bottom; topMargin: 0.8 * settings.mm }
                    readonly property var f: c.modelData.figures || {}
                    text: f.below > 0 ? "⚠ " + f.below + " below 4.5:1"
                        : "ink " + f.ink + " · text " + f.text + " · term " + f.terminal
                    font.family: Fonts.mono; font.pixelSize: settings.font * 0.75
                    color: f.below > 0 ? board.t ? board.t.alert : "red" : settings.inkA(0.6)
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: { settings.row = 0; c.picked ? settings.keep() : settings.pickCard(c.index) }
                }
            }
        }

        // the builder
        Item {
            id: builder
            x: settings.x0
            y: board.inY + settings.gutter + 6 * settings.mm + settings.cardH + 16 * settings.mm
            width: settings.cw
            Text {
                text: "BUILD A SCHEME · the last drawing, every word readable"
                font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.3 * settings.mm
                font.bold: settings.row > 0
                color: settings.row > 0 ? settings.ink : settings.inkA(0.6)
            }
            Repeater {
                model: settings.rows
                Item {
                    id: r
                    required property string modelData
                    required property int index
                    readonly property bool here: settings.row === index + 1
                    y: 6 * settings.mm + index * settings.rowH
                    width: builder.width; height: settings.rowH
                    Rectangle {         // the rule above the row
                        width: parent.width; height: Math.max(1, 0.15 * settings.mm)
                        color: settings.inkA(0.35)
                    }
                    Text {
                        y: 2.5 * settings.mm
                        text: settings.rowNames[r.modelData]
                        font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.bold: r.here
                        color: r.here ? settings.ink : settings.inkA(0.75)
                    }
                    Repeater {
                        model: settings.options[r.modelData] || []
                        Item {
                            id: o
                            required property var modelData
                            required property int index
                            readonly property bool chosen: settings.choices[r.modelData] === modelData.value
                            x: 34 * settings.mm + index * settings.cellW
                            y: 1.5 * settings.mm
                            width: settings.cellW - 2 * settings.mm; height: settings.rowH - 2.5 * settings.mm
                            Rectangle {     // the swatch; for the terminal's text, letters on its background
                                id: sw
                                width: parent.width; height: parent.height * 0.5
                                color: o.modelData.on || o.modelData.colour
                                border.width: Math.max(1, 0.15 * settings.mm); border.color: settings.inkA(0.7)
                                Text {
                                    anchors.centerIn: parent
                                    visible: o.modelData.on !== null && o.modelData.on !== undefined
                                    text: "Aa"
                                    font.family: Fonts.mono; font.pixelSize: parent.height * 0.6
                                    color: o.modelData.colour
                                }
                            }
                            Text {
                                anchors { left: parent.left; top: sw.bottom; topMargin: 0.6 * settings.mm }
                                text: o.modelData.value
                                font.family: Fonts.mono; font.pixelSize: settings.font * 0.75; font.bold: o.chosen
                                color: o.chosen ? settings.ink : settings.inkA(0.7)
                            }
                            Text {
                                anchors { right: parent.right; top: sw.bottom; topMargin: 0.6 * settings.mm }
                                text: o.modelData.ratio + ":1"
                                font.family: Fonts.mono; font.pixelSize: settings.font * 0.65
                                color: settings.inkA(0.55)
                            }
                            Rectangle {     // the chosen value, outlined in heavier ink when the row is the one moving
                                visible: o.chosen
                                anchors.fill: parent; anchors.margins: -0.8 * settings.mm
                                color: "transparent"
                                border.width: r.here ? Math.max(2, 0.6 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                                border.color: r.here ? settings.ink : settings.inkA(0.6)
                            }
                            MouseArea {
                                anchors.fill: parent; anchors.margins: -0.8 * settings.mm
                                onClicked: { settings.row = r.index + 1; settings.choose(r.modelData, o.modelData.value) }
                            }
                        }
                    }
                }
            }
        }
    }
}
