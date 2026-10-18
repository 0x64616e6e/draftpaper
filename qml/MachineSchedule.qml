// The machine schedule (issue #49, one per screen): what neofetch tells, as a drawing's schedule of
// equipment, ruled, on an empty workspace's sheet: REF, ITEM, DESCRIPTION rows for the machine, processor,
// graphics, memory, storage, system and desktop (`draftpaper machine`, polled by shell.qml), each display
// (live, from the compositor) and the sheet. On the bottom layer, under every window; with a wallpaper, on
// the theme's paper tinted as the title block is.
//
// Where: [sheet] machine_at, a zone ("A1", the default: 7 mm into it) or millimetres from the frame's
// corner ([12, 30]), as dragging it leaves it: it follows the pointer a whole millimetre at a time, never
// onto the title block or the plotted load, and on release config.toml keeps the place. Everything in it
// is whole millimetres from the frame's corner (sheet.js; docs/wayland-design.md, "Invariants: drawing on
// the sheet"). [sheet] machine = false takes it away.
import QtQuick
import Quickshell
import Quickshell.Wayland
import "sheet.js" as Sheet

Item {
    id: sched
    required property var shell
    required property var edge
    property var other: null              // the plotted load (LoadGraph.qml): never on top of it

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property var m: shell.machine
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property color rule: Qt.rgba(ink.r, ink.g, ink.b, 0.55)
    readonly property real hair: 1 / dpr
    readonly property real heavy: Math.max(2, Math.round(0.35 * mm * dpr)) / dpr

    readonly property bool shown: theme.sheet.machine !== false && !!out && !!edge.activeSpace && edge.activeSpace.windows === 0
        && m !== null && rows.length > 0 && wMm >= 70

    // ---- the rows: REF, ITEM, DESCRIPTION and a note; a heavier rule where the REF's letter changes ----
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
        // each row's top, in whole millimetres from the schedule's: the title 7, the heads 5, a row 6 (9
        // with a note)
        let y = 12
        return r.map((row, i) => {
            const h = row.note ? 9 : 6
            const o = Object.assign(row, { heavyAbove: i > 0 && r[i - 1].ref[0] !== row.ref[0], yMm: y, hMm: h })
            y += h
            return o
        })
    }
    readonly property int hMm: rows.length ? rows[rows.length - 1].yMm + rows[rows.length - 1].hMm : 0
    readonly property var frame: Sheet.frameMm(out)

    // ---- where: whole millimetres from the frame's corner ----
    readonly property var place: {
        const at = theme.sheet.machine_at
        if (Array.isArray(at) && at.length === 2) return { x: Math.round(at[0]), y: Math.round(at[1]) }
        return Sheet.zoneCorner(out, at || "A1", 7) || Sheet.zoneCorner(out, "A1", 7) || { x: 7, y: 7 }
    }
    property var dragged: null                         // the place while it is being dragged
    readonly property int xMm: (dragged || place).x
    readonly property int yMm: (dragged || place).y
    // the title block's left edge and top, in millimetres from the frame's corner (its rect is the output's)
    readonly property var titleMm: out && out.title && out.title.rect
        ? { x: Math.floor((out.title.rect.x - (out.frame.x - out.x)) / mm), y: Math.floor((out.title.rect.y - (out.frame.y - out.y)) / mm) } : null
    // 112 mm wide; narrower where the frame, or the title block beside it (6 mm clear), ends sooner
    readonly property int wMm: {
        let right = frame.w - 2
        if (titleMm && yMm + hMm > titleMm.y - 2) right = Math.min(right, titleMm.x - 6)
        return Math.max(0, Math.min(112, right - xMm))
    }
    // a place for it: inside the frame by 2 mm, off the title block and 4 mm clear of the other note
    function clear(x, y) {
        const w = 112
        if (x < 2 || y < 2 || x + 70 > frame.w - 2 || y + hMm > frame.h - 2) return false
        if (titleMm && x + w > titleMm.x - 6 && y + hMm > titleMm.y - 2 && x + 70 > titleMm.x - 6) return false
        if (other && other.shown && x < other.xMm + other.wMm + 4 && other.xMm < x + w + 4
            && y < other.yMm + other.hMm + 4 && other.yMm < y + hMm + 4) return false
        return true
    }

    // the surface: its corner on a whole logical and physical pixel, at or before the schedule's own
    readonly property int sx: out ? Sheet.surfaceAt(Sheet.at(out, dpr, xMm, true), dpr) : 0
    readonly property int sy: out ? Sheet.surfaceAt(Sheet.at(out, dpr, yMm, false), dpr) : 0
    function ix(u) { return out ? Sheet.at(out, dpr, xMm + u, true) - sx : 0 }   // u mm into it, across
    function iy(v) { return out ? Sheet.at(out, dpr, yMm + v, false) - sy : 0 }  // v mm into it, down

    PanelWindow {
        screen: sched.edge.screen
        visible: sched.shown
        anchors { top: true; left: true }
        margins { top: sched.sy; left: sched.sx }
        implicitWidth: Math.ceil(sched.ix(sched.wMm)) + 1
        implicitHeight: Math.max(1, Math.ceil(sched.iy(sched.hMm)) + 1)
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        WlrLayershell.namespace: "draftpaper-machine"

        Rectangle {     // with a wallpaper: the paper under it
            x: sched.ix(0); y: sched.iy(0); width: sched.ix(sched.wMm) - x; height: sched.iy(sched.hMm) - y
            visible: sched.theme.pictured
            color: Qt.rgba(sched.theme.paper.r, sched.theme.paper.g, sched.theme.paper.b, Math.max(0.8, sched.theme.titleTint))
        }
        // the title: what it is, and whose
        Text {
            x: sched.ix(2); y: sched.iy(0); height: sched.iy(7) - y
            verticalAlignment: Text.AlignVCenter
            text: "SCHEDULE OF EQUIPMENT"
            font.family: Fonts.mono; font.pixelSize: 2.8 * sched.mm; font.bold: true; font.letterSpacing: 0.25 * sched.mm
            color: sched.ink
        }
        Text {
            x: sched.ix(sched.wMm - 2) - width; y: sched.iy(0); height: sched.iy(7) - y
            verticalAlignment: Text.AlignVCenter
            text: String(sched.theme.title.project || "").toUpperCase() + " · SHEET " + (sched.edge.activeSpace ? sched.edge.activeSpace.number : 1)
            font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
            color: sched.faint
        }
        Rectangle { x: sched.ix(0); y: sched.iy(7); width: sched.ix(sched.wMm) - x; height: sched.heavy; color: sched.ink }
        // the columns' heads: REF 9 mm, ITEM 27 mm, DESCRIPTION the rest
        Repeater {
            model: [["REF", 0], ["ITEM", 9], ["DESCRIPTION", 36]]
            Text {
                required property var modelData
                x: sched.ix(modelData[1] + 1.6); y: sched.iy(7); height: sched.iy(12) - y
                verticalAlignment: Text.AlignVCenter
                text: modelData[0]
                font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                color: sched.faint
            }
        }
        Rectangle { x: sched.ix(0); y: sched.iy(12); width: sched.ix(sched.wMm) - x; height: sched.hair; color: sched.ink }
        Repeater {
            model: sched.rows
            Item {
                id: row
                required property var modelData
                // the rule under it; a heavier one above where a group starts
                Rectangle {
                    x: sched.ix(0); y: sched.iy(row.modelData.yMm + row.modelData.hMm)
                    width: sched.ix(sched.wMm) - x; height: sched.hair; color: sched.rule
                }
                Rectangle {
                    visible: row.modelData.heavyAbove
                    x: sched.ix(0); y: sched.iy(row.modelData.yMm); width: sched.ix(sched.wMm) - x; height: sched.heavy; color: sched.ink
                }
                Text {
                    x: sched.ix(1.6); y: sched.iy(row.modelData.yMm + 1.5)
                    text: row.modelData.ref
                    font.family: Fonts.mono; font.pixelSize: 2.5 * sched.mm; font.bold: true
                    color: sched.ink
                }
                Text {
                    x: sched.ix(10.6); y: sched.iy(row.modelData.yMm + 1.7); width: sched.ix(35) - x
                    text: row.modelData.item
                    elide: Text.ElideRight
                    font.family: Fonts.mono; font.pixelSize: 2.1 * sched.mm; font.letterSpacing: 0.15 * sched.mm
                    color: sched.ink
                }
                Text {
                    x: sched.ix(37.6); y: sched.iy(row.modelData.yMm + 1.4); width: sched.ix(sched.wMm - 1.6) - x
                    text: row.modelData.text
                    elide: Text.ElideRight
                    font.family: Fonts.mono; font.pixelSize: 2.6 * sched.mm
                    color: sched.ink
                }
                Text {
                    visible: row.modelData.note !== ""
                    x: sched.ix(37.6); y: sched.iy(row.modelData.yMm + 5); width: sched.ix(sched.wMm - 1.6) - x
                    text: row.modelData.note
                    elide: Text.ElideRight
                    font.family: Fonts.mono; font.pixelSize: 2.1 * sched.mm
                    color: sched.faint
                }
            }
        }
        // the columns' rules, from the heads down
        Repeater {
            model: [9, 36]
            Rectangle {
                required property int modelData
                x: sched.ix(modelData); y: sched.iy(7); width: sched.hair; height: sched.iy(sched.hMm) - y; color: sched.rule
            }
        }
        // the outline, heavier, over the rows' own rules
        Rectangle {
            x: sched.ix(0); y: sched.iy(0); width: sched.ix(sched.wMm) - x; height: sched.iy(sched.hMm) - y
            color: "transparent"
            border.width: sched.heavy; border.color: sched.ink
        }

        // dragged a whole millimetre at a time to a clear place; config.toml keeps it on release
        MouseArea {
            anchors.fill: parent
            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            property point from
            property var start: null
            onPressed: e => { from = mapToGlobal(e.x, e.y); start = { x: sched.xMm, y: sched.yMm }; keep.stop(); sched.dragged = start }
            onPositionChanged: e => {
                if (!pressed || !start) return
                const p = mapToGlobal(e.x, e.y)
                const x = start.x + Math.round((p.x - from.x) / sched.mm), y = start.y + Math.round((p.y - from.y) / sched.mm)
                if (sched.clear(x, y)) sched.dragged = { x: x, y: y }
            }
            onReleased: {
                const d = sched.dragged
                if (d && start && (d.x !== start.x || d.y !== start.y)) sched.shell.setConfig(["sheet.machine_at=[" + d.x + ", " + d.y + "]"])
                keep.restart()
            }
        }
        // the place dragged to holds until config.toml says it (theme.json follows in a moment)
        Timer { id: keep; interval: 1500; onTriggered: sched.dragged = null }
    }
}
