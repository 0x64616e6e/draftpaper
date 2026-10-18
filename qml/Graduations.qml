// One ruler's graduations (Rulers.qml): along both its long edges a tick every millimetre, longer at five,
// longest at ten and numbered there by one edge only (`far`: the bottom, the right; else the top, the left;
// read along the ruler), counted out both ways from the ruler's own zero (`zero`, whole millimetres from the
// crossing at its middle); none where the other ruler crosses (`clearance` either side of the crossing);
// and a very thin line down its middle. `along`: the ruler runs across (its length is the width); else
// down. Drawn again only when these change: the ruler is moved and turned as a whole.
import QtQuick

Canvas {
    id: marks
    property real mm: 4.2
    property color ink: "#3E342A"
    property real hair: 1
    property real clearance: 0
    property bool along: true
    property int zero: 0                // the ruler's zero, whole millimetres from the crossing at its middle
    property bool centreLine: true      // the very thin line down its middle
    property bool far: false            // the numbers by the far edge (the bottom, the right), else the near one
    // where this canvas starts on the screen, along the ruler, in physical pixels, and the scale: each tick
    // on the pixel nearest its true millimetre, as the sheet's dots are (src/sheet.rs), so a level
    // ruler's ticks land on them
    property real dpr: 1
    property real originPhys: 0
    function onPixel(p) { return (Math.floor(originPhys + p * dpr + 0.5) + 0.5 - originPhys) / dpr }
    onOriginPhysChanged: requestPaint()
    transformOrigin: Item.Center
    renderStrategy: Canvas.Cooperative

    onMmChanged: requestPaint()
    onInkChanged: requestPaint()
    onClearanceChanged: requestPaint()
    onZeroChanged: requestPaint()
    onFarChanged: requestPaint()
    onCentreLineChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        const ctx = getContext("2d")
        ctx.reset()
        const len = along ? width : height, wide = along ? height : width
        const mid = len / 2
        ctx.strokeStyle = ink
        ctx.fillStyle = ink
        ctx.lineWidth = hair
        // the centre line, very thin
        if (centreLine) {
            ctx.globalAlpha = 0.45
            ctx.beginPath()
            if (along) { ctx.moveTo(0, wide / 2); ctx.lineTo(len, wide / 2) } else { ctx.moveTo(wide / 2, 0); ctx.lineTo(wide / 2, len) }
            ctx.stroke()
            ctx.globalAlpha = 1
        }
        // the ticks on both edges
        ctx.beginPath()
        // the numbers' line: 6.4 mm in from the edge on a 16 mm ruler, never into the long ticks
        const numberAt = Math.max(0.4 * wide, 5.6 * mm)
        const most = Math.floor(mid / mm)
        for (let d = -most; d <= most; d++) {
            const off = d * mm
            if (Math.abs(off) <= clearance) continue
            const v = d - zero              // counted from the ruler's zero
            const t = (v % 10 === 0 ? 4 : v % 5 === 0 ? 2.6 : 1.6) * mm
            const p = onPixel(mid + off)
            if (along) {
                ctx.moveTo(p, 0); ctx.lineTo(p, t)
                ctx.moveTo(p, wide); ctx.lineTo(p, wide - t)
            } else {
                ctx.moveTo(0, p); ctx.lineTo(t, p)
                ctx.moveTo(wide, p); ctx.lineTo(wide - t, p)
            }
        }
        ctx.stroke()
        // the numbers, every ten, by each edge, upright
        ctx.font = Math.round(2.2 * mm) + "px 'IBM Plex Mono'"
        ctx.textAlign = "center"
        ctx.textBaseline = "middle"
        for (let d = -most; d <= most; d++) {
            const v = d - zero
            if (v === 0 || v % 10 !== 0) continue
            const off = d * mm
            if (Math.abs(off) <= clearance + 2 * mm) continue
            const p = onPixel(mid + off), label = String(Math.abs(v))
            if (along) {
                ctx.fillText(label, p, far ? wide - numberAt : numberAt)
            } else {
                // read along the ruler, as on a ruler laid down the sheet
                ctx.save()
                ctx.translate(far ? wide - numberAt : numberAt, p)
                ctx.rotate(-Math.PI / 2)
                ctx.fillText(label, 0, 0)
                ctx.restore()
            }
        }
    }
}
