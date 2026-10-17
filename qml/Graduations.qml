// One ruler's graduations (Rulers.qml): along both its long edges a tick every millimetre, longer at five,
// longest and numbered at ten (read along the ruler), counted out from the zero at its middle both ways;
// none where the other
// ruler crosses (`clearance` either side of the zero); and a very thin line down its middle. `along`: the
// ruler runs across (its length is the width); else down. Drawn once, again when the sizes change: the
// ruler is moved and turned as a whole.
import QtQuick

Canvas {
    id: marks
    property real mm: 4.2
    property color ink: "#3E342A"
    property real hair: 1
    property real clearance: 0
    property bool along: true
    transformOrigin: Item.Center
    renderStrategy: Canvas.Cooperative

    onMmChanged: requestPaint()
    onInkChanged: requestPaint()
    onClearanceChanged: requestPaint()
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
        ctx.globalAlpha = 0.45
        ctx.beginPath()
        if (along) { ctx.moveTo(0, wide / 2); ctx.lineTo(len, wide / 2) } else { ctx.moveTo(wide / 2, 0); ctx.lineTo(wide / 2, len) }
        ctx.stroke()
        ctx.globalAlpha = 1
        // the ticks on both edges
        ctx.beginPath()
        const most = Math.floor(mid / mm)
        for (let d = -most; d <= most; d++) {
            const off = d * mm
            if (Math.abs(off) <= clearance) continue
            const t = (d % 10 === 0 ? 4 : d % 5 === 0 ? 2.6 : 1.6) * mm
            const p = mid + off
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
            if (d === 0 || d % 10 !== 0) continue
            const off = d * mm
            if (Math.abs(off) <= clearance + 2 * mm) continue
            const p = mid + off, label = String(Math.abs(d))
            if (along) {
                ctx.fillText(label, p, 6.4 * mm)
                ctx.fillText(label, p, wide - 6.4 * mm)
            } else {
                // read along the ruler, as on a ruler laid down the sheet: two upright columns would not fit
                for (const at of [6.4 * mm, wide - 6.4 * mm]) {
                    ctx.save()
                    ctx.translate(at, p)
                    ctx.rotate(-Math.PI / 2)
                    ctx.fillText(label, 0, 0)
                    ctx.restore()
                }
            }
        }
    }
}
