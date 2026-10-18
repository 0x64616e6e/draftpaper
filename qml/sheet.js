// Placing things on the sheet (the notes: MachineSchedule.qml, LoadGraph.qml). The invariant: everything
// drawn on the sheet sits at whole millimetres from the frame's corner, and is as long as whole
// millimetres; only then is each edge put on the nearest physical pixel, so a line is sharp and its
// place is exact to the millimetre at every scale.
.pragma library

// "B3": the zone in row B (A at the top), column 3 (1 at the left); null when it is not on the sheet
function zone(ref, across, down) {
    const m = /^([A-Za-z])([0-9]+)$/.exec(String(ref || "").trim())
    if (!m) return null
    const row = m[1].toUpperCase().charCodeAt(0) - 65, col = Number(m[2]) - 1
    return row >= 0 && row < down && col >= 0 && col < across ? { row: row, col: col } : null
}

// a zone's corner, in whole millimetres from the frame's corner (rounded up: inside the zone), plus `inset`
function zoneCorner(out, ref, inset) {
    const z = out ? zone(ref, out.zones.across, out.zones.down) : null
    if (!z) return null
    const mm = out.px_per_mm
    return { x: Math.ceil(z.col * out.zones.width / mm - 1e-6) + inset, y: Math.ceil(z.row * out.zones.height / mm - 1e-6) + inset }
}

// the frame's size in whole millimetres (what fits inside it)
function frameMm(out) { return out ? { w: Math.floor(out.frame.width / out.px_per_mm + 1e-6), h: Math.floor(out.frame.height / out.px_per_mm + 1e-6) } : { w: 0, h: 0 } }

// `k` millimetres from the frame's corner, across (x) or down: the output's logical pixels, on the
// nearest physical pixel
function at(out, dpr, k, across) {
    const o = across ? out.frame.x - out.x : out.frame.y - out.y
    return Math.round((o + k * out.px_per_mm) * dpr) / dpr
}

// a layer surface's corner for content starting at `v` (logical): whole logical pixels that are whole
// physical pixels too, at or before `v` (at 1.5, the even ones), so what the surface draws lands on pixels
function surfaceAt(v, dpr) {
    let x = Math.floor(v + 1e-6)
    for (let k = 0; k < 8 && Math.abs(x * dpr - Math.round(x * dpr)) > 1e-6; k++) x--
    return x
}
