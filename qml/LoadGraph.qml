// The plotted load (issue #49, one per screen): what top's meters tell, blended into the sheet: the
// processor's load and the memory in use (0 to 100 %) and the processor's temperature (30 to 100 °C, in the
// alert ink) over the last ten minutes, on millimetre squares (every tenth heavier) that fade out toward
// their edges, to nothing at every edge: no title, no figures, no frame, no margins; each line lettered
// at its end with its name and its value now.
// From the readings the title block already has (shell.qml keeps a sample every 5 s): no polling of its
// own. On the bottom layer, under every window; over a wallpaper, each line on a halo of the paper.
//
// Where: [sheet] load_at, a zone ("C1": 7 mm into it) or millimetres from the frame's corner ([12, 100]),
// as dragging it leaves it; without one, in the bottom left, under the machine schedule. Up to 210 mm by
// 72 mm, smaller where the sheet, the title block or the schedule leave less; not shown under 100 by 40.
// All of it whole millimetres from the frame's corner (sheet.js). [sheet] load = false takes it away.
import QtQuick
import Quickshell
import Quickshell.Wayland
import "sheet.js" as Sheet

Item {
    id: graph
    required property var shell
    required property var edge
    required property var other           // the machine schedule (MachineSchedule.qml): never on top of it

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color alert: theme.alert
    readonly property var samples: shell.loadHistory
    readonly property int span: 120                    // samples across: ten minutes at one every 5 s
    readonly property int maxW: 210
    readonly property int maxH: 72
    readonly property var frame: Sheet.frameMm(out)
    readonly property var titleMm: out && out.title && out.title.rect
        ? { x: Math.floor((out.title.rect.x - (out.frame.x - out.x)) / mm), y: Math.floor((out.title.rect.y - (out.frame.y - out.y)) / mm) } : null
    readonly property bool scheduleUp: other.shown

    // ---- where and how large: whole millimetres from the frame's corner ----
    readonly property var place: {
        const at = theme.sheet.load_at
        if (Array.isArray(at) && at.length === 2) return { x: Math.round(at[0]), y: Math.round(at[1]) }
        const z = Sheet.zoneCorner(out, at, 7)
        if (z) return z
        // without one: the first clear of the bottom left, beside the schedule, under it, the top left
        const low = frame.h - 7 - maxH
        const tries = [{ x: 7, y: low }]
        if (scheduleUp) tries.push({ x: other.xMm + other.wMm + 8, y: low }, { x: 7, y: other.yMm + other.hMm + 8 })
        tries.push({ x: 7, y: 7 })
        return tries.find(p => clear(p.x, p.y)) || tries[0]
    }
    property var dragged: null
    readonly property int xMm: (dragged || place).x
    readonly property int yMm: (dragged || place).y
    // the room it has from (x, y), up to maxW by maxH: inside the frame (2 mm, 7 at the bottom), giving way
    // to the title block (6 mm clear beside it, 2 above it) and the schedule (4 mm clear), along whichever
    // edge keeps more of it
    function room(x, y) {
        let right = frame.w - 2, bottom = frame.h - 7
        const obstacles = []
        if (titleMm) obstacles.push({ x: titleMm.x - 6, y: titleMm.y - 2, r: frame.w, b: frame.h })
        if (scheduleUp) obstacles.push({ x: other.xMm - 4, y: other.yMm - 4, r: other.xMm + other.wMm + 4, b: other.yMm + other.hMm + 4 })
        for (const o of obstacles) {
            const r = Math.min(right, x + maxW), b = Math.min(bottom, y + maxH)
            if (o.r <= x || o.x >= r || o.b <= y || o.y >= b) continue
            const across = o.x > x ? (o.x - x) * (b - y) : -1, down = o.y > y ? (r - x) * (o.y - y) : -1
            if (across < 0 && down < 0) return { w: 0, h: 0 }
            if (across >= down) right = Math.min(right, o.x)
            else bottom = Math.min(bottom, o.y)
        }
        return { w: Math.min(maxW, right - x), h: Math.min(maxH, bottom - y) }
    }
    readonly property var size: room(xMm, yMm)
    readonly property int wMm: Math.max(0, size.w)
    readonly property int hMm: Math.max(0, size.h)
    // a place for it: inside the frame, with room enough (100 by 40 mm) clear of everything else
    function clear(x, y) {
        if (x < 2 || y < 2) return false
        const r = room(x, y)
        return r.w >= 100 && r.h >= 40
    }

    // which readings: "load", "temperature", "power", "battery" (a click on their cell in the title block),
    // or none
    property string domain: ""
    readonly property bool shown: domain !== "" && !!out && wMm >= 100 && hMm >= 40 && samples.length > 0
    property real fade: shown ? 1 : 0
    Behavior on fade { enabled: graph.theme.animations; NumberAnimation { duration: 240; easing.type: Easing.InOutQuad } }
    property string drawn: ""                          // the domain drawn, kept while it fades out
    onDomainChanged: if (domain !== "") drawn = domain
    // the highest of a reading in the ten minutes, for a scale that fits it
    function most(key) { return samples.reduce((m, s) => Math.max(m, s[key] === undefined ? -1 : s[key]), -1) }
    // each line of the domain: the sample's key, its scale (min, max), its ink, weight (mm) and dash, and how
    // its name and value are lettered at its end
    readonly property var series: {
        const dash = [1.4 * mm, 0.7 * mm], dots = [0.3 * mm, 0.9 * mm]
        if (drawn === "load") return [
            { key: "cpu", min: 0, max: 1, colour: ink, w: 0.4, dash: [], bold: true, text: v => "CPU " + Math.round(v * 100) + "%" },
            { key: "mem", min: 0, max: 1, colour: ink, w: 0.3, dash: dash, text: v => "MEMORY " + Math.round(v * 100) + "%" }]
        if (drawn === "temperature") return [
            { key: "temp", min: 30, max: 100, colour: alert, w: 0.4, dash: [], bold: true, text: v => "CPU " + Math.round(v) + "°" },
            { key: "nvme", min: 30, max: 100, colour: ink, w: 0.3, dash: dash, text: v => "NVME " + Math.round(v) + "°" },
            { key: "fan", min: 0, max: Math.max(6000, Math.ceil(most("fan") / 1000) * 1000), colour: ink, w: 0.3, dash: dots, text: v => "FAN " + Math.round(v) + " RPM" }]
        if (drawn === "power") return [
            { key: "power", min: 0, max: Math.max(20, Math.ceil(most("power") / 10) * 10), colour: ink, w: 0.4, dash: [], bold: true, text: v => "POWER " + v.toFixed(1) + " W" }]
        if (drawn === "battery") return [
            { key: "battery", min: 0, max: 1, colour: ink, w: 0.4, dash: [], bold: true, text: v => "BATTERY " + Math.round(v * 100) + "%" }]
        return []
    }
    // nothing of the domain in the ten minutes (the power on mains, no battery): said where the line would be
    readonly property string none: drawn === "" || series.some(s => samples.some(x => x[s.key] !== undefined && x[s.key] >= 0)) ? ""
        : drawn === "power" ? "POWER · ON MAINS, NOTHING DRAWN TO PLOT" : drawn === "battery" ? "BATTERY · NONE" : ""

    // the surface: its corner on a whole logical and physical pixel, at or before the graph's own
    readonly property int sx: out ? Sheet.surfaceAt(Sheet.at(out, dpr, xMm, true), dpr) : 0
    readonly property int sy: out ? Sheet.surfaceAt(Sheet.at(out, dpr, yMm, false), dpr) : 0
    function ix(u) { return out ? Sheet.at(out, dpr, xMm + u, true) - sx : 0 }
    function iy(v) { return out ? Sheet.at(out, dpr, yMm + v, false) - sy : 0 }
    // the readings to where they are drawn (lines between the millimetres, not things placed); 2 mm
    // inside the top and the bottom, so a full or an idle processor stays clear of the edge
    function xAt(i) { return ix(0) + (ix(wMm) - ix(0)) * (1 - (samples.length - 1 - i) / span) }
    function yPct(v) { return iy(hMm - 2) - Math.max(0, Math.min(1, v)) * (iy(hMm - 2) - iy(2)) }
    function yOf(v, s) { return yPct((v - s.min) / (s.max - s.min)) }

    PanelWindow {
        screen: graph.edge.screen
        visible: graph.fade > 0
        anchors { top: true; left: true }
        margins { top: graph.sy; left: graph.sx }
        implicitWidth: Math.max(1, Math.ceil(graph.ix(graph.wMm)) + 1)
        implicitHeight: Math.max(1, Math.ceil(graph.iy(graph.hMm)) + 1)
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        WlrLayershell.namespace: "draftpaper-load"

        Canvas {
            id: paper
            anchors.fill: parent
            opacity: graph.fade
            renderStrategy: Canvas.Cooperative
            onPaint: {
                const c = getContext("2d"), g = graph, hair = 1 / g.dpr
                c.reset()
                const mid = v => v + hair / 2                       // a 1-pixel line on its pixel
                const ink = (a) => Qt.rgba(g.ink.r, g.ink.g, g.ink.b, a)
                const left = g.ix(0), right = g.ix(g.wMm), top = g.iy(0), bottom = g.iy(g.hMm)
                // the squares: a millimetre each, every tenth heavier, on the sheet's own millimetres: counted from
                // the frame's corner, as its 5 mm dots are (every fifth line on them, the heavy ones on every
                // second dot)
                c.lineWidth = hair
                for (const heavy of [false, true]) {
                    c.strokeStyle = ink(heavy ? 0.32 : 0.13)
                    c.beginPath()
                    for (let i = 0; i <= g.wMm; i++) {
                        if (((g.xMm + i) % 10 === 0) !== heavy) continue
                        const x = mid(g.ix(i)); c.moveTo(x, top); c.lineTo(x, bottom)
                    }
                    for (let i = 0; i <= g.hMm; i++) {
                        if (((g.yMm + i) % 10 === 0) !== heavy) continue
                        const y = mid(g.iy(i)); c.moveTo(left, y); c.lineTo(right, y)
                    }
                    c.stroke()
                }
                // ... fading out toward the edges: no line where they end (the readings are drawn after)
                // to nothing at every edge: across over a quarter of the width (30 mm at most), down over a
                // third of the height (20 mm at most), the two multiplied; no edge of its own anywhere
                const w = right - left, h = bottom - top
                const fx = Math.min(30 * g.mm, w * 0.25) / w, fy = Math.min(20 * g.mm, h * 0.35) / h
                c.globalCompositeOperation = "destination-in"
                const across = c.createLinearGradient(left, 0, right, 0)
                across.addColorStop(0, Qt.rgba(0, 0, 0, 0)); across.addColorStop(fx, Qt.rgba(0, 0, 0, 1))
                across.addColorStop(1 - fx, Qt.rgba(0, 0, 0, 1)); across.addColorStop(1, Qt.rgba(0, 0, 0, 0))
                c.fillStyle = across
                c.fillRect(0, 0, width, height)
                const down = c.createLinearGradient(0, top, 0, bottom)
                down.addColorStop(0, Qt.rgba(0, 0, 0, 0)); down.addColorStop(fy, Qt.rgba(0, 0, 0, 1))
                down.addColorStop(1 - fy, Qt.rgba(0, 0, 0, 1)); down.addColorStop(1, Qt.rgba(0, 0, 0, 0))
                c.fillStyle = down
                c.fillRect(0, 0, width, height)
                c.globalCompositeOperation = "source-over"
                // the readings: the processor solid, the memory dashed, the temperature in the alert ink; over a
                // wallpaper each on a halo of the paper first
                const line = (key, at, colour, w, dash) => {
                    c.strokeStyle = colour; c.lineWidth = w; c.setLineDash(dash)
                    c.beginPath()
                    let on = false
                    g.samples.forEach((s, i) => {
                        const v = s[key]
                        if (v === undefined || v < 0) { on = false; return }
                        const x = g.xAt(i), y = at(v)
                        if (x < left) return
                        on ? c.lineTo(x, y) : c.moveTo(x, y)
                        on = true
                    })
                    c.stroke()
                }
                c.lineJoin = "round"; c.lineCap = "round"
                if (g.theme.pictured) {
                    const halo = Qt.rgba(g.theme.paper.r, g.theme.paper.g, g.theme.paper.b, 0.85)
                    for (const s of g.series) line(s.key, v => g.yOf(v, s), halo, (s.w + 0.8) * g.mm, [])
                }
                for (const s of g.series) line(s.key, v => g.yOf(v, s), s.colour, s.w * g.mm, s.dash)
                c.setLineDash([])
            }
            Connections {
                target: graph
                function onSamplesChanged() { paper.requestPaint() }
                function onSeriesChanged() { paper.requestPaint() }
                function onXMmChanged() { paper.requestPaint() }
                function onYMmChanged() { paper.requestPaint() }
                function onWMmChanged() { paper.requestPaint() }
                function onHMmChanged() { paper.requestPaint() }
            }
            Connections { target: graph.theme; function onDataChanged() { paper.requestPaint() } }
        }

        // each line's name and its value now, lettered just above its end, inside the graph; kept apart; over a
        // wallpaper on the paper's halo as the lines are
        Repeater {
            model: {
                const l = graph.samples.length ? graph.samples[graph.samples.length - 1] : null
                if (!l) return []
                const out = []
                for (const s of graph.series) {
                    const v = l[s.key]
                    if (v !== undefined && v >= 0) out.push({ y: graph.yOf(v, s), text: s.text(v), colour: s.colour, bold: !!s.bold })
                }
                if (graph.none) out.push({ y: graph.iy(graph.hMm / 2), text: graph.none, colour: graph.ink, bold: false })
                out.sort((a, b) => a.y - b.y)
                const gap = 3.2 * graph.mm
                for (let i = 1; i < out.length; i++) out[i].y = Math.max(out[i].y, out[i - 1].y + gap)
                return out
            }
            // the lines broken where a name is lettered, as a drawing breaks them: the paper behind it
            Rectangle {
                id: tag
                required property var modelData
                opacity: graph.fade
                readonly property real pad: 0.6 * graph.mm
                width: label.implicitWidth + 2 * pad; height: label.implicitHeight
                x: graph.ix(graph.wMm - 1) - width
                y: Math.max(graph.iy(0), Math.min(graph.iy(graph.hMm) - height, modelData.y - height - 0.6 * graph.mm))
                radius: pad
                color: Qt.rgba(graph.theme.paper.r, graph.theme.paper.g, graph.theme.paper.b, 0.9)
                Text {
                    id: label
                    x: tag.pad
                    text: tag.modelData.text
                    font.family: Fonts.mono; font.pixelSize: 2.1 * graph.mm; font.bold: tag.modelData.bold
                    color: tag.modelData.colour
                }
            }
        }

        // dragged a whole millimetre at a time to a clear place; config.toml keeps it on release
        MouseArea {
            anchors.fill: parent
            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            property point from
            property var start: null
            onPressed: e => { from = mapToGlobal(e.x, e.y); start = { x: graph.xMm, y: graph.yMm }; keep.stop(); graph.dragged = start }
            onPositionChanged: e => {
                if (!pressed || !start) return
                const p = mapToGlobal(e.x, e.y)
                const x = start.x + Math.round((p.x - from.x) / graph.mm), y = start.y + Math.round((p.y - from.y) / graph.mm)
                if (graph.clear(x, y)) graph.dragged = { x: x, y: y }
            }
            onReleased: {
                const d = graph.dragged
                if (d && start && (d.x !== start.x || d.y !== start.y)) graph.shell.setConfig(["sheet.load_at=[" + d.x + ", " + d.y + "]"])
                keep.restart()
            }
        }
        Timer { id: keep; interval: 1500; onTriggered: graph.dragged = null }
    }
}
