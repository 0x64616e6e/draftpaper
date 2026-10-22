//@ pragma UseQApplication
// Draftpaper's shell: the lettering on the sheet the compositor draws. There is no bar. The title block
// (TitleBlock.qml) carries the readings; the top and bottom margins letter what it does not (workspaces,
// notifications, alerts, volume and brightness while they change, the tray, Bluetooth, screenshot,
// theme); the side margins hold parked windows (SideCells.qml). Park every window of a workspace and the
// whole sheet shows.
// The margins and the title block are layer-shell surfaces on the bottom layer, under every window, so
// fullscreen windows cover them; their geometry and the workspaces come from the compositor's IPC
// (Compositor.qml). Started by `draftpaper start` (draftpaper-shell.service, with the session), which
// sets DRAFTPAPER_BIN, _CACHE and _RUNTIME.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import Quickshell.Services.UPower
import Quickshell.Services.Pipewire
import Quickshell.Services.SystemTray
import Quickshell.Services.Notifications

ShellRoot {
    id: root
    Theme { id: themeData }
    property alias theme: themeData
    readonly property color ink: theme.ink
    readonly property string bin: Quickshell.env("DRAFTPAPER_BIN") || "draftpaper"

    function run(cmd) { Quickshell.execDetached(cmd) }
    function profileNext() {
        const order = ["power-saver", "balanced", "performance"]
        const i = order.indexOf(readings.profile)
        // over D-Bus (busctl), as the readings read it: powerprofilesctl is a Python script
        if (i >= 0) run(["busctl", "set-property", "net.hadess.PowerProfiles", "/net/hadess/PowerProfiles", "net.hadess.PowerProfiles",
                         "ActiveProfile", "s", order[(i + 1) % order.length]])
    }
    function spaceLabel(space) { return space ? String(space.number) : "" }

    // ---- glyphs: Nerd Font symbols before a reading, when a Nerd Font is installed and config.toml does not
    //      say glyphs = false; otherwise words. g() is the glyph in its own face (so it keeps its width),
    //      then a space; ramp() picks one of `gs` by v in 0..1.
    readonly property bool glyphs: theme.glyphs && Fonts.glyph !== ""
    function g(c) { return glyphs ? "<font face='" + Fonts.glyph + "'>" + c + "</font> " : "" }
    function ramp(v, gs) { return gs[Math.min(gs.length - 1, Math.max(0, Math.floor(v * gs.length)))] }
    // workspace glyphs: the apps on a workspace by their app_id (X11: class), first match wins; any other
    // app is a window. Up to three glyphs follow the number.
    readonly property var appGlyphs: [
        [["firefox", "librewolf", "zen"], "󰈹"],
        [["chromium", "chrome", "brave"], "󰊯"],
        [["alacritty", "foot", "kitty", "xterm", "wezterm", "terminal", "konsole", "ghostty"], ""],
        [["code", "codium"], "󰨞"],
        [["claude"], "󰚩"],
        [["thunderbird", "evolution", "geary"], "󰇰"],
        [["signal"], "󰭹"],
        [["telegram"], ""],
        [["teams"], "󰊻"],
        [["slack"], "󰒱"],
        [["discord", "vesktop"], "󰙯"],
        [["spotify"], "󰓇"],
        [["mpv", "vlc", "totem", "celluloid"], "󰕼"],
        [["gimp", "inkscape", "krita"], "󰸾"],
        [["libreoffice", "soffice"], "󰈙"],
        [["nautilus", "thunar", "pcmanfm", "nemo", "dolphin"], "󰉋"],
        [["evince", "zathura", "okular", "papers"], "󰈦"],
        [["keepassxc", "bitwarden"], "󰌋"],
        [["obs"], "󰑋"],
        [["steam"], ""],
        [["emacs"], ""]
    ]
    function appGlyph(appId) {
        const id = (appId || "").toLowerCase()
        const hit = appGlyphs.find(e => e[0].some(f => id.indexOf(f) >= 0))
        return hit ? hit[1] : "󰖯"
    }
    // "2" and, with glyphs, the apps on it: "2 <firefox> <terminal>"
    // a temperature's label as a glyph (CPU, SSD, memory, fan), or the label itself

    // ---- what the sheet says (docs/sheet-content.md) ----
    // One account, shared with the sheet's own client: `draftpaper sheet` says what is lettered in every
    // slot of every display, and this decides none of it. What is left here is the one question a
    // renderer must answer — whether this machine can draw an entry's glyphs — and the live state a
    // snapshot cannot hold: the notifications, and the levels lettered for a moment after they move.
    // a schedule that is a note of something moving is read oftener while it stands on the sheet, as
    // the shell polled it before the document carried it
    Poll {
        id: sheetPoll
        // the shell holds which schedule is out, so it says so rather than the compositor being asked
        command: [root.bin, "sheet", root.schedule, root.scheduleOn]
        interval: root.schedule === "processes" ? 2000 : root.schedule === "revisions" ? 4000 : 5000
    }
    readonly property var doc: { try { return JSON.parse(sheetPoll.output).displays || {} } catch (e) { return {} } }
    // the legend's millimetres, the same for every display and for both renderers (docs/keyboard.md):
    // the document keeps them so that neither renderer has a copy of its own to drift
    readonly property var legendMm: { try { return JSON.parse(sheetPoll.output).legend || null } catch (e) { return null } }
    // the backlight's own path never changes while the session lasts, so it is asked for rarely
    Poll { id: readingsPoll; command: [root.bin, "readings"]; interval: 60000 }
    readonly property var readings: { try { return JSON.parse(readingsPoll.output) } catch (e) { return {} } }

    // what a click on a slot does, by the name the document gives it
    function act(name, screenName) {
        if (!name) return
        const dot = name.indexOf("."), what = name.slice(0, dot), which = name.slice(dot + 1)
        if (what === "graph") toggleGraph(which, screenName)
        else if (what === "schedule") toggleSchedule(which, screenName)
        else if (what === "profile") run([root.bin, "profile", which])
        else if (what === "theme") run([root.bin, "theme", which])
        else if (what === "settings") settings.visible ? settings.cancel() : settings.open()
        else if (what === "window") Compositor.action("unpark " + which)
        else if (name === "run.screenshot") run(theme.commands.screenshot)
        else if (name === "run.bluetooth") run(["blueman-manager"])
        // a cell or an extension of the user's own: what it runs is its own to say, and `draftpaper`
        // looks it up rather than this holding a copy of the config
        else run([root.bin, "cell", name])
    }
    // the backlight is read in place (Watch.qml: no process started); the memory and the processor are
    // the document's now (docs/sheet-content.md)
    Watch { id: backlightNow; path: root.readings.backlight ? root.readings.backlight + "/brightness" : ""; interval: 1000 }
    Watch { id: backlightMax; path: root.readings.backlight ? root.readings.backlight + "/max_brightness" : ""; interval: 60000 }


    // ---- the notes on the sheet (issue #49): the schedules (Mod+i) and a reading's last ten minutes ----
    // the schedule shown ("", "equipment", "processes") and on which screen: the focused one when it came
    property string schedule: ""
    property string scheduleOn: ""
    function focusedName() { const o = Compositor.outputs.find(o => o.focused); return o ? o.name : "" }
    function showSchedule(which) {
        scheduleOn = focusedName()
        schedule = which
        // the document says which schedule is out and what it holds, so it is asked for again at once
        // rather than at the next tick
        sheetPoll.rerun()
    }
    // a schedule's cell in the title block (PROJECT, PROCESSES): it on that screen, or away if it is there
    function toggleSchedule(which, screenName) {
        // away again: the document carries what the schedule holds, so it is asked for at once rather
        // than at the next tick — otherwise the cell is clicked and the schedule sits there for seconds
        if (schedule === which && scheduleOn === screenName) { schedule = ""; sheetPoll.rerun(); return }
        showSchedule(which)
        scheduleOn = screenName
    }
    // Mod+i goes through them in turn, but which schedules there are is `draftpaper schedule next`'s to
    // say (docs/sheet-content.md): the three the sheet comes with and whatever the user has added. This
    // is left for anything still calling it, and it asks the same way the key does.
    function nextSchedule() { root.run([root.bin, "schedule", "next"]) }

    // the graph: which readings ("", "load", "temperature", "power", "battery", "network", "disk") on which
    // screen; a click on their cell in the title block (the disks: the STORAGE row of the equipment schedule)
    // shows them, again puts them away, another's switches
    property string graph: ""
    property string graphOn: ""
    function toggleGraph(which, screenName) {
        if (graph === which && graphOn === screenName) graph = ""
        else { graph = which; graphOn = screenName }
    }
    // `draftpaper config set KEY=VALUE ...`: a note dragged to its place keeps it (Schedule.qml, LoadGraph.qml)
    function setConfig(pairs) { configSet.command = [root.bin, "config", "set"].concat(pairs); configSet.running = true }
    Process { id: configSet }

    // everything the title block letters is in (the readings, the network, memory and CPU): told to the
    // compositor once, a few frames later, so the session's opening reveals a lettered desktop and nothing
    // is painted in after it (compositor/src/opening.rs)
    readonly property bool lettered: sheetPoll.ran && Compositor.outputs.length > 0
    property bool readySent: false
    onLetteredChanged: if (lettered && !readySent) { readySent = true; readyTimer.start() }
    Timer { id: readyTimer; interval: 150; onTriggered: Compositor.send({ request: "ready" }) }

    readonly property real brightness: {
        const now = Number(backlightNow.output), max = Number(backlightMax.output)
        return backlightNow.output !== "" && max > 0 ? now / max : -1
    }
    PwObjectTracker { objects: [Pipewire.defaultAudioSink] }
    readonly property var audio: Pipewire.defaultAudioSink ? Pipewire.defaultAudioSink.audio : null
    SystemClock { id: clock; precision: SystemClock.Minutes }
    readonly property var now: clock.date

    // the battery is UPower's still, for the lock and greeter; what the sheet says of it is the
    // document's (docs/sheet-content.md)
    readonly property var battery: UPower.displayDevice

    readonly property var focusedWindow: { const w = Compositor.windows.find(w => w.focused); return w ? w.id : null } // the revisions follow it
    onFocusedWindowChanged: sheetPoll.rerun()

    // volume and brightness are lettered for a few seconds after they change (volume stays while muted)
    property bool settled: false
    Timer { interval: 3000; running: true; onTriggered: root.settled = true }

    // the parked windows by workspace number ({"2": [{id, cls, title, cell}]}), from the compositor
    readonly property var parked: {
        const byWorkspace = {}
        for (const w of Compositor.windows) {
            if (!w.parked) continue
            const k = String(w.workspace)
            if (!byWorkspace[k]) byWorkspace[k] = []
            byWorkspace[k].push({ id: w.id, cls: w.app_id, title: w.title, cell: w.cell })
        }
        return byWorkspace
    }
    // ---- notifications: the shell is the notification daemon; each is lettered in the top margin of the
    //      focused screen (see the notice zone), the newest first, the others waiting behind a count ----
    NotificationServer {
        id: notices
        keepOnReload: false
        bodySupported: true
        actionsSupported: true
        onNotification: n => { n.tracked = true }
    }
    readonly property var noticeList: notices.trackedNotifications.values
    readonly property var notice: noticeList.length ? noticeList[noticeList.length - 1] : null
    // one line: the app's glyph (or name), the summary, the body without markup or line breaks
    function noticeText(n) {
        if (!n) return ""
        const plain = s => (s || "").replace(/<[^>]*>/g, "").replace(/\s+/g, " ").trim()
        const app = glyphs ? g(appGlyph(n.desktopEntry || n.appName)) : (n.appName ? n.appName + " · " : "")
        const body = plain(n.body)
        return app + plain(n.summary) + (body ? " — " + body : "")
    }
    // the shown one goes after its timeout (the app's, or 5 s); a critical one stays until clicked
    Timer {
        id: noticeTimer
        interval: root.notice && root.notice.expireTimeout > 0 ? root.notice.expireTimeout * 1000 : 5000
        running: root.notice !== null && root.notice.urgency !== NotificationUrgency.Critical
        onTriggered: if (root.notice) root.notice.expire()
    }
    onNoticeChanged: if (notice) noticeTimer.restart()
    function noticeClicked(left) {
        const n = root.notice
        if (!n) return
        const action = left ? n.actions.find(a => a.identifier === "default") : null
        if (action) action.invoke()
        n.dismiss()
    }

    // the settings (Mod+t: `draftpaper settings`, which calls this handler; or the theme cell)
    Settings { id: settings; shell: root }
    IpcHandler {
        target: "settings"
        // also what `draftpaper settings` asks to find out whether a shell is listening at all
        function state(): string { return settings.visible ? "open" : "" }
        function toggle(): void { settings.visible ? settings.cancel() : settings.open() }
        // what the keys do, for scripts: a row (0 the drawings, 1 and on the builder's), a step along it
        function row(n: int): void {
            if (!settings.visible) return
            if (settings.page === "wallpaper") settings.wpRow = Math.max(0, Math.min(settings.wpRows.length, n))
            else if (settings.cfgRows.length) settings.cfgRow = Math.max(0, Math.min(settings.cfgRows.length - 1, n))
            else settings.row = Math.max(0, Math.min(settings.rows.length, n))
        }
        function page(name: string): void { if (settings.visible) settings.page = name }
        // a row by the key it writes, not by where it falls on the page: the pages are rearranged
        // from time to time and an index is a promise about an arrangement. Returns "" when the page
        // showing has no such row, so a test says which key it could not find.
        function rowKey(key: string): string {
            if (!settings.visible) return ""
            const i = settings.cfgRows.findIndex(r => r.key === key)
            if (i < 0) return ""
            settings.cfgRow = i
            return key
        }
        function step(n: int): void { if (settings.visible) settings.step(n) }
        function keep(): void { if (settings.visible) settings.keep() }
        function cancel(): void { if (settings.visible) settings.cancel() }
        // how many displays the Displays page was built from. The page is a snapshot of the outputs as
        // they stood when the settings opened, so a shell started for the settings alone must wait for
        // the compositor to answer before opening them; this is how the test sees that it did.
        function displays(): int { return settings.dispOutputs.length }
    }
    // the launcher (Mod+d: `draftpaper launcher`) and the menu (`draftpaper menu`), Launcher.qml
    // the schedules on the sheet (Mod+i: `draftpaper schedule next`), and the graph of a reading (its cell)
    IpcHandler {
        target: "schedules"
        function next(): void { root.nextSchedule() }
        function equipment(): void { root.showSchedule("equipment") }
        function processes(): void { root.showSchedule("processes") }
        function revisions(): void { root.showSchedule("revisions") }
        function hide(): void { root.showSchedule("") }
        // a schedule of the user's own, by the name of its extension (docs/sheet-content.md). Not
        // `show`: `qs ipc` has a subcommand of that name and would swallow the argument.
        function named(which: string): void { root.showSchedule(which) }
        function state(): string { return root.schedule }
    }
    IpcHandler {
        target: "graph"
        // load, temperature, power, battery: as a click on its cell on the focused screen (again: away)
        function toggle(which: string): void { root.toggleGraph(which, root.focusedName()) }
        function state(): string { return root.graph }
    }
    Rulers { id: rulers; shell: root }
    IpcHandler {
        target: "rulers"
        function toggle(): void { rulers.visible ? rulers.close() : rulers.open() }
        // for scripts and tests: where they cross (mm from the frame's corner) and how they stand
        function set(zeroX: real, zeroY: real, angleH: real, angleV: real): void {
            if (!rulers.visible) rulers.open()
            rulers.zeroX = Math.round(zeroX); rulers.zeroY = Math.round(zeroY)
            rulers.angleH = angleH; rulers.angleV = angleV
        }
        function lock(on: bool): void { rulers.isLocked = on }
        function panel(on: bool): void { rulers.panelShown = on }
        // each ruler's zero, whole millimetres along it from the crossing
        function zeros(h: int, v: int): void { rulers.zeroH = h; rulers.zeroV = v }
        function turn(which: string, to: real): void { which === "v" ? rulers.turnV(to) : rulers.turnH(to) }
        function state(): string {
            return rulers.visible ? JSON.stringify({ crossing: [rulers.zeroX, rulers.zeroY], zeros: [rulers.zeroH, rulers.zeroV], angles: [rulers.angleH, rulers.angleV], locked: rulers.isLocked, snap: rulers.snapOn }) : ""
        }
    }
    Launcher { id: launcher; shell: root }
    IpcHandler {
        target: "launcher"
        function toggle(): void { launcher.visible ? launcher.close() : launcher.open() }
        // a menu over the lines of the file `lines` (`qs ipc` splits a JSON list given as an argument);
        // the choice goes to the FIFO `fifo`
        function menu(prompt: string, lines: string, fifo: string): void { launcher.openMenu(prompt, lines, fifo) }
        // what the keys do, for scripts and tests: type, move, launch (or choose), close
        function type(text: string): void { if (launcher.visible) launcher.query = text }
        function move(n: int): void { if (launcher.visible) launcher.move(n) }
        function launch(): void { if (launcher.visible) launcher.launch() }
        function close(): void { if (launcher.visible) launcher.close() }
        function picked(): string { return launcher.visible && launcher.matches[launcher.sel] ? launcher.matches[launcher.sel].name : "" }
    }
    IpcHandler {        // the name the theme picker had (`draftpaper picker`, an installed 0.3.0 binding)
        target: "themes"
        function toggle(): void { settings.visible ? settings.cancel() : settings.open() }
    }

    property bool volumeLit: false
    property bool brightnessLit: false
    Timer { id: volumeOff; interval: 4000; onTriggered: root.volumeLit = false }
    Timer { id: brightnessOff; interval: 4000; onTriggered: root.brightnessLit = false }
    // the brightness as it was last read: the first reading is not a change. The backlight is only
    // known once its two files have been read, and on a loaded machine that lands after `settled`,
    // which lit the indicator at login for no reason at all -- and made the two renderers disagree
    // on the top margin, which is what compositor/tests/lettering.sh caught.
    property real wasBrightness: -1
    onBrightnessChanged: {
        const had = root.wasBrightness >= 0
        root.wasBrightness = root.brightness
        if (root.settled && had && root.brightness >= 0) { root.brightnessLit = true; brightnessOff.restart() }
    }
    Connections {
        target: root.audio
        function onVolumeChanged() { if (root.settled) { root.volumeLit = true; volumeOff.restart() } }
        function onMutedChanged() { if (root.settled) { root.volumeLit = true; volumeOff.restart() } }
    }

    // DRAFTPAPER_TOOLS=1: started for one of the tools (the settings, the launcher, the rulers) while the
    // sheet is lettered by its own client (sheet/, docs/sheet-client.md), so none of the sheet itself is
    // drawn here and the process goes once the tool is closed
    // DRAFTPAPER_TOOLS names the tool this shell was started for, and it opens at once: a Quickshell with
    // no window of its own would have nothing to do and would go again straight away
    readonly property string toolsOnly: Quickshell.env("DRAFTPAPER_TOOLS") || ""
    property bool toolWasOpen: false
    readonly property bool toolOpen: settings.visible || launcher.visible || rulers.visible
    onToolOpenChanged: {
        if (toolOpen) toolWasOpen = true
        else if (toolsOnly !== "" && toolWasOpen) Qt.quit()
    }
    // ...but not before the compositor has answered. The Displays page is built from the outputs as
    // they stand when the settings open (Settings.qml), and at startup they have not arrived yet: the
    // socket is opened and subscribed to in the same breath as this. Opening straight away left that
    // page with no display on it at all. A tool opens anyway after a moment, so one still comes up
    // where there is no compositor to answer.
    Timer {
        interval: 25
        repeat: true
        running: root.toolsOnly !== ""
        property int waited: 0
        onTriggered: {
            waited += interval
            if (Compositor.outputs.length === 0 && waited < 2000)
                return
            running = false
            if (root.toolsOnly === "settings") settings.open()
            else if (root.toolsOnly === "launcher") launcher.open()
            else if (root.toolsOnly === "rulers") rulers.open()
        }
    }

    Variants {
        model: root.toolsOnly !== "" ? [] : Quickshell.screens

        Scope {         // one screen: its sheet's geometry (logical pixels) and workspaces, from the compositor
            id: edge
            required property var modelData
            readonly property var screen: modelData
            readonly property var output: Compositor.output(modelData.name)
            // what the sheet says on this display (docs/sheet-content.md)
            readonly property var doc: root.doc[modelData.name] || ({})
            // what the document says about one of this display's workspaces; the compositor says which is
            // focused and shown, and a click on one is its business too
            function said(n) { return ((doc.top || {}).workspaces || []).find(w => w.number === n) }
            readonly property real dpr: output ? output.scale : (modelData.devicePixelRatio || 1)
            readonly property real mm: output ? output.px_per_mm : 6.2 / dpr        // logical px per millimetre
            readonly property real margin: output ? output.margin : Math.round(6 * mm)
            readonly property real width: output ? output.width : modelData.width
            readonly property real height: output ? output.height : modelData.height
            readonly property int zonesAcross: output ? output.zones.across : Math.max(2, Math.round((width - 2 * margin) / (50 * mm)))
            readonly property int zonesDown: output ? output.zones.down : Math.max(2, Math.round((height - 2 * margin) / (50 * mm)))
            readonly property real fontPx: Math.min(margin * 0.5, 3.2 * mm)
            // the scale bar in the bottom margin (the compositor's; null when off or without room): it takes
            // the zone next to the last
            readonly property var scaleBar: output && output.scale_bar ? output.scale_bar : null
            // the key plan and its north mark (the compositor's, with several screens): the zone left of
            // the scale bar's
            readonly property var keyPlan: output && output.key_plan ? output.key_plan : null
            // the bottom margin's zones taken from the right: the theme's, the scale bar's, the key plan's; the
            // custom cells and the column numbers stop short of the first of them
            readonly property int bottomFree: zonesAcross - 1 - (scaleBar !== null || keyPlan !== null ? 1 : 0) - (keyPlan !== null ? 1 : 0)
            // the margins' lettering: the theme's ink, or with a wallpaper's ambient margins whichever ink
            // the compositor found to read on them (and the focused workspace in it, bold)
            readonly property color ink: output && output.margin_ink ? output.margin_ink : root.ink
            readonly property color strong: output && output.margin_ink ? ink : theme.strong

            // this screen's workspaces, by number (the compositor sends them sorted)
            readonly property var spaces: Compositor.workspaces.filter(w => w.output === modelData.name)
            readonly property var activeSpace: spaces.find(w => w.shown) || null

            // cells that take more than one zone are one cell: the compositor leaves out the ticks inside
            // (the workspaces past five, a notification); [first, last] zones from the frame's corner
            readonly property var mergedCells: {
                const top = []
                if (spacesZone.span > 1) top.push([spacesZone.first, spacesZone.first + spacesZone.span - 1])
                if (noticeZone.visible && noticeZone.span > 1) top.push([noticeZone.first, noticeZone.first + noticeZone.span - 1])
                return { top: top }
            }
            onMergedCellsChanged: Compositor.setMerged(modelData.name, mergedCells)
            Component.onCompleted: Compositor.setMerged(modelData.name, mergedCells)

            TitleBlock { shell: root; edge: edge; spaceName: root.spaceLabel(edge.activeSpace) }
            // the schedules take turns in one place; the graph keeps clear of whichever is out
            // whichever is out, as the document gives it: its heading, its columns and its rows
            // (docs/sheet-content.md). Which that is, and whether the revisions are out by themselves
            // with [title] revisions = true, is settled there too.
            Schedule {
                id: sched
                shell: root; edge: edge; other: load
                readonly property var said: edge.doc.schedule || null
                wanted: said !== null
                title: said ? said.title : ""
                meta: said ? said.meta : ""
                columns: said ? said.columns : []
                rows: said ? said.rows.map(r => ({ cells: r.cells, note: r.note || "", heavyAbove: r.heavy === true, share: r.share, action: r.action, ink: r.ink })) : []
                noteCol: said ? said.note : -1
                barCol: said ? said.bar : -1
            }
            LoadGraph { id: load; shell: root; edge: edge; domain: root.graphOn === edge.screen.name ? root.graph : "" }
            SideCells { shell: root; edge: edge; space: edge.activeSpace ? String(edge.activeSpace.number) : "" }
            // Mod held alone long enough: the keymap lettered on the sheet's free side, gone the moment
            // it is let go. The compositor says when; the keys are its own (docs/keyboard.md).
            Legend { shell: root; edge: edge; wanted: Compositor.legend; leader: Compositor.leader
                     word: Compositor.word
                     keys: Compositor.keys; avoid: sched
                     mmFrom: root.legendMm; windows: Compositor.windows }

            // ---- top margin: workspaces in the first zone | alerts, volume, brightness, tray in the last ----
            Margin {
                id: topMargin
                edge: edge; ink: edge.ink; inkAlpha: theme.inkAlpha
                side: "top"
                Zone {          // one workspace: centred; up to five: the zone divided equally, a tick between
                                // each; more: fixed slots of a fifth of a zone, running on into the next zones
                    id: spacesZone
                    band: topMargin; index: 0
                    readonly property int perZone: 5
                    readonly property bool slotted: spaceCells.count > perZone
                    readonly property real slot: slotted ? topMargin.zoneWidth / perZone : width / Math.max(1, spaceCells.count)
                    span: slotted ? Math.ceil(spaceCells.count / perZone) : 1
                    Repeater {
                        id: spaceCells
                        model: edge.spaces
                        Item {
                            required property var modelData
                            required property int index
                            x: index * width; width: spacesZone.slot; height: parent.height
                            Text {
                                anchors.centerIn: parent
                                width: Math.min(implicitWidth, parent.width - 3 * edge.mm)
                                horizontalAlignment: Text.AlignHCenter
                                fontSizeMode: Text.HorizontalFit
                                minimumPixelSize: edge.fontPx * 0.6
                                text: Fonts.lettered(edge.said(modelData.number), root.glyphs)
                                textFormat: Text.StyledText
                                // focused: bold in the theme's strong ink; shown on its screen but not focused: bold in the others'
                                // ink; the rest: regular, lighter
                                font.family: Fonts.mono; font.pixelSize: edge.fontPx
                                font.bold: modelData.focused || modelData.shown
                                color: modelData.focused ? edge.strong : Qt.rgba(edge.ink.r, edge.ink.g, edge.ink.b, 0.6)
                            }
                            Rectangle {     // separator: the full height of the margin, like the zone ticks (whose
                                            // own ticks a cell of several zones leaves out)
                                visible: index > 0
                                width: Math.max(1, 1 / edge.dpr); height: parent.height
                                color: topMargin.tick
                            }
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Compositor.action("workspace " + modelData.number) }
                        }
                    }
                }
                Zone {          // a notification: the zone before the last, growing leftwards as far as the
                                // workspaces' zones; one line, cut with … when it is longer still. While
                                // the theme picker is open, its keys instead.
                    id: noticeZone
                    band: topMargin; index: -2
                    readonly property string hint: settings.hint
                    readonly property bool here: (root.notice !== null || hint !== "") && (!edge.output || edge.output.focused)
                    readonly property int room: Math.max(1, topMargin.zones - 1 - spacesZone.span)
                    readonly property real pad: 3 * edge.mm
                    span: Math.min(room, Math.max(1, Math.ceil((noticeMeasure.implicitWidth + countText.implicitWidth + 2 * pad) / topMargin.zoneLength)))
                    visible: here
                    Text {          // the text at its full length, to size the zone
                        id: noticeMeasure
                        visible: false
                        text: noticeZone.hint || root.noticeText(root.notice)
                        textFormat: Text.StyledText
                        font.family: Fonts.mono; font.pixelSize: edge.fontPx
                    }
                    Text {          // "+2": notifications waiting behind this one
                        id: countText
                        anchors { right: parent.right; rightMargin: noticeZone.pad; verticalCenter: parent.verticalCenter }
                        visible: noticeZone.hint === "" && root.noticeList.length > 1
                        text: "+" + (root.noticeList.length - 1)
                        font.family: Fonts.mono; font.pixelSize: edge.fontPx * 0.8
                        color: Qt.rgba(edge.ink.r, edge.ink.g, edge.ink.b, 0.6)
                    }
                    Text {
                        anchors { left: parent.left; leftMargin: noticeZone.pad; right: countText.visible ? countText.left : parent.right
                                  rightMargin: noticeZone.pad; verticalCenter: parent.verticalCenter }
                        text: noticeMeasure.text
                        textFormat: Text.StyledText
                        elide: Text.ElideRight
                        horizontalAlignment: noticeZone.span < noticeZone.room ? Text.AlignHCenter : Text.AlignLeft
                        font.family: Fonts.mono; font.pixelSize: edge.fontPx
                        color: noticeZone.hint === "" && root.notice && root.notice.urgency === NotificationUrgency.Critical ? theme.alert : edge.ink
                    }
                    MouseArea {     // left: the notification's default action, then gone; right: gone
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        cursorShape: Qt.PointingHandCursor
                        onClicked: e => { if (noticeZone.hint === "") root.noticeClicked(e.button === Qt.LeftButton) }
                    }
                }
                ZoneNumbers {   // the columns' numbers, in the zones nothing else takes
                    band: topMargin; edge: edge
                    taken: k => k < spacesZone.span
                        || (noticeZone.visible && k >= noticeZone.first && k < noticeZone.first + noticeZone.span)
                        || (k === topMargin.zones - 1 && topStatus.implicitWidth > 0)
                }
                Zone {
                    band: topMargin; index: -1
                    Row {
                        id: topStatus
                        anchors.centerIn: parent
                        spacing: 4 * edge.mm
                        // what wants saying now, as the document says it (docs/sheet-content.md)
                        Repeater {
                            model: (edge.doc.top || {}).status || []
                            Cell {
                                required property var modelData
                                size: edge.fontPx
                                text: Fonts.lettered(modelData, root.glyphs)
                                color: modelData.ink === "alert" ? theme.alert : edge.ink
                            }
                        }
                        Cell {
                            readonly property var a: root.audio
                            visible: a !== null && (root.volumeLit || a.muted)
                            size: edge.fontPx
                            text: !a ? "" : root.glyphs ? (a.muted ? root.g("󰝟") + "muted" : root.g(root.ramp(a.volume, ["󰕿", "󰖀", "󰕾"])) + Math.round(a.volume * 100) + "%")
                                : a.muted ? "VOL muted" : "VOL " + Math.round(a.volume * 100) + "%"
                            color: edge.ink
                            onLeft: () => a.muted = !a.muted
                            onScroll: d => a.volume = Math.max(0, Math.min(1.5, a.volume + d * 0.05))
                        }
                        Cell {
                            visible: root.brightness >= 0 && root.brightnessLit
                            size: edge.fontPx
                            text: (root.glyphs ? root.g(root.ramp(root.brightness, ["󰃞", "󰃟", "󰃠"])) : "BRT ") + Math.round(root.brightness * 100) + "%"
                            color: edge.ink
                        }
                        Row {
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 2 * edge.mm
                            Repeater {
                                model: SystemTray.items
                                IconImage {
                                    required property var modelData
                                    source: modelData.icon
                                    implicitSize: edge.fontPx * 1.1
                                    MouseArea {
                                        anchors.fill: parent
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        onClicked: e => e.button === Qt.RightButton && modelData.hasMenu
                                            ? modelData.display(topMargin.window, parent.mapToItem(null, 0, 0).x, edge.margin) : modelData.activate()
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- bottom margin: Bluetooth in the first zone | custom cells after the first, one per zone |
            //      screenshot, theme in the last ----
            Margin {
                id: bottomMargin
                edge: edge; ink: edge.ink; inkAlpha: theme.inkAlpha
                side: "bottom"
                Zone {
                    band: bottomMargin; index: 0
                    Cell {
                        anchors.centerIn: parent
                        size: edge.fontPx
                        readonly property var said: (edge.doc.bottom || {}).start
                        text: Fonts.lettered(said, root.glyphs)
                        color: edge.ink
                        onLeft: () => root.act(said ? said.action : "", edge.screen.name)
                    }
                }
                ZoneNumbers {   // the columns' numbers, in the zones nothing else takes
                    band: bottomMargin; edge: edge
                    taken: k => k === 0 || k >= edge.bottomFree
                        || (extraCells.itemAt(k - 1)?.busy ?? false)
                }
                ScaleBar {      // the scale bar's numbers (or NTS); the compositor draws the bar
                    visible: edge.scaleBar !== null
                    band: bottomMargin; edge: edge; bar: edge.scaleBar || { zone: { x: 0, width: 0 }, measured: false, marks: [] }
                }
                Repeater {      // the user's own, after the first, a zone each (issue #6). What each says
                                // and how often it is asked are the document's (docs/sheet-content.md)
                    id: extraCells
                    model: (edge.doc.bottom || {}).cells || []
                    Item {
                        id: extra
                        required property var modelData
                        required property int index
                        // the last zone is taken, and the one before by the scale bar; the rest are dropped
                        visible: index + 1 < edge.bottomFree
                        readonly property bool busy: visible
                        Zone {
                            band: bottomMargin; index: extra.index + 1
                            Cell {
                                anchors.centerIn: parent
                                size: edge.fontPx
                                text: (extra.modelData.label ? extra.modelData.label + " " : "") + Fonts.lettered(extra.modelData, root.glyphs)
                                color: extra.modelData.ink === "alert" ? theme.alert : edge.ink
                                onLeft: () => root.act(extra.modelData.action, edge.screen.name)
                                onRight: extra.modelData.action_right ? () => root.act(extra.modelData.action_right, edge.screen.name) : null
                            }
                        }
                    }
                }
                Zone {
                    band: bottomMargin; index: -1
                    Row {       // the screenshot and the theme, as the document gives them
                        anchors.centerIn: parent
                        spacing: 4 * edge.mm
                        Repeater {
                            model: (edge.doc.bottom || {}).end || []
                            Cell {
                                required property var modelData
                                size: edge.fontPx
                                text: Fonts.lettered(modelData, root.glyphs)
                                color: edge.ink
                                onLeft: () => root.act(modelData.action, edge.screen.name)
                                onRight: modelData.action_right ? () => root.act(modelData.action_right, edge.screen.name) : null
                            }
                        }
                    }
                }
            }
        }
    }
}
