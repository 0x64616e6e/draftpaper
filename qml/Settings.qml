// Draftpaper's settings (Mod+t, a click on the theme cell, `draftpaper settings`), on the board
// (Board.qml: the inside of the sheet's frame cleared to the paper, or the wallpaper blurred). The
// pages run down the left like the title block's cells (Tab, or a click, moves between them): the
// colour scheme and the wallpaper (more to come, #36: displays, text).
//
// The wallpaper: the pictures as drawings (config.toml's wallpaper_dirs: ~/Pictures/Wallpapers and the
// system's backgrounds; and the current one; none first: the paper), then its settings as rows: the
// margin cells in the theme's paper or in the picture's colours (ambient), how much paper tints the
// picture under the title block, how it is mounted (fill, fit, its own size) and where, its
// brightness, a wash of paper over it, and blur. Each change shows on the sheet at once
// (`draftpaper wallpaper set`); Escape puts back what there was.
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
    property var wpVals: ({})           // the settings below as picked: margins, tint, fit, position, ...
    property int wpRow: 0               // 0: the pictures; 1 and on: the rows below
    // the wallpaper's settings: (key, as `draftpaper wallpaper set` names it; the row's name; the values
    // offered, each [value, label])
    readonly property var wpRows: [
        { key: "margins", name: "MARGIN CELLS", options: [["paper", "paper · the theme's"], ["ambient", "ambient · the picture's colours"]] },
        { key: "tint", name: "TITLE BLOCK TINT", options: [[0.4, "40% paper"], [0.55, "55%"], [0.7, "70%"], [0.85, "85%"]] },
        { key: "fit", name: "MOUNTED", options: [["fill", "fill · cropped to the frame"], ["fit", "fit · whole, on the paper"], ["centre", "its own size"]] },
        { key: "position", name: "POSITION", options: [["centre", "centre"], ["top", "top"], ["bottom", "bottom"], ["left", "left"], ["right", "right"]] },
        { key: "brightness", name: "BRIGHTNESS", options: [[1.0, "100%"], [0.85, "85%"], [0.7, "70%"], [0.55, "55%"]] },
        { key: "wash", name: "PAPER WASH", options: [[0.0, "none"], [0.15, "15%"], [0.3, "30%"], [0.45, "45%"]] },
        { key: "blur", name: "BLUR", options: [[0.0, "none"], [1.0, "soft"], [3.0, "medium"], [8.0, "strong"]] }
    ]
    readonly property var wpPictures: wp ? [""].concat(wp.pictures) : [""]
    function wpArgs(picture, vals) { return ["wallpaper", "set", "picture=" + (picture || "none")].concat(wpRows.map(r => r.key + "=" + vals[r.key])) }
    readonly property bool wpTouched: wpWas !== null && (wpPicture !== wpWas.picture || wpRows.some(r => wpVals[r.key] !== wpWas.vals[r.key]))
    property bool wpApplied: false      // the sheet shows a picked wallpaper (Escape puts the old back)
    // the pages, in Tab's order: the two above, then the ones made of config.toml's rows (cfgPages)
    readonly property var pageList: [["scheme", "COLOUR SCHEME"], ["wallpaper", "WALLPAPER"], ["sheet", "SHEET"], ["rulers", "RULERS"],
                                     ["session", "SESSION"], ["input", "INPUT"], ["idle", "IDLE · LOCK"]]
    // each row: config.toml's key (dotted: table.key), its name, the values offered ([value, label]), the
    // value when config.toml has none, and a note; `draftpaper config set` writes it, comments kept
    readonly property var cfgPages: ({
        sheet: [
            { key: "margin", name: "MARGIN", def: 6, note: "between the screen's edge and the frame",
              options: [[4, "4 mm"], [5, "5 mm"], [6, "6 mm"], [8, "8 mm"], [10, "10 mm"]] },
            { key: "title.scale", name: "SCALE BAR", def: true, note: "true to size; NTS where a display reports no size",
              options: [[true, "on"], [false, "off"]] },
            { key: "title.revisions", name: "REVISION BLOCK", def: false, note: "the focused repository's last commits",
              options: [[false, "off"], [true, "on"]] },
            { key: "title.key_plan", name: "KEY PLAN", def: true, note: "with several screens: their plan, the north mark",
              options: [[true, "on"], [false, "off"]] },
            { key: "sheet.hatch_strength", name: "HATCHING", def: 0.35, note: "parked windows' cells",
              options: [[0, "none"], [0.2, "light"], [0.35, "standard"], [0.55, "strong"]] },
            { key: "sheet.hatch_spacing", name: "HATCH SPACING", def: 1.5, note: "",
              options: [[1, "1 mm"], [1.5, "1.5 mm"], [2, "2 mm"]] },
            { key: "glyphs", name: "GLYPHS", def: true, note: "Nerd Font glyphs before the readings",
              options: [[true, "on"], [false, "words only"]] },
            { key: "animations.enabled", name: "MOTION", def: true, note: "parking, workspaces, the settings",
              options: [[true, "on"], [false, "reduced"]] }
        ],
        rulers: [
            { key: "rulers.step", name: "MOVE STEP", def: 1, note: "the crossing, by key or drag; the pointer held to it",
              options: [[0.5, "0.5 mm"], [1, "1 mm"], [2, "2 mm"]] },
            { key: "rulers.step_shift", name: "WITH SHIFT", def: 10, note: "",
              options: [[5, "5 mm"], [10, "10 mm"], [20, "20 mm"]] },
            { key: "rulers.turn", name: "TURN STEP", def: 1, note: "",
              options: [[0.5, "0.5°"], [1, "1°"], [5, "5°"]] },
            { key: "rulers.turn_shift", name: "TURN WITH SHIFT", def: 15, note: "",
              options: [[10, "10°"], [15, "15°"], [30, "30°"], [45, "45°"]] },
            { key: "rulers.numbers_h", name: "HORIZONTAL NUMBERED", def: "top", note: "ticks on both edges either way",
              options: [["top", "top"], ["bottom", "bottom"]] },
            { key: "rulers.numbers_v", name: "VERTICAL NUMBERED", def: "right", note: "measuring in the top right quadrant",
              options: [["left", "left"], ["right", "right"]] },
            { key: "rulers.centre_line", name: "CENTRE LINE", def: true, note: "",
              options: [[true, "on"], [false, "off"]] },
            { key: "rulers.hold_pointer", name: "POINTER", def: true, note: "as the rulers come out; s toggles",
              options: [[true, "held to the steps"], [false, "free"]] },
            { key: "rulers.width", name: "WIDTH", def: 16, note: "",
              options: [[12, "12 mm"], [16, "16 mm"], [20, "20 mm"]] }
        ],
        session: [
            { key: "session.restore", name: "RESTORE", def: true, note: "the windows back in their places at login",
              options: [[true, "on"], [false, "off"]] },
            { key: "session.apps", name: "APPLICATIONS' STATE", def: true, note: "tabs, files, editors, tmux",
              options: [[true, "on"], [false, "off"]] }
        ],
        input: [
            { key: "input.repeat_delay", name: "REPEAT DELAY", def: 200, note: "before a held key repeats",
              options: [[150, "150 ms"], [200, "200 ms"], [300, "300 ms"], [400, "400 ms"]] },
            { key: "input.repeat_rate", name: "REPEAT RATE", def: 25, note: "",
              options: [[20, "20/s"], [25, "25/s"], [30, "30/s"], [40, "40/s"]] },
            { key: "input.tap", name: "TAP TO CLICK", def: true, note: "the touchpad",
              options: [[true, "on"], [false, "off"]] },
            { key: "input.natural_scroll", name: "NATURAL SCROLLING", def: false, note: "the content follows the fingers",
              options: [[false, "off"], [true, "on"]] },
            { key: "input.focus_follows_mouse", name: "FOCUS FOLLOWS THE MOUSE", def: true, note: "",
              options: [[true, "on"], [false, "off"]] }
        ],
        idle: [
            { key: "idle.screens_off", name: "SCREENS OFF AFTER", def: 15, note: "any input wakes them",
              options: [[5, "5 min"], [10, "10 min"], [15, "15 min"], [30, "30 min"], [0, "never"]] },
            { key: "idle.lock_after", name: "LOCK AFTER", def: 0, note: "Mod+Shift+x locks now",
              options: [[0, "never"], [5, "5 min"], [10, "10 min"], [15, "15 min"]] },
            { key: "idle.lock_on_sleep", name: "LOCK BEFORE SUSPEND", def: false, note: "Mod+Shift+p: for the day",
              options: [[false, "off"], [true, "on"]] }
        ]
    })
    readonly property var cfgRows: cfgPages[page] || []
    property var cfg: null              // `draftpaper config json`
    property var cfgVals: ({})          // the rows changed, key: value
    property bool cfgMarked: false      // config.toml kept as it was (`config mark`): Escape puts it back
    property int cfgRow: 0
    onPageChanged: cfgRow = 0
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
        : cfgRows.length ? "↑↓ rows · ←→ pick · tab next page · enter keep · esc back"
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
        cfg = null
        cfgVals = {}
        cfgMarked = false
        cfgRow = 0
        // a mark left by settings that did not close (the shell restarted) goes: config.toml stays as it is
        queue(["config", "keep"], "")
        loadConfig.running = true
        reverting = false
        themes = []
        custom = null
        loaded = false
        loadThemes.running = true
        wall.running = true
        visible = true
    }
    function turnPage(dir) {
        const i = pageList.findIndex(p => p[0] === page)
        page = pageList[(i + dir + pageList.length) % pageList.length][0]
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
        if (cfgRows.length) return cfgStep(dir)
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
        if (cfgSettle.running) { cfgSettle.stop(); cfgApply() }
        if (cfgMarked) queue(["config", "keep"], "")
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
        cfgSettle.stop()
        // config.toml as it was: the rows, and anything else changed meanwhile (the wallpaper restore below agrees)
        if (cfgMarked) queue(["config", "back"], "")
        if (shown !== original) queue(["theme", original, "--preview"], "")
        if (wpWas && wpApplied) queue(wpArgs(wpWas.restore, wpWas.vals), "")
        visible = false
    }

    // ---- the pages of config.toml's rows ----
    function cfgGet(row) {
        if (row.key in cfgVals) return cfgVals[row.key]
        let v = cfg
        for (const part of row.key.split(".")) v = (v && typeof v === "object") ? v[part] : undefined
        return v === undefined || v === null ? row.def : v
    }
    // the values offered, and the one set when it is none of them
    function cfgOptions(row) {
        const v = cfgGet(row)
        return row.options.some(o => o[0] === v) ? row.options : row.options.concat([[v, String(v)]])
    }
    function cfgSet(row, value) {
        if (!cfgMarked) { queue(["config", "mark"], ""); cfgMarked = true }
        const v = Object.assign({}, cfgVals)
        v[row.key] = value
        cfgVals = v
        cfgSettle.restart()
    }
    function cfgStep(dir) {
        const row = cfgRows[cfgRow]
        if (!row) return
        const opts = cfgOptions(row), vals = opts.map(o => o[0])
        const i = vals.indexOf(cfgGet(row))
        const n = Math.max(0, Math.min(vals.length - 1, (i < 0 ? 0 : i) + dir))
        if (n !== i) cfgSet(row, vals[n])
    }
    function cfgApply() {
        const keys = Object.keys(cfgVals)
        if (keys.length) queue(["config", "set"].concat(keys.map(k => k + "=" + String(cfgVals[k]))), "config")
    }
    // the sheet shows a change a moment after the picking pauses
    Timer { id: cfgSettle; interval: 200; onTriggered: settings.cfgApply() }
    Process {
        id: loadConfig
        command: [settings.shell.bin, "config", "json"]
        stdout: StdioCollector { onStreamFinished: { try { settings.cfg = JSON.parse(this.text) } catch (e) {} } }
    }
    // ---- the wallpaper page ----
    function wpPick(path) { wpPicture = path; wpSettle.restart() }
    function wpSet(key, value) {
        const v = Object.assign({}, wpVals)
        v[key] = value
        wpVals = v
        wpSettle.restart()
    }
    function wpStep(dir) {
        if (wpRow === 0) {
            const i = wpPictures.indexOf(wpPicture)
            wpPick(wpPictures[Math.max(0, Math.min(wpPictures.length - 1, i + dir))])
            return
        }
        const r = wpRows[wpRow - 1], vals = r.options.map(o => o[0])
        const i = vals.indexOf(wpVals[r.key])
        wpSet(r.key, vals[Math.max(0, Math.min(vals.length - 1, (i < 0 ? 0 : i) + dir))])
    }
    function wpApply() {
        queue(wpArgs(wpPicture, wpVals), "wallpaper")
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
                    const vals = {}
                    for (const r of settings.wpRows) vals[r.key] = d[r.key]
                    settings.wpWas = { picture: d.exists ? d.current : "", restore: d.current, vals: vals }
                    settings.wpPicture = settings.wpWas.picture
                    settings.wpVals = Object.assign({}, vals)
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
            else if (k === Qt.Key_Tab || k === Qt.Key_Backtab) settings.turnPage(k === Qt.Key_Tab && !(e.modifiers & Qt.ShiftModifier) ? 1 : -1)
            else if (settings.page === "wallpaper" && (k === Qt.Key_Up || k === Qt.Key_K)) settings.wpRow = Math.max(0, settings.wpRow - 1)
            else if (settings.page === "wallpaper" && (k === Qt.Key_Down || k === Qt.Key_J)) settings.wpRow = Math.min(settings.wpRows.length, settings.wpRow + 1)
            else if (settings.cfgRows.length && (k === Qt.Key_Up || k === Qt.Key_K)) settings.cfgRow = Math.max(0, settings.cfgRow - 1)
            else if (settings.cfgRows.length && (k === Qt.Key_Down || k === Qt.Key_J)) settings.cfgRow = Math.min(settings.cfgRows.length - 1, settings.cfgRow + 1)
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
            model: settings.pageList
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
            readonly property real rowH: 10 * settings.mm     // a settings row
            readonly property int cols: Math.max(1, Math.floor((settings.cw + gap) / (36 * settings.mm + gap)))
            readonly property real picW: (settings.cw - (cols - 1) * gap) / cols
            readonly property real picH: picW * settings.height / Math.max(1, settings.width)
            readonly property int rowsShown: Math.max(1, Math.floor((board.inY + board.inH - settings.gutter - topY - 6 * settings.mm - settings.wpRows.length * rowH - 4 * settings.mm) / (picH + 7 * settings.mm)))
            // every picture, in rows that scroll: as many shown as fit, the rest a wheel, a drag or the
            // arrows away (the picked one kept in view)
            readonly property real rowStep: picH + 7 * settings.mm
            readonly property int rowsTotal: Math.max(1, Math.ceil(settings.wpPictures.length / cols))
            readonly property int rowsView: Math.min(rowsTotal, rowsShown)
            // the first row in view: a row, not pixels, so the picture stays put while the page lays out
            property int topRow: 0
            readonly property int firstRow: Math.max(0, Math.min(rowsTotal - rowsView, topRow))
            function scrollTo(row) { topRow = Math.max(0, Math.min(rowsTotal - rowsView, row)) }
            // the picked picture's row brought into view (the view slides to it)
            function reveal() {
                const i = settings.wpPictures.indexOf(settings.wpPicture)
                if (i < 0) return
                const row = Math.floor(i / cols)
                if (row < firstRow) scrollTo(row)
                else if (row >= firstRow + rowsView) scrollTo(row - rowsView + 1)
            }
            Connections { target: settings; function onWpPictureChanged() { wallPage.reveal() } function onWpPicturesChanged() { wallPage.reveal() } }
            onRowsViewChanged: reveal()
            onColsChanged: reveal()
            Text {
                x: settings.x0; y: wallPage.topY
                text: "PICTURES · " + (settings.wp ? settings.wp.dirs.join(", ") : "") + " (config.toml: wallpaper_dirs)"
                    + (wallPage.rowsTotal > wallPage.rowsShown
                       ? " · " + (wallPage.firstRow * wallPage.cols + 1) + "–" + Math.min(settings.wpPictures.length, (wallPage.firstRow + wallPage.rowsShown) * wallPage.cols)
                         + " OF " + settings.wpPictures.length
                       : "")
                    + (settings.wp && settings.wp.current && !settings.wp.exists ? " · yours is gone: " + settings.wp.current : "")
                font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.2 * settings.mm; font.bold: settings.wpRow === 0
                color: settings.wpRow === 0 ? settings.ink : settings.inkA(0.6)
            }
            Flickable {
                id: picFlick
                // a millimetre around the pictures, for the picked one's heavier frame
                readonly property real pad: 1 * settings.mm
                x: settings.x0 - pad; y: wallPage.topY + 6 * settings.mm - pad
                width: settings.cw + 2 * pad
                // the rows in view and their names, not the frame of the next row peeking in
                height: wallPage.rowsView * wallPage.rowStep - pad
                contentWidth: width
                contentHeight: wallPage.rowsTotal * wallPage.rowStep + pad
                clip: true
                // scrolled a row at a time: the wheel (a touchpad's small steps added up to a notch), and the
                // arrows keeping the picked picture in view; sliding there
                interactive: false
                contentY: wallPage.firstRow * wallPage.rowStep
                Behavior on contentY { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
                WheelHandler {
                    property real acc: 0
                    onWheel: e => {
                        acc += e.angleDelta.y !== 0 ? e.angleDelta.y : e.angleDelta.x
                        const n = Math.trunc(acc / 120)
                        if (n !== 0) { acc -= n * 120; wallPage.scrollTo(wallPage.firstRow - n) }
                    }
                }
                Repeater {
                    model: settings.wpPictures.length
                    Item {
                        id: pic
                        required property int index
                        readonly property string path: settings.wpPictures[index]
                        readonly property bool chosen: settings.wpPicture === path
                        readonly property int row: Math.floor(index / wallPage.cols)
                        // loaded only in view (and a row either side): a folder of hundreds stays light
                        readonly property bool near: row >= wallPage.firstRow - 1 && row <= wallPage.firstRow + wallPage.rowsShown
                        x: picFlick.pad + (index % wallPage.cols) * (wallPage.picW + wallPage.gap)
                        y: picFlick.pad + row * wallPage.rowStep
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
                            source: pic.path && pic.near ? "file://" + pic.path : ""
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
            }
            // the wallpaper's settings, as the builder's rows
            Repeater {
                model: settings.wpRows
                Item {
                    id: wr
                    required property var modelData
                    required property int index
                    readonly property bool here: settings.wpRow === index + 1
                    x: settings.x0
                    y: wallPage.topY + 6 * settings.mm + wallPage.rowsView * wallPage.rowStep + 2 * settings.mm + index * wallPage.rowH
                    width: settings.cw; height: wallPage.rowH
                    Rectangle { width: parent.width; height: Math.max(1, 0.15 * settings.mm); color: settings.inkA(0.35) }
                    Text {
                        y: (wallPage.rowH - height) / 2
                        text: wr.modelData.name
                        font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.bold: wr.here
                        color: wr.here ? settings.ink : settings.inkA(0.75)
                    }
                    Row {          // the options, each as wide as its words
                        x: 40 * settings.mm; y: (wallPage.rowH - 7 * settings.mm) / 2
                        spacing: 3 * settings.mm
                    Repeater {
                        model: wr.modelData.options
                        Item {
                            id: wo
                            required property var modelData
                            required property int index
                            readonly property bool chosen: settings.wpVals[wr.modelData.key] === modelData[0]
                            width: optLabel.implicitWidth + 5 * settings.mm; height: 7 * settings.mm
                            Text {
                                id: optLabel
                                anchors.centerIn: parent
                                text: wo.modelData[1]
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
                                onPressed: { settings.wpRow = wr.index + 1; settings.wpSet(wr.modelData.key, wo.modelData[0]) }
                            }
                        }
                    }
                    }
                }
            }
        }

        // ---- the pages of config.toml's rows (Sheet, Session, Input, Idle) ----
        Item {
            id: cfgPage
            anchors.fill: parent
            visible: settings.cfgRows.length > 0
            readonly property real rowH: 12 * settings.mm
            readonly property real optX: 58 * settings.mm
            Repeater {
                model: settings.cfgRows
                Item {
                    id: cr
                    required property var modelData
                    required property int index
                    readonly property bool here: settings.cfgRow === index
                    readonly property var value: settings.cfgGet(modelData)
                    x: settings.x0
                    y: board.inY + settings.gutter + index * cfgPage.rowH
                    width: settings.cw; height: cfgPage.rowH
                    Rectangle { y: parent.height - height; width: parent.width; height: Math.max(1, 0.15 * settings.mm); color: settings.inkA(0.35) }
                    Column {
                        y: (cfgPage.rowH - height) / 2
                        spacing: 0.6 * settings.mm
                        Text {
                            text: cr.modelData.name
                            font.family: Fonts.mono; font.pixelSize: settings.font * 0.85; font.bold: cr.here
                            color: cr.here ? settings.ink : settings.inkA(0.75)
                        }
                        Text {     // where it goes in config.toml
                            readonly property int dot: cr.modelData.key.lastIndexOf(".")
                            text: dot < 0 ? cr.modelData.key : "[" + cr.modelData.key.slice(0, dot) + "] " + cr.modelData.key.slice(dot + 1)
                            font.family: Fonts.mono; font.pixelSize: settings.font * 0.62
                            color: settings.inkA(0.55)
                        }
                    }
                    Row {          // the values, each as wide as its words; the note after them
                        x: cfgPage.optX; y: (cfgPage.rowH - 7 * settings.mm) / 2
                        spacing: 3 * settings.mm
                        Repeater {
                            model: settings.cfgOptions(cr.modelData)
                            Item {
                                id: co
                                required property var modelData
                                readonly property bool chosen: cr.value === modelData[0]
                                width: coLabel.implicitWidth + 5 * settings.mm; height: 7 * settings.mm
                                Text {
                                    id: coLabel
                                    anchors.centerIn: parent
                                    text: co.modelData[1]
                                    font.family: Fonts.mono; font.pixelSize: settings.font * 0.8; font.bold: co.chosen
                                    color: co.chosen ? settings.ink : settings.inkA(0.7)
                                }
                                Rectangle {
                                    visible: co.chosen
                                    anchors.fill: parent
                                    color: "transparent"
                                    border.width: cr.here ? Math.max(2, 0.6 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                                    border.color: cr.here ? settings.ink : settings.inkA(0.6)
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    onPressed: { settings.cfgRow = cr.index; if (!co.chosen) settings.cfgSet(cr.modelData, co.modelData[0]) }
                                }
                            }
                        }
                        Text {
                            height: 7 * settings.mm
                            verticalAlignment: Text.AlignVCenter
                            leftPadding: 3 * settings.mm
                            text: cr.modelData.note
                            font.family: Fonts.mono; font.pixelSize: settings.font * 0.7
                            color: settings.inkA(0.6)
                        }
                    }
                    MouseArea {    // the row's name picks the row
                        width: cfgPage.optX; height: parent.height
                        onPressed: settings.cfgRow = cr.index
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
