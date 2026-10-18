// The machine schedule (issue #49, one per screen): what neofetch tells, as a drawing's schedule of
// equipment, ruled, in the top left of an empty workspace's sheet: REF, ITEM, DESCRIPTION rows for the
// machine, processor, graphics, memory, storage, system and desktop (`draftpaper machine`, polled by
// shell.qml), each display (live, from the compositor) and the sheet. On the bottom layer, under every
// window; with a wallpaper, on the theme's paper tinted as the title block is. [sheet] machine = false
// takes it away (the settings' Sheet page).
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: sched
    required property var shell
    required property var edge

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property var m: shell.machine
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property color rule: Qt.rgba(ink.r, ink.g, ink.b, 0.55)
    function snap(v) { return Math.round(v * dpr) / dpr }
    readonly property real hair: 1 / dpr
    readonly property real heavy: Math.max(2, Math.round(0.35 * mm * dpr)) / dpr

    readonly property bool shown: theme.sheet.machine !== false && !!out && !!edge.activeSpace && edge.activeSpace.windows === 0
        && m !== null && rows.length > 0

    // the rows: REF, ITEM, DESCRIPTION and a note; a heavier rule where the REF's letter changes
    function uptime(s) {
        const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), mi = Math.floor(s % 3600 / 60)
        return "up " + (d > 0 ? d + " d " + h + " h" : h > 0 ? h + " h " + mi + " min" : mi + " min")
    }
    readonly property var rows: {
        if (!m) return []
        const r = []
        const add = (ref, item, text, note) => { if (text) r.push({ ref: ref, item: item, text: text, note: note || "" }) }
        add("M1", "MACHINE", m.machine)
        if (m.cpu) add("M2", "PROCESSOR", m.cpu.model, m.cpu.cores + " cores, " + m.cpu.threads + " threads")
        ;(m.gpus || []).forEach((g, i) => add("M" + (3 + i), "GRAPHICS", g))
        const n = 3 + Math.max(1, (m.gpus || []).length)
        if (m.memory_gib) add("M" + n, "MEMORY", Math.round(m.memory_gib) + " GiB")
        if (m.storage) add("M" + (n + 1), "STORAGE", m.storage.disk,
            [m.storage.fs, m.storage.used_gb !== undefined ? Math.round(m.storage.used_gb) + " of " + Math.round(m.storage.size_gb) + " GB used" : ""].filter(x => x).join(" · "))
        add("S1", "SYSTEM", [m.system, m.kernel].filter(x => x).join(" · "), m.uptime_s ? uptime(m.uptime_s) : "")
        add("S2", "DESKTOP", "Draftpaper " + m.draftpaper + (m.quickshell ? " · Quickshell " + m.quickshell : ""))
        Compositor.outputs.forEach((o, i) => {
            const p = o.physical || [0, 0], inches = Math.hypot(p[0], p[1]) / 25.4
            const mode = o.mode || [Math.round(o.width * o.scale), Math.round(o.height * o.scale)]
            add("D" + (i + 1), "DISPLAY " + o.name,
                (inches > 0 ? inches.toFixed(1) + "″ · " : "") + mode[0] + " × " + mode[1] + " · scale " + o.scale,
                (o.px_per_mm * o.scale).toFixed(1) + " px/mm · " + o.px_per_mm.toFixed(1) + " logical px a mm"
                + (o.model && o.model !== "Unknown" ? " · " + o.model : ""))
        })
        const margin = out ? Math.round(out.margin / out.px_per_mm) : 6
        add("P1", "SHEET", theme.label + " · margin " + margin + " mm",
            (out ? out.zones.across + " × " + out.zones.down + " zones · " : "") + Fonts.mono)
        return r.map((row, i) => Object.assign(row, { heavyAbove: i > 0 && r[i - 1].ref[0] !== row.ref[0] }))
    }

    // the schedule: 7 mm in from the frame's corner, 112 mm wide (less on a small sheet)
    readonly property real frameX: out ? out.frame.x - out.x : 0
    readonly property real frameY: out ? out.frame.y - out.y : 0
    readonly property real w: snap(Math.min(112 * mm, (out ? out.frame.width : 400) * 0.6))
    readonly property real refW: snap(9 * mm)
    readonly property real itemW: snap(27 * mm)
    // the surface's corner on whole logical and physical pixels both, so the rules stay on pixels
    function onPixel(v) { let x = Math.ceil(v); for (let k = 0; k < 4 && Math.abs(x * dpr - Math.round(x * dpr)) > 1e-6; k++) x++; return x }
    readonly property int atX: onPixel(frameX + 7 * mm)
    readonly property int atY: onPixel(frameY + 7 * mm)
    // where it ends, for the plotted load below it (LoadGraph.qml)
    readonly property real endY: atY + table.height

    PanelWindow {
        screen: sched.edge.screen
        visible: sched.shown
        anchors { top: true; left: true }
        margins { top: sched.atY; left: sched.atX }
        implicitWidth: Math.ceil(sched.w)
        implicitHeight: Math.max(1, Math.ceil(table.height))
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        mask: Region {}
        WlrLayershell.namespace: "draftpaper-machine"

        Rectangle {     // with a wallpaper: the paper under it
            anchors.fill: parent
            visible: sched.theme.pictured
            color: Qt.rgba(sched.theme.paper.r, sched.theme.paper.g, sched.theme.paper.b, Math.max(0.8, sched.theme.titleTint))
        }
        Column {
            id: table
            width: sched.w
            // the title: what it is, and whose
            Item {
                width: parent.width; height: sched.snap(7 * sched.mm)
                Text {
                    x: 2 * sched.mm; anchors.verticalCenter: parent.verticalCenter
                    text: "SCHEDULE OF EQUIPMENT"
                    font.family: Fonts.mono; font.pixelSize: 2.8 * sched.mm; font.bold: true; font.letterSpacing: 0.25 * sched.mm
                    color: sched.ink
                }
                Text {
                    anchors.right: parent.right; anchors.rightMargin: 2 * sched.mm; anchors.verticalCenter: parent.verticalCenter
                    text: String(sched.theme.title.project || "").toUpperCase() + " · SHEET " + (sched.edge.activeSpace ? sched.edge.activeSpace.number : 1)
                    font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                    color: sched.faint
                }
            }
            Rectangle { width: parent.width; height: sched.heavy; color: sched.ink }
            // the columns' heads
            Item {
                width: parent.width; height: sched.snap(4.6 * sched.mm)
                Repeater {
                    model: [["REF", 0], ["ITEM", sched.refW], ["DESCRIPTION", sched.refW + sched.itemW]]
                    Text {
                        required property var modelData
                        x: modelData[1] + 1.6 * sched.mm; anchors.verticalCenter: parent.verticalCenter
                        text: modelData[0]
                        font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                        color: sched.faint
                    }
                }
                Rectangle { x: sched.refW; width: sched.hair; height: parent.height; color: sched.rule }
                Rectangle { x: sched.refW + sched.itemW; width: sched.hair; height: parent.height; color: sched.rule }
            }
            Rectangle { width: parent.width; height: sched.hair; color: sched.ink }
            Repeater {
                model: sched.rows
                Item {
                    id: row
                    required property var modelData
                    width: table.width
                    height: sched.snap((modelData.note ? 9 : 5.8) * sched.mm)
                    Rectangle { visible: row.modelData.heavyAbove; width: parent.width; height: sched.heavy - sched.hair; color: sched.ink }
                    Text {
                        x: 1.6 * sched.mm; y: 1.6 * sched.mm
                        text: row.modelData.ref
                        font.family: Fonts.mono; font.pixelSize: 2.5 * sched.mm; font.bold: true
                        color: sched.ink
                    }
                    Text {
                        x: sched.refW + 1.6 * sched.mm; y: 1.8 * sched.mm; width: sched.itemW - 2.4 * sched.mm
                        text: row.modelData.item
                        elide: Text.ElideRight
                        font.family: Fonts.mono; font.pixelSize: 2.1 * sched.mm; font.letterSpacing: 0.15 * sched.mm
                        color: sched.ink
                    }
                    Column {
                        x: sched.refW + sched.itemW + 1.6 * sched.mm; y: 1.5 * sched.mm
                        width: table.width - x - 1.6 * sched.mm
                        spacing: 0.6 * sched.mm
                        Text {
                            width: parent.width; elide: Text.ElideRight
                            text: row.modelData.text
                            font.family: Fonts.mono; font.pixelSize: 2.6 * sched.mm
                            color: sched.ink
                        }
                        Text {
                            visible: row.modelData.note !== ""
                            width: parent.width; elide: Text.ElideRight
                            text: row.modelData.note
                            font.family: Fonts.mono; font.pixelSize: 2.1 * sched.mm
                            color: sched.faint
                        }
                    }
                    Rectangle { x: sched.refW; width: sched.hair; height: parent.height; color: sched.rule }
                    Rectangle { x: sched.refW + sched.itemW; width: sched.hair; height: parent.height; color: sched.rule }
                    Rectangle { y: parent.height - height; width: parent.width; height: sched.hair; color: sched.rule }
                }
            }
        }
        // the outline, heavier, over the rows' own rules
        Rectangle {
            width: sched.w; height: table.height
            color: "transparent"
            border.width: sched.heavy; border.color: sched.ink
        }
    }
}
