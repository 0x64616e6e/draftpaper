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
import "Board.js" as Board

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
    // the canvas's ten, in its order. Draftpaper's settings are distributed into them: the drawings,
    // the wallpaper, the faces and the rulers are all how the sheet is finished, so they are one
    // Appearance page as the canvas has one, and it scrolls as its artboards do.
    readonly property var pageList: [["layout", "LAYOUT"], ["workspaces", "WORKSPACES"], ["rules", "WINDOW RULES"],
                                     ["displays", "DISPLAYS"], ["keyboard", "KEYBOARD"], ["pointer", "POINTER"],
                                     ["bindings", "BINDINGS"], ["appearance", "APPEARANCE"], ["motion", "MOTION"],
                                     ["startup", "STARTUP"]]
    // each row: config.toml's key (dotted: table.key), its name, the values offered ([value, label]), the
    // value when config.toml has none, and a note; `draftpaper config set` writes it, comments kept
    readonly property var cfgPagesRaw: ({
        layout: [
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
            { key: "sheet.schedule_at", zone: "row", name: "SCHEDULES · ZONE ROW", def: "auto", note: "Mod+i; auto: the top left, 10 mm in; or drag one",
              options: settings.zoneRows(true) },
            { key: "sheet.schedule_at", zone: "col", name: "SCHEDULES · ZONE COLUMN", def: "auto", note: "",
              options: settings.zoneCols() },
            { key: "glyphs", name: "GLYPHS", def: true, note: "Nerd Font glyphs before the readings",
              options: [[true, "on"], [false, "words only"]] }
        ],
        // [animations]: what Motion sets, and the programme it draws
        motion: [
            { key: "animations.enabled", name: "MOTION", def: true, note: "parking, unparking, switching workspaces",
              options: [[true, "on"], [false, "reduced"]] },
            { key: "animations.duration_ms", name: "DURATION", def: 220, scale: true,
              note: "how long one of them takes, end to end",
              options: [[120, "120 ms"], [160, "160 ms"], [220, "220 ms"], [300, "300 ms"], [420, "420 ms"]] }
        ],
        rulerRowsHolder: [
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
        startup: [
            { key: "session.restore", name: "RESTORE", def: true, note: "the windows back in their places at login",
              options: [[true, "on"], [false, "off"]] },
            { key: "session.apps", name: "APPLICATIONS' STATE", def: true, note: "tabs, files, editors, tmux",
              options: [[true, "on"], [false, "off"]] }
        ],
        keyboard: [
            { key: "input.repeat_delay", name: "REPEAT DELAY", def: 200, note: "before a held key repeats",
              options: [[150, "150 ms"], [200, "200 ms"], [300, "300 ms"], [400, "400 ms"]] },
            { key: "input.repeat_rate", name: "REPEAT RATE", def: 25,
              note: "how fast a held key repeats once it has started",
              options: [[20, "20/s"], [25, "25/s"], [30, "30/s"], [40, "40/s"]] },
        ],
        mouseRows: [
            { key: "input.focus_follows_mouse", name: "FOCUS FOLLOWS THE MOUSE", def: true, note: "",
              options: [[true, "on"], [false, "off"]] },
            { key: "pointer.natural_scroll", name: "MOUSE · NATURAL SCROLLING", def: false, note: "a wheel, and the TrackPoint",
              options: [[false, "off"], [true, "on"]] },
            { key: "pointer.speed", name: "MOUSE · SPEED", def: 0, scale: true,
              note: "how far the mouse or the TrackPoint carries the pointer",
              options: settings.speeds },
            { key: "pointer.left_handed", name: "MOUSE · LEFT-HANDED", def: false, note: "the buttons swapped",
              options: [[false, "off"], [true, "on"]] }
        ],
        // the built-in pad has a table of its own: a touchpad is not a mouse, and one [input] read
        // into both was one setting doing two jobs (compositor/src/config.rs)
        pointerRows: [
            { key: "touchpad.tap", name: "TAP TO CLICK", def: true, note: "a tap is a click, without pressing the pad",
              options: [[true, "on"], [false, "off"]] },
            { key: "touchpad.tap_and_drag", name: "TAP AND DRAG", def: true, note: "tap, then tap and hold: the second tap drags",
              options: [[true, "on"], [false, "off"]] },
            { key: "touchpad.natural_scroll", name: "NATURAL SCROLLING", def: false, note: "the content follows the fingers",
              options: [[false, "off"], [true, "on"]] },
            { key: "touchpad.disable_while_typing", name: "QUIET WHILE TYPING", def: true, note: "the heel of a hand is not a click",
              options: [[true, "on"], [false, "off"]] },
            { key: "touchpad.click_method", name: "CLICK", def: "button-areas", note: "where a press of the pad is left, right or middle",
              options: [["button-areas", "by area"], ["clickfinger", "by fingers"]] },
            { key: "touchpad.scroll_method", name: "SCROLL", def: "two-finger",
              note: "two fingers anywhere on the pad, or one along its right and bottom edges",
              options: [["two-finger", "two fingers"], ["edge", "the edge"]] },
            { key: "touchpad.speed", name: "SPEED", def: 0, scale: true,
              note: "how far a finger carries the pointer: libinput's acceleration, −1 to 1",
              options: settings.speeds },
            { key: "touchpad.middle_emulation", name: "MIDDLE BUTTON", def: false, note: "both buttons at once is the middle one",
              options: [[false, "off"], [true, "on"]] },
            { key: "touchpad.left_handed", name: "LEFT-HANDED", def: false, note: "the buttons swapped",
              options: [[false, "off"], [true, "on"]] }
        ],
        // a touch device reports where it was touched and nothing else, so there is little to set:
        // whether it is listened to, and which display it is the surface of
        idleRows: [
            { key: "idle.screens_off", name: "SCREENS OFF AFTER", def: 15, note: "any input wakes them",
              options: [[5, "5 min"], [10, "10 min"], [15, "15 min"], [30, "30 min"], [0, "never"]] },
            { key: "idle.lock_after", name: "LOCK AFTER", def: 0, note: "Mod+Shift+x locks now",
              options: [[0, "never"], [5, "5 min"], [10, "10 min"], [15, "15 min"]] },
            { key: "idle.lock_on_sleep", name: "LOCK BEFORE SUSPEND", def: false, note: "Mod+Shift+p: for the day",
              options: [[false, "off"], [true, "on"]] }
        ]
    })
    // the groups above, gathered onto the canvas's pages. The rulers, the faces and the wallpaper's
    // treatments are all how the sheet is finished, so they are the one Appearance page it has.
    readonly property var rulerRows: cfgPagesRaw.rulerRowsHolder
    readonly property var wallRows: wpRows
    readonly property var pointerAll: cfgPagesRaw.mouseRows.concat(cfgPagesRaw.pointerRows, touchRows)
    readonly property var startupAll: cfgPagesRaw.startup.concat(cfgPagesRaw.idleRows)
    readonly property var cfgPages: ({
        layout: cfgPagesRaw.layout,
        workspaces: [],
        rules: [],
        displays: dispRows,
        keyboard: cfgPagesRaw.keyboard,
        pointer: pointerAll,
        bindings: [],
        appearance: textRows.concat(rulerRows),
        motion: cfgPagesRaw.motion,
        startup: startupAll
    })
    // the speeds a pointing device is set to: libinput takes -1 (slow) to 1 (fast), and these are the
    // five of them the sheet offers rather than a slider it has nowhere to draw
    readonly property var speeds: [[-0.6, "slow"], [-0.3, "steady"], [0, "standard"], [0.4, "quick"], [0.8, "fast"]]
    // the TOUCH page: whether the touchscreen is listened to, and which display it is the surface of.
    // The displays are the ones there are now, so this is a property rather than a constant.
    readonly property var touchRows: [
        { key: "touchscreen.enabled", name: "TOUCHSCREEN", def: true, note: "a finger moves the pointer and presses",
          options: [[true, "on"], [false, "off"]] },
        { key: "touchscreen.output", name: "THE SURFACE OF", def: "", note: "which display a finger is touching",
          options: [["", "the built-in panel"]].concat(Compositor.outputs.map(o => [o.name, o.name])) }
    ]

    // the pages, numbered as the canvas numbers them, each with the line that says what it is for
    readonly property var pageSays: ({
        layout: "How windows divide a display. The sheet is a grid of zones — columns numbered across it, rows lettered down it — and the plan shows what is built on them.",
        workspaces: "Workspaces stack like the floors of a building, workspace 1 at ground level. Empty floors are hatched.",
        rules: "A specification for new windows. Each clause matches windows as they open and decides where they go; clauses are read top to bottom, and later ones override earlier ones.",
        displays: "The site plan of your desktop. Each display is drawn at its logical size — resolution divided by scale — measured from a single origin.",
        keyboard: "What each key means and how it repeats. The elevation shows one second of a key held down.",
        pointer: "How the hand reaches the screen. The plan maps what each part of the touchpad does, drawn at the pad's own proportion.",
        bindings: "Every combination the compositor acts on. Choose a modifier layer; the schedule lists the whole of it.",
        appearance: "How the sheet is finished: the drawing it is made in, the picture behind the frame, the faces it is lettered in and the rulers that cross it.",
        motion: "How long things take, and how they arrive. The profile plots progress against time for one event; the programme sets every event on the same clock.",
        startup: "What runs between login and a ready desktop. Phases climb in order; each waits for the one below it."
    })
    function pageSaid(name) { return pageSays[name] || "" }
    // The canvas sets its names in sentence case, not capitals. The rows are written here in capitals
    // because the sheet letters everything that way; this is the one place they are read as an
    // application's labels instead, so they are set as one. A short run of capitals is an initialism
    // and stays: CPU, DPI, NTS.
    readonly property var keptCaps: ["CPU", "DPI", "NTS", "RAM", "GPU", "SSD", "TTY", "VT", "OF"]
    function sentence(s) {
        if (!s) return ""
        const words = String(s).split(" ").map(w => {
            const bare = w.replace(/[^A-Za-z0-9-]/g, "")
            if (keptCaps.indexOf(bare) >= 0) return w
            if (/[a-z]/.test(w)) return w          // already mixed: a file name, a face, eDP-1
            return w.toLowerCase()
        })
        const out = words.join(" ")
        return out.charAt(0).toUpperCase() + out.slice(1)
    }

    // the row the keys are on brought into the scroller's view, as the wallpaper page keeps the
    // picked picture in sight: the keys walk the page and the page follows them
    function revealRow() {
        if (!cfgRows.length) return
        const top = cfgTop + cfgRow * rowPitch
        const bottom = top + rowPitch
        if (top < scroll.contentY + 8) scroll.contentY = Math.max(0, top - 8)
        else if (bottom > scroll.contentY + scroll.height - 8)
            scroll.contentY = Math.min(Math.max(0, scroll.contentHeight - scroll.height), bottom - scroll.height + 8)
    }
    onCfgRowChanged: revealRow()
    function twoDigits(n) { return (n < 10 ? "0" : "") + n }
    // the pages with their numbers, and the search narrowing them. A page matches on its own name or
    // on any row it carries, so "scroll" finds the touchpad.
    property string find: ""
    readonly property var numbered: pageList.map((p, i) => [p[0], p[1], twoDigits(i + 1)])
    readonly property var shownPages: {
        const q = find.trim().toLowerCase()
        if (q === "") return numbered
        return numbered.filter(p => {
            if (p[1].toLowerCase().indexOf(q) >= 0) return true
            const rows = cfgPages[p[0]] || []
            return rows.some(r => (r.name || "").toLowerCase().indexOf(q) >= 0
                                  || (r.key || "").toLowerCase().indexOf(q) >= 0)
        })
    }
    // whether anything is waiting to be applied: a row changed, a drawing or a picture picked
    readonly property bool dirty: Object.keys(cfgVals).length > 0 || shown !== original || wpTouched || cfgMarked

    readonly property var cfgRows: cfgPages[page] || []
    property var cfg: null              // `draftpaper config json`
    property var cfgVals: ({})          // the rows changed, key: value
    property bool cfgMarked: false      // config.toml kept as it was (`config mark`): Escape puts it back
    property int cfgRow: 0
    onPageChanged: { cfgRow = 0; scroll.contentY = 0 }
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
        buildDisplays()
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

    // ---- the Text page: [text]'s faces among those installed, and sizes against the legibility target ----
    readonly property var ifaceFaces: ["IBM Plex Sans", "Noto Sans", "DejaVu Sans", "Cantarell", "Source Sans 3", "Fira Sans", "Ubuntu", "Liberation Sans"]
    readonly property var monoFaces: ["IBM Plex Mono", "JetBrains Mono", "DejaVu Sans Mono", "Fira Mono", "Source Code Pro", "Hack", "Noto Sans Mono", "Liberation Mono"]
    // up to five installed, the one set first among them when it is not one of the five
    function faceOptions(list, now) {
        const all = Qt.fontFamilies()
        const have = list.filter(f => all.indexOf(f) >= 0).slice(0, 5)
        if (now && have.indexOf(now) < 0) have.unshift(now)
        return have.map(f => [f, f])
    }
    readonly property var ptSizes: [9, 10, 10.5, 11, 12, 13, 14].map(s => [s, s + " pt"])
    readonly property var textRows: {
        const t = (cfg && cfg.text) || {}
        return [
            { key: "text.interface", name: "INTERFACE", def: t.interface || "IBM Plex Sans", face: true, note: "GTK (Firefox's interface with it), Qt",
              options: faceOptions(ifaceFaces, t.interface) },
            { key: "text.monospace", name: "MONOSPACE", def: t.monospace || "IBM Plex Mono", face: true, note: "terminals, Emacs, the sheet's lettering",
              options: faceOptions(monoFaces, t.monospace) },
            { key: "text.size", name: "INTERFACE SIZE", def: t.size || 11, legible: "interface", options: ptSizes },
            { key: "text.mono_size", name: "MONOSPACE SIZE", def: t.mono_size || 11, legible: "monospace", options: ptSizes },
            { key: "text.min_degrees", name: "LEGIBILITY TARGET", def: t.min_degrees || 0.2, note: "sizes below it are lettered so, not refused",
              options: [[0.18, "0.18°"], [0.2, "0.2°"], [0.25, "0.25°"]] }
        ]
    }
    function textRow(k) { return textRows.find(r => r.key === k) }
    // a size's x-height in mm on the focused display, and the angle it makes at the viewing distance
    function legibility(pt, which) {
        const m = which === "monospace" ? monoMetrics : ifaceMetrics
        const ratio = m.xHeight / 100
        const pxPerMm = out ? out.px_per_mm : 4.27
        const xh = pt * 96 / 72 * ratio / pxPerMm
        const dist = Number(cfgGet({ key: "viewing_distance", def: 55 }))
        const deg = Math.atan(xh / (dist * 10)) * 180 / Math.PI
        return { xh: xh, deg: deg, dist: dist, ok: deg >= Number(cfgGet(textRow("text.min_degrees"))) - 1e-9 }
    }
    FontMetrics { id: ifaceMetrics; font.family: settings.cfgGet(settings.textRow("text.interface")); font.pixelSize: 100 }
    FontMetrics { id: monoMetrics; font.family: settings.cfgGet(settings.textRow("text.monospace")); font.pixelSize: 100 }
    // ---- the Displays page: built when the settings open, from the displays as they are then ----
    property var dispOutputs: []        // the outputs then: the built-in panel first, then left to right
    property var dispPlace: ({})        // each further display: { side: right|left|above|below, align: start|centre|end }
    property var dispKeys: ({})         // each display's table in config.toml: { table, path, label }
    property var dispRows: []
    readonly property var scales: [[1, "1"], [1.25, "1.25"], [1.5, "1.5"], [1.75, "1.75"], [2, "2"]]
    function buildDisplays() {
        const builtIn = n => /^(eDP|LVDS|DSI|headless$)/.test(n)
        // (a compositor before 0.7 reports no mode or size: the mode from the logical size and scale)
        const outs = Compositor.outputs.map(o => Object.assign({ mode: [Math.round(o.width * o.scale), Math.round(o.height * o.scale)],
                                                                 physical: [0, 0], model: "" }, o)).sort((a, b) => (builtIn(b.name) - builtIn(a.name)) || (a.x - b.x) || (a.y - b.y))
        const place = {}
        for (let i = 1; i < outs.length; i++) {
            const o = outs[i], p = outs[i - 1]
            const side = o.x >= p.x + p.width ? "right" : o.x + o.width <= p.x ? "left" : o.y + o.height <= p.y ? "above" : "below"
            const across = side === "right" || side === "left"
            const a0 = across ? o.y : o.x, a1 = across ? o.y + o.height : o.x + o.width
            const b0 = across ? p.y : p.x, b1 = across ? p.y + p.height : p.x + p.width
            const align = Math.abs(a0 - b0) <= 1 ? "start" : Math.abs(a1 - b1) <= 1 ? "end" : "centre"
            place[o.name] = { side: side, align: align }
        }
        // each display's table: by its model, so its settings follow it from port to port, where the model
        // reads as a name and no other display here has it; else by its connector
        const readable = m => !!m && m !== "Unknown" && !/^0x/.test(m)
        const keys = {}
        for (const o of outs) {
            const unique = outs.filter(p => p.model === o.model).length === 1
            const byModel = readable(o.model) && unique
            keys[o.name] = { table: "outputs." + (byModel ? "\"" + o.model.replace(/"/g, "\\\"") + "\"" : o.name),
                             path: ["outputs", byModel ? o.model : o.name], label: byModel ? o.model : o.name }
        }
        dispKeys = keys
        dispOutputs = outs
        dispPlace = place
        const rows = []
        outs.forEach((o, i) => {
            const k = keys[o.name]
            rows.push({ key: k.table + ".scale", path: k.path.concat(["scale"]), name: k.label + " · SCALE", def: o.scale, noteOf: "scale", output: o.name,
                        options: scales })
            if (i === 0) return
            const p = outs[i - 1].name
            rows.push({ key: "~" + o.name + ".side", pseudo: "side", output: o.name, name: o.name + " · BESIDE", def: "right", note: "",
                        options: [["right", "right of " + p], ["left", "left of " + p], ["above", "above"], ["below", "below"]] })
            rows.push({ key: "~" + o.name + ".align", pseudo: "align", output: o.name, name: o.name + " · ALIGNED", def: "start", noteOf: "align",
                        options: [["start", "start"], ["centre", "centred"], ["end", "end"]] })
        })
        rows.push({ key: "viewing_distance", name: "VIEWING DISTANCE", def: 55, note: "the Text page's legibility is worked out at it",
                    options: [[40, "40 cm"], [55, "55 cm"], [70, "70 cm"], [90, "90 cm"]] })
        dispRows = rows
    }
    function dispScale(name) {
        const r = dispRows.find(r => r.noteOf === "scale" && r.output === name)
        return r ? cfgGet(r) : 1
    }
    // each display's place and logical size at the scales picked: the first at the origin, each further
    // one beside the one before it, then all moved so none is left of or above 0, 0
    function dispLayout() {
        const out = []
        dispOutputs.forEach((o, i) => {
            const s = dispScale(o.name)
            // rounded up, as the compositor rounds a logical size
            const w = Math.ceil(o.mode[0] / s - 1e-9), h = Math.ceil(o.mode[1] / s - 1e-9)
            let x = 0, y = 0
            if (i > 0) {
                const p = out[i - 1], pl = dispPlace[o.name] || { side: "right", align: "start" }
                const across = pl.side === "right" || pl.side === "left"
                const lay = (len, plen, at) => pl.align === "start" ? at : pl.align === "end" ? at + plen - len : at + Math.round((plen - len) / 2)
                if (pl.side === "right") x = p.x + p.w
                else if (pl.side === "left") x = p.x - w
                else if (pl.side === "above") y = p.y - h
                else y = p.y + p.h
                if (across) y = lay(h, p.h, p.y)
                else x = lay(w, p.w, p.x)
            }
            out.push({ name: o.name, x: x, y: y, w: w, h: h, o: o })
        })
        const mx = Math.min(0, ...out.map(d => d.x)), my = Math.min(0, ...out.map(d => d.y))
        for (const d of out) { d.x -= mx; d.y -= my }
        return out
    }
    // the positions written for every display, so the arrangement is whole whatever changed
    function dispWrite() {
        if (dispOutputs.length < 2) return
        const v = Object.assign({}, cfgVals)
        for (const d of dispLayout()) v[dispKeys[d.name].table + ".position"] = [d.x, d.y]
        cfgVals = v
    }
    function rowNote(row) {
        if (row.zone === "row" && Array.isArray(cfgRaw(row))) {
            const v = cfgRaw(row)
            return "dragged: " + v[0] + ", " + v[1] + " mm from the frame's corner; a zone puts it back"
        }
        if (row.legible) {
            const l = legibility(cfgGet(row), row.legible)
            return "x-height " + l.xh.toFixed(2) + " mm · " + l.deg.toFixed(3) + "° at " + l.dist + " cm · "
                + (l.ok ? "meets " : "below ") + cfgGet(textRow("text.min_degrees")) + "°"
        }
        if (row.noteOf === "scale") {
            const o = dispOutputs.find(o => o.name === row.output)
            if (!o) return ""
            const s = cfgGet(row), mm = o.px_per_mm * o.scale / s
            return Math.ceil(o.mode[0] / s - 1e-9) + " × " + Math.ceil(o.mode[1] / s - 1e-9) + " logical · " + mm.toFixed(1) + " px a mm · 11 pt reads "
                + (11 * 96 / 72 / mm).toFixed(1) + " mm"
        }
        if (row.noteOf === "align") {
            const pl = dispPlace[row.output]
            return pl && (pl.side === "above" || pl.side === "below") ? "start: left edges level; end: right edges" : "start: tops level; end: bottoms level"
        }
        return row.note || ""
    }
    // ---- the pages of config.toml's rows ----
    function cfgGet(row) {
        if (row.pseudo) return (dispPlace[row.output] || {})[row.pseudo] || row.def
        if (row.zone) return zonePart(cfgRaw(row), row.zone)
        return cfgRaw(row)
    }
    function cfgRaw(row) {
        if (row.key in cfgVals) return cfgVals[row.key] === "" ? row.def : cfgVals[row.key]
        let v = cfg
        for (const part of (row.path || row.key.split("."))) v = (v && typeof v === "object") ? v[part] : undefined
        return v === undefined || v === null ? row.def : v
    }
    // ---- a note's place on the sheet: a zone ("B2"), "auto", or millimetres where it was dragged ----
    function zoneRows(auto) {
        const n = out ? out.zones.down : 4
        const rows = Array.from({ length: n }, (_, i) => { const l = String.fromCharCode(65 + i); return [l, l] })
        return auto ? [["auto", "auto"]].concat(rows) : rows
    }
    function zoneCols() {
        const n = out ? out.zones.across : 6
        return Array.from({ length: n }, (_, i) => [i + 1, String(i + 1)])
    }
    function zonePart(v, part) {
        if (Array.isArray(v)) return "dragged"
        if (v === "auto") return part === "row" ? "auto" : "auto"
        const m = /^([A-Za-z])([0-9]+)$/.exec(String(v))
        if (!m) return part === "row" ? "A" : 1
        return part === "row" ? m[1].toUpperCase() : Number(m[2])
    }
    // the place with one part changed: "auto" (the key taken out), or a zone, its other part as it was
    function zoneWith(row, value) {
        if (value === "auto") return ""
        const v = cfgRaw(row)
        const m = /^([A-Za-z])([0-9]+)$/.exec(Array.isArray(v) ? "" : String(v))
        const letter = row.zone === "row" ? value : m ? m[1].toUpperCase() : "A"
        const col = row.zone === "col" ? value : m ? Number(m[2]) : 1
        return letter + col
    }
    // the values offered, and the one set when it is none of them
    function cfgOptions(row) {
        const v = cfgGet(row)
        return row.options.some(o => o[0] === v) ? row.options : row.options.concat([[v, String(v)]])
    }
    function cfgSet(row, value) {
        if (!cfgMarked) { queue(["config", "mark"], ""); cfgMarked = true }
        if (row.pseudo) {
            // a display's side or alignment: not a key of its own, the positions follow from it
            const p = Object.assign({}, dispPlace)
            p[row.output] = Object.assign({ side: "right", align: "start" }, p[row.output] || {})
            p[row.output][row.pseudo] = value
            dispPlace = p
        } else {
            const v = Object.assign({}, cfgVals)
            v[row.key] = row.zone ? zoneWith(row, value) : value
            cfgVals = v
        }
        // a display's scale or place: every display's position written again
        if (row.pseudo || row.noteOf === "scale") dispWrite()
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
    // a value as TOML writes it: a list in brackets, the rest as it is.
    function raw(v) { return Array.isArray(v) ? "[" + v.join(", ") + "]" : String(v) }
    // the Bindings board, reached from the IPC so a test can bind without a keyboard to press
    function bindKey(code) { bindPage.picked = code }
    function bindSet(action) { bindPage.setTo(action); return bindPage.chord }
    function cfgApply() {
        const keys = Object.keys(cfgVals)
        if (keys.length) queue(["config", "set"].concat(keys.map(k => k + "=" + raw(cfgVals[k]))), "config")
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
                    settings.sheetWhite = at >= 0 ? at : 0
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
                        if (wasCustom) settings.sheetWhite = settings.cards.length - 1
                        // the current theme is the custom one: pick it
                        else if (settings.original === "custom" && settings.row === 0) settings.sheetWhite = settings.cards.length - 1
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
        // keepBlock false: the settings are the sheet while they are up. The board paints the whole
        // of the frame's interior, title block included, and only the margins stay showing -- the
        // panel is one surface rather than a thing arranged around the furniture underneath it.
        id: board; out: settings.out; t: settings.picked; keepBlock: false
        wallpaper: settings.wp ? (settings.wpPicture ? "file://" + settings.wpPicture : "") : settings.wallpaper
    }

    // The application's body, over the whole of the frame's interior and no further -- the margins
    // stay the sheet's. It is the canvas's own #F3F3F0 on every theme: a settings application looks
    // like itself, and the sheet behind it is what changes when a drawing is picked.
    Rectangle {
        x: board.inX; y: board.inY; width: board.inW; height: board.inH
        color: settings.bodyBg
    }

    // ---- the layout ----
    // Built to the Window Manager Settings canvas, to its own numbers. Its sizes are CSS pixels at a
    // 1440-wide artboard and are used here as logical pixels; its colours are its own, literally, on
    // every theme. A settings application looks like itself -- the sheet behind it is what changes
    // when a drawing is picked.
    readonly property real dpr: board.dpr
    // the canvas names its two faces; the sheet's own [text] choices do not reach this application
    readonly property string sans: Qt.fontFamilies().indexOf("IBM Plex Sans") >= 0 ? "IBM Plex Sans" : "Noto Sans"
    readonly property string code: Qt.fontFamilies().indexOf("IBM Plex Mono") >= 0 ? "IBM Plex Mono" : "DejaVu Sans Mono"
    readonly property color bodyBg: "#F3F3F0"      // the body
    readonly property color chrome: "#EAEBE7"      // the header and the sidebar
    readonly property color sheetWhite: "#FBFBF9"  // a field, a nav item in use, a knob
    readonly property color fieldBg: "#F6F6F3"     // the search
    readonly property color line: "#D3D5CF"        // the ordinary rule
    readonly property color edge: "#C9CCC5"        // a field's border
    readonly property color firm: "#B9BCB5"        // a ghost button's border
    readonly property color ink: "#16171A"         // the text, and every strong line
    readonly property color ink2: "#2A2C30"        // a nav item not in use
    readonly property color ink3: "#575B60"        // a hint, a value
    readonly property color ink4: "#63676C"        // a label, a placeholder, a number
    readonly property color accent: "#2B45B5"
    readonly property color accentDark: "#1E3290"
    readonly property color hatch: "#C6C9C2"
    readonly property color plan: "#E9EAE6"
    readonly property color navHover: "#E1E2DD"
    readonly property color ghostHover: "#E6E7E2"
    function inkA(a) { return Qt.rgba(0.086, 0.09, 0.102, a) }

    // type, as the canvas sets it
    readonly property real fTitle: 40          // the page's name
    readonly property real fLead: 15           // its description
    readonly property real fRow: 14            // a row's label, a nav item, a button
    readonly property real fSmall: 13          // the search field, a link
    readonly property real fHint: 12           // a hint, a mono value, the status
    readonly property real fTag: 11            // an uppercase mono label, a nav number

    // the frame's interior is the app; the margins stay the sheet's
    readonly property real appX: board.inX
    readonly property real appY: board.inY
    readonly property real appW: board.inW
    readonly property real appH: board.inH
    readonly property real barH: 64            // min-height, padding 10px 32px
    readonly property real sideW: 240          // flex 1 1 240px
    readonly property real navH: 44
    readonly property real padX: 56            // main's padding: 48px 56px 64px
    readonly property real padTop: 48
    readonly property real mainX: padX
    readonly property real mainW: appW - sideW - 2 * padX
    readonly property real mainY: padTop
    // how deep the page showing is, so the scroller knows how far it goes
    // how deep the page showing is: its figure, and then the rows under it
    readonly property real figureDeep:
          page === "layout" ? layoutPage.deep
        : page === "workspaces" ? wsPage.deep
        : page === "rules" ? rulesPage.deep
        : page === "keyboard" ? kbdPage.deep
        : page === "pointer" ? padPage.deep
        : page === "bindings" ? bindPage.deep
        : page === "motion" ? motionPage.deep
        : page === "startup" ? startPage.deep
        : 0
    readonly property real pageDeep: page === "appearance" ? builderY + rows.length * rowH + 64
        : cfgTop + figureDeep + cfgRows.length * rowPitch + 64
    readonly property real rowPitch: 69        // 16px padding, a 14px label over a 12px hint
    readonly property real cfgTop: pageHead.foot + 24 + (cfgPage.bandH || 0)

    // kept for the pages that still set themselves out their own way
    readonly property real gutter: 8 * mm
    readonly property real font: 2.8 * mm
    readonly property real cardGap: 5 * mm
    readonly property real cardW: Math.min(70 * mm, (mainW - (cards.length - 1) * cardGap) / Math.max(1, cards.length))
    readonly property real cardH: cardW * height / Math.max(1, width)
    readonly property real builderY: cfgTop + 22 + cardH + 40
    readonly property real rowH: Math.max(8 * mm, Math.min(12 * mm, (appH - barH - 64 - (builderY - cfgTop)) / rows.length))
    readonly property int most: Math.max(8, ...rows.map(k => (options[k] || []).length))
    readonly property real cellW: Math.min(24 * mm, (mainW - 34 * mm) / most)

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
            // two keys close the settings and no more: Return keeps, Escape goes back. Space kept
            // them too, which the hint along the top margin never said and nothing else here does.
            else if (k === Qt.Key_Return || k === Qt.Key_Enter) settings.keep()
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
            // nothing here dismisses. The settings are a full-screen layer surface, so a press that
            // lands on no control lands here — and this used to cancel, which is not "close" but
            // "put the theme, the wallpaper and config.toml back". A half-built scheme went to a
            // stray click. Esc goes back and Return keeps, as the hint along the top margin says,
            // and those two are the whole of it: the title block is under the board while the
            // settings are up, so nothing on the sheet is a way out either.
            // It still takes the press, the settings being modal: it simply does nothing with it.
            //
            // It takes no wheel either. Rolling it used to step the row the keys were on, so a scroll
            // meant to move the page changed a setting instead -- the pointer need not even be near
            // the row. The wheel moves the pages and the sections it is over, and picks nothing.
            anchors.fill: parent
        }

        // ---- the header ----
        // The canvas's bar: the name on the left, what has changed and the two buttons on the right.
        // Apply keeps and Revert goes back -- the same two things Return and Escape have always done,
        // said out loud. An affordance with no signifier is one nobody finds (docs/design-research.md).
        Rectangle {
            id: bar
            x: settings.appX; y: settings.appY
            width: settings.appW; height: settings.barH
            color: settings.chrome
            Rectangle { y: parent.height - height; width: parent.width; height: 1; color: settings.line }

            Row {
                x: 32; anchors.verticalCenter: parent.verticalCenter
                spacing: 14
                Canvas {        // the sheet's own mark: a frame and two cells, as the canvas draws it
                    id: mark
                    width: 22; height: 22; anchors.verticalCenter: parent.verticalCenter
                    onPaint: {
                        const c = getContext("2d")
                        c.reset(); c.strokeStyle = settings.ink; c.lineWidth = 1.5
                        c.strokeRect(1.75, 1.75, 11, 18.5)
                        c.strokeRect(14.75, 1.75, 5.5, 8.25)
                        c.strokeRect(14.75, 12, 5.5, 8.25)
                    }
                    Connections { target: settings; function onInkChanged() { mark.requestPaint() } }
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Draftpaper"
                    font.family: settings.sans; font.pixelSize: 15; font.weight: Font.DemiBold
                    font.letterSpacing: -0.15
                    color: settings.ink
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Settings"
                    font.family: settings.code; font.pixelSize: settings.fHint
                    color: settings.ink4
                }
            }

            Row {
                anchors { right: parent.right; rightMargin: 32; verticalCenter: parent.verticalCenter }
                spacing: 12
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    rightPadding: 8
                    text: settings.dirty ? "Unapplied changes" : "Up to date"
                    font.family: settings.code; font.pixelSize: settings.fHint
                    color: settings.dirty ? settings.accent : settings.ink4
                }
                Rectangle {     // Revert
                    width: revertLabel.implicitWidth + 40; height: settings.navH
                    color: revertArea.containsMouse && settings.dirty ? settings.ghostHover : "transparent"
                    border.width: 1; border.color: settings.firm
                    opacity: settings.dirty ? 1 : 0.45
                    Text {
                        id: revertLabel
                        anchors.centerIn: parent; text: "Revert"
                        font.family: settings.sans; font.pixelSize: settings.fRow; font.weight: Font.Medium
                        color: settings.ink
                    }
                    MouseArea {
                        id: revertArea
                        anchors.fill: parent; hoverEnabled: true
                        cursorShape: settings.dirty ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onPressed: if (settings.dirty) settings.cancel()
                    }
                }
                Rectangle {     // Apply
                    width: applyLabel.implicitWidth + 48; height: settings.navH
                    color: settings.dirty ? settings.ink : "transparent"
                    border.width: 1; border.color: settings.dirty ? settings.ink : settings.firm
                    opacity: settings.dirty ? 1 : 0.45
                    Text {
                        id: applyLabel
                        anchors.centerIn: parent; text: "Apply"
                        font.family: settings.sans; font.pixelSize: settings.fRow; font.weight: Font.Medium
                        color: settings.dirty ? settings.sheetWhite : settings.ink
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: settings.dirty ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onPressed: settings.keep()
                    }
                }
            }
        }

        // ---- the sidebar ----
        Rectangle {
            id: side
            x: settings.appX; y: settings.appY + settings.barH
            width: settings.sideW; height: settings.appH - settings.barH
            color: settings.chrome
            Rectangle { x: parent.width - width; width: 1; height: parent.height; color: settings.line }

            Rectangle {     // the search field
                x: 20; y: 28
                width: parent.width - 40; height: 40
                color: settings.fieldBg
                border.width: 1; border.color: settings.edge
                Canvas {
                    id: lens
                    width: 14; height: 14; x: 12; anchors.verticalCenter: parent.verticalCenter
                    onPaint: {
                        const c = getContext("2d")
                        c.reset(); c.strokeStyle = settings.ink3; c.lineWidth = 1.4
                        c.beginPath(); c.arc(6, 6, 4.5, 0, 2 * Math.PI); c.stroke()
                        c.beginPath(); c.moveTo(9.4, 9.4); c.lineTo(13, 13); c.stroke()
                    }
                    Connections { target: settings; function onInkChanged() { lens.requestPaint() } }
                }
                TextInput {
                    id: search
                    anchors { left: parent.left; leftMargin: 34; right: parent.right; rightMargin: 10
                              verticalCenter: parent.verticalCenter }
                    font.family: settings.sans; font.pixelSize: settings.fSmall
                    color: settings.ink
                    selectByMouse: true
                    onTextChanged: settings.find = text
                    Text {
                        anchors.fill: parent; verticalAlignment: Text.AlignVCenter
                        visible: search.text === ""
                        text: "Search settings"
                        font: search.font; color: settings.ink4
                    }
                    Keys.onEscapePressed: { text = ""; keys.forceActiveFocus() }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onPressed: search.forceActiveFocus() }
            }

            // the pages, numbered as the canvas numbers them; scrolled where there are more than
            // the column is tall, so the last one is never cut off by the file at the foot
            Flickable {
                id: navView
                x: 20; y: 28 + 40 + 24
                width: parent.width - 40
                height: Math.max(settings.navH, foot.y - y - 16)
                clip: true
                contentWidth: width
                contentHeight: nav.height
                boundsBehavior: Flickable.StopAtBounds
                Column {
                    id: nav
                    width: navView.width
                    spacing: 2
                Repeater {
                    model: settings.shownPages
                    Rectangle {
                        id: pg
                        required property var modelData
                        readonly property bool here: settings.page === modelData[0]
                        width: parent.width; height: settings.navH
                        color: pg.here ? settings.sheetWhite : navArea.containsMouse ? settings.navHover : "transparent"
                        border.width: 1; border.color: pg.here ? settings.line : "transparent"
                        Text {
                            x: 12; anchors.verticalCenter: parent.verticalCenter
                            text: pg.modelData[2]
                            font.family: settings.code; font.pixelSize: settings.fTag
                            color: pg.here ? settings.accent : settings.ink4
                        }
                        Text {
                            x: 12 + 16 + 16; anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 8; elide: Text.ElideRight
                            text: settings.sentence(pg.modelData[1])
                            font.family: settings.sans; font.pixelSize: settings.fRow
                            font.weight: pg.here ? Font.Medium : Font.Normal
                            color: pg.here ? settings.ink : settings.ink2
                        }
                        MouseArea {
                            id: navArea
                            anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onPressed: settings.page = pg.modelData[0]
                        }
                    }
                }
                }
            }

            Column {        // the file all of this is written to
                id: foot
                x: 20; width: parent.width - 40
                anchors { bottom: parent.bottom; bottomMargin: 24 }
                spacing: 6
                Rectangle { width: parent.width; height: 1; color: settings.line }
                Item { width: 1; height: 14 }
                Text {
                    text: "CONFIG FILE"
                    font.family: settings.code; font.pixelSize: settings.fTag; font.letterSpacing: 0.9
                    color: settings.ink4
                }
                Text {
                    width: parent.width; wrapMode: Text.WrapAnywhere
                    text: "~/.config/draftpaper/config.toml"
                    font.family: settings.code; font.pixelSize: settings.fHint
                    color: settings.ink
                }
                Item { width: 1; height: 4 }
                Text {
                    text: "Open in editor"
                    font.family: settings.sans; font.pixelSize: settings.fSmall
                    color: openArea.containsMouse ? settings.accentDark : settings.accent
                    MouseArea {
                        id: openArea
                        anchors.fill: parent; anchors.margins: -4
                        hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onPressed: settings.queue(["config", "edit"], "")
                    }
                }
            }
        }

        // ---- the main column ----
        // The canvas's artboards are 1400 to 1800 px deep and scroll; this panel is the frame's
        // interior and is not. So the pages sit in a scroller: clipped to the column, as tall as the
        // page it is showing, and moved by the wheel or by the keys walking past its foot.
        Flickable {
            id: scroll
            x: settings.appX + settings.sideW
            y: settings.appY + settings.barH
            width: settings.appW - settings.sideW
            height: settings.appH - settings.barH
            clip: true
            contentWidth: width
            contentHeight: Math.max(height, settings.pageDeep)
            boundsBehavior: Flickable.StopAtBounds
            flickDeceleration: 4000
            Item {
                id: pageBody
                width: scroll.width
                height: scroll.contentHeight
        // ---- the page's head ----
        Item {
            id: pageHead
            x: settings.mainX; y: settings.mainY
            width: settings.mainW
            readonly property var row: settings.numbered.find(p => p[0] === settings.page) || ["", "", ""]
            Text {
                id: pageCount
                text: pageHead.row[2] + " / " + settings.twoDigits(settings.pageList.length)
                font.family: settings.code; font.pixelSize: settings.fHint; font.weight: Font.Medium
                font.letterSpacing: 0.9
                color: settings.accent
            }
            Text {
                id: pageName
                y: pageCount.height + 12
                text: settings.sentence(pageHead.row[1])
                font.family: settings.sans; font.pixelSize: settings.fTitle; font.weight: Font.Medium
                font.letterSpacing: -1
                color: settings.ink
            }
            Text {
                id: pageLead
                y: pageName.y + pageName.height + 12
                width: Math.min(parent.width, 620)
                wrapMode: Text.WordWrap
                lineHeight: 1.55
                text: settings.pageSaid(settings.page)
                font.family: settings.sans; font.pixelSize: settings.fLead
                color: settings.ink3
            }
            Rectangle {
                y: pageLead.y + pageLead.height + 32
                width: parent.width; height: 1; color: settings.line
            }
            readonly property real foot: y + pageLead.y + pageLead.height + 32 + 1
        }


        // ---- 01 Layout: the plan ----
        // The canvas draws a plan of the display with the windows on it, hatched where nothing is
        // built, dimensioned along the top. Draftpaper's layout is a zone grid rather than a tree of
        // gaps, so what is dimensioned is the zones: the columns numbered across the sheet and the
        // rows lettered down it, which the sheet already prints and the compositor already measures
        // in millimetres. The drawing is the canvas's; the thing drawn is ours.
        Item {
            id: layoutPage
            visible: settings.page === "layout"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property var o: settings.out
            readonly property var zn: o && o.zones ? o.zones : { across: 6, down: 4, width: 1, height: 1 }
            readonly property real mmPer: o ? o.px_per_mm : 4
            // the plan is the frame's own proportion, so what is drawn is the sheet and not a box
            readonly property real planW: Math.min(width - 72, 620)
            readonly property real planH: o && o.frame ? planW * o.frame.height / Math.max(1, o.frame.width) : planW * 0.625

            // the columns dimensioned across the top, each in whole millimetres as the sheet is
            Chain {
                id: acrossChain
                width: layoutPage.planW; height: 40
                face: settings.code
                marks: {
                    const n = layoutPage.zn.across, out = []
                    for (let i = 0; i <= n; i++) out.push(i / n)
                    return out
                }
                labels: {
                    const n = layoutPage.zn.across, out = []
                    const each = Math.round(layoutPage.zn.width / layoutPage.mmPer)
                    for (let i = 0; i < n; i++) out.push({ text: each + " mm", strong: i === 0 })
                    return out
                }
            }

            Item {          // the plan itself
                id: plan
                y: acrossChain.height
                width: layoutPage.planW; height: layoutPage.planH
                Rectangle {
                    anchors.fill: parent
                    color: settings.plan
                    border.width: 1; border.color: settings.ink
                    Hatching { anchors.fill: parent; anchors.margins: 1; stroke: settings.hatch; pitch: 8 }
                }
                // the zones, ruled as a plan rules its grid: thin, and not over the windows
                Repeater {
                    model: layoutPage.zn.across - 1
                    Rectangle {
                        required property int index
                        x: Math.round((index + 1) * plan.width / layoutPage.zn.across)
                        width: 1; height: plan.height
                        color: settings.hatch
                    }
                }
                Repeater {
                    model: layoutPage.zn.down - 1
                    Rectangle {
                        required property int index
                        y: Math.round((index + 1) * plan.height / layoutPage.zn.down)
                        width: plan.width; height: 1
                        color: settings.hatch
                    }
                }
                // the windows, built on it: the canvas's own tile, with the name and the size in it
                Repeater {
                    model: layoutPage.o ? Compositor.windows.filter(w => w.geometry && w.output === layoutPage.o.name) : []
                    Rectangle {
                        id: tile
                        required property var modelData
                        readonly property var g: modelData.geometry
                        readonly property var f: layoutPage.o.frame
                        x: Math.round((g.x - layoutPage.o.x - f.x) / f.width * plan.width)
                        y: Math.round((g.y - layoutPage.o.y - f.y) / f.height * plan.height)
                        width: Math.round(g.width / f.width * plan.width)
                        height: Math.round(g.height / f.height * plan.height)
                        color: settings.sheetWhite
                        border.width: modelData.focused ? 2 : 1
                        border.color: settings.ink
                        clip: true
                        Column {
                            x: 12; y: 10; spacing: 3
                            width: parent.width - 24
                            Row {
                                spacing: 7
                                Rectangle {
                                    visible: tile.modelData.focused
                                    width: 6; height: 6; color: settings.accent
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                                Text {
                                    text: tile.modelData.title || tile.modelData.app_id || "window"
                                    font.family: settings.sans; font.pixelSize: 12; font.weight: Font.Medium
                                    color: settings.ink
                                    elide: Text.ElideRight
                                    width: Math.max(0, tile.width - 24 - (tile.modelData.focused ? 13 : 0))
                                }
                            }
                            Text {
                                visible: tile.height > 46
                                text: Math.round(tile.g.width / layoutPage.mmPer) + " × " + Math.round(tile.g.height / layoutPage.mmPer) + " mm"
                                font.family: settings.code; font.pixelSize: 11
                                color: settings.ink4
                            }
                        }
                    }
                }
            }

            // and the rows dimensioned down the side, lettered as the sheet letters them
            Column {
                x: layoutPage.planW + 16; y: acrossChain.height
                Repeater {
                    model: layoutPage.zn.down
                    Item {
                        required property int index
                        width: 60; height: plan.height / layoutPage.zn.down
                        Rectangle { width: 1; height: parent.height; color: settings.line }
                        Text {
                            x: 10; anchors.verticalCenter: parent.verticalCenter
                            text: "ABCDEFGHJKLMNPQRSTUVWXYZ".charAt(index) + "   "
                                  + Math.round(layoutPage.zn.height / layoutPage.mmPer) + " mm"
                            font.family: settings.code; font.pixelSize: 11
                            color: settings.ink4
                        }
                    }
                }
            }

            Text {          // what the plan is, lettered under it as a drawing titles its figure
                y: acrossChain.height + layoutPage.planH + 14
                text: layoutPage.o
                      ? layoutPage.o.name + " · " + layoutPage.zn.across + " × " + layoutPage.zn.down + " zones · "
                        + Math.round((layoutPage.o.frame ? layoutPage.o.frame.width : 0) / layoutPage.mmPer) + " × "
                        + Math.round((layoutPage.o.frame ? layoutPage.o.frame.height : 0) / layoutPage.mmPer) + " mm within the frame"
                      : ""
                font.family: settings.code; font.pixelSize: 11
                font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: acrossChain.height + planH + 14 + 20
        }

        // ---- 02 Workspaces: the section ----
        // "Workspaces stack like the floors of a building, workspace 1 at ground level." So it is a
        // section rather than a plan: the floors drawn one above another, hatched where nothing is
        // built on them, the one you are on outlined, and the ground ruled under the lot.
        Item {
            id: wsPage
            visible: settings.page === "workspaces"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property var floors: {
                const all = Compositor.workspaces.filter(w => !settings.out || w.output === settings.out.name)
                return all.slice().sort((a, b) => b.number - a.number)      // the top floor first
            }
            readonly property real storey: 46
            readonly property real planW: Math.min(width - 120, 560)

            Repeater {
                model: wsPage.floors
                Item {
                    id: floor
                    required property var modelData
                    required property int index
                    readonly property bool empty: modelData.windows === 0 && modelData.parked === 0
                    y: index * wsPage.storey
                    width: wsPage.planW; height: wsPage.storey
                    Rectangle {         // the storey itself
                        anchors.fill: parent; anchors.bottomMargin: 6
                        color: floor.empty ? settings.plan : settings.sheetWhite
                        border.width: floor.modelData.focused ? 2 : 1
                        border.color: settings.ink
                        Hatching {
                            anchors.fill: parent; anchors.margins: 1
                            visible: floor.empty
                            stroke: settings.hatch; pitch: 8
                        }
                    }
                    Text {              // its number, outside the building as a section numbers a level
                        x: -34; anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: -3
                        text: settings.twoDigits(floor.modelData.number)
                        font.family: settings.code; font.pixelSize: 11
                        color: floor.modelData.focused ? settings.accent : settings.ink4
                    }
                    Row {
                        x: 14; anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: -3
                        spacing: 8
                        Rectangle {
                            visible: floor.modelData.focused
                            width: 6; height: 6; color: settings.accent
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: floor.empty ? "empty"
                                : floor.modelData.windows + (floor.modelData.windows === 1 ? " window" : " windows")
                                  + (floor.modelData.parked ? " · " + floor.modelData.parked + " parked" : "")
                            font.family: settings.sans; font.pixelSize: 12
                            font.weight: floor.modelData.focused ? Font.Medium : Font.Normal
                            color: floor.empty ? settings.ink4 : settings.ink
                        }
                    }
                }
            }
            Rectangle {         // the ground the building stands on, heavier than the storeys
                y: wsPage.floors.length * wsPage.storey - 6
                width: wsPage.planW + 24; x: -12; height: 2
                color: settings.ink
            }
            Text {
                y: wsPage.floors.length * wsPage.storey + 6
                text: "GROUND · workspace 1 at this level, and a workspace made when one is wanted"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: floors.length * storey + 40
        }

        // ---- 03 Window rules: the clauses ----
        // "Each clause matches windows as they open and decides where they go; clauses are read top to
        // bottom, and later ones override earlier ones." Drawn as a specification is drawn: numbered,
        // ruled between, the match on the left and what it decides on the right.
        Item {
            id: rulesPage
            visible: settings.page === "rules"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property var clauses: (settings.cfg && settings.cfg.rule) || []
            Repeater {
                model: rulesPage.clauses
                Item {
                    required property var modelData
                    required property int index
                    y: index * 56
                    width: rulesPage.width; height: 56
                    Rectangle { width: parent.width; height: 1; color: settings.line }
                    Text {
                        x: 0; y: 18
                        text: settings.twoDigits(index + 1)
                        font.family: settings.code; font.pixelSize: 11
                        color: settings.ink4
                    }
                    Text {
                        x: 38; y: 15
                        text: modelData.app_id ? "app_id ~ " + modelData.app_id
                            : modelData.title ? "title ~ " + modelData.title : "every window"
                        font.family: settings.code; font.pixelSize: 12
                        color: settings.ink
                    }
                    Text {
                        x: 38; y: 32
                        text: {
                            const says = []
                            for (const k of ["float", "sticky", "place", "focus", "pip"])
                                if (modelData[k] !== undefined) says.push(k + " " + modelData[k])
                            return says.join(" · ")
                        }
                        font.family: settings.sans; font.pixelSize: 12
                        color: settings.ink3
                    }
                }
            }
            Item {              // nothing written: say so, and say where it would go
                visible: rulesPage.clauses.length === 0
                width: parent.width; height: 120
                Rectangle {
                    anchors.fill: parent
                    color: settings.plan
                    border.width: 1; border.color: settings.line
                    Hatching { anchors.fill: parent; anchors.margins: 1; stroke: settings.hatch; pitch: 8 }
                }
                Column {
                    anchors.centerIn: parent; spacing: 6
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "NO CLAUSES"
                        font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                        color: settings.ink3
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "every window opens where the layout puts it"
                        font.family: settings.sans; font.pixelSize: 12
                        color: settings.ink4
                    }
                }
            }
            Text {
                y: rulesPage.clauses.length ? rulesPage.clauses.length * 56 + 14 : 134
                text: "[[rule]] in config.toml · read top to bottom, later clauses overriding earlier"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: (clauses.length ? clauses.length * 56 + 14 : 134) + 30
        }

        // ---- 05 Keyboard: one second of a key held ----
        // "The elevation shows one second of a key held down." Time runs along the baseline; the first
        // stroke is the press, then nothing until [input] repeat_delay is up, then a stroke at every
        // 1/repeat_rate. The whole of the keyboard's timing is in the drawing rather than in a number.
        Item {
            id: kbdPage
            visible: settings.page === "keyboard"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property real delay: Number(settings.cfgGet({ key: "input.repeat_delay", def: 200 }))
            readonly property real rate: Number(settings.cfgGet({ key: "input.repeat_rate", def: 25 }))
            readonly property real planW: Math.min(width - 72, 620)
            readonly property real second: 1000          // the elevation is one second wide

            Chain {
                id: kbdChain
                width: kbdPage.planW; height: 40
                face: settings.code
                marks: [0, 0.25, 0.5, 0.75, 1]
                labels: [{ text: "250 ms", strong: true }, { text: "500", strong: false },
                         { text: "750", strong: false }, { text: "1 s", strong: false }]
            }
            Item {
                id: kbdPlot
                y: kbdChain.height
                width: kbdPage.planW; height: 120
                Rectangle {
                    anchors.fill: parent
                    color: settings.plan
                    border.width: 1; border.color: settings.ink
                }
                Rectangle {     // the stretch before it repeats, hatched: nothing happens there
                    x: 1; y: 1
                    width: Math.max(0, kbdPage.delay / kbdPage.second * (kbdPlot.width - 2))
                    height: kbdPlot.height - 2
                    color: "transparent"
                    Hatching { anchors.fill: parent; stroke: settings.hatch; pitch: 8 }
                }
                Repeater {      // the press, then every repeat until the second is out
                    model: {
                        const out = [0]
                        const every = 1000 / Math.max(1, kbdPage.rate)
                        for (let t = kbdPage.delay; t <= kbdPage.second; t += every) out.push(t)
                        return out
                    }
                    Rectangle {
                        required property real modelData
                        required property int index
                        x: Math.round(modelData / kbdPage.second * (kbdPlot.width - 2)) + 1
                        y: index === 0 ? 10 : 34
                        width: index === 0 ? 2 : 1
                        height: kbdPlot.height - y - 10
                        color: index === 0 ? settings.accent : settings.ink
                    }
                }
                Text {
                    x: 10; y: 10
                    text: "PRESS"
                    font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                    color: settings.accent
                }
                Text {
                    x: Math.round(kbdPage.delay / kbdPage.second * (kbdPlot.width - 2)) + 10
                    y: 34
                    visible: x < kbdPlot.width - 90
                    text: "REPEATS"
                    font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                    color: settings.ink3
                }
            }
            Text {
                y: kbdChain.height + kbdPlot.height + 14
                text: kbdPage.delay + " ms held before it repeats, then " + kbdPage.rate + " a second · "
                      + (1 + Math.max(0, Math.floor((kbdPage.second - kbdPage.delay) / (1000 / Math.max(1, kbdPage.rate))) + 1))
                      + " strokes in the second drawn"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: kbdChain.height + kbdPlot.height + 14 + 30
        }

        // ---- 06 Pointer: the pad, drawn to its own size ----
        // "The plan maps what each part of the touchpad does." The pad is 114 by 58 mm on this
        // machine and is drawn at that proportion: the click areas where a press lands left, right or
        // middle, and the edges that scroll where edge scrolling is set.
        Item {
            id: padPage
            visible: settings.page === "pointer"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property string clickBy: String(settings.cfgGet({ key: "touchpad.click_method", def: "button-areas" }))
            readonly property string scrollBy: String(settings.cfgGet({ key: "touchpad.scroll_method", def: "two-finger" }))
            readonly property real padW: Math.min(width - 160, 420)
            readonly property real padH: padW * 58 / 114

            Chain {
                id: padChain
                width: padPage.padW; height: 40
                face: settings.code
                marks: [0, 1]
                labels: [{ text: "114 mm", strong: true }]
            }
            Item {
                id: padPlan
                y: padChain.height
                width: padPage.padW; height: padPage.padH
                Rectangle {
                    anchors.fill: parent
                    color: settings.sheetWhite
                    border.width: 1; border.color: settings.ink
                }
                // button areas: the foot of the pad divided left, middle, right
                Item {
                    anchors.fill: parent
                    visible: padPage.clickBy === "button-areas"
                    Rectangle {
                        y: parent.height * 0.75; width: parent.width; height: 1
                        color: settings.ink
                    }
                    Repeater {
                        model: [["left", 0, 0.4], ["middle", 0.4, 0.6], ["right", 0.6, 1]]
                        Item {
                            required property var modelData
                            x: modelData[1] * padPlan.width
                            y: padPlan.height * 0.75
                            width: (modelData[2] - modelData[1]) * padPlan.width
                            height: padPlan.height * 0.25
                            Rectangle {
                                visible: modelData[1] > 0
                                width: 1; height: parent.height; color: settings.ink
                            }
                            Text {
                                anchors.centerIn: parent
                                text: modelData[0]
                                font.family: settings.code; font.pixelSize: 11
                                color: settings.ink3
                            }
                        }
                    }
                }
                Text {      // clickfinger: the whole pad is the button, and the fingers decide
                    visible: padPage.clickBy === "clickfinger"
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 12 }
                    text: "one finger left · two right · three middle"
                    font.family: settings.code; font.pixelSize: 11
                    color: settings.ink3
                }
                // the scrolling edges, hatched as a drawing hatches a reserved strip
                Item {
                    visible: padPage.scrollBy === "edge"
                    x: padPlan.width - padPlan.width * 0.12; y: 1
                    width: padPlan.width * 0.12 - 1; height: padPlan.height * 0.75 - 1
                    Hatching { anchors.fill: parent; stroke: settings.hatch; pitch: 6 }
                    Rectangle { width: 1; height: parent.height; color: settings.line }
                }
                Text {
                    visible: padPage.scrollBy === "two-finger"
                    anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 14 }
                    text: "two fingers scroll, anywhere"
                    font.family: settings.code; font.pixelSize: 11
                    color: settings.ink3
                }
            }
            Text {
                x: padPage.padW + 16; y: padChain.height + padPage.padH / 2 - 7
                text: "58 mm"
                font.family: settings.code; font.pixelSize: 11
                color: settings.ink4
            }
            Text {
                y: padChain.height + padPage.padH + 14
                text: "ELAN0676 · the built-in pad, drawn to its own proportion"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: padChain.height + padH + 14 + 30
        }

        // ---- 07 Bindings: the board, and what is set on it ----
        // "Every combination, drawn on the keyboard it lives on. Choose a modifier layer, then a key."
        //
        // The positions are a standard board's, by evdev code (qml/Board.js): xkb's geometry is
        // vestigial and no layout in use says where its keys sit. The legends are the live keymap's,
        // so the board is drawn in the letters actually on it. The bindings are the compositor's own
        // table, which is the only honest account of what a key does (docs/keyboard.md).
        Item {
            id: bindPage
            visible: settings.page === "bindings"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            property string tier: "Mod"
            property int picked: -1                 // the evdev code of the key chosen, or none

            readonly property var kb: Compositor.keyboard
            readonly property var tiers: ["Mod", "Mod+Shift", "Mod+Ctrl", "Mod+Shift+Ctrl", "plain"]
            // every binding of the layer showing, by the key it ends on
            readonly property var onKey: {
                const out = {}
                for (const b of Compositor.keys) {
                    const parts = b.keys.split("+")
                    const held = parts.slice(0, -1).join("+") || "plain"
                    if (held !== bindPage.tier) continue
                    const last = parts[parts.length - 1]
                    ;(out[last] = out[last] || []).push(b)
                }
                return out
            }
            function boundTo(sym) { return sym && onKey[sym] ? onKey[sym] : [] }

            readonly property real boardW: Math.min(width, 760)
            readonly property real unit: boardW / (Board.UNITS + Board.GAP + Board.ARROW_UNITS)
            readonly property real keyH: unit

            // ---- the layer, picked as the canvas picks one ----
            Row {
                id: tierRow
                spacing: 0
                Repeater {
                    model: bindPage.tiers
                    Rectangle {
                        required property var modelData
                        required property int index
                        readonly property bool here: bindPage.tier === modelData
                        width: tierLabel.implicitWidth + 28; height: 38
                        color: here ? settings.ink : "transparent"
                        Rectangle { width: 1; height: parent.height; visible: index > 0; color: settings.ink }
                        Rectangle { y: 0; width: parent.width; height: 1; color: settings.ink }
                        Rectangle { y: parent.height - 1; width: parent.width; height: 1; color: settings.ink }
                        Rectangle { x: 0; width: 1; height: parent.height; visible: index === 0; color: settings.ink }
                        Rectangle { x: parent.width - 1; width: 1; height: parent.height
                                    visible: index === bindPage.tiers.length - 1; color: settings.ink }
                        Text {
                            id: tierLabel
                            anchors.centerIn: parent
                            text: modelData
                            font.family: settings.code; font.pixelSize: 12; font.weight: Font.Medium
                            color: parent.here ? settings.sheetWhite : settings.ink3
                        }
                        MouseArea {
                            anchors.fill: parent; cursorShape: Qt.PointingHandCursor
                            onPressed: { bindPage.tier = modelData; bindPage.picked = -1 }
                        }
                    }
                }
            }

            // ---- the board ----
            Item {
                id: plank
                y: tierRow.height + 28
                width: bindPage.boardW; height: 5 * bindPage.keyH + 4
                Repeater {
                    model: Board.rows()
                    Item {
                        id: boardRow
                        required property var modelData
                        required property int index
                        y: index * bindPage.keyH
                        width: plank.width; height: bindPage.keyH
                        Repeater {
                            model: boardRow.modelData
                            Rectangle {
                                id: cap
                                required property var modelData
                                required property int index
                                readonly property real before: {
                                    let u = 0
                                    for (let i = 0; i < index; i++) u += boardRow.modelData[i].w
                                    return u
                                }
                                readonly property string sym: Board.symbol(bindPage.kb, modelData)
                                readonly property var bound: modelData.mod ? [] : bindPage.boundTo(sym)
                                readonly property bool held: modelData.mod === "Mod"
                                    || (modelData.mod !== "" && bindPage.tier.indexOf(modelData.mod) >= 0)
                                readonly property bool chosen: bindPage.picked === modelData.code
                                x: before * bindPage.unit + 1
                                y: 1
                                width: modelData.w * bindPage.unit - 2
                                height: bindPage.keyH - 2
                                color: cap.chosen ? "#FFFFFF" : cap.held ? settings.ink
                                     : cap.bound.length ? settings.sheetWhite : "#EEEFEB"
                                border.width: cap.chosen ? 2 : 1
                                border.color: cap.chosen ? settings.accent : cap.held ? settings.ink
                                            : cap.bound.length ? "#8F938C" : settings.edge
                                Text {
                                    x: 5; y: 4
                                    text: Board.legend(bindPage.kb, cap.modelData)
                                    font.family: settings.code
                                    font.pixelSize: cap.modelData.w > 1.4 ? 10 : 12
                                    color: cap.held ? settings.bodyBg : cap.bound.length ? settings.ink : settings.ink4
                                }
                                Text {      // what it does, where there is room to say it
                                    visible: cap.bound.length > 0 && cap.modelData.w >= 1
                                    anchors { left: parent.left; right: parent.right; bottom: parent.bottom
                                              leftMargin: 5; rightMargin: 4; bottomMargin: 3 }
                                    elide: Text.ElideRight
                                    text: cap.bound.length > 1 ? cap.bound.length + "×"
                                        : cap.bound.length ? cap.bound[0].action : ""
                                    font.family: settings.sans; font.pixelSize: 9
                                    color: cap.bound.length > 1 ? "#B0441A" : settings.ink3
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    enabled: !cap.modelData.mod
                                    cursorShape: Qt.PointingHandCursor
                                    onPressed: bindPage.picked = bindPage.picked === cap.modelData.code ? -1 : cap.modelData.code
                                }
                            }
                        }
                    }
                }
                // the arrows, in the inverted T they are made in
                Repeater {
                    model: Board.arrows()
                    Item {
                        id: arrowRow
                        required property var modelData
                        required property int index
                        x: (Board.UNITS + Board.GAP) * bindPage.unit
                        y: (3 + index) * bindPage.keyH
                        width: Board.ARROW_UNITS * bindPage.unit; height: bindPage.keyH
                        Repeater {
                            model: arrowRow.modelData
                            Rectangle {
                                id: arrowCap
                                required property var modelData
                                required property int index
                                visible: !!modelData
                                readonly property string sym: modelData ? Board.symbol(bindPage.kb, modelData) : ""
                                readonly property var bound: modelData ? bindPage.boundTo(sym) : []
                                readonly property bool chosen: modelData && bindPage.picked === modelData.code
                                x: index * bindPage.unit + 1; y: 1
                                width: bindPage.unit - 2; height: bindPage.keyH - 2
                                color: chosen ? "#FFFFFF" : bound.length ? settings.sheetWhite : "#EEEFEB"
                                border.width: chosen ? 2 : 1
                                border.color: chosen ? settings.accent : bound.length ? "#8F938C" : settings.edge
                                Text {
                                    anchors.centerIn: parent
                                    text: arrowCap.modelData ? arrowCap.modelData.cap : ""
                                    font.family: settings.code; font.pixelSize: 12
                                    color: arrowCap.bound.length ? settings.ink : settings.ink4
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onPressed: bindPage.picked = bindPage.picked === arrowCap.modelData.code
                                                                 ? -1 : arrowCap.modelData.code
                                }
                            }
                        }
                    }
                }
            }

            // ---- what a key is set to ----
            // Picked on the board, set here. The catalogue is the compositor's own vocabulary
            // (compositor/src/config.rs parses exactly these), and the applications are the ones
            // installed, spawned by the command their .desktop file gives.
            readonly property var catalogue: [
                ["Focus", ["focus left", "focus right", "focus up", "focus down"]],
                ["Window", ["move left", "move right", "move up", "move down",
                            "resize left", "resize right", "resize up", "resize down",
                            "close", "fullscreen", "park", "park_prev", "park_next", "zoom"]],
                ["Workspace", ["workspace 1", "workspace 2", "workspace 3", "workspace 4", "workspace 5",
                               "send 1", "send 2", "send 3", "send 4", "send 5"]],
                ["System", ["screenshot", "session_save", "reload", "quit"]]
            ]
            readonly property var apps: DesktopEntries.applications.values
                .filter(e => !e.noDisplay)
                .sort((a, b) => a.name.localeCompare(b.name))
            readonly property var pickedKey: {
                if (bindPage.picked < 0) return null
                for (const row of Board.rows())
                    for (const k of row) if (k.code === bindPage.picked) return k
                for (const row of Board.arrows())
                    for (const k of row) if (k && k.code === bindPage.picked) return k
                return null
            }
            readonly property string chord: {
                if (!pickedKey) return ""
                const sym = Board.symbol(kb, pickedKey)
                return tier === "plain" ? sym : tier + "+" + sym
            }
            function setTo(action) {
                if (chord === "") return
                // config.toml is marked before the first write, as every other row does: Escape puts
                // the whole file back byte for byte (cli/src/config.rs)
                if (!settings.cfgMarked) { settings.queue(["config", "mark"], ""); settings.cfgMarked = true }
                settings.queue(["config", "set", 'bindings."' + chord + '"=' + action], "")
            }

            Item {
                id: chooser
                y: plank.y + plank.height + 40
                width: bindPage.width
                visible: bindPage.picked >= 0
                height: visible ? 420 : 0

                Rectangle { width: parent.width; height: 1; color: settings.line }
                Text {
                    id: chordName
                    y: 20
                    text: bindPage.chord
                    font.family: settings.code; font.pixelSize: 20; font.weight: Font.Medium
                    color: settings.ink
                }
                Text {
                    id: chordNow
                    y: 50
                    readonly property var has: bindPage.boundTo(bindPage.pickedKey ? Board.symbol(bindPage.kb, bindPage.pickedKey) : "")
                    text: has.length ? "set to " + has.map(b => b.action).join(" and ") : "not set"
                    font.family: settings.sans; font.pixelSize: 13
                    color: settings.ink3
                }
                Rectangle {     // take it off the key again
                    x: bindPage.width - width; y: 20
                    width: clearLabel.implicitWidth + 40; height: 38
                    visible: chordNow.has.length > 0
                    color: clearArea.containsMouse ? settings.ghostHover : "transparent"
                    border.width: 1; border.color: settings.firm
                    Text {
                        id: clearLabel
                        anchors.centerIn: parent; text: "Take it off"
                        font.family: settings.sans; font.pixelSize: settings.fRow
                        color: settings.ink
                    }
                    MouseArea {
                        id: clearArea
                        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onPressed: bindPage.setTo("none")
                    }
                }

                // the actions, by what they act on
                Row {
                    y: 86
                    spacing: 32
                    Repeater {
                        model: bindPage.catalogue
                        Column {
                            required property var modelData
                            width: 150
                            spacing: 2
                            Text {
                                text: modelData[0].toUpperCase()
                                font.family: settings.code; font.pixelSize: settings.fTag
                                font.letterSpacing: 0.9
                                color: settings.ink4
                                bottomPadding: 8
                            }
                            Repeater {
                                model: modelData[1]
                                Rectangle {
                                    required property var modelData
                                    readonly property bool now: chordNow.has.some(b => b.action === modelData)
                                    width: 150; height: 24
                                    color: now ? settings.ink : actArea.containsMouse ? settings.ghostHover : "transparent"
                                    Text {
                                        x: 6; anchors.verticalCenter: parent.verticalCenter
                                        text: parent.modelData
                                        font.family: settings.code; font.pixelSize: settings.fHint
                                        color: parent.now ? settings.sheetWhite : settings.ink
                                    }
                                    MouseArea {
                                        id: actArea
                                        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                        onPressed: bindPage.setTo(parent.modelData)
                                    }
                                }
                            }
                        }
                    }
                    // and an application, which is a spawn of whatever its .desktop file runs
                    Column {
                        width: 220
                        spacing: 2
                        Text {
                            text: "APPLICATION"
                            font.family: settings.code; font.pixelSize: settings.fTag
                            font.letterSpacing: 0.9
                            color: settings.ink4
                            bottomPadding: 8
                        }
                        Flickable {
                            width: 220; height: 280
                            clip: true
                            contentWidth: width
                            contentHeight: appList.height
                            boundsBehavior: Flickable.StopAtBounds
                            Column {
                                id: appList
                                width: 220
                                spacing: 2
                                Repeater {
                                    model: bindPage.apps
                                    Rectangle {
                                        required property var modelData
                                        readonly property string cmd: "spawn " + (modelData.command || [])
                                            .filter(a => !/^%[a-zA-Z]$/.test(a)).join(" ")
                                        readonly property bool now: chordNow.has.some(b => b.action === cmd)
                                        width: 220; height: 24
                                        color: now ? settings.ink : appArea.containsMouse ? settings.ghostHover : "transparent"
                                        Text {
                                            x: 6; anchors.verticalCenter: parent.verticalCenter
                                            width: 208; elide: Text.ElideRight
                                            text: parent.modelData.name
                                            font.family: settings.sans; font.pixelSize: settings.fHint
                                            color: parent.now ? settings.sheetWhite : settings.ink
                                        }
                                        MouseArea {
                                            id: appArea
                                            anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                            onPressed: bindPage.setTo(parent.cmd)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Text {      // which keyboard this is a board of
                y: plank.y + plank.height + 14
                text: {
                    const kb = bindPage.kb
                    if (!kb) return "no keymap from the compositor: a US board, until it can say"
                    const who = kb.device || "no keyboard device"
                    const lay = kb.layout ? " · " + kb.layout : ""
                    return who + lay + " · positions a standard board's, legends this keymap's"
                }
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: plank.y + plank.height + 14 + 30 + chooser.height
        }


        // ---- 09 Motion: progress against time ----
        // "The profile plots progress against time for one event; the programme sets every event on
        // the same clock." The curve is the easing the compositor animates with, drawn on axes, and
        // the programme is the events ranged under it against the same seconds.
        Item {
            id: motionPage
            visible: settings.page === "motion"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property bool on: settings.cfgGet({ key: "animations.enabled", def: true }) === true
            readonly property real ms: Number(settings.cfgGet({ key: "animations.duration_ms", def: 220 }))
            readonly property real plotW: Math.min(width - 160, 420)
            readonly property real plotH: 200

            Item {
                id: motionPlot
                width: motionPage.plotW; height: motionPage.plotH
                Rectangle { anchors.fill: parent; color: settings.plan; border.width: 1; border.color: settings.ink }
                Hatching { anchors.fill: parent; anchors.margins: 1; visible: !motionPage.on; stroke: settings.hatch; pitch: 8 }
                Canvas {        // the curve: ease-out cubic, which is what the shell animates with
                    id: curve
                    anchors.fill: parent; anchors.margins: 1
                    visible: motionPage.on
                    onWidthChanged: requestPaint()
                    onHeightChanged: requestPaint()
                    onPaint: {
                        const c = getContext("2d")
                        c.reset(); c.strokeStyle = "#2B45B5"; c.lineWidth = 2
                        c.beginPath()
                        for (let i = 0; i <= 60; i++) {
                            const t = i / 60, p = 1 - Math.pow(1 - t, 3)
                            const x = t * width, y = height - p * height
                            if (i === 0) c.moveTo(x, y); else c.lineTo(x, y)
                        }
                        c.stroke()
                    }
                }
                Text {
                    x: 10; y: 8
                    text: motionPage.on ? "EASE OUT" : "REDUCED"
                    font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                    color: motionPage.on ? settings.accent : settings.ink3
                }
                Text {
                    x: 10; anchors { bottom: parent.bottom; bottomMargin: 8 }
                    text: "0"
                    font.family: settings.code; font.pixelSize: 11
                    color: settings.ink4
                }
                Text {
                    anchors { right: parent.right; rightMargin: 10; bottom: parent.bottom; bottomMargin: 8 }
                    text: motionPage.on ? motionPage.ms + " ms" : "0 ms"
                    font.family: settings.code; font.pixelSize: 11
                    color: settings.ink4
                }
            }
            Text {
                x: -34; y: motionPage.plotH / 2 - 7
                rotation: -90
                text: "PROGRESS"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            Column {            // the programme: every event on the same clock
                x: motionPage.plotW + 56; y: 0
                spacing: 14
                Repeater {
                    model: [["Parking a window", 1], ["Unparking", 1], ["Switching workspace", 1], ["The settings opening", 0.7]]
                    Item {
                        required property var modelData
                        width: 260; height: 26
                        Text {
                            text: modelData[0]
                            font.family: settings.sans; font.pixelSize: 13
                            color: settings.ink
                        }
                        Rectangle {
                            y: 18; width: parent.width; height: 1; color: settings.line
                        }
                        Rectangle {
                            y: 16; height: 5
                            width: motionPage.on ? Math.max(2, modelData[1] * motionPage.ms / 420 * parent.width) : 2
                            color: settings.accent
                        }
                    }
                }
            }
            Text {
                y: motionPage.plotH + 14
                text: motionPage.on ? "every event eased out over " + motionPage.ms + " ms, measured against 420"
                                    : "reduced motion: every change arrives at once"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: plotH + 14 + 30
        }

        // ---- 10 Startup: the phases, climbing ----
        // "Phases climb in order; items in a phase start together, and a phase lasts as long as the
        // slowest item it waits for." Drawn as a section again: each phase a tread above the last.
        Item {
            id: startPage
            visible: settings.page === "startup"
            x: settings.mainX; y: settings.cfgTop
            width: settings.mainW
            readonly property var phases: [
                ["Compositor", ["the sheet, the frame, the zones"], true],
                ["Shell", ["the margins, the title block, the legend"], true],
                ["Session", settings.cfgGet({ key: "session.restore", def: true }) === true
                            ? ["the windows back in their places"] : ["nothing restored"],
                 settings.cfgGet({ key: "session.restore", def: true }) === true],
                ["Applications", settings.cfgGet({ key: "session.apps", def: true }) === true
                                 ? ["their own state: tabs, files, editors, tmux"] : ["started bare"],
                 settings.cfgGet({ key: "session.apps", def: true }) === true]
            ]
            readonly property real tread: 64
            readonly property real rise: 22

            Repeater {
                model: startPage.phases
                Item {
                    id: step
                    required property var modelData
                    required property int index
                    readonly property int fromTop: startPage.phases.length - 1 - index
                    x: index * 26
                    y: fromTop * startPage.tread
                    width: Math.min(startPage.width - index * 26 - 40, 460)
                    height: startPage.tread
                    Rectangle {
                        anchors.fill: parent; anchors.bottomMargin: 8
                        color: modelData[2] ? settings.sheetWhite : settings.plan
                        border.width: 1; border.color: settings.ink
                        Hatching {
                            anchors.fill: parent; anchors.margins: 1
                            visible: !step.modelData[2]
                            stroke: settings.hatch; pitch: 8
                        }
                    }
                    Text {
                        x: -24; y: 14
                        text: settings.twoDigits(index + 1)
                        font.family: settings.code; font.pixelSize: 11
                        color: settings.ink4
                    }
                    Text {
                        x: 14; y: 12
                        text: modelData[0]
                        font.family: settings.sans; font.pixelSize: 13; font.weight: Font.Medium
                        color: settings.ink
                    }
                    Text {
                        x: 14; y: 30
                        text: modelData[1].join(" · ")
                        font.family: settings.sans; font.pixelSize: 12
                        color: settings.ink3
                    }
                }
            }
            Rectangle {
                y: startPage.phases.length * startPage.tread - 8
                x: -12; width: Math.min(startPage.width, 520); height: 2
                color: settings.ink
            }
            Text {
                y: startPage.phases.length * startPage.tread + 6
                text: "LOGIN · each phase waits for the one below it"
                font.family: settings.code; font.pixelSize: 11; font.letterSpacing: 0.9
                color: settings.ink4
            }
            readonly property real deep: phases.length * tread + 40
        }

        // ---- the wallpaper page ----
        Item {
            id: wallPage
            anchors.fill: parent
            visible: settings.page === "appearance"
            // This page previews the picked picture behind the whole panel -- that is what it is for,
            // since BRIGHTNESS, PAPER WASH and BLUR are only judgeable against the real thing. So its
            // lettering has no fixed ground, and no one ink can sit on a photograph that is near-white
            // at the top and dark at the foot: an adaptive colour assumes a ground that does not vary,
            // and a picture's does (docs/design-research.md, on colour being relative).
            //
            // So the lettering takes paper under it and the picture shows around it -- the legend's
            // rule, which is settled: lettering always has paper. Three blocks, because there are
            // three places with words on this page.
            // below the shared heading and its rule, as every other page sits
            readonly property real topY: (pageHead.foot + 24)
            // the pictures, a grid of drawings as wide as the page allows
            readonly property real gap: 4 * settings.mm
            readonly property real rowH: settings.rowPitch       // a settings row, on the one ladder
            readonly property int cols: Math.max(1, Math.floor((settings.mainW + gap) / (36 * settings.mm + gap)))
            readonly property real picW: (settings.mainW - (cols - 1) * gap) / cols
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
                x: settings.mainX; y: wallPage.topY
                text: "PICTURES · " + (settings.wp ? settings.wp.dirs.join(", ") : "") + " (config.toml: wallpaper_dirs)"
                    + (wallPage.rowsTotal > wallPage.rowsShown
                       ? " · " + (wallPage.firstRow * wallPage.cols + 1) + "–" + Math.min(settings.wpPictures.length, (wallPage.firstRow + wallPage.rowsShown) * wallPage.cols)
                         + " OF " + settings.wpPictures.length
                       : "")
                    + (settings.wp && settings.wp.current && !settings.wp.exists ? " · yours is gone: " + settings.wp.current : "")
                font.family: settings.code; font.pixelSize: settings.fHint; font.letterSpacing: 0.2 * settings.mm
                color: settings.wpRow === 0 ? settings.ink : settings.ink3
            }
            Flickable {
                id: picFlick
                // a millimetre around the pictures, for the picked one's heavier frame
                readonly property real pad: 1 * settings.mm
                x: settings.mainX - pad; y: wallPage.topY + 6 * settings.mm - pad
                width: settings.mainW + 2 * pad
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
                                font.family: settings.code; font.pixelSize: settings.font * 0.8; font.letterSpacing: 0.2 * settings.mm
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
                            border.color: pic.chosen ? settings.ink : settings.ink4
                        }
                        Text {
                            anchors { left: parent.left; top: parent.bottom; topMargin: 1.2 * settings.mm }
                            width: parent.width; elide: Text.ElideMiddle
                            text: pic.path ? pic.path.split("/").pop() : "none"
                            font.family: settings.code; font.pixelSize: settings.font * 0.7; font.bold: pic.chosen
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
                    x: settings.mainX
                    y: wallPage.topY + 6 * settings.mm + wallPage.rowsView * wallPage.rowStep + 2 * settings.mm + index * wallPage.rowH
                    width: settings.mainW; height: wallPage.rowH
                    // the row the keys are on, marked once, in the one accent on the sheet
                    Rectangle {
                        visible: wr.here
                        x: -16; y: (parent.height - 18) / 2
                        width: 2; height: 18
                        color: settings.accent
                    }
                    Text {
                        y: (wallPage.rowH - height) / 2
                        text: wr.modelData.name
                        font.family: settings.code; font.pixelSize: settings.fRow
                        color: wr.here ? settings.ink : settings.ink3
                    }
                    Row {          // the values: alignment holds them, so nothing is drawn to
                        x: 44 * settings.mm; y: 0; height: parent.height
                        spacing: 6 * settings.mm
                    Repeater {
                        model: wr.modelData.options
                        Item {
                            id: wo
                            required property var modelData
                            required property int index
                            readonly property bool chosen: settings.wpVals[wr.modelData.key] === modelData[0]
                            width: optLabel.implicitWidth; height: wallPage.rowH
                            Text {
                                id: optLabel
                                y: (parent.height - height) / 2
                                text: wo.modelData[1]
                                font.family: settings.code; font.pixelSize: settings.fRow
                                color: wo.chosen ? settings.ink : settings.ink4
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
            // the Text page: the faces at the sizes picked, at true size, above the rows
            Row {
                visible: settings.page === "text"
                x: settings.mainX; y: (pageHead.foot + 24)
                width: settings.mainW; height: cfgPage.bandH - 6 * settings.mm
                spacing: 8 * settings.mm
                Repeater {
                    model: [
                        { which: "interface", face: "text.interface", size: "text.size", title: "AS IT READS · INTERFACE",
                          sample: "The quick sketch fits the sheet: every word at 4.5:1, every letter a fifth of a degree tall at the distance you sit." },
                        { which: "monospace", face: "text.monospace", size: "text.mono_size", title: "MONOSPACE",
                          sample: "fn zones(len: i32) -> u32 {\n    (len as f64 / 50.0).round() as u32\n}" }
                    ]
                    Column {
                        id: sample
                        required property var modelData
                        readonly property real pt: Number(settings.cfgGet(settings.textRow(modelData.size)))
                        width: (settings.mainW - 8 * settings.mm) / 2
                        spacing: 2 * settings.mm
                        Text {
                            text: sample.modelData.title + ", " + sample.pt + " PT"
                            font.family: settings.code; font.pixelSize: settings.font * 0.75; font.bold: true; font.letterSpacing: 0.2 * settings.mm
                            color: settings.ink
                        }
                        Text {
                            width: parent.width
                            text: sample.modelData.sample
                            wrapMode: sample.modelData.which === "interface" ? Text.WordWrap : Text.NoWrap
                            // points as the applications letter them: 96 to the inch, in the display's logical pixels
                            font.family: settings.cfgGet(settings.textRow(sample.modelData.face))
                            font.pixelSize: sample.pt * 96 / 72
                            color: settings.ink
                        }
                    }
                }
            }
            // the Displays page: the displays as arranged at the scales picked, to scale, above the rows
            readonly property bool arranging: settings.page === "displays" && settings.dispOutputs.length > 0
            readonly property real bandH: arranging ? 52 * settings.mm : settings.page === "text" ? 40 * settings.mm : 0
            Item {
                id: arrangement
                visible: cfgPage.arranging
                x: settings.mainX; y: (pageHead.foot + 24)
                width: settings.mainW; height: cfgPage.bandH - 6 * settings.mm
                readonly property var lay: settings.page === "displays" ? settings.dispLayout() : []
                readonly property real spanW: Math.max(1, ...lay.map(d => d.x + d.w))
                readonly property real spanH: Math.max(1, ...lay.map(d => d.y + d.h))
                readonly property real k: Math.min(width / spanW, height / spanH)
                readonly property real offsetX: (width - spanW * k) / 2
                Repeater {
                    model: arrangement.lay
                    Rectangle {
                        id: box
                        required property var modelData
                        readonly property var o: modelData.o
                        // the display the row picked is about, in heavier ink
                        readonly property bool here: (settings.cfgRows[settings.cfgRow] || {}).output === modelData.name
                        x: arrangement.offsetX + modelData.x * arrangement.k
                        y: modelData.y * arrangement.k
                        width: modelData.w * arrangement.k; height: modelData.h * arrangement.k
                        color: here ? settings.inkA(0.12) : "transparent"
                        border.width: here ? Math.max(2, 0.6 * settings.mm) : Math.max(1, 0.25 * settings.mm)
                        border.color: settings.ink
                        Behavior on x { enabled: settings.shell.theme.animations; NumberAnimation { duration: 160 } }
                        Behavior on y { enabled: settings.shell.theme.animations; NumberAnimation { duration: 160 } }
                        Column {
                            x: 2 * settings.mm; y: 1.6 * settings.mm
                            width: parent.width - 4 * settings.mm
                            spacing: 0.6 * settings.mm
                            clip: true
                            Text {
                                width: parent.width; elide: Text.ElideRight
                                text: box.modelData.name + (box.o.model && box.o.model !== "Unknown" ? " · " + box.o.model : "")
                                font.family: settings.code; font.pixelSize: settings.font * 0.8; font.bold: true
                                color: settings.ink
                            }
                            Text {
                                readonly property real inches: Math.hypot(box.o.physical[0], box.o.physical[1]) / 25.4
                                width: parent.width; elide: Text.ElideRight
                                text: (inches > 0 ? inches.toFixed(1) + "″ · " : "") + box.o.mode[0] + " × " + box.o.mode[1]
                                font.family: settings.code; font.pixelSize: settings.font * 0.68
                                color: settings.ink3
                            }
                            Text {
                                width: parent.width; elide: Text.ElideRight
                                text: (box.o.px_per_mm * box.o.scale).toFixed(1) + " px/mm · at " + settings.dispScale(box.modelData.name)
                                      + " · " + box.modelData.w + " × " + box.modelData.h
                                font.family: settings.code; font.pixelSize: settings.font * 0.68
                                color: settings.ink3
                            }
                        }
                    }
                }
            }
            // The canvas's rows: a grid of label-and-hint against value-and-control, 16 px of padding
            // and a rule above each. The control is chosen by what the value *is* -- a switch for the
            // two-state ones, a segmented group for a short list, a slider for an ordered magnitude --
            // which is the canvas's own vocabulary and also the right reading of the data.
            Repeater {
                model: settings.cfgRows
                Item {
                    id: cr
                    required property var modelData
                    required property int index
                    readonly property bool here: settings.cfgRow === index
                    readonly property var value: settings.cfgGet(modelData)
                    readonly property var opts: settings.cfgOptions(modelData)
                    readonly property bool isSwitch: opts.length === 2 && opts.every(o => typeof o[0] === "boolean")
                    readonly property bool isSlider: !!modelData.scale
                    readonly property string said: (opts.find(o => o[0] === cr.value) || ["", ""])[1]
                    x: settings.mainX
                    y: settings.cfgTop + settings.figureDeep + index * settings.rowPitch
                    width: settings.mainW; height: settings.rowPitch

                    Rectangle { width: parent.width; height: 1; color: settings.line }
                    Rectangle {     // the row the keys are on: a mark in the gutter, in the accent
                        visible: cr.here
                        x: -16; y: (parent.height - 18) / 2
                        width: 2; height: 18
                        color: settings.accent
                    }
                    Text {
                        id: rowName
                        x: 0; y: (parent.height - rowName.height - rowHint.height - 3) / 2
                        text: settings.sentence(cr.modelData.name)
                        font.family: settings.sans; font.pixelSize: settings.fRow; font.weight: Font.Medium
                        color: settings.ink
                    }
                    Text {
                        id: rowHint
                        y: rowName.y + rowName.height + 3
                        width: Math.min(parent.width * 0.55, 460); elide: Text.ElideRight
                        text: cr.modelData.note || ""
                        font.family: settings.sans; font.pixelSize: settings.fHint
                        color: settings.ink3
                    }

                    // ---- the value, and the control for it ----
                    Row {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        spacing: 14
                        Text {      // what it is set to, in the mono face, as the canvas letters it
                            visible: cr.isSwitch || cr.isSlider
                            anchors.verticalCenter: parent.verticalCenter
                            text: cr.said
                            font.family: settings.code; font.pixelSize: settings.fHint
                            color: settings.ink3
                        }
                        // a slider, for an ordered magnitude: a 1 px track and a 12 px square thumb
                        Item {
                            visible: cr.isSlider
                            width: 180; height: settings.navH
                            anchors.verticalCenter: parent.verticalCenter
                            readonly property int at: Math.max(0, cr.opts.findIndex(o => o[0] === cr.value))
                            readonly property real step: cr.opts.length > 1 ? (width - 12) / (cr.opts.length - 1) : 0
                            Rectangle {
                                y: (parent.height - 1) / 2
                                x: 6; width: parent.width - 12; height: 1
                                color: settings.ink
                            }
                            Rectangle {
                                x: parent.at * parent.step; y: (parent.height - 12) / 2
                                width: 12; height: 12
                                color: settings.fieldBg
                                border.width: 1.5; border.color: settings.ink
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onPressed: e => {
                                    settings.cfgRow = cr.index
                                    const i = Math.round((e.x - 6) / Math.max(1, parent.step))
                                    const v = cr.opts[Math.max(0, Math.min(cr.opts.length - 1, i))]
                                    if (v && v[0] !== cr.value) settings.cfgSet(cr.modelData, v[0])
                                }
                            }
                        }
                        // a switch, for the two-state ones: 44 by 24, square, a 16 px knob
                        Rectangle {
                            visible: cr.isSwitch
                            anchors.verticalCenter: parent.verticalCenter
                            width: 44; height: 24
                            color: cr.value === true ? settings.ink : "transparent"
                            border.width: 1; border.color: settings.ink
                            Rectangle {
                                y: 3; x: cr.value === true ? parent.width - 3 - width : 3
                                width: 16; height: 16
                                color: cr.value === true ? settings.sheetWhite : settings.ink
                                Behavior on x { enabled: settings.shell.theme.animations; NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onPressed: {
                                    settings.cfgRow = cr.index
                                    const other = cr.opts.find(o => o[0] !== cr.value)
                                    if (other) settings.cfgSet(cr.modelData, other[0])
                                }
                            }
                        }
                        // everything else: a segmented group, one border around the lot
                        Row {
                            visible: !cr.isSwitch && !cr.isSlider
                            anchors.verticalCenter: parent.verticalCenter
                            Repeater {
                                model: cr.isSwitch || cr.isSlider ? [] : cr.opts
                                Rectangle {
                                    id: seg
                                    required property var modelData
                                    required property int index
                                    readonly property bool chosen: cr.value === modelData[0]
                                    // a size below the legibility target: lettered in the alert ink, still to be picked
                                    readonly property bool low: !!cr.modelData.legible && !settings.legibility(modelData[0], cr.modelData.legible).ok
                                    width: segLabel.implicitWidth + 32; height: 38
                                    color: seg.chosen ? settings.ink : segArea.containsMouse ? settings.ghostHover : "transparent"
                                    Rectangle { width: 1; height: parent.height; visible: seg.index > 0; color: settings.ink }
                                    Rectangle { y: 0; width: parent.width; height: 1; color: settings.ink }
                                    Rectangle { y: parent.height - 1; width: parent.width; height: 1; color: settings.ink }
                                    Rectangle { x: 0; width: 1; height: parent.height; visible: seg.index === 0; color: settings.ink }
                                    Rectangle { x: parent.width - 1; width: 1; height: parent.height
                                                visible: seg.index === cr.opts.length - 1; color: settings.ink }
                                    Text {
                                        id: segLabel
                                        anchors.centerIn: parent
                                        text: seg.modelData[1]
                                        font.family: cr.modelData.face ? seg.modelData[0] : Fonts.mono
                                        font.pixelSize: settings.fHint; font.weight: Font.Medium
                                        color: seg.low ? "#C4501F" : seg.chosen ? settings.sheetWhite : settings.ink3
                                    }
                                    MouseArea {
                                        id: segArea
                                        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                        onPressed: { settings.cfgRow = cr.index; if (!seg.chosen) settings.cfgSet(cr.modelData, seg.modelData[0]) }
                                    }
                                }
                            }
                        }
                    }
                    MouseArea {    // the row's name picks the row
                        width: parent.width * 0.5; height: parent.height
                        onPressed: settings.cfgRow = cr.index
                    }
                }
            }
        }

        // ---- the colour scheme page ----
        Item {
            anchors.fill: parent
            visible: settings.page === "appearance"

        // the drawings
        Text {
            x: settings.mainX; y: pageHead.foot + 24
            text: "THEMES"
            font.family: settings.code; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.3 * settings.mm; font.bold: settings.row === 0
            color: settings.row === 0 ? settings.ink : settings.ink3
        }
        Repeater {
            model: settings.cards.length           // a count, so delegates stay and only a changed drawing repaints
            Item {
                id: c
                required property int index
                readonly property var modelData: settings.cards[index]
                readonly property bool picked: index === settings.sheetWhite
                x: settings.mainX + index * (settings.cardW + settings.cardGap)
                y: pageHead.foot + 24 + 22
                width: settings.cardW; height: settings.cardH
                Desktop { anchors.fill: parent; t: c.modelData }
                Rectangle {
                    readonly property real w: c.picked ? Math.max(2, 0.7 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                    anchors.fill: parent; anchors.margins: -w
                    color: "transparent"; border.width: w
                    border.color: c.picked ? settings.ink : settings.ink3
                }
                Text {
                    id: name
                    anchors { left: parent.left; top: parent.bottom; topMargin: 2 * settings.mm }
                    width: parent.width; elide: Text.ElideRight
                    text: c.modelData.label.toUpperCase()
                    font.family: settings.code; font.pixelSize: settings.font * 0.9; font.letterSpacing: 0.2 * settings.mm; font.bold: c.picked
                    color: c.picked ? settings.ink : settings.ink3
                }
                Text {          // its contrasts, or how many words it leaves unreadable
                    anchors { left: parent.left; top: name.bottom; topMargin: 0.8 * settings.mm }
                    readonly property var f: c.modelData.figures || {}
                    text: f.below > 0 ? "⚠ " + f.below + " below 4.5:1"
                        : "ink " + f.ink + " · text " + f.text + " · term " + f.terminal
                    font.family: settings.code; font.pixelSize: settings.font * 0.75
                    color: f.below > 0 ? board.t ? board.t.alert : "red" : settings.ink3
                }
                MouseArea {
                    anchors.fill: parent
                    // a drawing is picked, and only picked: pressing the one already picked used to
                    // keep it and close, which is a second way out by pointer and the same surprise
                    // as the catch-all above. Return keeps (docs: the hint along the top margin).
                    onPressed: { settings.row = 0; settings.pickCard(c.index) }
                }
            }
        }

        // the builder
        Item {
            id: builder
            x: settings.mainX
            y: settings.builderY
            width: settings.mainW
            Text {
                text: "BUILD A SCHEME · the last drawing, every word readable"
                font.family: settings.code; font.pixelSize: settings.font * 0.85; font.letterSpacing: 0.3 * settings.mm
                font.bold: settings.row > 0
                color: settings.row > 0 ? settings.ink : settings.ink3
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
                        font.family: settings.code; font.pixelSize: settings.font * 0.85; font.bold: r.here
                        color: r.here ? settings.ink : settings.ink3
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
                                    font.family: settings.code; font.pixelSize: parent.height * 0.6
                                    color: o.modelData.colour
                                }
                            }
                            Text {
                                anchors { left: parent.left; top: sw.bottom; topMargin: 0.6 * settings.mm }
                                text: o.modelData.label || o.modelData.value
                                font.family: settings.code; font.pixelSize: settings.font * 0.75; font.bold: o.chosen
                                color: o.chosen ? settings.ink : settings.inkA(0.7)
                            }
                            Text {
                                anchors { right: parent.right; top: sw.bottom; topMargin: 0.6 * settings.mm }
                                text: o.modelData.ratio + ":1"
                                font.family: settings.code; font.pixelSize: settings.font * 0.65
                                color: settings.ink4
                            }
                            Rectangle {     // the chosen value, outlined in heavier ink when the row is the one moving
                                visible: o.chosen
                                anchors.fill: parent; anchors.margins: -0.8 * settings.mm
                                color: "transparent"
                                border.width: r.here ? Math.max(2, 0.6 * settings.mm) : Math.max(1, 0.2 * settings.mm)
                                border.color: r.here ? settings.ink : settings.ink3
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
    }
}
