// The plotted load (issue #49, one per screen): what top's meters tell, as a drawing's figure: the
// processor's load and the memory in use (0 to 100 %, left) and the processor's temperature (30 to 100 °C,
// right, in the alert ink) over the last ten minutes, on millimetre graph paper with every tenth line
// heavier, each line lettered at its end. From the readings the title block already has (shell.qml keeps
// a sample every 5 s): no polling of its own. In the bottom left of an empty workspace's sheet, under the
// machine schedule, on the bottom layer; with a wallpaper, on the theme's paper. [sheet] load = false
// takes it away.
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: graph
    required property var shell
    required property var edge
    required property var above           // the machine schedule (MachineSchedule.qml): the graph stays below it

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color alert: theme.alert
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property var samples: shell.loadHistory
    readonly property int span: 120                    // samples across: 10 minutes at one every 5 s

    // the figure: 150 mm wide (less on a small sheet), 72 mm tall, 7 mm in from the frame's bottom left
    // corner; shorter where the schedule leaves less, and not at all under 40 mm
    readonly property real frameX: out ? out.frame.x - out.x : 0
    readonly property real frameY: out ? out.frame.y - out.y : 0
    readonly property real frameH: out ? out.frame.height : 300
    function onPixel(v) { let x = Math.ceil(v); for (let k = 0; k < 4 && Math.abs(x * dpr - Math.round(x * dpr)) > 1e-6; k++) x++; return x }
    readonly property int atX: onPixel(frameX + 7 * mm)
    readonly property real floor: frameY + frameH - 7 * mm
    readonly property real ceiling: above.theme.sheet.machine !== false && above.shown ? above.endY + 8 * mm : frameY + 7 * mm
    readonly property real h: Math.min(72 * mm, floor - ceiling)
    readonly property int atY: onPixel(floor - h)
    // and 6 mm short of the title block, which takes the frame's bottom right (its rect is the output's own)
    readonly property real titleX: out && out.title && out.title.rect ? out.title.rect.x : (out ? frameX + out.frame.width : 400)
    readonly property real w: Math.round(Math.min(150 * mm, titleX - 6 * mm - atX))

    readonly property bool shown: theme.sheet.load !== false && !!out && !!edge.activeSpace && edge.activeSpace.windows === 0
        && h >= 40 * mm && samples.length > 0

    // the plot inside the figure: room for the figures of the left and right scales, the title, the time
    readonly property real titleH: 6 * mm
    readonly property real px: 9 * mm
    readonly property real py: titleH + 1.5 * mm
    readonly property real pw: Math.floor((w - 2 * px) / mm) * mm      // whole millimetres: the paper's squares
    readonly property real ph: Math.floor((h - py - 6 * mm) / mm) * mm
    function xAt(i) { return px + pw - (samples.length - 1 - i) * pw / span }
    function yPct(v) { return py + ph - Math.max(0, Math.min(1, v)) * ph }
    function yTemp(c) { return py + ph - Math.max(0, Math.min(1, (c - 30) / 70)) * ph }
    readonly property var last: samples.length ? samples[samples.length - 1] : null

    PanelWindow {
        screen: graph.edge.screen
        visible: graph.shown
        anchors { top: true; left: true }
        margins { top: graph.atY; left: graph.atX }
        implicitWidth: Math.max(1, graph.w)
        implicitHeight: Math.max(1, Math.ceil(graph.h))
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        mask: Region {}
        WlrLayershell.namespace: "draftpaper-load"

        Rectangle {
            anchors.fill: parent
            visible: graph.theme.pictured
            color: Qt.rgba(graph.theme.paper.r, graph.theme.paper.g, graph.theme.paper.b, Math.max(0.8, graph.theme.titleTint))
        }
        Text {
            x: 0; y: 0; height: graph.titleH
            verticalAlignment: Text.AlignVCenter
            text: "LOAD · THE LAST 10 MINUTES"
            font.family: Fonts.mono; font.pixelSize: 2.8 * graph.mm; font.bold: true; font.letterSpacing: 0.25 * graph.mm
            color: graph.ink
        }
        Text {
            anchors.right: parent.right; y: 0; height: graph.titleH
            verticalAlignment: Text.AlignVCenter
            text: "FIG. 1 · 1 MIN = " + Math.round(graph.pw / graph.mm / 10) + " MM"
            font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm; font.letterSpacing: 0.3 * graph.mm
            color: graph.faint
        }
        Canvas {
            id: paper
            anchors.fill: parent
            renderStrategy: Canvas.Cooperative
            onPaint: {
                const c = getContext("2d"), g = graph, mm = g.mm, hair = 1 / g.dpr
                c.reset()
                const sx = v => (Math.round(v * g.dpr) + 0.5) / g.dpr        // a 1-pixel line on a pixel
                const ink = (a) => Qt.rgba(g.ink.r, g.ink.g, g.ink.b, a)
                // the squares: a millimetre each, every tenth heavier, counted from the bottom left
                c.lineWidth = hair
                for (const heavy of [false, true]) {
                    c.strokeStyle = ink(heavy ? 0.32 : 0.12)
                    c.beginPath()
                    for (let i = 0; i * mm <= g.pw + 0.01; i++) {
                        if ((i % 10 === 0) !== heavy) continue
                        const x = sx(g.px + i * mm); c.moveTo(x, g.py); c.lineTo(x, g.py + g.ph)
                    }
                    for (let i = 0; i * mm <= g.ph + 0.01; i++) {
                        if ((i % 10 === 0) !== heavy) continue
                        const y = sx(g.py + g.ph - i * mm); c.moveTo(g.px, y); c.lineTo(g.px + g.pw, y)
                    }
                    c.stroke()
                }
                c.strokeStyle = ink(1); c.lineWidth = 2 * hair
                c.strokeRect(sx(g.px), sx(g.py), Math.round(g.pw * g.dpr) / g.dpr, Math.round(g.ph * g.dpr) / g.dpr)
                // the readings: the processor solid, the memory dashed, the temperature in the alert ink
                const line = (key, at, colour, width, dash) => {
                    c.strokeStyle = colour; c.lineWidth = width; c.setLineDash(dash)
                    c.beginPath()
                    let on = false
                    g.samples.forEach((s, i) => {
                        const v = s[key]
                        if (v === undefined || v < 0) { on = false; return }
                        const x = g.xAt(i), y = at(v)
                        if (x < g.px) return
                        on ? c.lineTo(x, y) : c.moveTo(x, y)
                        on = true
                    })
                    c.stroke()
                }
                line("cpu", g.yPct, ink(1), 0.4 * mm, [])
                line("mem", g.yPct, ink(1), 0.3 * mm, [1.4 * mm, 0.7 * mm])
                line("temp", g.yTemp, g.alert, 0.3 * mm, [])
                c.setLineDash([])
            }
            Connections { target: graph; function onSamplesChanged() { paper.requestPaint() } function onWidthChanged() { paper.requestPaint() } }
            Connections { target: graph.theme; function onDataChanged() { paper.requestPaint() } }
        }
        // the scales' figures: % on the left, °C on the right, the time below
        Repeater {
            model: [0, 25, 50, 75, 100]
            Text {
                required property int modelData
                x: graph.px - width - 1.2 * graph.mm; y: graph.yPct(modelData / 100) - height / 2
                text: modelData === 100 ? "100%" : String(modelData)
                font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm
                color: graph.ink
            }
        }
        Repeater {
            model: [30, 50, 70, 90]
            Text {
                required property int modelData
                x: graph.px + graph.pw + 1.2 * graph.mm; y: graph.yTemp(modelData) - height / 2
                text: modelData + (modelData === 90 ? "°" : "")
                font.family: Fonts.mono; font.pixelSize: 2.0 * graph.mm
                color: graph.alert
            }
        }
        Repeater {
            model: [10, 8, 6, 4, 2, 0]
            Text {
                required property int modelData
                x: graph.px + graph.pw - modelData / 10 * graph.pw - width / 2; y: graph.py + graph.ph + 1.2 * graph.mm
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
                x: graph.px + graph.pw - width - 1.2 * graph.mm
                y: Math.max(graph.py, Math.min(graph.py + graph.ph - height, modelData.y - height - 0.6 * graph.mm))
                text: modelData.text
                font.family: Fonts.mono; font.pixelSize: 2.1 * graph.mm; font.bold: modelData.bold
                color: modelData.colour
            }
        }
    }
}
