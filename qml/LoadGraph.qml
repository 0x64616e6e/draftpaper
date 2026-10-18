// The plotted load (issue #49, one per screen): what top's meters tell, as a drawing's figure: the
// processor's load and the memory in use (0 to 100 %, left) and the processor's temperature (30 to 100 °C,
// right, in the alert ink) over the last ten minutes, on millimetre graph paper with every tenth line
// heavier, each line lettered at its end. From the readings the title block already has (shell.qml keeps
// a sample every 5 s): no polling of its own. On the bottom layer, under every window; with a wallpaper,
// on the theme's paper.
//
// Where: [sheet] load_at, a zone ("C1": 7 mm into it) or millimetres from the frame's corner ([12, 100]),
// as dragging it leaves it; without one, in the bottom left, under the machine schedule. 150 mm by 72 mm,
// smaller where the sheet, the title block or the schedule leave less; not shown under 100 by 40. All of
// it whole millimetres from the frame's corner (sheet.js). [sheet] load = false takes it away.
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
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property var samples: shell.loadHistory
    readonly property int span: 120                    // samples across: ten minutes at one every 5 s
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
        const low = frame.h - 7 - 72
        const tries = [{ x: 7, y: low }]
        if (scheduleUp) tries.push({ x: other.xMm + other.wMm + 8, y: low }, { x: 7, y: other.yMm + other.hMm + 8 })
        tries.push({ x: 7, y: 7 })
        return tries.find(p => clear(p.x, p.y)) || tries[0]
    }
    property var dragged: null
    readonly property int xMm: (dragged || place).x
    readonly property int yMm: (dragged || place).y
    // the room it has from (x, y), up to 150 by 72 mm: inside the frame (2 mm, 7 at the bottom), giving way
    // to the title block (6 mm clear beside it, 2 above it) and the schedule (4 mm clear), along whichever
    // edge keeps more of it
    function room(x, y) {
        let right = frame.w - 2, bottom = frame.h - 7
        const obstacles = []
        if (titleMm) obstacles.push({ x: titleMm.x - 6, y: titleMm.y - 2, r: frame.w, b: frame.h })
        if (scheduleUp) obstacles.push({ x: other.xMm - 4, y: other.yMm - 4, r: other.xMm + other.wMm + 4, b: other.yMm + other.hMm + 4 })
        for (const o of obstacles) {
            const r = Math.min(right, x + 150), b = Math.min(bottom, y + 72)
            if (o.r <= x || o.x >= r || o.b <= y || o.y >= b) continue
            const across = o.x > x ? (o.x - x) * (b - y) : -1, down = o.y > y ? (r - x) * (o.y - y) : -1
            if (across < 0 && down < 0) return { w: 0, h: 0 }
            if (across >= down) right = Math.min(right, o.x)
            else bottom = Math.min(bottom, o.y)
        }
        return { w: Math.min(150, right - x), h: Math.min(72, bottom - y) }
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

    // the plot inside it: the title 6 mm and 2 mm under it, the figures of the scales 9 mm either side,
    // the time 6 mm below; whole millimetres, the paper's squares
    readonly property int pxMm: 9
    readonly property int pyMm: 8
    readonly property int pwMm: wMm - 2 * pxMm
    readonly property int phMm: hMm - pyMm - 6

    // the surface: its corner on a whole logical and physical pixel, at or before the figure's own
    readonly property int sx: out ? Sheet.surfaceAt(Sheet.at(out, dpr, xMm, true), dpr) : 0
    readonly property int sy: out ? Sheet.surfaceAt(Sheet.at(out, dpr, yMm, false), dpr) : 0
    function ix(u) { return out ? Sheet.at(out, dpr, xMm + u, true) - sx : 0 }
    function iy(v) { return out ? Sheet.at(out, dpr, yMm + v, false) - sy : 0 }
    // the plot's values to where they are drawn (lines between the millimetres, not things placed)
    function xAt(i) { return ix(pxMm) + (ix(pxMm + pwMm) - ix(pxMm)) * (1 - (samples.length - 1 - i) / span) }
    function yPct(v) { return iy(pyMm + phMm) - Math.max(0, Math.min(1, v)) * (iy(pyMm + phMm) - iy(pyMm)) }
    function yTemp(c) { return yPct((c - 30) / 70) }
    readonly property var last: samples.length ? samples[samples.length - 1] : null

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

        Rectangle {
            x: graph.ix(0); y: graph.iy(0); width: graph.ix(graph.wMm) - x; height: graph.iy(graph.hMm) - y
            visible: graph.theme.pictured
            color: Qt.rgba(graph.theme.paper.r, graph.theme.paper.g, graph.theme.paper.b, Math.max(0.8, graph.theme.titleTint))
        }
        Text {
            x: graph.ix(0); y: graph.iy(0); height: graph.iy(6) - y
            verticalAlignment: Text.AlignVCenter
            text: "LOAD · THE LAST 10 MINUTES"
            font.family: Fonts.mono; font.pixelSize: 2.8 * graph.mm; font.bold: true; font.letterSpacing: 0.25 * graph.mm
            color: graph.ink
        }
        Text {
            x: graph.ix(graph.wMm) - width; y: graph.iy(0); height: graph.iy(6) - y
            verticalAlignment: Text.AlignVCenter
            text: "FIG. 1 · 1 MIN = " + (graph.pwMm / 10) + " MM"
            font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm; font.letterSpacing: 0.3 * graph.mm
            color: graph.faint
        }
        Canvas {
            id: paper
            anchors.fill: parent
            renderStrategy: Canvas.Cooperative
            onPaint: {
                const c = getContext("2d"), g = graph, hair = 1 / g.dpr
                c.reset()
                const mid = v => v + hair / 2                       // a 1-pixel line on its pixel
                const ink = (a) => Qt.rgba(g.ink.r, g.ink.g, g.ink.b, a)
                const left = g.ix(g.pxMm), right = g.ix(g.pxMm + g.pwMm), top = g.iy(g.pyMm), bottom = g.iy(g.pyMm + g.phMm)
                // the squares: a millimetre each, every tenth heavier, counted from the bottom left
                c.lineWidth = hair
                for (const heavy of [false, true]) {
                    c.strokeStyle = ink(heavy ? 0.32 : 0.12)
                    c.beginPath()
                    for (let i = 0; i <= g.pwMm; i++) {
                        if ((i % 10 === 0) !== heavy) continue
                        const x = mid(g.ix(g.pxMm + i)); c.moveTo(x, top); c.lineTo(x, bottom)
                    }
                    for (let i = 0; i <= g.phMm; i++) {
                        if ((i % 10 === 0) !== heavy) continue
                        const y = mid(g.iy(g.pyMm + g.phMm - i)); c.moveTo(left, y); c.lineTo(right, y)
                    }
                    c.stroke()
                }
                c.strokeStyle = ink(1); c.lineWidth = 2 * hair
                c.strokeRect(left + hair, top + hair, right - left, bottom - top)
                // the readings: the processor solid, the memory dashed, the temperature in the alert ink
                const line = (key, at, colour, width, dash) => {
                    c.strokeStyle = colour; c.lineWidth = width; c.setLineDash(dash)
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
                line("cpu", g.yPct, ink(1), 0.4 * g.mm, [])
                line("mem", g.yPct, ink(1), 0.3 * g.mm, [1.4 * g.mm, 0.7 * g.mm])
                line("temp", g.yTemp, g.alert, 0.3 * g.mm, [])
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
        // the scales' figures: % on the left, °C on the right, the time below
        Repeater {
            model: [0, 25, 50, 75, 100]
            Text {
                required property int modelData
                x: graph.ix(graph.pxMm - 1) - width; y: graph.yPct(modelData / 100) - height / 2
                text: modelData === 100 ? "100%" : String(modelData)
                font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm
                color: graph.ink
            }
        }
        Repeater {
            model: [30, 50, 70, 90]
            Text {
                required property int modelData
                x: graph.ix(graph.pxMm + graph.pwMm + 1); y: graph.yTemp(modelData) - height / 2
                text: modelData + (modelData === 90 ? "°" : "")
                font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm
                color: graph.alert
            }
        }
        Repeater {
            model: [10, 8, 6, 4, 2, 0]
            Text {
                required property int modelData
                x: graph.ix(graph.pxMm + graph.pwMm - modelData / 10 * graph.pwMm) - width / 2; y: graph.iy(graph.pyMm + graph.phMm + 1)
                text: modelData === 0 ? "now" : "−" + modelData + " min"
                font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm
                color: graph.ink
            }
        }
        // each line lettered at its end, just above it, inside the plot; kept apart
        Repeater {
            model: {
                const l = graph.last
                if (!l) return []
                const out = []
                if (l.cpu >= 0) out.push({ y: graph.yPct(l.cpu), text: "CPU " + Math.round(l.cpu * 100) + "%", colour: graph.ink, bold: true })
                if (l.mem >= 0) out.push({ y: graph.yPct(l.mem), text: "MEMORY " + Math.round(l.mem * 100) + "%", colour: graph.ink, bold: false })
                if (l.temp >= 0) out.push({ y: graph.yTemp(l.temp), text: "CPU " + Math.round(l.temp) + "°", colour: graph.alert, bold: false })
                out.sort((a, b) => a.y - b.y)
                const gap = 3.2 * graph.mm
                for (let i = 1; i < out.length; i++) out[i].y = Math.max(out[i].y, out[i - 1].y + gap)
                return out
            }
            Text {
                required property var modelData
                x: graph.ix(graph.pxMm + graph.pwMm - 1) - width
                y: Math.max(graph.iy(graph.pyMm), Math.min(graph.iy(graph.pyMm + graph.phMm) - height, modelData.y - height - 0.6 * graph.mm))
                text: modelData.text
                font.family: Fonts.mono; font.pixelSize: 2.1 * graph.mm; font.bold: modelData.bold
                color: modelData.colour
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
