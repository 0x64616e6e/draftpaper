// The plotted load (issue #49, one per screen): what top's meters tell, blended into the sheet: the
// processor's load and the memory in use (0 to 100 %) and the processor's temperature (30 to 100 °C, in the
// alert ink) over the last ten minutes, on millimetre squares (every tenth heavier) that fade out toward
// their edges: no title, no figures, no frame, no margins (the title block letters the readings now).
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

    readonly property bool shown: theme.sheet.load !== false && !!out && !!edge.activeSpace && edge.activeSpace.windows === 0
        && wMm >= 100 && hMm >= 40 && samples.length > 0

    // the surface: its corner on a whole logical and physical pixel, at or before the graph's own
    readonly property int sx: out ? Sheet.surfaceAt(Sheet.at(out, dpr, xMm, true), dpr) : 0
    readonly property int sy: out ? Sheet.surfaceAt(Sheet.at(out, dpr, yMm, false), dpr) : 0
    function ix(u) { return out ? Sheet.at(out, dpr, xMm + u, true) - sx : 0 }
    function iy(v) { return out ? Sheet.at(out, dpr, yMm + v, false) - sy : 0 }
    // the readings to where they are drawn (lines between the millimetres, not things placed); 2 mm
    // inside the top and the bottom, so a full or an idle processor stays clear of the edge
    function xAt(i) { return ix(0) + (ix(wMm) - ix(0)) * (1 - (samples.length - 1 - i) / span) }
    function yPct(v) { return iy(hMm - 2) - Math.max(0, Math.min(1, v)) * (iy(hMm - 2) - iy(2)) }
    function yTemp(c) { return yPct((c - 30) / 70) }

    PanelWindow {
        screen: graph.edge.screen
        visible: graph.shown
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
            renderStrategy: Canvas.Cooperative
            onPaint: {
                const c = getContext("2d"), g = graph, hair = 1 / g.dpr
                c.reset()
                const mid = v => v + hair / 2                       // a 1-pixel line on its pixel
                const ink = (a) => Qt.rgba(g.ink.r, g.ink.g, g.ink.b, a)
                const left = g.ix(0), right = g.ix(g.wMm), top = g.iy(0), bottom = g.iy(g.hMm)
                // the squares: a millimetre each, every tenth heavier, counted from the bottom left
                c.lineWidth = hair
                for (const heavy of [false, true]) {
                    c.strokeStyle = ink(heavy ? 0.32 : 0.13)
                    c.beginPath()
                    for (let i = 0; i <= g.wMm; i++) {
                        if ((i % 10 === 0) !== heavy) continue
                        const x = mid(g.ix(i)); c.moveTo(x, top); c.lineTo(x, bottom)
                    }
                    for (let i = 0; i <= g.hMm; i++) {
                        if ((i % 10 === 0) !== heavy) continue
                        const y = mid(g.iy(g.hMm - i)); c.moveTo(left, y); c.lineTo(right, y)
                    }
                    c.stroke()
                }
                // ... fading out toward the edges: no line where they end (the readings are drawn after)
                // an ellipse as the graph is proportioned: 0.62 of its width across, of its height down
                const w = right - left, h = bottom - top
                c.save()
                c.translate((left + right) / 2, top + h * 0.55)
                c.scale(1, h / Math.max(1, w))
                const fade = c.createRadialGradient(0, 0, 0, 0, 0, 0.62 * w)
                fade.addColorStop(0.45, Qt.rgba(0, 0, 0, 1))
                fade.addColorStop(1, Qt.rgba(0, 0, 0, 0))
                c.globalCompositeOperation = "destination-in"
                c.fillStyle = fade
                c.fillRect(-w, -w, 2 * w, 2 * w)
                c.restore()
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
                const readings = [["cpu", g.yPct, ink(1), 0.4, []], ["mem", g.yPct, ink(1), 0.3, [1.4 * g.mm, 0.7 * g.mm]], ["temp", g.yTemp, g.alert, 0.3, []]]
                if (g.theme.pictured) {
                    const halo = Qt.rgba(g.theme.paper.r, g.theme.paper.g, g.theme.paper.b, 0.85)
                    for (const [key, at, , w] of readings) line(key, at, halo, (w + 0.8) * g.mm, [])
                }
                for (const [key, at, colour, w, dash] of readings) line(key, at, colour, w * g.mm, dash)
                c.setLineDash([])
            }
            Connections {
                target: graph
                function onSamplesChanged() { paper.requestPaint() }
                function onXMmChanged() { paper.requestPaint() }
                function onYMmChanged() { paper.requestPaint() }
                function onWMmChanged() { paper.requestPaint() }
                function onHMmChanged() { paper.requestPaint() }
            }
            Connections { target: graph.theme; function onDataChanged() { paper.requestPaint() } }
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
