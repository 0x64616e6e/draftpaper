// A miniature desktop in theme `t` (the theme picker's drawings, the theme editor's): the sheet's paper,
// glow, frame and ticks, the margins lettered (workspaces, a notification, the bottom cells, a parked
// window), two windows with a terminal and a file list, the title block filled in.
import QtQuick

Canvas {
    id: desk
    property var t
    onTChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onPaint: {
        const c = getContext("2d"), w = width, h = height, t = desk.t
        if (!t || w < 10) return
        const s = t.sheet, mono = "'" + Fonts.mono + "'"
        const col = v => Qt.color(v)
        const rgba = (v, a) => { const q = col(v); return Qt.rgba(q.r, q.g, q.b, a) }
        const font = (px, bold) => (bold ? "bold " : "") + px + "px " + mono
        // the paper: light -> paper -> shade along the angle, and three blooms of glow
        const ang = s.angle * Math.PI / 180
        const g = c.createLinearGradient(0, 0, w * Math.cos(ang) * 1.4, h * Math.sin(ang) * 1.4)
        g.addColorStop(0, t.paper_light); g.addColorStop(s.midpoint, t.paper); g.addColorStop(1, t.paper_shade)
        c.fillStyle = g; c.fillRect(0, 0, w, h)
        for (const [bx, by, r, k] of [[0.16, 0.20, 0.55, 0.50], [0.80, 0.30, 0.40, 0.26], [0.42, 0.88, 0.48, 0.30]]) {
            const rr = r * w * 0.6 * s.glow_size
            const b = c.createRadialGradient(bx * w, by * h, 0, bx * w, by * h, rr)
            b.addColorStop(0, rgba(t.glow, Math.min(1, k * s.glow_strength))); b.addColorStop(1, rgba(t.glow, 0))
            c.fillStyle = b; c.fillRect(0, 0, w, h)
        }
        // the frame, the zone ticks and the dots in the margins
        const m = Math.round(w * 0.022), zx = 6, zy = 4
        const zw = (w - 2 * m) / zx, zh = (h - 2 * m) / zy
        const line = 0.85 * s.pattern_strength + 0.15, tick = 0.6 * s.pattern_strength + 0.1
        c.strokeStyle = rgba(t.ink, line); c.lineWidth = Math.max(1, w / 900)
        c.strokeRect(m, m, w - 2 * m, h - 2 * m)
        c.strokeStyle = rgba(t.ink, tick); c.lineWidth = Math.max(0.6, w / 1600)
        c.beginPath()
        for (let i = 1; i < zx; i++) {
            if (i === 3 || i === 4) continue               // a notification merges three zones
            const x = m + i * zw; c.moveTo(x, 0); c.lineTo(x, m); c.moveTo(x, h - m); c.lineTo(x, h)
        }
        for (let j = 1; j < zy; j++) {
            const y = m + j * zh; c.moveTo(0, y); c.lineTo(m, y); c.moveTo(w - m, y); c.lineTo(w, y)
        }
        c.stroke()
        // the margins' lettering: workspaces (the focused one in strong ink), a notification across
        // merged zones, an alert; the bottom cells; the side cells' letters and a parked window
        const fp = m * 0.5, base = m * 0.68
        c.font = font(fp); c.fillStyle = rgba(t.ink, 0.6); c.fillText("1", m + zw * 0.15, base); c.fillText("3", m + zw * 0.6, base)
        c.font = font(fp, true); c.fillStyle = t.strong; c.fillText("2", m + zw * 0.38, base)
        c.font = font(fp); c.fillStyle = t.ink; c.fillText("Build finished — draftpaper 0.2 is ready", m + zw * 2.3, base)
        c.fillStyle = t.alert; c.fillText("89°", w - m - zw * 0.55, base)
        c.fillStyle = t.ink; c.fillText("BT", m + zw * 0.4, h - m * 0.32)
        c.fillText(t.label.toUpperCase(), w - m - zw * 0.75, h - m * 0.32)
        c.font = font(fp * 0.8); c.fillStyle = rgba(t.ink, 0.8)
        for (let j = 0; j < zy; j++) { c.fillText("ABCD"[j], m * 0.32, m + j * zh + m * 0.9); c.fillText("ABCD"[j], w - m * 0.7, m + j * zh + m * 0.9) }
        c.save(); c.translate(m * 0.68, m + zh * 1.75); c.rotate(-Math.PI / 2)
        c.font = font(fp); c.fillStyle = t.ink; c.fillText("notes.md", 0, 0); c.restore()
        // two windows: a terminal (focused, framed in its colour) in the theme's terminal colours and
        // ANSI palette, as alacritty and foot have them, and a file list in the applications' colours
        // (GTK); a theme without an [apps] palette falls back to its window colours
        const a = t.apps || {}, tw = t.windows
        const termBg = a.term_bg || tw.background, termFg = a.term_fg || tw.text, dim = a.br_black || tw.text_inactive
        const top = m + 3, left = m + 3, inner = h - 2 * m - 6
        const ww = (w - 2 * m - 9) * 0.55, wh = inner * 0.66
        const win = (x, width, focused, bg, draw) => {
            c.fillStyle = bg; c.fillRect(x, top, width, wh)
            c.strokeStyle = focused ? tw.focused : tw.unfocused; c.lineWidth = focused ? Math.max(1.5, w / 700) : 1
            c.strokeRect(x, top, width, wh)
            c.save(); c.beginPath(); c.rect(x, top, width, wh); c.clip(); draw(x + width * 0.04, top + wh * 0.09); c.restore()
        }
        const lh = wh * 0.075, tf = font(lh * 0.72)
        win(left, ww, true, termBg, (x, y) => {
            // a line: runs of [colour, text]
            const prompt = [a.green || tw.focused, "❯ "]
            const rows = [[prompt, [termFg, "cargo build --release"]],
                          [[a.br_green || dim, "   Compiling "], [termFg, "draftpaper-compositor v0.2.0"]],
                          [[a.br_green || dim, "    Finished "], [dim, "release in 17.5s"]],
                          [prompt, [termFg, "git status --short"]],
                          [[a.red || tw.urgent, " M "], [termFg, "qml/Desktop.qml"]],
                          [[a.green || tw.focused, "A  "], [termFg, "themes/" + t.name + ".toml"]],
                          [prompt, [termFg, "ls"]],
                          [[a.blue || termFg, "compositor  docs  lib  qml  "], [termFg, "README.md"]],
                          [prompt]]
            c.font = tf
            rows.forEach((runs, i) => {
                let at = x
                for (const [colour, text] of runs) { c.fillStyle = colour; c.fillText(text, at, y + i * lh); at += c.measureText(text).width }
                if (i === rows.length - 1) { c.fillStyle = a.cursor || termFg; c.fillRect(at, y + i * lh - lh * 0.72, lh * 0.45, lh * 0.85) }
            })
        })
        win(left + ww + 3, w - 2 * m - 9 - ww, false, a.bg || tw.background, (x, y) => {
            c.font = tf
            const files = ["compositor/", "docs/", "lib/", "qml/", "themes/", "README.md", "LICENSE"]
            files.forEach((f, i) => {
                if (i === 2 && a.sel_bg) { c.fillStyle = a.sel_bg; c.fillRect(x - lh * 0.3, y + i * lh - lh * 0.78, (w - 2 * m - 9 - ww) * 0.9, lh) }
                c.fillStyle = i === 2 && a.sel_fg ? a.sel_fg : f.endsWith("/") ? (a.fg || tw.text) : (a.fg_muted || tw.text_inactive)
                c.fillText((f.endsWith("/") ? "▸ " : "  ") + f, x, y + i * lh)
            })
        })
        // the title block, filled in, bottom right
        const bw = w * 0.36, bh = h * 0.17, bx = w - m - bw, by = h - m - bh
        c.strokeStyle = rgba(t.ink, line); c.lineWidth = Math.max(1, w / 1100); c.strokeRect(bx, by, bw, bh)
        c.strokeStyle = rgba(t.ink, tick); c.beginPath()
        c.moveTo(bx + bw * 0.26, by); c.lineTo(bx + bw * 0.26, by + bh)
        c.moveTo(bx + bw * 0.26, by + bh / 3); c.lineTo(bx + bw, by + bh / 3)
        c.moveTo(bx + bw * 0.26, by + 2 * bh / 3); c.lineTo(bx + bw, by + 2 * bh / 3)
        c.moveTo(bx + bw * 0.62, by); c.lineTo(bx + bw * 0.62, by + bh / 3); c.stroke()
        c.fillStyle = t.ink; c.font = font(bh * 0.26); c.fillText("12:00", bx + bw * 0.025, by + bh * 0.6)
        c.font = font(bh * 0.16)
        c.fillText("Thu 15 Oct", bx + bw * 0.28, by + bh * 0.24); c.fillText("SHEET 2", bx + bw * 0.64, by + bh * 0.24)
        c.fillText("mains   79%   perf", bx + bw * 0.28, by + bh * 0.57)
        c.fillStyle = t.alert; c.fillText("89°", bx + bw * 0.28, by + bh * 0.9)
        c.fillStyle = t.ink; c.fillText(t.label.toUpperCase(), bx + bw * 0.5, by + bh * 0.9)
    }
}
