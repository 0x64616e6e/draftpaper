// Draftpaper's settings (Mod+t, a click on the theme cell, `draftpaper settings`), on the board
// (Board.qml: the inside of the sheet's frame cleared to the paper, or the wallpaper blurred). The
// pages run down the left like the title block's cells (Tab, or a click, moves between them): the
// colour scheme and the wallpaper (more to come, #36: displays, text).
//
// The wallpaper: the pictures as drawings (~/Pictures/Wallpapers, the system's backgrounds, and the
// current one; none first: the paper), the margin cells in the theme's paper or in the picture's
// colours (ambient), and how much paper tints the picture under the title block. Each change shows on
// the sheet at once (`draftpaper wallpaper set`); Escape puts back what there was.
//
// The colour scheme: the themes as drawings along the top (Desktop.qml), each lettered with its main
// contrasts, or with how many of its words fall below 4.5:1; and below them the builder, six rows of
// discrete choices (paper, tone, accent, windows and their shade, the terminal's background and text,
// contrast) from which `draftpaper scheme`
// derives a whole theme whose every word is readable (cli/src/scheme.rs). Building shapes the last drawing,
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
    property int row: 0                 // 0: the drawings; 1 and on: the builder's rows
    property int card: 0                // the drawing picked
    property string original: ""        // the theme when it opened: Escape goes back to it
    property string shown: ""           // the theme the sheet previews
    property string wallpaper: ""
    property bool reverting: false
    property string page: "scheme"      // "scheme" or "wallpaper"
    property bool schemeTouched: false  // a drawing or a choice was picked: Enter switches the theme
    // the wallpaper page: what `draftpaper wallpaper json` said, the values as they were (Escape), and as
    // picked
    property var wp: null
    property var wpWas: null
    property string wpPicture: ""       // a path, or "" for none
    property string wpMargins: "paper"
    property real wpTint: 0.7
    property int wpRow: 0               // 0: the pictures; 1: the margin cells; 2: the tint
    readonly property var tints: [0.4, 0.55, 0.7, 0.85]
    readonly property var wpPictures: wp ? [""].concat(wp.pictures) : [""]
    readonly property bool wpTouched: wpWas !== null && (wpPicture !== wpWas.picture || wpMargins !== wpWas.margins || wpTint !== wpWas.tint)
    property bool wpApplied: false      // the sheet shows a picked wallpaper (Escape puts the old back)
    readonly property var rows: ["paper", "tone", "accent", "windows", "shade", "terminal", "text", "contrast"]
    readonly property var rowNames: ({ paper: "PAPER", tone: "TONE", accent: "ACCENT", windows: "WINDOWS", shade: "WINDOWS SHADE",
                                       terminal: "TERMINAL", text: "TERMINAL TEXT", contrast: "CONTRAST" })
    // the drawings: the themes, then the one the choices build
    readonly property var cards: custom ? themes.concat([custom]) : themes
    readonly property var picked: cards[card] || null
    readonly property bool isCustom: picked !== null && picked.name === "custom"

    readonly property string hint: !visible ? ""
        : reverting ? "r again: " + (picked ? picked.label : "") + " as shipped"
        : page === "wallpaper" ? "↑↓ pictures, margins, tint · ←→ pick · tab colour scheme · enter keep · esc back"
        : "↑↓ drawings and choices · ←→ pick · tab wallpaper · enter keep · esc back to " + (themes.find(t => t.name === original) || { label: original }).label
          + (picked && picked.own && picked.shipped ? " · r revert" : "")

    // ---- opening, picking, keeping ----
    function open() {
        screen = focusedScreen()
        row = 0
        wpRow = 0
        page = "scheme"
        schemeTouched = false
        wp = null
        wpWas = null
        wpApplied = false
        loadWallpaper.running = true
        reverting = false
        themes = []
        custom = null
        loaded = false
        loadThemes.running = true
        wall.running = true
        visible = true
    }
    function args() { return rows.map(k => k + "=" + choices[k]) }
    function pickCard(i) {
        card = Math.max(0, Math.min(cards.length - 1, i))
        schemeTouched = true
        settle.restart()
    }
    function choose(k, v) {
        const c = Object.assign({}, choices)
        c[k] = v
        choices = c                                    // at once: the outline moves with the click
        card = cards.length - 1                        // Custom
        schemeTouched = true
        requestScheme()
        settle.restart()
    }
    function step(dir) {
        if (page === "wallpaper") return wpStep(dir)
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
        id: settle; interval: 200
        onTriggered: {
            if (!settings.picked) return
            if (settings.isCustom) { settings.queue(["scheme", "preview"].concat(settings.args()), "theme"); settings.shown = "custom.draft" }
            else if (settings.picked.name !== settings.shown) { settings.queue(["theme", settings.picked.name, "--preview"], "theme"); settings.shown = settings.picked.name }
        }
    }
    // Enter: the theme switched whole if a drawing or a choice was picked; the wallpaper is set already
    function keep() {
        settle.stop()
        wpSettle.stop()
        if (wpTouched) wpApply()
        if (schemeTouched && picked) {
            if (isCustom) queue(["scheme", "keep"].concat(args()), "")
            else queue(["theme", picked.name], "")
        }
        visible = false
    }
    // Escape: the theme and the wallpaper as they were
    function cancel() {
        settle.stop()
        wpSettle.stop()
        if (shown !== original) queue(["theme", original, "--preview"], "")
        if (wpWas && wpApplied) queue(["wallpaper", "set", wpWas.restore || "none", wpWas.margins, String(wpWas.tint)], "")
        visible = false
    }

    // ---- the wallpaper page ----
    function wpPick(path) { wpPicture = path; wpSettle.restart() }
    function wpStep(dir) {
        if (wpRow === 0) {
            const i = wpPictures.indexOf(wpPicture)
            wpPick(wpPictures[Math.max(0, Math.min(wpPictures.length - 1, i + dir))])
        } else if (wpRow === 1) {
            wpMargins = dir > 0 ? "ambient" : "paper"
            wpSettle.restart()
        } else {
            const i = tints.indexOf(wpTint)
            wpTint = tints[Math.max(0, Math.min(tints.length - 1, (i < 0 ? 2 : i) + dir))]
            wpSettle.restart()
        }
    }
    function wpApply() {
        queue(["wallpaper", "set", wpPicture || "none", wpMargins, String(wpTint)], "wallpaper")
        wpApplied = true
    }
    // the sheet shows the change a moment after the picking pauses
    Timer { id: wpSettle; interval: 200; onTriggered: settings.wpApply() }
    Process {
        id: loadWallpaper
        command: [settings.shell.bin, "wallpaper", "json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const d = JSON.parse(this.text)
                    settings.wp = d
                    // picture: what is shown (none when the file has gone); restore: what config.toml said
                    settings.wpWas = { picture: d.exists ? d.current : "", restore: d.current, margins: d.margins, tint: d.tint }
                    settings.wpPicture = settings.wpWas.picture
                    settings.wpMargins = d.margins
                    settings.wpTint = d.tint
                } catch (e) {}
            }
        }
    }
    function revert() {
        if (!picked || !picked.own || !picked.shipped) return
        queue(["theme", "revert", picked.name], "")
        reverting = false
        loadThemes.running = true
    }
    Timer { id: revertWindow; interval: 2000; onTriggered: settings.reverting = false }

    // `draftpaper ...` one at a time, in order; a newer preview of a kind ("theme", "wallpaper")
    // replaces one of that kind still waiting
    property var waiting: []
    function queue(a, kind) {
        const w = waiting.filter(x => !(kind && x.kind === kind))
        w.push({ args: a, kind: kind })
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
                if (!settings.custom) settings.requestScheme()
            }
        }
    }
    // the builder's options and the scheme they build, from one `draftpaper scheme options`. A click
    // while one runs is not lost: the request runs again with the latest choices when it ends, and an
    // answer never undoes a choice made since (only the first, on opening, sets them)
    property int wanted: 0
    property int asked: 0
    property bool loaded: false
    function requestScheme() {
        wanted++
        if (!loadOptions.running) startScheme()
    }
    function startScheme() {
        asked = wanted
        loadOptions.command = [shell.bin, "scheme", "options"].concat(loaded ? args() : [])
        loadOptions.running = true
    }
    Process {
        id: loadOptions
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const d = JSON.parse(this.text)
                    settings.options = d.options
                    if (!settings.loaded) { settings.choices = d.choices; settings.loaded = true }
                    if (settings.asked === settings.wanted) {
                        const t = d.theme
                        t.label = "Custom · " + t.label
                        const wasCustom = settings.isCustom
                        settings.custom = t
                        if (wasCustom) settings.card = settings.cards.length - 1
                        // the current theme is the custom one: pick it
                        else if (settings.original === "custom" && settings.row === 0) settings.card = settings.cards.length - 1
                    }
                } catch (e) {}
                if (settings.asked < settings.wanted) settings.startScheme()
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
    // and the picture picked on the wallpaper page, once it has loaded
    Board {
        id: board; out: settings.out; t: settings.picked; keepBlock: false
        wallpaper: settings.wp ? (settings.wpPicture ? "file://" + settings.wpPicture : "") : settings.wallpaper
    }

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
    // the builder's rows: 12 mm each, less when they would run past the frame (the laptop's panel)
    readonly property real builderY: board.inY + gutter + 6 * mm + cardH + 16 * mm
    readonly property real rowH: Math.max(8 * mm, Math.min(12 * mm, (board.inY + board.inH - gutter - builderY - 6 * mm) / rows.length))
    // the cells as wide as the longest row allows
    readonly property int most: Math.max(8, ...rows.map(k => (options[k] || []).length))
    readonly property real cellW: Math.min(24 * mm, (cw - 34 * mm) / most)

    // the mouse acts on the press, not the click: a compositor that hands the keyboard to the window
    // under the settings as the button goes down (Draftpaper's did, until it kept clicks on layer
    // surfaces to themselves) makes Qt cancel the press, and every second click was lost
    Item {
        id: keys
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const k = e.key
            if (k !== Qt.Key_R) settings.reverting = false
            if (k === Qt.Key_Escape) settings.cancel()
            else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) settings.keep()
            else if (k === Qt.Key_Tab || k === Qt.Key_Backtab) settings.page = settings.page === "scheme" ? "wallpaper" : "scheme"
            else if (settings.page === "wallpaper" && (k === Qt.Key_Up || k === Qt.Key_K)) settings.wpRow = Math.max(0, settings.wpRow - 1)
            else if (settings.page === "wallpaper" && (k === Qt.Key_Down || k === Qt.Key_J)) settings.wpRow = Math.min(2, settings.wpRow + 1)
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
            onPressed: settings.cancel()
            onWheel: w => settings.step(w.angleDelta.y < 0 || w.angleDelta.x < 0 ? 1 : -1)
        }

        // the pages, ruled like the title block's cells: the one shown in heavier ink
        Repeater {
            model: [["scheme", "COLOUR SCHEME"], ["wallpaper", "WALLPAPER"]]
            Rectangle {
                id: pg
                required property var modelData
                required property int index
                readonly property bool here: settings.page === modelData[0]
                x: settings.catX; y: board.inY + settings.gutter + index * 16 * settings.mm
                width: settings.catW; height: 14 * settings.mm
                color: "transparent"
                border.width: here ? Math.max(2, 0.6 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                border.color: here ? settings.ink : settings.inkA(0.5)
                Text {
                    x: 2 * settings.mm; y: 1.4 * settings.mm
                    text: "SETTINGS · " + (pg.index + 1)
                    font.family: Fonts.mono; font.pixelSize: settings.font * 0.75; font.letterSpacing: 0.3 * settings.mm
                    color: settings.inkA(0.6)
                }
                Text {
                    x: 2 * settings.mm; anchors.bottom: parent.bottom; anchors.bottomMargin: 2 * settings.mm
                    text: pg.modelData[1]
                    font.family: Fonts.mono; font.pixelSize: settings.font * 1.1; font.bold: pg.here
                    color: pg.here ? settings.ink : settings.inkA(0.7)
                }
                MouseArea { anchors.fill: parent; onPressed: settings.page = pg.modelData[0] }
            }
        }

        // ---- the wallpaper page ----
        Item {
            id: wallPage
            anchors.fill: parent
            visible: settings.page === "wallpaper"
            readonly property real topY: board.inY + settings.gutter
            // the pictures, a grid of drawings as wide as the page allows
            readonly property real gap: 4 * settings.mm
            readonly property int cols: Math.max(1, Math.floor((settings.cw + gap) / (36 * settings.mm + gap)))
            readonly property real picW: (settings.cw - (cols - 1) * gap) / cols
            readonly property real picH: picW * settings.height / Math.max(1, settings.width)
            readonly property int rowsShown: Math.max(1, Math.floor((board.inY + board.inH - settings.gutter - topY - 6 * settings.mm - 2 * 14 * settings.mm - 10 * settings.mm) / (picH + 7 * settings.mm)))
            readonly property int shown: Math.min(settings.wpPictures.length, cols * rowsShown)
            Text {
                x: settings.x0; y: wallPage.topY
                text: "PICTURES · " + (settings.wp ? settings.wp.folder : "") + ", and the system's backgrounds"
                    + (settings.wp && settings.wp.current && !settings.wp.exists ? " · yours is gone: " + settings.wp.current : "")
                font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.2 * settings.mm; font.bold: settings.wpRow === 0
                color: settings.wpRow === 0 ? settings.ink : settings.inkA(0.6)
            }
            Repeater {
                model: wallPage.shown
                Item {
                    id: pic
                    required property int index
                    readonly property string path: settings.wpPictures[index]
                    readonly property bool chosen: settings.wpPicture === path
                    x: settings.x0 + (index % wallPage.cols) * (wallPage.picW + wallPage.gap)
                    y: wallPage.topY + 6 * settings.mm + Math.floor(index / wallPage.cols) * (wallPage.picH + 7 * settings.mm)
                    width: wallPage.picW; height: wallPage.picH
                    Rectangle {         // none: the paper
                        anchors.fill: parent
                        visible: pic.path === ""
                        color: board.t ? board.t.paper : "#C9BDAE"
                        Text {
                            anchors.centerIn: parent
                            text: "NO PICTURE · PAPER"
                            font.family: Fonts.mono; font.pixelSize: settings.font * 0.8; font.letterSpacing: 0.2 * settings.mm
                            color: settings.ink
                        }
                    }
                    Image {             // cropped to fill, as the compositor mounts it in the frame
                        anchors.fill: parent
                        visible: pic.path !== ""
                        source: pic.path ? "file://" + pic.path : ""
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(Math.round(wallPage.picW * 2), Math.round(wallPage.picH * 2))
                        asynchronous: true
                        smooth: true
                    }
                    Rectangle {
                        readonly property real w: pic.chosen ? Math.max(2, 0.7 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                        anchors.fill: parent; anchors.margins: -w
                        color: "transparent"; border.width: w
                        border.color: pic.chosen ? settings.ink : settings.inkA(0.5)
                    }
                    Text {
                        anchors { left: parent.left; top: parent.bottom; topMargin: 1.2 * settings.mm }
                        width: parent.width; elide: Text.ElideMiddle
                        text: pic.path ? pic.path.split("/").pop() : "none"
                        font.family: Fonts.mono; font.pixelSize: settings.font * 0.7; font.bold: pic.chosen
                        color: pic.chosen ? settings.ink : settings.inkA(0.7)
                    }
                    MouseArea { anchors.fill: parent; onPressed: { settings.wpRow = 0; settings.wpPick(pic.path) } }
                }
            }
            // the margin cells and the title block's tint, as the builder's rows
            Repeater {
                model: [
                    { name: "MARGIN CELLS", row: 1, options: [{ v: "paper", label: "paper · the theme's" }, { v: "ambient", label: "ambient · the picture's colours" }] },
                    { name: "TITLE BLOCK TINT", row: 2, options: settings.tints.map(t => ({ v: t, label: Math.round(t * 100) + "% paper" })) }
                ]
                Item {
                    id: wr
                    required property var modelData
                    required property int index
                    readonly property bool here: settings.wpRow === modelData.row
                    x: settings.x0
                    y: wallPage.topY + 6 * settings.mm + Math.ceil(wallPage.shown / wallPage.cols) * (wallPage.picH + 7 * settings.mm) + 2 * settings.mm + index * 14 * settings.mm
                    width: settings.cw; height: 14 * settings.mm
                    Rectangle { width: parent.width; height: Math.max(1, 0.15 * settings.mm); color: settings.inkA(0.35) }
                    Text {
                        y: 4 * settings.mm
                        text: wr.modelData.name
                        font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.bold: wr.here
                        color: wr.here ? settings.ink : settings.inkA(0.75)
                    }
                    Row {          // the options, each as wide as its words
                        x: 40 * settings.mm; y: 2.5 * settings.mm
                        spacing: 4 * settings.mm
                    Repeater {
                        model: wr.modelData.options
                        Item {
                            id: wo
                            required property var modelData
                            required property int index
                            readonly property bool chosen: wr.modelData.row === 1 ? settings.wpMargins === modelData.v : settings.wpTint === modelData.v
                            width: optLabel.implicitWidth + 6 * settings.mm; height: 9 * settings.mm
                            Text {
                                id: optLabel
                                anchors.centerIn: parent
                                text: wo.modelData.label
                                font.family: Fonts.mono; font.pixelSize: settings.font * 0.8; font.bold: wo.chosen
                                color: wo.chosen ? settings.ink : settings.inkA(0.7)
                            }
                            Rectangle {
                                visible: wo.chosen
                                anchors.fill: parent
                                color: "transparent"
                                border.width: wr.here ? Math.max(2, 0.6 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                                border.color: wr.here ? settings.ink : settings.inkA(0.6)
                            }
                            MouseArea {
                                anchors.fill: parent
                                onPressed: {
                                    settings.wpRow = wr.modelData.row
                                    if (wr.modelData.row === 1) settings.wpMargins = wo.modelData.v
                                    else settings.wpTint = wo.modelData.v
                                    wpSettle.restart()
                                }
                            }
                        }
                    }
                    }
                }
            }
        }

        // ---- the colour scheme page ----
        Item {
            anchors.fill: parent
            visible: settings.page === "scheme"

        // the drawings
        Text {
            x: settings.x0; y: board.inY + settings.gutter
            text: "THEMES"
            font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.3 * settings.mm; font.bold: settings.row === 0
            color: settings.row === 0 ? settings.ink : settings.inkA(0.6)
        }
        Repeater {
            model: settings.cards.length           // a count, so delegates stay and only a changed drawing repaints
            Item {
                id: c
                required property int index
                readonly property var modelData: settings.cards[index]
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
                    onPressed: { settings.row = 0; c.picked ? settings.keep() : settings.pickCard(c.index) }
                }
            }
        }

        // the builder
        Item {
            id: builder
            x: settings.x0
            y: settings.builderY
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
                                text: o.modelData.label || o.modelData.value
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
                                onPressed: { settings.row = r.index + 1; settings.choose(r.modelData, o.modelData.value) }
                            }
                        }
                    }
                }
            }
        }
        }
    }
}
