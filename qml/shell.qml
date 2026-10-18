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
    function spaceMarkup(space) {
        if (!space) return ""
        if (!glyphs) return String(space.number)
        const seen = []
        for (const w of Compositor.windows)
            if (w.workspace === space.number && !w.parked) {
                const c = appGlyph(w.app_id)
                if (seen.indexOf(c) < 0) seen.push(c)
            }
        return String(space.number) + (seen.length ? " " + seen.slice(0, 3).map(c => g(c).trim()).join(" ") : "")
    }
    // a temperature's label as a glyph (CPU, SSD, memory, fan), or the label itself
    function tempLabel(label) {
        const k = { CPU: "󰘚", NVME: "󰋊", SSD: "󰋊", RAM: "󰍛", FAN: "󰈐" }[label]
        return glyphs && k ? g(k) : label + " "
    }

    // ---- readings ----
    Poll { id: readingsPoll; command: [root.bin, "readings"]; interval: 5000 }
    readonly property var readings: { try { return JSON.parse(readingsPoll.output) } catch (e) { return { temps: [], hot: false, power: null, profile: "", bt: [] } } }
    Poll { id: wifi; command: ["sh", "-c", "command -v nmcli >/dev/null && nmcli -t -f IN-USE,SIGNAL,SSID dev wifi list --rescan no | grep '^\\*' | head -1"]; interval: 5000 }
    Poll { id: eth; command: ["sh", "-c", "command -v nmcli >/dev/null || exit 0; d=$(nmcli -t -f DEVICE,TYPE,STATE dev | awk -F: '$2==\"ethernet\" && $3==\"connected\"{print $1; exit}'); [ -n \"$d\" ] && ip -4 -o addr show dev \"$d\" | awk '{print $4}' | cut -d/ -f1"]; interval: 5000 }
    Poll { id: meminfo; command: ["sh", "-c", "grep -E '^(MemTotal|MemAvailable):' /proc/meminfo"]; interval: 5000 }
    Poll { id: stat; command: ["head", "-1", "/proc/stat"]; interval: 2000 }
    Poll { id: backlight; command: ["sh", "-c", "d=$(ls -d /sys/class/backlight/* 2>/dev/null | head -1); [ -n \"$d\" ] && cat \"$d/brightness\" \"$d/max_brightness\""]; interval: 1000 }
    Poll {
        id: customPoll
        command: theme.cells.length > 0 && theme.cells[0].command ? theme.cells[0].command : ["true"]
        interval: theme.cells.length > 0 ? (theme.cells[0].interval || 5) * 1000 : 60000
    }

    readonly property real memUsed: {
        const m = {}
        meminfo.output.split("\n").forEach(l => { const p = l.split(/\s+/); m[p[0]] = Number(p[1]) })
        return m["MemTotal:"] ? 1 - m["MemAvailable:"] / m["MemTotal:"] : -1
    }
    property var cpuPrev: null
    property real cpuBusy: -1
    Connections {
        target: stat
        function onOutputChanged() {
            const f = stat.output.split(/\s+/).slice(1, 9).map(Number)
            const idle = f[3] + f[4], total = f.reduce((a, b) => a + b, 0)
            if (root.cpuPrev && total > root.cpuPrev[1]) root.cpuBusy = 1 - (idle - root.cpuPrev[0]) / (total - root.cpuPrev[1])
            root.cpuPrev = [idle, total]
        }
    }
    // a second sample a quarter second in, so the CPU reading is there from the start (not after 2 s)
    Timer { interval: 250; running: true; onTriggered: stat.rerun() }

    // everything the title block letters is in (the readings, the network, memory and CPU): told to the
    // compositor once, a few frames later, so the session's opening reveals a lettered desktop and nothing
    // is painted in after it (compositor/src/opening.rs)
    readonly property bool lettered: readingsPoll.ran && wifi.ran && eth.ran && meminfo.ran && cpuBusy >= 0 && Compositor.outputs.length > 0
    property bool readySent: false
    onLetteredChanged: if (lettered && !readySent) { readySent = true; readyTimer.start() }
    Timer { id: readyTimer; interval: 150; onTriggered: Compositor.send({ request: "ready" }) }

    readonly property real brightness: {
        const v = backlight.output.split("\n").map(Number)
        return v.length === 2 && v[1] > 0 ? v[0] / v[1] : -1
    }
    PwObjectTracker { objects: [Pipewire.defaultAudioSink] }
    readonly property var audio: Pipewire.defaultAudioSink ? Pipewire.defaultAudioSink.audio : null
    SystemClock { id: clock; precision: SystemClock.Minutes }
    readonly property var now: clock.date

    readonly property var battery: UPower.displayDevice
    readonly property bool batteryLow: battery.isLaptopBattery && battery.percentage * 100 < (theme.alerts.battery || 15)
        && battery.state !== UPowerDeviceState.Charging
    readonly property bool charging: battery.state === UPowerDeviceState.Charging
    readonly property string batteryText: !battery.isLaptopBattery ? "-"
        : glyphs ? g(charging ? "󰂄" : ramp(battery.percentage, ["󰁺", "󰁼", "󰁾", "󰂀", "󰁹"])) + Math.round(battery.percentage * 100) + "%"
        : Math.round(battery.percentage * 100) + "%" + (charging ? " charging" : "")
    readonly property string powerText: g("󱐋") + (readings.power !== null && readings.power !== undefined ? readings.power + " W" : "mains")
    // with its glyph the cell holds a shorter word: "power-saver" and "performance" do not fit beside it
    readonly property string profileText: !readings.profile ? ""
        : !glyphs ? readings.profile
        : g({ "power-saver": "󰌪", balanced: "󰾅", performance: "󰓅" }[readings.profile] || "󰾅")
          + ({ "power-saver": "saver", performance: "perf" }[readings.profile] || readings.profile)
    readonly property string loadText: {
        const cpu = cpuBusy >= 0 ? Math.round(cpuBusy * 100) + "%" : "-", mem = memUsed >= 0 ? Math.round(memUsed * 100) + "%" : "-"
        return glyphs ? g("󰻠") + cpu + "  " + g("󰍛") + mem : cpu + " / " + mem
    }
    // the Wi-Fi in use: "*:SIGNAL:SSID" (an SSID may hold escaped colons)
    readonly property var wifiNow: {
        const p = wifi.output.split(":")
        return p.length >= 3 && p.slice(2).join(":") !== "" ? { signal: Number(p[1]) / 100, ssid: p.slice(2).join(":").replace(/\\:/g, ":") } : null
    }
    readonly property string networkText: wifiNow ? g(ramp(wifiNow.signal, ["󰤯", "󰤟", "󰤢", "󰤥", "󰤨"])) + wifiNow.ssid
        : eth.output !== "" ? (glyphs ? g("󰈁") : "wired ") + eth.output
        : (glyphs ? g("󰤮") : "") + "offline"
    readonly property bool tempHot: readings.hot === true
    readonly property string tempText: (readings.temps || []).map(t => tempLabel(t.label) + t.value).join("  ")
    readonly property string customText: customPoll.output.split("\n")[0]

    // the revision block's rows (RevisionBlock.qml): the focused window's repository, polled while
    // [title] revisions = true, and again at once when the focus moves
    Poll {
        id: revisionsPoll
        command: [root.bin, "revisions"]
        interval: 4000
        running: theme.title.revisions === true
    }
    // nothing focused (every window parked): the last rows stay, so the bare sheet shows the work just left
    property var revisions: ({})
    Connections {
        target: revisionsPoll
        function onOutputChanged() {
            let v = {}
            try { v = JSON.parse(revisionsPoll.output) } catch (e) {}
            if (v.focused !== false) root.revisions = v
        }
    }
    readonly property var focusedWindow: { const w = Compositor.windows.find(w => w.focused); return w ? w.id : null }
    onFocusedWindowChanged: revisionsPoll.rerun()

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
        function toggle(): void { settings.visible ? settings.cancel() : settings.open() }
        // what the keys do, for scripts: a row (0 the drawings, 1 and on the builder's), a step along it
        function row(n: int): void {
            if (!settings.visible) return
            if (settings.page === "wallpaper") settings.wpRow = Math.max(0, Math.min(settings.wpRows.length, n))
            else settings.row = Math.max(0, Math.min(settings.rows.length, n))
        }
        function page(name: string): void { if (settings.visible) settings.page = name }
        function step(n: int): void { if (settings.visible) settings.step(n) }
        function keep(): void { if (settings.visible) settings.keep() }
        function cancel(): void { if (settings.visible) settings.cancel() }
    }
    // the launcher (Mod+d: `draftpaper launcher`) and the menu (`draftpaper menu`), Launcher.qml
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
    onBrightnessChanged: if (root.settled) { root.brightnessLit = true; brightnessOff.restart() }
    Connections {
        target: root.audio
        function onVolumeChanged() { if (root.settled) { root.volumeLit = true; volumeOff.restart() } }
        function onMutedChanged() { if (root.settled) { root.volumeLit = true; volumeOff.restart() } }
    }

    Variants {
        model: Quickshell.screens

        Scope {         // one screen: its sheet's geometry (logical pixels) and workspaces, from the compositor
            id: edge
            required property var modelData
            readonly property var screen: modelData
            readonly property var output: Compositor.output(modelData.name)
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
            RevisionBlock { shell: root; edge: edge }
            SideCells { shell: root; edge: edge; space: edge.activeSpace ? String(edge.activeSpace.number) : "" }

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
                                text: root.spaceMarkup(modelData)
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
                        Cell { visible: root.tempHot; size: edge.fontPx; text: root.tempText; color: theme.alert }
                        Cell { visible: root.batteryLow; size: edge.fontPx; text: (root.glyphs ? root.g("󰂃") : "BAT ") + Math.round(root.battery.percentage * 100) + "%"; color: theme.alert }
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
                        readonly property var devices: root.readings.bt || []
                        text: root.glyphs ? root.g(devices.length ? "󰂱" : "󰂯") + devices.join(", ")
                            : "BT " + (devices.length ? devices.join(", ") : "-")
                        color: edge.ink
                        onLeft: () => root.run(["blueman-manager"])
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
                Repeater {      // the second and later [[cell]]s of the config, each in its own zone (issue #6)
                    id: extraCells
                    model: theme.cells.slice(1)
                    Item {
                        id: extra
                        required property var modelData
                        required property int index
                        // the last zone is taken, and the one before by the scale bar; the rest are dropped
                        visible: index + 1 < edge.bottomFree
                        readonly property bool busy: visible && extraPoll.output !== ""
                        Poll {
                            id: extraPoll
                            command: extra.modelData.command || ["true"]
                            interval: (extra.modelData.interval || 5) * 1000
                        }
                        Zone {
                            band: bottomMargin; index: extra.index + 1
                            Cell {
                                anchors.centerIn: parent
                                visible: extraPoll.output !== ""
                                size: edge.fontPx
                                text: (extra.modelData.label ? extra.modelData.label + " " : "") + extraPoll.output.split("\n")[0]
                                color: edge.ink
                                onLeft: extra.modelData.left ? () => root.run(extra.modelData.left) : null
                                onRight: extra.modelData.right ? () => root.run(extra.modelData.right) : null
                            }
                        }
                    }
                }
                Zone {
                    band: bottomMargin; index: -1
                    Row {
                        anchors.centerIn: parent
                        spacing: 4 * edge.mm
                        Cell {
                            visible: (theme.commands.screenshot || []).length > 0
                            size: edge.fontPx; text: root.glyphs ? root.g("󰹑").trim() : "SHOT"; color: edge.ink
                            onLeft: () => root.run(theme.commands.screenshot)
                        }
                        Cell {      // the theme: left, the picker; right, the next theme
                            size: edge.fontPx; text: theme.label.toUpperCase(); color: edge.ink
                            onLeft: () => settings.visible ? settings.cancel() : settings.open()
                            onRight: () => root.run([root.bin, "theme", "next"])
                        }
                    }
                }
            }
        }
    }
}
