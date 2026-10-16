// The launcher (Mod+d, `draftpaper launcher`), drawn on the sheet: a register of the applications in the
// frame's top-left corner, whole zones wide and down, ruled like the title block (number, glyph, name,
// what it is), and what you type lettered in the top margin above it. The matches are fuzzy (the name
// first, then the generic name, the keywords and the id); with nothing typed, the most used and the
// most recent come first (`$DRAFTPAPER_CACHE/launcher.json` counts the launches). Enter (or a click)
// launches through the compositor (`spawn`), so the application gets the session's environment and opens
// on the active workspace; Escape or a click outside closes it. Up and down (Ctrl+k/j, Ctrl+p/n, Tab)
// move along the register, Page up and down a page, Ctrl+u clears.
//
// The menu (`draftpaper menu`, for scripts): the same register over a list of lines instead of the
// applications; the choice (or nothing, on Escape) goes back to the command through a FIFO it made.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: launcher
    required property var shell
    visible: false
    screen: Quickshell.screens[0]
    function focusedScreen() { return Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0] }
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-launcher"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"

    property string query: ""
    property int sel: 0
    property int top: 0                 // the first match shown
    // the menu: its lines, prompt and the FIFO the choice goes to; null: the applications
    property var menu: null
    property var history: ({})

    // ---- the sheet's geometry (logical pixels, from the output's top-left corner) ----
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: out ? out.px_per_mm : 4.2
    readonly property real dpr: out ? out.scale : 1
    readonly property real margin: out ? out.margin : Math.round(6 * mm)
    readonly property real zoneW: out ? out.zones.width : 50 * mm
    readonly property real zoneH: out ? out.zones.height : 50 * mm
    readonly property real frameX: out ? out.frame.x - out.x : margin
    readonly property real frameY: out ? out.frame.y - out.y : margin
    // whole zones: about 160 mm across, 200 mm down, inside the frame
    readonly property int spanX: out ? Math.max(2, Math.min(out.zones.across - 1, Math.round(160 * mm / zoneW))) : 3
    readonly property int spanY: out ? Math.max(2, Math.min(out.zones.down, Math.round(200 * mm / zoneH))) : 4
    function px(v) { return Math.round(v * dpr) / dpr }     // on a physical pixel
    readonly property real line: 2 / dpr                    // the frame's line, as the sheet draws it
    readonly property real hair: 1 / dpr
    // inside the frame's line, which is the register's top and left rule
    readonly property real regX: px(frameX) + line
    readonly property real regY: px(frameY) + line
    // its right and bottom rules start on the ticks, as the title block's do
    readonly property real regW: px(frameX + spanX * zoneW) + line - regX
    readonly property real regH: px(frameY + spanY * zoneH) + line - regY
    readonly property real headH: px(10 * mm)
    readonly property int rows: Math.max(3, Math.floor((regH - headH - line) / (8 * mm)))
    readonly property real rowH: (regH - headH - line) / rows
    readonly property real font: 2.8 * mm
    readonly property color ink: shell.theme.ink
    readonly property color paper: shell.theme.data.paper || "#ffffff"
    function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }
    // the columns: number, glyph (applications only), name, what it is
    readonly property real colNo: px(12 * mm)
    readonly property real colGlyph: menu || !shell.glyphs ? 0 : px(9 * mm)
    readonly property real colName: menu ? regW - colNo : px((regW - colNo - colGlyph) * 0.48)

    // ---- the entries and the matching ----
    readonly property var apps: DesktopEntries.applications.values.filter(e => !e.noDisplay)
    readonly property var entries: menu
        ? menu.lines.map((l, i) => ({ key: l, name: l, about: "", glyph: "", index: i }))
        : apps.map(e => ({ key: e.id, name: e.name, about: e.genericName || e.comment || "", glyph: shell.appGlyph(e.startupClass || e.id),
                           words: [e.genericName || "", (e.keywords || []).join(" "), e.id], entry: e }))
    // how well `q` (lower case) matches `s`: a prefix best, then a word's start, then anywhere, then its
    // letters in order (each run and word start counting); -1 not at all
    function score(q, s) {
        s = (s || "").toLowerCase()
        if (!q) return 0
        if (s.startsWith(q)) return 1000 - s.length
        const at = s.indexOf(q)
        if (at > 0) return (/[\s\-_.]/.test(s[at - 1]) ? 800 : 600) - at
        let i = 0, sc = 0, run = 0
        for (let j = 0; j < s.length && i < q.length; j++) {
            if (s[j] === q[i]) {
                run++
                sc += 10 + 5 * run + (j === 0 || /[\s\-_.]/.test(s[j - 1]) ? 20 : 0)
                i++
            } else run = 0
        }
        return i === q.length ? sc : -1
    }
    // launches counted, worth less as they age (a week halves them)
    function used(key) {
        const h = history[key]
        if (!h) return 0
        const days = (Date.now() - h.last) / 86400000
        return h.count / (1 + days / 7)
    }
    readonly property var matches: {
        const q = query.trim().toLowerCase()
        const scored = []
        for (const e of entries) {
            let s = score(q, e.name)
            if (q && !menu) for (const [w, k] of [[e.words[0], 0.8], [e.words[1], 0.7], [e.words[2], 0.6]]) {
                const t = score(q, w)
                if (t >= 0) s = Math.max(s, t * k)
            }
            if (s < 0) continue
            scored.push({ e: e, s: menu && !q ? -e.index : s + (menu ? 0 : Math.min(200, 40 * Math.log(1 + used(e.key)))) })
        }
        scored.sort((a, b) => b.s - a.s || a.e.name.localeCompare(b.e.name))
        return scored.map(x => x.e)
    }
    onMatchesChanged: { sel = 0; top = 0 }
    function move(n) {
        if (matches.length === 0) return
        sel = Math.max(0, Math.min(matches.length - 1, sel + n))
        if (sel < top) top = sel
        if (sel >= top + rows) top = sel - rows + 1
    }

    // ---- opening, launching, closing ----
    function open() {
        if (menu) answer("")
        menu = null
        query = ""
        screen = focusedScreen()
        historyFile.reload()
        visible = true
    }
    // a menu over the lines of the file `file` (read at once: the command waits on the FIFO meanwhile)
    function openMenu(prompt, file, fifo) {
        if (visible) close()
        menuFile.path = file
        menuFile.reload()
        const lines = (menuFile.text() || "").split("\n").filter(l => l.trim() !== "")
        menu = { prompt: prompt, lines: lines, fifo: fifo }
        query = ""
        screen = focusedScreen()
        visible = true
    }
    function close() {
        if (menu) answer("")
        visible = false
        menu = null
    }
    // a word for sh -c: in single quotes
    function quote(s) { return "'" + String(s).replace(/'/g, "'\\''") + "'" }
    function launch(i) {
        const m = matches[i === undefined ? sel : i]
        if (!m) return
        if (menu) {
            answer(m.name)
            visible = false
            menu = null
            return
        }
        const e = m.entry
        // the command as the desktop entry has it, without field codes (%f, %U...)
        let cmd = (e.command || []).filter(a => !/^%[a-zA-Z]$/.test(a))
        if (cmd.length === 0) return
        if (e.runInTerminal) cmd = ["x-terminal-emulator", "-e"].concat(cmd)
        let line = cmd.map(quote).join(" ")
        if (e.workingDirectory) line = "cd " + quote(e.workingDirectory) + " && exec " + line
        Compositor.action("spawn " + line)
        const h = Object.assign({}, history)
        h[e.id] = { count: (h[e.id] ? h[e.id].count : 0) + 1, last: Date.now() }
        history = h
        historyFile.setText(JSON.stringify(h))
        visible = false
    }
    // the menu's choice to the waiting command (a line; nothing when it was closed)
    function answer(text) {
        if (!menu || !menu.fifo) return
        Quickshell.execDetached(["sh", "-c", 'printf "%s\\n" "$1" > "$2"', "sh", text, menu.fifo])
        menu.fifo = ""
    }

    FileView {
        id: historyFile
        path: Quickshell.env("DRAFTPAPER_CACHE") + "/launcher.json"
        printErrors: false
        onTextChanged: { try { launcher.history = JSON.parse(text()) || {} } catch (e) { launcher.history = {} } }
    }

    FileView { id: menuFile; blockLoading: true; printErrors: false }

    onVisibleChanged: if (visible) keys.forceActiveFocus()

    Item {
        id: keys
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const k = e.key, ctrl = e.modifiers & Qt.ControlModifier
            if (k === Qt.Key_Escape) launcher.close()
            else if (k === Qt.Key_Return || k === Qt.Key_Enter) launcher.launch()
            else if (k === Qt.Key_Up || k === Qt.Key_Backtab || (ctrl && (k === Qt.Key_K || k === Qt.Key_P))) launcher.move(-1)
            else if (k === Qt.Key_Down || k === Qt.Key_Tab || (ctrl && (k === Qt.Key_J || k === Qt.Key_N))) launcher.move(1)
            else if (k === Qt.Key_PageUp) launcher.move(-launcher.rows)
            else if (k === Qt.Key_PageDown) launcher.move(launcher.rows)
            else if (ctrl && k === Qt.Key_U) launcher.query = ""
            else if (ctrl && k === Qt.Key_W) launcher.query = launcher.query.replace(/\S*\s*$/, "")
            else if (k === Qt.Key_Backspace) launcher.query = launcher.query.slice(0, -1)
            else if (!ctrl && e.text && e.text >= " ") launcher.query += e.text
            else return
            e.accepted = true
        }
        // a press outside the register closes it (on the press: see Settings.qml)
        MouseArea {
            anchors.fill: parent
            onPressed: launcher.close()
        }

        // what you type, in the top margin over the register's zones
        Rectangle {
            x: launcher.regX; y: 0
            width: launcher.regW; height: launcher.px(launcher.margin)
            color: launcher.paper
            Text {
                anchors.verticalCenter: parent.verticalCenter
                x: 2 * launcher.mm; width: parent.width - 4 * launcher.mm
                elide: Text.ElideLeft
                textFormat: Text.PlainText
                text: (launcher.menu ? (launcher.menu.prompt || "CHOOSE") : "RUN").toUpperCase() + " ▸ " + launcher.query + "▏"
                font.family: Fonts.mono; font.pixelSize: Math.min(launcher.margin * 0.5, 3.2 * launcher.mm)
                color: launcher.ink
            }
        }

        // the register
        Rectangle {
            id: reg
            x: launcher.regX; y: launcher.regY
            width: launcher.regW; height: launcher.regH
            color: launcher.paper
            MouseArea {             // a press on the register is not a press outside it
                anchors.fill: parent
                onWheel: w => launcher.move(w.angleDelta.y < 0 ? 3 : -3)
            }
            // the outer rule, as heavy as the frame's line, on the right and the bottom (the frame's own
            // line is its top and left)
            Rectangle { x: reg.width - launcher.line; width: launcher.line; height: reg.height; color: launcher.ink }
            Rectangle { y: reg.height - launcher.line; width: reg.width; height: launcher.line; color: launcher.ink }
            Rectangle { y: launcher.headH; width: reg.width; height: launcher.hair; color: launcher.ink }

            // the heading, like a title block cell: the register's name and how many match, then the
            // columns' names
            Text {
                x: 2 * launcher.mm; y: 1.2 * launcher.mm
                text: (launcher.menu ? "MENU" : "REGISTER OF APPLICATIONS") + " · " + launcher.matches.length
                      + (launcher.query ? " OF " + launcher.entries.length : "")
                      + (launcher.matches.length > launcher.rows ? " · " + (launcher.top + 1) + "–" + Math.min(launcher.matches.length, launcher.top + launcher.rows) : "")
                font.family: Fonts.mono; font.pixelSize: launcher.font * 0.75; font.letterSpacing: 0.3 * launcher.mm
                color: launcher.inkA(0.6)
            }
            Repeater {
                model: launcher.menu ? [["NO.", 0], ["ENTRY", launcher.colNo]]
                    : [["NO.", 0], ["APPLICATION", launcher.colNo + launcher.colGlyph], ["WHAT IT IS", launcher.colNo + launcher.colGlyph + launcher.colName]]
                Text {
                    required property var modelData
                    x: modelData[1] + 2 * launcher.mm; y: launcher.headH - height - 1.2 * launcher.mm
                    text: modelData[0]
                    font.family: Fonts.mono; font.pixelSize: launcher.font * 0.7; font.letterSpacing: 0.3 * launcher.mm
                    color: launcher.inkA(0.6)
                }
            }

            // the rows: rules between them and the columns, the picked one in heavier ink
            Repeater {
                model: launcher.rows
                Item {
                    id: row
                    required property int index
                    readonly property int n: launcher.top + index
                    readonly property var m: launcher.matches[n] || null
                    readonly property bool here: m !== null && n === launcher.sel
                    x: 0; y: launcher.headH + launcher.hair + index * launcher.rowH
                    width: reg.width - launcher.line; height: launcher.rowH
                    Rectangle { visible: row.index > 0; width: parent.width; height: launcher.hair; color: launcher.inkA(0.35) }
                    Repeater {
                        model: launcher.menu ? [launcher.colNo] : launcher.colGlyph > 0 ? [launcher.colNo, launcher.colNo + launcher.colGlyph, launcher.colNo + launcher.colGlyph + launcher.colName] : [launcher.colNo, launcher.colNo + launcher.colName]
                        Rectangle { required property real modelData; x: launcher.px(modelData); width: launcher.hair; height: row.height; color: launcher.inkA(0.35) }
                    }
                    Rectangle {
                        visible: row.here
                        anchors.fill: parent
                        color: launcher.inkA(0.12)
                        border.width: Math.max(launcher.line, 0.5 * launcher.mm); border.color: launcher.ink
                    }
                    Text {
                        visible: row.m !== null
                        x: 2 * launcher.mm; anchors.verticalCenter: parent.verticalCenter
                        text: String(row.n + 1).padStart(2, "0")
                        font.family: Fonts.mono; font.pixelSize: launcher.font * 0.85
                        color: launcher.inkA(0.7)
                    }
                    Text {
                        visible: row.m !== null && launcher.colGlyph > 0
                        x: launcher.colNo; width: launcher.colGlyph; horizontalAlignment: Text.AlignHCenter
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.m ? row.m.glyph : ""
                        font.family: Fonts.glyph; font.pixelSize: launcher.font * 1.1
                        color: launcher.ink
                    }
                    Text {
                        x: launcher.colNo + launcher.colGlyph + 2 * launcher.mm; width: launcher.colName - 4 * launcher.mm
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Text.ElideRight; textFormat: Text.PlainText
                        text: row.m ? row.m.name : ""
                        font.family: Fonts.mono; font.pixelSize: launcher.font; font.bold: row.here
                        color: launcher.ink
                    }
                    Text {
                        visible: !launcher.menu
                        x: launcher.colNo + launcher.colGlyph + launcher.colName + 2 * launcher.mm
                        width: reg.width - launcher.line - x - 2 * launcher.mm
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Text.ElideRight; textFormat: Text.PlainText
                        text: row.m ? row.m.about : ""
                        font.family: Fonts.mono; font.pixelSize: launcher.font * 0.85
                        color: launcher.inkA(0.7)
                    }
                    MouseArea {
                        anchors.fill: parent
                        enabled: row.m !== null
                        onPressed: launcher.launch(row.n)
                    }
                }
            }
        }
    }
}
