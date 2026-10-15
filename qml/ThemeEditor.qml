// The theme editor (`draftpaper theme edit [NAME]`, or E in the theme picker): on the same board as the
// picker (Board.qml, the inside of the frame cleared), a schedule of the theme's values down the left
// (the sheet's colours and shading, the window borders, the terminal's colours and ANSI palette, which
// the applications take on saving), scrolled to the value picked, and on the right the theme drawn as a
// miniature desktop (Desktop.qml) over the instrument for the value picked: a colour on three ruled
// scales (hue, saturation, lightness) and as hex, a number on one scale. The miniature follows every
// change at once; the sheet itself (paper, margins, title block) a moment later, from a draft of the
// theme beside the user's own (`draftpaper theme draft`). Enter saves the user's own copy of the theme
// (`draftpaper theme save`, which a shipped theme of that name yields to, and switches to it whole);
// Escape drops the draft and goes back (to the picker, when it came from there); R twice puts a shipped
// theme back as shipped. Up and down (k, j) pick a value, left and right (h, l) move its scale (with
// Shift ten times as far), Tab the next scale, # types the hex value; the mouse picks and drags too.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: editor
    required property var shell
    visible: false
    screen: Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0]
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: board.mm
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-theme-editor"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"

    property string name: ""            // the theme edited
    property string back: ""            // the theme to show again on Escape
    property bool fromPicker: false
    property var entry: null            // the theme as it was (`draftpaper theme json`): label, own, shipped
    property var v: null                // the values as edited
    property bool edited: false
    property string wallpaper: ""
    property int row: 0
    property int scale: 0               // a colour's scale: 0 hue, 1 saturation, 2 lightness
    property var hsl: [0, 0, 0]         // the picked colour's, kept apart so a grey keeps its hue
    property bool reverting: false      // R pressed once: again within two seconds reverts
    readonly property bool typing: hex.activeFocus

    readonly property var rows: [
        { group: "SHEET" },
        { key: "paper", label: "paper" },
        { key: "paper_light", label: "paper, lit" },
        { key: "paper_shade", label: "paper, shaded" },
        { key: "glow", label: "glow" },
        { key: "ink", label: "ink" },
        { key: "alert", label: "alert" },
        { key: "strong", label: "focused workspace" },
        { group: "SHADING" },
        { section: "sheet", key: "angle", label: "angle", min: 0, max: 90, step: 1, unit: "°" },
        { section: "sheet", key: "midpoint", label: "midpoint", min: 0, max: 1, step: 0.01 },
        { section: "sheet", key: "blend", label: "blend", min: 0.05, max: 2, step: 0.01 },
        { section: "sheet", key: "shading", label: "shading", min: 0, max: 1.5, step: 0.01 },
        { section: "sheet", key: "glow_strength", label: "glow strength", min: 0, max: 3, step: 0.05 },
        { section: "sheet", key: "glow_size", label: "glow size", min: 0.2, max: 3, step: 0.05 },
        { section: "sheet", key: "pattern_strength", label: "pattern", min: 0, max: 1, step: 0.01 },
        { group: "BORDERS" },
        { section: "windows", key: "focused", label: "focused · accent" },
        { section: "windows", key: "unfocused", label: "the others" },
        { group: "TERMINAL · applied on saving" },
        { section: "apps", key: "term_bg", label: "background" },
        { section: "apps", key: "term_fg", label: "text" },
        { section: "apps", key: "cursor", label: "cursor" },
        { section: "apps", key: "cursor_text", label: "under the cursor" },
        { section: "apps", key: "sel_bg", label: "selection" },
        { section: "apps", key: "sel_fg", label: "selected text" },
        { section: "apps", key: "black", label: "black" },
        { section: "apps", key: "red", label: "red" },
        { section: "apps", key: "green", label: "green" },
        { section: "apps", key: "yellow", label: "yellow" },
        { section: "apps", key: "blue", label: "blue" },
        { section: "apps", key: "magenta", label: "magenta" },
        { section: "apps", key: "cyan", label: "cyan" },
        { section: "apps", key: "white", label: "white" },
        { section: "apps", key: "br_black", label: "bright black" },
        { section: "apps", key: "br_red", label: "bright red" },
        { section: "apps", key: "br_green", label: "bright green" },
        { section: "apps", key: "br_yellow", label: "bright yellow" },
        { section: "apps", key: "br_blue", label: "bright blue" },
        { section: "apps", key: "br_magenta", label: "bright magenta" },
        { section: "apps", key: "br_cyan", label: "bright cyan" },
        { section: "apps", key: "br_white", label: "bright white" }
    ]
    readonly property var current: rows[row] || rows[1]
    readonly property bool isColour: current.min === undefined
    function get(r) { return !v || !r.key ? undefined : r.section ? (v[r.section] || {})[r.key] : v[r.key] }

    readonly property string hint: !visible ? ""
        : typing ? "enter use this colour · esc leave it"
        : reverting ? "r again: back to " + (entry ? entry.label : "") + " as shipped"
        : "↑↓ value · ←→ adjust (shift ×10) · tab H S L · # hex · enter save · esc " + (fromPicker ? "back" : "discard")
          + (entry && entry.own && entry.shipped ? " · r revert" : "")

    // ---- opening, values, the draft ----
    function open(themeName, backTo, picker) {
        name = themeName
        back = backTo
        fromPicker = picker
        entry = null
        v = null
        edited = false
        reverting = false
        row = 1
        scale = 0
        load.running = true
        wall.running = true
        visible = true
    }
    function values() {
        const o = {}
        for (const k of ["paper", "paper_light", "paper_shade", "glow", "ink", "alert", "strong"]) o[k] = v[k]
        o.sheet = v.sheet
        o.windows = v.windows
        // the terminal's values only: the rest of the [apps] palette stays as the file has it
        o.apps = {}
        for (const r of rows) if (r.section === "apps" && v.apps && v.apps[r.key]) o.apps[r.key] = v.apps[r.key]
        return o
    }
    function set(r, value) {
        const n = JSON.parse(JSON.stringify(v))
        if (r.section) { n[r.section] = n[r.section] || {}; n[r.section][r.key] = value }
        else n[r.key] = value
        v = n
        edited = true
        draft.restart()
    }
    // a moment after the last change, the sheet previews the draft
    Timer { id: draft; interval: 150; onTriggered: editor.queue(["draft", editor.name, JSON.stringify(editor.values())], true) }

    // `draftpaper theme ...` one at a time, in order: a draft still being written must not land after a
    // save or a discard; a newer draft replaces one still waiting
    property var waiting: []
    function queue(args, isDraft) {
        const w = waiting.filter(a => !(isDraft && a[0] === "draft"))
        w.push(args)
        waiting = w
        pump()
    }
    function pump() {
        if (runner.running || waiting.length === 0) return
        runner.command = [shell.bin, "theme"].concat(waiting[0])
        waiting = waiting.slice(1)
        runner.running = true
    }
    Process { id: runner; onExited: editor.pump() }

    function save() {
        draft.stop()
        if (edited) queue(["save", name, JSON.stringify(values())], false)
        else if (fromPicker) queue([name], false)            // nothing changed: keep the theme as it is
        visible = false
    }
    function discard() {
        draft.stop()
        const to = fromPicker ? name : back
        queue(["discard", name, to], false)
        visible = false
        if (fromPicker) shell.resumePicker(name)
    }
    function revert() {
        draft.stop()
        queue(["revert", name], false)
        visible = false
    }
    Timer { id: revertWindow; interval: 2000; onTriggered: editor.reverting = false }

    Process {
        id: load
        command: [editor.shell.bin, "theme", "json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const all = JSON.parse(this.text)
                    const t = all.find(x => x.name === editor.name) || all.find(x => x.current) || all[0]
                    editor.name = t.name
                    if (!editor.back) editor.back = (all.find(x => x.current) || t).name
                    editor.entry = t
                    const v = JSON.parse(JSON.stringify(t))
                    v.strong = v.strong || "#000000"        // as the shell has it when a theme leaves it out
                    editor.v = v
                    editor.pickRow(1)
                    // the sheet shows the theme edited (the picker has shown it already)
                    if (!t.current && !editor.fromPicker) editor.queue([t.name, "--preview"], false)
                } catch (e) {}
            }
        }
    }
    Process {
        id: wall
        command: [editor.shell.bin, "wallpaper"]
        stdout: StdioCollector {
            onStreamFinished: {
                const p = this.text.trim().replace(/^~(?=\/)/, Quickshell.env("HOME"))
                editor.wallpaper = p && p !== "none" ? "file://" + p : ""
            }
        }
    }

    // ---- picking and moving ----
    function pickRow(i) {
        // group headings are not values: step over them
        let n = Math.max(1, Math.min(rows.length - 1, i))
        if (rows[n].group) n += (i < row ? -1 : 1)
        row = Math.max(1, Math.min(rows.length - 1, n))
        if (isColour) {
            const c = Qt.color(get(current))
            hsl = [c.hslHue < 0 ? hsl[0] : c.hslHue, c.hslSaturation, c.hslLightness]
        }
    }
    function hexOf(h, s, l) {
        const c = Qt.hsla(h, s, l, 1)
        const two = x => ("0" + Math.round(x * 255).toString(16)).slice(-2)
        return ("#" + two(c.r) + two(c.g) + two(c.b)).toUpperCase()
    }
    function setHsl(i, value) {
        const n = hsl.slice()
        n[i] = Math.max(0, Math.min(1, value))
        hsl = n
        set(current, hexOf(n[0], n[1], n[2]))
    }
    function setNumber(value) {
        const r = current
        const q = Math.round(Math.max(r.min, Math.min(r.max, value)) / r.step) * r.step
        set(r, Number(q.toFixed(2)))
    }
    function nudge(dir, far) {
        const k = far ? 10 : 1
        if (isColour) setHsl(scale, hsl[scale] + dir * k * (scale === 0 ? 1 / 360 : 0.01))
        else setNumber(get(current) + dir * k * current.step)
    }

    Board { id: board; out: editor.out; t: editor.v; wallpaper: editor.wallpaper }
    // the hex field shows the picked colour, but not while it is typed in
    onVChanged: if (!hex.activeFocus) hex.text = isColour ? (get(current) || "") : ""
    onRowChanged: if (!hex.activeFocus) hex.text = isColour ? (get(current) || "") : ""
    onVisibleChanged: if (visible) keys.forceActiveFocus()

    // ---- the layout: the schedule on the left; the drawing and the instrument on the right, clear of
    //      the title block when it shows ----
    readonly property real gutter: 8 * mm
    readonly property real colX: board.inX + gutter
    readonly property real colW: Math.min(100 * mm, board.inW * 0.36)
    readonly property real rightX: colX + colW + gutter
    readonly property real rightW: board.inX + board.inW - gutter - rightX
    readonly property real bottom: board.blockCovered ? board.inY + board.inH - gutter : board.block.y - gutter
    readonly property real instH: 40 * mm
    readonly property real drawH: Math.max(20, Math.min(rightW * height / Math.max(1, width), bottom - board.inY - 2 * gutter - instH))
    readonly property real drawW: drawH * width / Math.max(1, height)
    readonly property real font: 2.8 * mm
    readonly property color ink: board.ink
    function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }

    Item {
        id: keys
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const k = e.key, far = (e.modifiers & Qt.ShiftModifier) !== 0
            if (k !== Qt.Key_R) editor.reverting = false
            if (k === Qt.Key_Escape) editor.discard()
            else if (k === Qt.Key_Return || k === Qt.Key_Enter) editor.save()
            else if (k === Qt.Key_Up || k === Qt.Key_K) editor.pickRow(editor.row - 1)
            else if (k === Qt.Key_Down || k === Qt.Key_J) editor.pickRow(editor.row + 1)
            else if (k === Qt.Key_Left || k === Qt.Key_H) editor.nudge(-1, far)
            else if (k === Qt.Key_Right || k === Qt.Key_L) editor.nudge(1, far)
            else if (k === Qt.Key_Tab) editor.scale = (editor.scale + 1) % 3
            else if (k === Qt.Key_Backtab) editor.scale = (editor.scale + 2) % 3
            else if (k === Qt.Key_NumberSign && editor.isColour) { hex.text = editor.get(editor.current); hex.forceActiveFocus(); hex.selectAll() }
            else if (k === Qt.Key_R && editor.entry && editor.entry.own && editor.entry.shipped) {
                if (editor.reverting) editor.revert()
                else { editor.reverting = true; revertWindow.restart() }
            }
            else return
            e.accepted = true
        }

        // the theme's name, and whose it is
        Text {
            id: title
            x: editor.colX; y: board.inY + editor.gutter
            text: editor.entry ? editor.entry.label.toUpperCase() : ""
            font.family: Fonts.mono; font.pixelSize: 5 * editor.mm; font.letterSpacing: 0.5 * editor.mm; font.bold: true
            color: editor.ink
        }
        Text {
            id: whose
            x: editor.colX; anchors.top: title.bottom; anchors.topMargin: 1.2 * editor.mm
            text: !editor.entry ? ""
                : (editor.entry.own && editor.entry.shipped ? "your copy of the shipped theme"
                   : editor.entry.own ? "your own theme" : "shipped · saving makes your own copy")
                  + (editor.edited ? "  ·  edited" : "")
            font.family: Fonts.mono; font.pixelSize: 2.6 * editor.mm
            color: editor.inkA(0.7)
        }

        // the schedule: a table ruled like the title block, a value to a row
        Item {
            id: schedule
            x: editor.colX; y: whose.y + whose.height + 4 * editor.mm
            width: editor.colW
            height: board.inY + board.inH - editor.gutter - y
            readonly property real rowH: Math.max(5 * editor.mm, Math.min(6.5 * editor.mm, height / editor.rows.length))
            clip: true
            // the picked row stays in view, a row or two from the edge
            function follow() {
                const y = Math.max(Math.min(flick.contentY, (editor.row - 2) * rowH), (editor.row + 3) * rowH - height)
                flick.contentY = Math.max(0, Math.min(editor.rows.length * rowH - height, y))
            }
            Connections { target: editor; function onRowChanged() { schedule.follow() } }
            onHeightChanged: follow()
            Flickable {
                id: flick
                anchors.fill: parent
                contentWidth: width; contentHeight: schedule.rowH * editor.rows.length
                boundsBehavior: Flickable.StopAtBounds
            Rectangle {          // the outer rule
                width: parent.width; height: schedule.rowH * editor.rows.length
                color: "transparent"; border.width: Math.max(1, 0.3 * editor.mm); border.color: editor.inkA(0.85)
            }
            Repeater {
                model: editor.rows
                Item {
                    id: line
                    required property var modelData
                    required property int index
                    readonly property bool picked: index === editor.row
                    readonly property var value: editor.get(modelData)
                    y: index * schedule.rowH
                    width: schedule.width; height: schedule.rowH
                    Rectangle {      // the rule above every row but the first
                        visible: line.index > 0
                        width: parent.width; height: Math.max(1, 0.15 * editor.mm)
                        color: editor.inkA(line.modelData.group ? 0.85 : 0.35)
                    }
                    Text {           // a heading, or the value's name
                        x: 2 * editor.mm; anchors.verticalCenter: parent.verticalCenter
                        text: line.modelData.group || line.modelData.label
                        font.family: Fonts.mono; font.pixelSize: line.modelData.group ? editor.font * 0.8 : editor.font
                        font.letterSpacing: line.modelData.group ? 0.3 * editor.mm : 0
                        font.bold: line.picked
                        color: line.modelData.group ? editor.inkA(0.6) : editor.ink
                    }
                    Rectangle {      // a colour's swatch
                        visible: line.modelData.key !== undefined && line.modelData.min === undefined
                        x: parent.width * 0.58; anchors.verticalCenter: parent.verticalCenter
                        width: 9 * editor.mm; height: schedule.rowH * 0.55
                        color: line.value || "transparent"
                        border.width: Math.max(1, 0.15 * editor.mm); border.color: editor.inkA(0.8)
                    }
                    Text {           // the value
                        anchors { right: parent.right; rightMargin: 2 * editor.mm; verticalCenter: parent.verticalCenter }
                        visible: line.modelData.key !== undefined
                        text: line.value === undefined ? "" : line.modelData.min === undefined ? line.value
                            : (line.modelData.step >= 1 ? Math.round(line.value) : Number(line.value).toFixed(2)) + (line.modelData.unit || "")
                        font.family: Fonts.mono; font.pixelSize: editor.font; font.bold: line.picked
                        color: editor.ink
                    }
                    Rectangle {      // the picked row, outlined in heavier ink
                        visible: line.picked
                        anchors.fill: parent
                        color: "transparent"
                        border.width: Math.max(2, 0.6 * editor.mm); border.color: editor.ink
                    }
                    MouseArea {
                        anchors.fill: parent
                        enabled: line.modelData.key !== undefined
                        onClicked: { editor.row = line.index; editor.pickRow(line.index) }
                    }
                }
            }
            }
        }

        // the theme drawn, following every change
        Desktop {
            id: drawing
            x: editor.rightX; y: board.inY + editor.gutter
            width: editor.drawW; height: editor.drawH
            t: editor.v ? Object.assign({}, editor.v, { label: editor.entry ? editor.entry.label : "", name: editor.name }) : null
        }
        Rectangle {
            anchors.fill: drawing; anchors.margins: -border.width
            color: "transparent"; border.width: Math.max(1, 0.2 * editor.mm); border.color: editor.inkA(0.7)
        }

        // the instrument for the picked value
        Column {
            id: instrument
            x: editor.rightX; y: drawing.y + drawing.height + editor.gutter
            width: Math.max(editor.drawW, 110 * editor.mm)
            spacing: 2.5 * editor.mm
            Text {
                text: editor.current.label ? editor.current.label.toUpperCase() : ""
                font.family: Fonts.mono; font.pixelSize: editor.font * 0.9; font.letterSpacing: 0.3 * editor.mm; font.bold: true
                color: editor.ink
            }
            Repeater {
                model: editor.isColour ? 3 : 0
                Ruler {
                    required property int index
                    ed: editor
                    width: instrument.width
                    label: ["H", "S", "L"][index]
                    value: editor.hsl[index]
                    active: editor.scale === index
                    text: index === 0 ? Math.round(editor.hsl[0] * 360) + "°" : Math.round(editor.hsl[index] * 100) + "%"
                    // the band: the colour along this scale, the other two as they are
                    colourAt: f => { const h = editor.hsl.slice(); h[index] = f; return Qt.hsla(h[0], h[1], h[2], 1) }
                    onMoved: f => { editor.scale = index; editor.setHsl(index, f) }
                }
            }
            Ruler {
                visible: !editor.isColour
                ed: editor
                width: instrument.width
                label: ""
                active: true
                readonly property var r: editor.current
                value: editor.isColour || editor.get(r) === undefined ? 0 : (editor.get(r) - r.min) / (r.max - r.min)
                text: editor.isColour || editor.get(r) === undefined ? "" : (r.step >= 1 ? Math.round(editor.get(r)) : Number(editor.get(r)).toFixed(2)) + (r.unit || "")
                onMoved: f => editor.setNumber(r.min + f * (r.max - r.min))
            }
            Row {
                visible: editor.isColour
                spacing: 3 * editor.mm
                Text {
                    text: "HEX"
                    font.family: Fonts.mono; font.pixelSize: editor.font; color: editor.inkA(0.7)
                    anchors.verticalCenter: parent.verticalCenter
                }
                Rectangle {
                    width: 30 * editor.mm; height: 6 * editor.mm
                    color: "transparent"
                    border.width: hex.activeFocus ? Math.max(2, 0.5 * editor.mm) : Math.max(1, 0.15 * editor.mm)
                    border.color: editor.inkA(hex.activeFocus ? 1 : 0.6)
                    TextInput {
                        id: hex
                        anchors { fill: parent; leftMargin: 2 * editor.mm }
                        verticalAlignment: TextInput.AlignVCenter
                        maximumLength: 7
                        font.family: Fonts.mono; font.pixelSize: editor.font
                        color: editor.ink
                        selectionColor: editor.inkA(0.35)
                        Keys.onReturnPressed: accept()
                        Keys.onEnterPressed: accept()
                        Keys.onEscapePressed: { text = editor.get(editor.current); keys.forceActiveFocus() }
                        function accept() {
                            const t = text.trim().replace(/^([0-9A-Fa-f]{6})$/, "#$1")
                            if (/^#[0-9A-Fa-f]{6}$/.test(t)) {
                                editor.set(editor.current, t.toUpperCase())
                                editor.pickRow(editor.row)
                            }
                            focusBack.restart()
                        }
                        Timer { id: focusBack; interval: 0; onTriggered: keys.forceActiveFocus() }
                    }
                }
            }
        }
    }

    // a ruled scale: a band of what it sets (for a colour), a rule ticked every twentieth and every
    // quarter longer, the value's mark below it and the value written at the end; dragged, it moves
    component Ruler: Item {
        id: ruler
        property var ed                  // the editor (an inline component does not see its ids)
        property string label
        property real value: 0
        property string text
        property bool active: false
        property var colourAt: null
        signal moved(real f)
        height: 9 * ruler.ed.mm
        readonly property real x0: 6 * ruler.ed.mm
        readonly property real len: width - x0 - 16 * ruler.ed.mm
        Text {
            anchors.verticalCenter: band.verticalCenter
            text: ruler.label
            font.family: Fonts.mono; font.pixelSize: ruler.ed.font; font.bold: ruler.active
            color: ruler.active ? ruler.ed.ink : ruler.ed.inkA(0.6)
        }
        Canvas {
            id: band
            x: ruler.x0; y: 0.5 * ruler.ed.mm
            width: ruler.len; height: ruler.colourAt ? 2.6 * ruler.ed.mm : 0
            visible: ruler.colourAt !== null
            onPaint: {
                const c = getContext("2d")
                c.clearRect(0, 0, width, height)
                if (!ruler.colourAt) return
                const n = 48
                for (let i = 0; i < n; i++) {
                    c.fillStyle = ruler.colourAt((i + 0.5) / n)
                    c.fillRect(Math.floor(i * width / n), 0, Math.ceil(width / n) + 1, height)
                }
                c.strokeStyle = ruler.ed.inkA(0.8); c.lineWidth = 1; c.strokeRect(0.5, 0.5, width - 1, height - 1)
            }
            Connections { target: editor; function onHslChanged() { band.requestPaint() } function onRowChanged() { band.requestPaint() } }
            Component.onCompleted: requestPaint()
        }
        Canvas {
            id: rule
            x: ruler.x0; y: band.y + band.height + 0.6 * ruler.ed.mm
            width: ruler.len; height: 3 * ruler.ed.mm
            onPaint: {
                const c = getContext("2d")
                c.clearRect(0, 0, width, height)
                c.strokeStyle = ruler.ed.inkA(ruler.active ? 0.9 : 0.55); c.lineWidth = 1
                c.beginPath(); c.moveTo(0, 0.5); c.lineTo(width, 0.5)
                for (let i = 0; i <= 20; i++) {
                    const x = Math.round(i * (width - 1) / 20) + 0.5
                    c.moveTo(x, 0); c.lineTo(x, i % 5 === 0 ? height * 0.7 : height * 0.35)
                }
                c.stroke()
            }
            Connections { target: editor; function onInkChanged() { rule.requestPaint() } }
            onVisibleChanged: requestPaint()
        }
        Text {               // the value's mark
            x: ruler.x0 + ruler.value * ruler.len - width / 2
            y: rule.y + rule.height * 0.15
            text: "▲"
            font.pixelSize: 2.6 * ruler.ed.mm
            color: ruler.active ? ruler.ed.ink : ruler.ed.inkA(0.6)
        }
        Text {
            anchors { right: parent.right; verticalCenter: band.visible ? band.verticalCenter : rule.verticalCenter }
            text: ruler.text
            font.family: Fonts.mono; font.pixelSize: ruler.ed.font; font.bold: ruler.active
            color: ruler.ed.ink
        }
        MouseArea {
            x: ruler.x0 - 2 * ruler.ed.mm; width: ruler.len + 4 * ruler.ed.mm; height: parent.height
            onPressed: m => ruler.moved(Math.max(0, Math.min(1, (m.x - 2 * ruler.ed.mm) / ruler.len)))
            onPositionChanged: m => { if (pressed) ruler.moved(Math.max(0, Math.min(1, (m.x - 2 * ruler.ed.mm) / ruler.len))) }
        }
    }
}
