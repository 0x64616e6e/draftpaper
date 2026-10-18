// The plotted readings (issue #49, one per screen): what top's meters tell, blended into the sheet, over a
// year on a logarithmic time scale: now at the right, then the last 5 minutes, the hour, the day, the week,
// the month and the year to the left, each period ticked and named at its end (dotted, on the sheet's
// millimetres). The processor's load and the memory in use (0 to 100 %), the temperatures (30 to 100 °C,
// the CPU's in the alert ink) and the fan, the power drawn on battery, the battery's charge: each period
// averaged into its slots (5 s, a minute, a quarter hour, an hour, a quarter day, a day), one line through
// all of them, the bold reading's least to most a band under it; broken where nothing was kept (the
// machine off). On millimetre squares (every tenth heavier) that fade out toward their edges, to nothing at
// every edge: no title, no frame, no margins; each line lettered at its end with its name and its value now.
// From `draftpaper metrics keep` (draftpaper-metrics.service, cli/src/metrics.rs), whose file in the
// runtime directory is read in place, every 5 s while the graph is out: no process of its own. On the
// bottom layer, under every window; over a wallpaper, each line on a halo of the paper.
//
// Where: [sheet] load_at, a zone ("C1": 7 mm into it) or millimetres from the frame's corner ([12, 100]),
// as dragging it leaves it; without one, in the bottom left, under the machine schedule. Up to 210 mm across and a
// zone of the sheet tall, smaller where the sheet, the title block or the schedule leave less; not shown
// under 100 by 40.
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
    readonly property int maxW: 210

    // ---- the readings: the keeper's file, read while the graph is out ----
    Watch {
        id: kept
        path: (Quickshell.env("DRAFTPAPER_RUNTIME") || (Quickshell.env("XDG_RUNTIME_DIR") + "/draftpaper")) + "/metrics.json"
        running: graph.domain !== ""
    }
    // the store and when it was read: ages count from the clock, so a keeper that stopped leaves a gap at now
    readonly property var store: {
        if (!kept.output) return null
        try { const s = JSON.parse(kept.output); return s && s.tiers ? { s: s, at: Date.now() / 1000 } : null } catch (e) { return null }
    }
    // the time scale: from 5 s ago (and now) at the right to a year ago at the left, logarithmic
    readonly property real youngest: 5
    readonly property real oldest: 31536000
    // each period's end, ticked and named
    readonly property var periods: [{ age: 300, name: "5 MIN" }, { age: 3600, name: "1 H" }, { age: 86400, name: "24 H" },
        { age: 604800, name: "7 D" }, { age: 2592000, name: "30 D" }, { age: 31536000, name: "1 Y" }]
    // a reading's points, newest first: { age (s, the slot's middle), avg, min, max, slot (s) }; each period
    // gives the slots between the last's end and its own, so the finer ones are drawn where they are kept and
    // the line goes on through the coarser ones
    function points(key) {
        if (!store) return []
        const s = store.s, m = (s.metrics || []).indexOf(key)
        if (m < 0) return []
        const out = []
        let from = 0
        for (const t of s.tiers) {
            for (const sl of t.slots) {
                const a = sl[1][m]
                if (!a || !a[0]) continue
                const age = store.at - Math.min(sl[0] + t.slot / 2, (sl[0] + s.now) / 2)
                if (age > from && age <= t.upto || (from === 0 && age <= 0)) out.push({ age: Math.max(0, age), avg: a[1] / a[0], min: a[2], max: a[3], slot: t.slot })
            }
            from = t.upto
        }
        return out.sort((a, b) => a.age - b.age)
    }
    // every reading of the domain's lines, worked out once a read
    readonly property var lines: {
        const o = {}
        for (const s of series) o[s.key] = points(s.key)
        return o
    }
    // no taller than a zone of the sheet (whole millimetres), however much room there is: taller, it looked
    // stretched
    readonly property int maxH: out && out.zones ? Math.min(72, Math.floor(out.zones.height / mm + 1e-6)) : 44
    readonly property var frame: Sheet.frameMm(out)
    readonly property var titleMm: out && out.title && out.title.rect
        ? { x: Math.floor((out.title.rect.x - (out.frame.x - out.x)) / mm), y: Math.floor((out.title.rect.y - (out.frame.y - out.y)) / mm) } : null
    readonly property bool scheduleUp: other.shown

    // ---- where and how large: whole millimetres from the frame's corner ----
    readonly property var place: {
        // where config.toml puts it (a zone, or millimetres a drag left: on every display), where it fits on
        // this one: a place that fits on the 4K display may leave too little on the panel
        const at = theme.sheet.load_at
        const set = Array.isArray(at) && at.length === 2 ? { x: Math.round(at[0]), y: Math.round(at[1]) } : Sheet.zoneCorner(out, at, 7)
        if (set && clear(set.x, set.y)) return set
        // else the first clear of the bottom left (the bottom row of zones, from the first whole millimetre under
        // its top line down to the frame's: within its zone), beside the schedule, under it, the top left
        const low = out && out.zones ? Math.ceil(out.zones.height * (out.zones.down - 1) / mm - 1e-6) : frame.h - maxH
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
        // down to the frame's line: its squares fade to nothing before it
        let right = frame.w - 2, bottom = frame.h
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
    readonly property bool shown: domain !== "" && !!out && wMm >= 100 && hMm >= 40
    property real fade: shown ? 1 : 0
    Behavior on fade { enabled: graph.theme.animations; NumberAnimation { duration: 240; easing.type: Easing.InOutQuad } }
    property string drawn: ""                          // the domain drawn, kept while it fades out
    onDomainChanged: if (domain !== "") drawn = domain
    // the highest of a reading kept, for a scale that fits it
    function most(key) { return points(key).reduce((m, p) => Math.max(m, p.max), -1) }
    // each line of the domain: the sample's key, its scale (min, max), its ink, weight (mm) and dash, and how
    // its name and value are lettered at its end
    readonly property var series: {
        const dash = [1.4 * mm, 0.7 * mm], dots = [0.3 * mm, 0.9 * mm]
        if (drawn === "load") return [
            { key: "cpu", name: "CPU", min: 0, max: 1, colour: ink, w: 0.4, dash: [], bold: true, text: v => "CPU " + Math.round(v * 100) + "%" },
            { key: "mem", name: "MEMORY", min: 0, max: 1, colour: ink, w: 0.3, dash: dash, text: v => "MEMORY " + Math.round(v * 100) + "%" }]
        if (drawn === "temperature") return [
            { key: "temp", name: "CPU", min: 30, max: 100, colour: alert, w: 0.4, dash: [], bold: true, text: v => "CPU " + Math.round(v) + "°" },
            { key: "nvme", name: "NVME", min: 30, max: 100, colour: ink, w: 0.3, dash: dash, text: v => "NVME " + Math.round(v) + "°" },
            { key: "fan", name: "FAN", min: 0, max: Math.max(6000, Math.ceil(most("fan") / 1000) * 1000), colour: ink, w: 0.3, dash: dots, text: v => "FAN " + Math.round(v) + " RPM" }]
        if (drawn === "power") return [
            { key: "power", name: "POWER", min: 0, max: Math.max(20, Math.ceil(most("power") / 10) * 10), colour: ink, w: 0.4, dash: [], bold: true, text: v => "POWER " + v.toFixed(1) + " W" }]
        if (drawn === "battery") return [
            { key: "battery", name: "BATTERY", min: 0, max: 1, colour: ink, w: 0.4, dash: [], bold: true, text: v => "BATTERY " + Math.round(v * 100) + "%" }]
        return []
    }
    // nothing to plot, said where the line would be: no keeper (draftpaper-metrics.service), or nothing of the
    // domain kept (the power on mains, no battery)
    readonly property string none: drawn === "" || !kept.ran ? ""
        : !store ? "NOTHING KEPT · DRAFTPAPER METRICS KEEP"
        : series.some(s => (lines[s.key] || []).length > 0) ? ""
        : drawn === "power" ? "POWER · ON MAINS, NOTHING DRAWN TO PLOT" : drawn === "battery" ? "BATTERY · NONE" : "NOTHING KEPT YET"

    // the surface: its corner on a whole logical and physical pixel, at or before the graph's own
    readonly property int sx: out ? Sheet.surfaceAt(Sheet.at(out, dpr, xMm, true), dpr) : 0
    readonly property int sy: out ? Sheet.surfaceAt(Sheet.at(out, dpr, yMm, false), dpr) : 0
    function ix(u) { return out ? Sheet.at(out, dpr, xMm + u, true) - sx : 0 }
    function iy(v) { return out ? Sheet.at(out, dpr, yMm + v, false) - sy : 0 }
    // the readings to where they are drawn (lines between the millimetres, not things placed): an age (s) to
    // its share of the width from the right, logarithmic; 2 mm inside the top and the bottom, so a full or an
    // idle processor stays clear of the edge
    function share(age) { return (Math.log(Math.max(age, youngest)) - Math.log(youngest)) / (Math.log(oldest) - Math.log(youngest)) }
    function xAge(age) { return ix(wMm) - (ix(wMm) - ix(0)) * Math.min(1, share(age)) }
    // a period's end, ticked on the sheet's millimetres: the whole one nearest (in from the edges)
    function tickMm(age) { return Math.max(1, Math.min(wMm - 1, Math.round(wMm * (1 - share(age))))) }
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
                // each period's end: a dotted line down the squares, on the whole millimetre nearest it
                c.strokeStyle = ink(0.55); c.setLineDash([0.3 * g.mm, 0.9 * g.mm])
                c.beginPath()
                for (const p of g.periods) { const x = mid(g.ix(g.tickMm(p.age))); c.moveTo(x, top); c.lineTo(x, bottom) }
                c.stroke()
                c.setLineDash([])
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
                // the readings: the processor solid, the memory dashed, the temperature in the alert ink, each one
                // line from now back through every period, broken where nothing was kept (further apart than half
                // as much again as their slots); the bold one's least to most a band under it; over a wallpaper
                // each on a halo of the paper first
                const runs = key => {
                    const out = []
                    let run = []
                    for (const p of (g.lines[key] || [])) {
                        const last = run[run.length - 1]
                        if (last && p.age - last.age > 1.5 * Math.max(p.slot, last.slot) + 5) { out.push(run); run = [] }
                        run.push(p)
                    }
                    if (run.length) out.push(run)
                    return out
                }
                const line = (s, colour, w, dash) => {
                    c.strokeStyle = colour; c.lineWidth = w; c.setLineDash(dash)
                    c.beginPath()
                    for (const run of runs(s.key)) {
                        run.forEach((p, i) => {
                            const x = g.xAge(p.age), y = g.yOf(p.avg, s)
                            i ? c.lineTo(x, y) : c.moveTo(x, y)
                        })
                        if (run.length === 1) c.lineTo(g.xAge(run[0].age) - hair, g.yOf(run[0].avg, s))
                    }
                    c.stroke()
                }
                for (const s of g.series) {
                    if (!s.bold) continue
                    c.fillStyle = Qt.rgba(s.colour.r, s.colour.g, s.colour.b, 0.12)
                    for (const run of runs(s.key)) {
                        if (run.length < 2) continue
                        c.beginPath()
                        run.forEach((p, i) => { const x = g.xAge(p.age), y = g.yOf(p.max, s); i ? c.lineTo(x, y) : c.moveTo(x, y) })
                        for (let i = run.length - 1; i >= 0; i--) c.lineTo(g.xAge(run[i].age), g.yOf(run[i].min, s))
                        c.closePath()
                        c.fill()
                    }
                }
                c.lineJoin = "round"; c.lineCap = "round"
                if (g.theme.pictured) {
                    const halo = Qt.rgba(g.theme.paper.r, g.theme.paper.g, g.theme.paper.b, 0.85)
                    for (const s of g.series) line(s, halo, (s.w + 0.8) * g.mm, [])
                }
                for (const s of g.series) line(s, s.colour, s.w * g.mm, s.dash)
                c.setLineDash([])
            }
            Connections {
                target: graph
                function onLinesChanged() { paper.requestPaint() }
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
                const out = []
                for (const s of graph.series) {
                    // its newest: its value if read in the last half minute, else its name alone (the keeper stopped)
                    const p = (graph.lines[s.key] || [])[0]
                    if (p) out.push({ y: graph.yOf(p.avg, s), text: p.age < 30 ? s.text(p.avg) : s.name, colour: s.colour, bold: !!s.bold })
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

        // each period named beside its tick, at the foot: on the paper, as the lines' names are
        Repeater {
            model: graph.periods
            Rectangle {
                id: period
                required property var modelData
                opacity: graph.fade
                readonly property real pad: 0.6 * graph.mm
                width: name.implicitWidth + 2 * pad; height: name.implicitHeight
                // just right of its tick: how long ago it is
                x: graph.ix(graph.tickMm(modelData.age)) + 0.4 * graph.mm
                y: graph.iy(graph.hMm - 5)
                visible: x + width < graph.ix(graph.wMm)
                radius: pad
                color: Qt.rgba(graph.theme.paper.r, graph.theme.paper.g, graph.theme.paper.b, 0.9)
                Text {
                    id: name
                    x: period.pad
                    text: period.modelData.name
                    font.family: Fonts.mono; font.pixelSize: 1.8 * graph.mm
                    color: Qt.rgba(graph.ink.r, graph.ink.g, graph.ink.b, 0.7)
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
