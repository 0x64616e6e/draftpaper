//@ pragma UseQApplication
// Draftpaper: the i3 desktop as a drafting sheet. There is no bar. The title block (TitleBlock.qml)
// carries the readings; the top and bottom margins letter what it does not (workspaces, alerts, volume
// and brightness while they change, the tray, Bluetooth, screenshot, theme); the side margins hold
// parked windows (SideCells.qml, `draftpaper park`). Park every window of a workspace and the whole
// sheet shows.
// i3 keeps a 3 px dock strip along the top (the shell's only managed window); the margins and the title
// block are unmanaged windows pushed under every i3 window (`draftpaper lower`), so fullscreen windows
// cover them. Started by `draftpaper start`, which sets DRAFTPAPER_BIN, _CACHE and _RUNTIME.
import QtQuick
import Quickshell
import Quickshell.I3
import Quickshell.Io
import Quickshell.Widgets
import Quickshell.Services.UPower
import Quickshell.Services.Pipewire
import Quickshell.Services.SystemTray

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
        if (i >= 0) run(["powerprofilesctl", "set", order[(i + 1) % order.length]])
    }
    // a named workspace is lettered by its number and the name's initial: "9:mail" -> "9M"
    function spaceLabel(name) { const m = /^(\d+):(.)/.exec(name); return m ? m[1] + m[2].toUpperCase() : name }

    // ---- readings ----
    Poll { id: readingsPoll; command: [root.bin, "readings"]; interval: 5000 }
    readonly property var readings: { try { return JSON.parse(readingsPoll.output) } catch (e) { return { temps: [], hot: false, power: null, profile: "", bt: [] } } }
    Poll { id: wifi; command: ["sh", "-c", "command -v nmcli >/dev/null && nmcli -t -f IN-USE,SSID dev wifi list --rescan no | grep '^\\*' | head -1"]; interval: 5000 }
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
    readonly property string batteryText: !battery.isLaptopBattery ? "-"
        : Math.round(battery.percentage * 100) + "%" + (battery.state === UPowerDeviceState.Charging ? " charging" : "")
    readonly property string powerText: readings.power !== null && readings.power !== undefined ? readings.power + " W" : "mains"
    readonly property string profileText: readings.profile || ""
    readonly property string loadText: (cpuBusy >= 0 ? Math.round(cpuBusy * 100) + "%" : "-") + " / " + (memUsed >= 0 ? Math.round(memUsed * 100) + "%" : "-")
    readonly property string networkText: {
        const p = wifi.output.split(":")
        if (p.length >= 2 && p.slice(1).join(":") !== "") return p.slice(1).join(":").replace(/\\:/g, ":")
        return eth.output !== "" ? "wired " + eth.output : "offline"
    }
    readonly property bool tempHot: readings.hot === true
    readonly property string tempText: (readings.temps || []).map(t => t.label + " " + t.value).join("  ")
    readonly property string customText: customPoll.output.split("\n")[0]

    // volume and brightness are lettered for a few seconds after they change (volume stays while muted)
    property bool settled: false
    Timer { interval: 3000; running: true; onTriggered: root.settled = true }
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

        PanelWindow {
            id: edge
            required property var modelData
            screen: modelData
            anchors { top: true; left: true; right: true }
            implicitHeight: 2                       // logical px (3 px at 140 DPI): i3's dock area, allowed for in the top gap
            color: "transparent"
            readonly property real dpr: devicePixelRatio > 0 ? devicePixelRatio : 1
            readonly property real mm: theme.mmFor(modelData.name) / dpr   // logical px per millimetre of this screen
            readonly property real margin: theme.marginPx / dpr
            readonly property real fontPx: Math.min(margin * 0.5, 3.2 * mm)

            readonly property var monitor: { I3.monitors.values; return I3.monitorFor(modelData) }
            readonly property var spaces: I3.workspaces.values
                .filter(w => w.monitor && edge.monitor && w.monitor.name === edge.monitor.name)
                .sort((a, b) => a.number - b.number)
            readonly property var activeSpace: I3.workspaces.values.find(w => w.monitor && edge.monitor && w.monitor.name === edge.monitor.name && w.active) || null

            TitleBlock { shell: root; edge: edge; spaceName: root.spaceLabel(edge.activeSpace ? edge.activeSpace.name : "") }
            SideCells { shell: root; edge: edge; space: edge.activeSpace ? edge.activeSpace.name : "" }

            // ---- top margin: workspaces in the first zone | alerts, volume, brightness, tray in the last ----
            Margin {
                id: topMargin
                edge: edge; ink: root.ink; inkAlpha: theme.inkAlpha
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
                                text: root.spaceLabel(modelData.name)
                                // focused: bold black; shown on its screen but not focused: bold in the others'
                                // ink; urgent: bold in the alert red; the rest: regular, lighter
                                font.family: Fonts.mono; font.pixelSize: edge.fontPx
                                font.bold: modelData.focused || modelData.active || modelData.urgent
                                color: modelData.urgent ? theme.alert : modelData.focused ? "black"
                                     : Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.6)
                            }
                            Rectangle {     // separator: the full height of the margin, like the zone ticks
                                visible: index > 0 && !(spacesZone.slotted && index % spacesZone.perZone === 0)
                                width: Math.max(1, 1 / edge.dpr); height: parent.height
                                color: topMargin.tick
                            }
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: modelData.activate() }
                        }
                    }
                }
                Zone {
                    band: topMargin; index: -1
                    Row {
                        anchors.centerIn: parent
                        spacing: 4 * edge.mm
                        Cell { visible: root.tempHot; size: edge.fontPx; text: root.tempText; color: theme.alert }
                        Cell { visible: root.batteryLow; size: edge.fontPx; text: "BAT " + Math.round(root.battery.percentage * 100) + "%"; color: theme.alert }
                        Cell {
                            readonly property var a: root.audio
                            visible: a !== null && (root.volumeLit || a.muted)
                            size: edge.fontPx
                            text: !a ? "" : a.muted ? "VOL muted" : "VOL " + Math.round(a.volume * 100) + "%"
                            color: root.ink
                            onLeft: () => a.muted = !a.muted
                            onScroll: d => a.volume = Math.max(0, Math.min(1.5, a.volume + d * 0.05))
                        }
                        Cell {
                            visible: root.brightness >= 0 && root.brightnessLit
                            size: edge.fontPx
                            text: "BRT " + Math.round(root.brightness * 100) + "%"
                            color: root.ink
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

            // ---- bottom margin: Bluetooth in the first zone | screenshot, theme in the last ----
            Margin {
                id: bottomMargin
                edge: edge; ink: root.ink; inkAlpha: theme.inkAlpha
                side: "bottom"
                Zone {
                    band: bottomMargin; index: 0
                    Cell {
                        anchors.centerIn: parent
                        size: edge.fontPx
                        text: "BT " + ((root.readings.bt || []).length ? root.readings.bt.join(", ") : "-")
                        color: root.ink
                        onLeft: () => root.run(["blueman-manager"])
                    }
                }
                Zone {
                    band: bottomMargin; index: -1
                    Row {
                        anchors.centerIn: parent
                        spacing: 4 * edge.mm
                        Cell {
                            visible: (theme.commands.screenshot || []).length > 0
                            size: edge.fontPx; text: "SHOT"; color: root.ink
                            onLeft: () => root.run(theme.commands.screenshot)
                        }
                        Cell {      // the next theme
                            size: edge.fontPx; text: theme.label.toUpperCase(); color: root.ink
                            onLeft: () => root.run([root.bin, "theme", "next"])
                        }
                    }
                }
            }
        }
    }
}
