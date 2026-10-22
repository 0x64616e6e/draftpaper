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
    readonly property var pageList: [["scheme", "COLOUR SCHEME"], ["wallpaper", "WALLPAPER"], ["displays", "DISPLAYS"], ["text", "TEXT"], ["sheet", "SHEET"], ["rulers", "RULERS"],
                                     ["session", "SESSION"], ["input", "INPUT"], ["touchpad", "TOUCHPAD"], ["touch", "TOUCH"],
                                     ["idle", "IDLE · LOCK"]]
    // each row: config.toml's key (dotted: table.key), its name, the values offered ([value, label]), the
    // value when config.toml has none, and a note; `draftpaper config set` writes it, comments kept
    readonly property var cfgPages: ({
        displays: settings.dispRows,
        text: settings.textRows,
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
            { key: "sheet.schedule_at", zone: "row", name: "SCHEDULES · ZONE ROW", def: "auto", note: "Mod+i; auto: the top left, 10 mm in; or drag one",
              options: settings.zoneRows(true) },
            { key: "sheet.schedule_at", zone: "col", name: "SCHEDULES · ZONE COLUMN", def: "auto", note: "",
              options: settings.zoneCols() },
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
            { key: "input.repeat_rate", name: "REPEAT RATE", def: 25,
              note: "how fast a held key repeats once it has started",
              options: [[20, "20/s"], [25, "25/s"], [30, "30/s"], [40, "40/s"]] },
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
        touchpad: [
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
        touch: settings.touchRows,
        idle: [
            { key: "idle.screens_off", name: "SCREENS OFF AFTER", def: 15, note: "any input wakes them",
              options: [[5, "5 min"], [10, "10 min"], [15, "15 min"], [30, "30 min"], [0, "never"]] },
            { key: "idle.lock_after", name: "LOCK AFTER", def: 0, note: "Mod+Shift+x locks now",
              options: [[0, "never"], [5, "5 min"], [10, "10 min"], [15, "15 min"]] },
            { key: "idle.lock_on_sleep", name: "LOCK BEFORE SUSPEND", def: false, note: "Mod+Shift+p: for the day",
              options: [[false, "off"], [true, "on"]] }
        ]
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
        scheme: "The drawing the sheet is made in: its paper, its ink and what it lights things with. Pick one, or build your own from the rows below it.",
        wallpaper: "The picture behind the frame, and how much of the paper is left in it. The sheet redraws as you pick; nothing is kept until you apply.",
        displays: "Every display this machine has, where it sits beside the others, and how large the sheet is drawn on each.",
        text: "The faces the sheet is lettered in, and their sizes against the distance you sit from it.",
        sheet: "The frame, the title block, and what the sheet carries inside them.",
        rulers: "The two lines that cross the sheet at the pointer, and where their figures are read.",
        session: "What comes back when you log in: the windows that were open, and where they were.",
        input: "The keyboard's repeat, and what a mouse or the TrackPoint does.",
        touchpad: "The built-in pad: what a tap means, which way the content moves, and how far a finger carries the pointer.",
        touch: "The touchscreen, and which display a finger is touching.",
        idle: "What happens when the machine is left alone, and what it takes to get back in."
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
        // keepBlock false: the settings are the sheet while they are up. The board paints the whole
        // of the frame's interior, title block included, and only the margins stay showing -- the
        // panel is one surface rather than a thing arranged around the furniture underneath it.
        id: board; out: settings.out; t: settings.picked; keepBlock: false
        wallpaper: settings.wp ? (settings.wpPicture ? "file://" + settings.wpPicture : "") : settings.wallpaper
    }

    // The settings' own paper: the theme's, opaque, over the whole of the frame's interior and no
    // further -- the margins stay the sheet's. The board under this shows the wallpaper blurred, and
    // lettering laid straight onto a photograph has no fixed contrast with it: a ground that varies
    // makes every opacity mean something different from place to place (docs/design-research.md, on
    // colour being relative). It is the same reason the legend's paper is opaque.
    //
    // Every page, the wallpaper page included. It had an exception so the picked picture could be
    // previewed full-bleed behind the panel, and the exception cost three opaque blocks of different
    // sizes laid over the lettering to make it readable again -- which is a box used to solve a
    // contrast problem, and became the loudest thing on the page. One paper, no exception. The
    // pictures are the thumbnails; what the treatments do to the sheet is seen when the settings
    // close.
    Rectangle {
        x: board.inX; y: board.inY; width: board.inW; height: board.inH
        color: board.t && board.t.paper ? board.t.paper : settings.shell.theme.paper
    }

    // ---- the layout ----
    // Built to the Window Manager Settings canvas. Its sizes are CSS pixels at a 1440-wide artboard
    // and are used here as logical pixels, which is the one place on the sheet that is not measured
    // in whole millimetres (docs/interface.md): the settings are the shell's alone, and the canvas
    // is a pixel drawing.
    //
    // Its colours are roles, not hexes. Page one of this panel picks the theme, so a panel fixed at
    // the canvas's light greys would stay light while you chose a dark drawing. #F3F3F0 is the
    // paper, #EAEBE7 the chrome's shade, #FBFBF9 a field, #16171A the ink and #2B45B5 whatever the
    // theme lights things with.
    readonly property real dpr: board.dpr
    readonly property color ink: board.ink
    function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }
    readonly property color accent: board.t && board.t.strong ? board.t.strong : shell.theme.accent
    readonly property color paper: board.t && board.t.paper ? board.t.paper : shell.theme.paper
    readonly property color chrome: board.t && board.t.paper_shade ? board.t.paper_shade : inkA(0.06)
    readonly property color field: board.t && board.t.paper_light ? board.t.paper_light : inkA(0.03)
    readonly property color line: inkA(0.18)            // #D3D5CF, the ordinary rule
    readonly property color edge: inkA(0.3)             // #C9CCC5, a field's border
    readonly property color firm: inkA(0.62)            // #B9BCB5, a button's border
    function hover(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }

    // The canvas letters its accent on its page: #2B45B5 on #F3F3F0, which is a long way apart. A
    // theme's `strong` is what it lights things with and carries no such promise -- on softglow it is
    // a pale amber that disappears on the paper. So the accent is used where it reads and the ink
    // stands in where it does not. Colour is relative; a token is not a contrast
    // (docs/design-research.md).
    function lum(c) {
        const f = v => { const s = v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); return s }
        return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
    }
    function apart(a, b) {
        const x = lum(a), y = lum(b)
        return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
    }
    // the accent where it is legible on the panel's paper, the ink where it is not
    readonly property color mark: apart(accent, paper) >= 3 ? accent : inkA(0.8)

    // type, as the canvas sets it
    readonly property real fTitle: 40          // the page's name
    readonly property real fLead: 15           // its description
    readonly property real fRow: 14            // a row's label, a nav item, a button
    readonly property real fSmall: 13          // the search field
    readonly property real fHint: 12           // a hint, a mono value, the status
    readonly property real fTag: 11            // an uppercase mono label, a nav number

    // the frame's interior is the app; the margins stay the sheet's
    readonly property real appX: board.inX
    readonly property real appY: board.inY
    readonly property real appW: board.inW
    readonly property real appH: board.inH
    readonly property real barH: 64            // the header
    readonly property real sideW: Math.min(240, Math.round(appW * 0.22))
    readonly property real navH: 44            // a nav item, a button: the canvas's touch target
    readonly property real padX: 56            // the content's padding
    readonly property real padTop: 48
    // inside the scroller: the column is placed, the content is laid out from its own corner
    readonly property real mainX: padX
    readonly property real mainW: appW - sideW - 2 * padX
    readonly property real mainY: padTop
    readonly property real rowPitch: 68        // 16 px of padding above and below a two-line row
    // how deep the page showing is, so the scroller knows how far it goes
    readonly property real pageDeep: cfgRows.length ? cfgTop + cfgRows.length * rowPitch + 64
        : page === "scheme" ? builderY + rows.length * rowH + 64 : 0
    // where a page's content begins: under the head, wherever the head's description leaves it
    readonly property real cfgTop: pageHead.foot + 24 + (cfgPage.bandH || 0)

    // kept for the pages that still set themselves out their own way (the drawings, the wallpaper
    // grid, the displays plan): these are their numbers, not the canvas's
    readonly property real gutter: 8 * mm
    readonly property real font: 2.8 * mm
    readonly property real cardGap: 5 * mm
    readonly property real cardW: Math.min(70 * mm, (mainW - (cards.length - 1) * cardGap) / Math.max(1, cards.length))
    readonly property real cardH: cardW * height / Math.max(1, width)
    readonly property real builderY: cfgTop + 22 + cardH + 40
    readonly property real rowH: Math.max(8 * mm, Math.min(12 * mm, (appH - barH - 64 - (builderY - cfgTop) ) / rows.length))
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
                    font.family: Fonts.iface; font.pixelSize: 15; font.weight: Font.DemiBold
                    font.letterSpacing: -0.15
                    color: settings.ink
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Settings"
                    font.family: Fonts.mono; font.pixelSize: settings.fHint
                    color: settings.inkA(0.55)
                }
            }

            Row {
                anchors { right: parent.right; rightMargin: 32; verticalCenter: parent.verticalCenter }
                spacing: 12
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    rightPadding: 8
                    text: settings.dirty ? "Unapplied changes" : "Up to date"
                    font.family: Fonts.mono; font.pixelSize: settings.fHint
                    color: settings.dirty ? settings.mark : settings.inkA(0.55)
                }
                Rectangle {     // Revert
                    width: revertLabel.implicitWidth + 40; height: settings.navH
                    color: revertArea.containsMouse && settings.dirty ? settings.hover(0.06) : "transparent"
                    border.width: 1; border.color: settings.firm
                    opacity: settings.dirty ? 1 : 0.45
                    Text {
                        id: revertLabel
                        anchors.centerIn: parent; text: "Revert"
                        font.family: Fonts.iface; font.pixelSize: settings.fRow; font.weight: Font.Medium
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
                        font.family: Fonts.iface; font.pixelSize: settings.fRow; font.weight: Font.Medium
                        color: settings.dirty ? settings.paper : settings.ink
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
                color: settings.field
                border.width: 1; border.color: settings.edge
                Canvas {
                    id: lens
                    width: 14; height: 14; x: 12; anchors.verticalCenter: parent.verticalCenter
                    onPaint: {
                        const c = getContext("2d")
                        c.reset(); c.strokeStyle = settings.inkA(0.6); c.lineWidth = 1.4
                        c.beginPath(); c.arc(6, 6, 4.5, 0, 2 * Math.PI); c.stroke()
                        c.beginPath(); c.moveTo(9.4, 9.4); c.lineTo(13, 13); c.stroke()
                    }
                    Connections { target: settings; function onInkChanged() { lens.requestPaint() } }
                }
                TextInput {
                    id: search
                    anchors { left: parent.left; leftMargin: 34; right: parent.right; rightMargin: 10
                              verticalCenter: parent.verticalCenter }
                    font.family: Fonts.iface; font.pixelSize: settings.fSmall
                    color: settings.ink
                    selectByMouse: true
                    onTextChanged: settings.find = text
                    Text {
                        anchors.fill: parent; verticalAlignment: Text.AlignVCenter
                        visible: search.text === ""
                        text: "Search settings"
                        font: search.font; color: settings.inkA(0.5)
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
                        color: pg.here ? settings.field : navArea.containsMouse ? settings.hover(0.05) : "transparent"
                        border.width: 1; border.color: pg.here ? settings.line : "transparent"
                        Text {
                            x: 12; anchors.verticalCenter: parent.verticalCenter
                            text: pg.modelData[2]
                            font.family: Fonts.mono; font.pixelSize: settings.fTag
                            color: pg.here ? settings.mark : settings.inkA(0.55)
                        }
                        Text {
                            x: 12 + 16 + 16; anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 8; elide: Text.ElideRight
                            text: settings.sentence(pg.modelData[1])
                            font.family: Fonts.iface; font.pixelSize: settings.fRow
                            font.weight: pg.here ? Font.Medium : Font.Normal
                            color: pg.here ? settings.ink : settings.inkA(0.82)
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
                    font.family: Fonts.mono; font.pixelSize: settings.fTag; font.letterSpacing: 0.9
                    color: settings.inkA(0.55)
                }
                Text {
                    width: parent.width; wrapMode: Text.WrapAnywhere
                    text: "~/.config/draftpaper/config.toml"
                    font.family: Fonts.mono; font.pixelSize: settings.fHint
                    color: settings.ink
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
                font.family: Fonts.mono; font.pixelSize: settings.fHint; font.weight: Font.Medium
                font.letterSpacing: 0.9
                color: settings.mark
            }
            Text {
                id: pageName
                y: pageCount.height + 12
                text: settings.sentence(pageHead.row[1])
                font.family: Fonts.iface; font.pixelSize: settings.fTitle; font.weight: Font.Medium
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
                font.family: Fonts.iface; font.pixelSize: settings.fLead
                color: settings.inkA(0.75)
            }
            Rectangle {
                y: pageLead.y + pageLead.height + 32
                width: parent.width; height: 1; color: settings.line
            }
            readonly property real foot: y + pageLead.y + pageLead.height + 32 + 1
        }


        // ---- the wallpaper page ----
        Item {
            id: wallPage
            anchors.fill: parent
            visible: settings.page === "wallpaper"
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
                font.family: Fonts.mono; font.pixelSize: settings.fHint; font.letterSpacing: 0.2 * settings.mm
                color: settings.wpRow === 0 ? settings.ink : settings.inkA(0.6)
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
                    x: settings.mainX
                    y: wallPage.topY + 6 * settings.mm + wallPage.rowsView * wallPage.rowStep + 2 * settings.mm + index * wallPage.rowH
                    width: settings.mainW; height: wallPage.rowH
                    // the row the keys are on, marked once, in the one accent on the sheet
                    Rectangle {
                        visible: wr.here
                        x: -16; y: (parent.height - 18) / 2
                        width: 2; height: 18
                        color: settings.mark
                    }
                    Text {
                        y: (wallPage.rowH - height) / 2
                        text: wr.modelData.name
                        font.family: Fonts.mono; font.pixelSize: settings.fRow
                        color: wr.here ? settings.ink : settings.inkA(0.62)
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
                                font.family: Fonts.mono; font.pixelSize: settings.fRow
                                color: wo.chosen ? settings.ink : settings.inkA(0.34)
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
                            font.family: Fonts.mono; font.pixelSize: settings.font * 0.75; font.bold: true; font.letterSpacing: 0.2 * settings.mm
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
                                font.family: Fonts.mono; font.pixelSize: settings.font * 0.8; font.bold: true
                                color: settings.ink
                            }
                            Text {
                                readonly property real inches: Math.hypot(box.o.physical[0], box.o.physical[1]) / 25.4
                                width: parent.width; elide: Text.ElideRight
                                text: (inches > 0 ? inches.toFixed(1) + "″ · " : "") + box.o.mode[0] + " × " + box.o.mode[1]
                                font.family: Fonts.mono; font.pixelSize: settings.font * 0.68
                                color: settings.inkA(0.75)
                            }
                            Text {
                                width: parent.width; elide: Text.ElideRight
                                text: (box.o.px_per_mm * box.o.scale).toFixed(1) + " px/mm · at " + settings.dispScale(box.modelData.name)
                                      + " · " + box.modelData.w + " × " + box.modelData.h
                                font.family: Fonts.mono; font.pixelSize: settings.font * 0.68
                                color: settings.inkA(0.75)
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
                    y: settings.cfgTop + index * settings.rowPitch
                    width: settings.mainW; height: settings.rowPitch

                    Rectangle { width: parent.width; height: 1; color: settings.line }
                    Rectangle {     // the row the keys are on: a mark in the gutter, in the accent
                        visible: cr.here
                        x: -16; y: (parent.height - 18) / 2
                        width: 2; height: 18
                        color: settings.mark
                    }
                    Text {
                        id: rowName
                        x: 0; y: (parent.height - rowName.height - rowHint.height - 3) / 2
                        text: settings.sentence(cr.modelData.name)
                        font.family: Fonts.iface; font.pixelSize: settings.fRow; font.weight: Font.Medium
                        color: settings.ink
                    }
                    Text {
                        id: rowHint
                        y: rowName.y + rowName.height + 3
                        width: Math.min(parent.width * 0.55, 460); elide: Text.ElideRight
                        text: cr.modelData.note || ""
                        font.family: Fonts.iface; font.pixelSize: settings.fHint
                        color: settings.inkA(0.75)
                    }

                    // ---- the value, and the control for it ----
                    Row {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        spacing: 14
                        Text {      // what it is set to, in the mono face, as the canvas letters it
                            visible: cr.isSwitch || cr.isSlider
                            anchors.verticalCenter: parent.verticalCenter
                            text: cr.said
                            font.family: Fonts.mono; font.pixelSize: settings.fHint
                            color: settings.inkA(0.75)
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
                                color: settings.field
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
                                color: cr.value === true ? settings.paper : settings.ink
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
                                    color: seg.chosen ? settings.ink : segArea.containsMouse ? settings.hover(0.06) : "transparent"
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
                                        color: seg.low ? settings.shell.theme.alert : seg.chosen ? settings.paper : settings.inkA(0.75)
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
            visible: settings.page === "scheme"

        // the drawings
        Text {
            x: settings.mainX; y: pageHead.foot + 24
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
                x: settings.mainX + index * (settings.cardW + settings.cardGap)
                y: pageHead.foot + 24 + 22
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
    }
}
