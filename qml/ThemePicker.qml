// The theme picker (Mod+t, or a click on the theme cell): the themes in a cover flow over the dimmed
// desktop, the one picked large in the middle and framed in its own accent, the others turned away on
// either side, its name below. Each card is a miniature desktop in that theme, drawn from its own values
// (`draftpaper theme json`): the sheet's paper, glow, frame and ticks, the margins lettered (workspaces,
// a notification, the bottom cells, a parked window), two windows with a terminal and a file list, the
// title block filled in. Browsing previews live: the desktop behind takes the picked theme a moment
// after it is picked; Enter keeps it (and runs config.toml's [hooks] theme: the applications), Escape
// goes back to the theme it was. Left, right or the wheel
// browse; a click on a side card picks it, on the middle one keeps it.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: picker
    required property var shell
    visible: false
    screen: Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0]
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: out ? out.px_per_mm : 4.2
    property var themes: []
    property int selected: 0
    // where the flow is: eased towards `selected` as it changes (at once when opening, or with reduced motion)
    property real at: 0
    property bool easing: false
    Behavior on at { enabled: picker.easing && picker.shell.theme.animations; NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
    property string original: ""        // the theme when the picker opened: Escape goes back to it
    property string shown: ""           // the theme applied for the preview

    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-themes"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"

    readonly property real cardW: width * 0.46
    readonly property real cardH: cardW * height / Math.max(1, width)

    function open() {
        easing = false
        load.running = true
        visible = true
    }
    // browsing switches the sheet only (`--preview`: no [hooks] theme); keeping switches it all
    function preview(name) {
        if (!name || name === shown) return
        shown = name
        shell.run([shell.bin, "theme", name, "--preview"])
    }
    function keep() {
        const name = themes[selected] ? themes[selected].name : ""
        if (name) shell.run([shell.bin, "theme", name])
        shown = name
        visible = false
    }
    function cancel() {
        preview(original)
        visible = false
    }
    function pick(i) {
        selected = Math.max(0, Math.min(themes.length - 1, i))
        at = selected
        settle.restart()
    }
    // the desktop takes the picked theme once the picking pauses
    Timer { id: settle; interval: 350; onTriggered: picker.preview(picker.themes[picker.selected] ? picker.themes[picker.selected].name : "") }

    Process {
        id: load
        command: [picker.shell.bin, "theme", "json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    picker.themes = JSON.parse(this.text)
                    const i = Math.max(0, picker.themes.findIndex(t => t.current))
                    picker.original = picker.shown = picker.themes[i] ? picker.themes[i].name : ""
                    picker.easing = false
                    picker.selected = i
                    picker.at = i
                    picker.easing = true
                } catch (e) {}
            }
        }
    }

    Rectangle {          // the desktop, dimmed
        anchors.fill: parent
        color: Qt.rgba(0, 0, 0, 0.42)
        focus: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Escape) picker.cancel()
            else if (e.key === Qt.Key_Left || e.key === Qt.Key_H) picker.pick(picker.selected - 1)
            else if (e.key === Qt.Key_Right || e.key === Qt.Key_L) picker.pick(picker.selected + 1)
            else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) picker.keep()
            else return
            e.accepted = true
        }
        MouseArea {
            anchors.fill: parent
            onClicked: picker.cancel()
            onWheel: w => picker.pick(picker.selected + (w.angleDelta.y < 0 || w.angleDelta.x < 0 ? 1 : -1))
        }

        Repeater {
            model: picker.themes
            Item {
                id: card
                required property var modelData
                required property int index
                readonly property real d: index - picker.at          // places from the middle
                readonly property real ad: Math.abs(d)
                readonly property real side: Math.min(1, ad)         // 0 in the middle, 1 turned away
                width: picker.cardW; height: picker.cardH
                x: picker.width / 2 - width / 2 + Math.sign(d) * (side * picker.width * 0.30 + Math.max(0, ad - 1) * picker.width * 0.06)
                y: picker.height / 2 - height / 2 - picker.height * 0.04
                z: 100 - ad * 10
                opacity: ad > 3.2 ? 0 : 1 - Math.max(0, ad - 2) * 0.6
                // scaled down before it turns: the turn's perspective grows with the size it turns
                transform: [
                    Scale { origin.x: card.width / 2; origin.y: card.height / 2; xScale: 1 - card.side * 0.42; yScale: 1 - card.side * 0.42 },
                    Rotation {
                        origin.x: card.width / 2; origin.y: card.height / 2
                        axis { x: 0; y: 1; z: 0 }
                        angle: -Math.sign(card.d) * card.side * 45
                    }
                ]
                Rectangle {          // the frame: the theme's own focused-window colour, around the picked one
                    anchors.fill: parent
                    anchors.margins: -6
                    color: "transparent"
                    border.width: 4
                    border.color: card.modelData.windows.focused
                    visible: card.index === picker.selected
                }
                Desktop { anchors.fill: parent; t: card.modelData }
                Rectangle {          // the ones turned away, shaded a little
                    anchors.fill: parent
                    color: Qt.rgba(0, 0, 0, 0.35 * card.side)
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: card.index === picker.selected ? picker.keep() : picker.pick(card.index)
                }
            }
        }

        Text {               // the picked theme's name
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height / 2 + picker.cardH / 2 + 6 * picker.mm - parent.height * 0.04
            text: picker.themes[picker.selected] ? picker.themes[picker.selected].label : ""
            font.family: Fonts.mono; font.pixelSize: 7 * picker.mm; font.letterSpacing: 0.6 * picker.mm
            color: "#F2EEE6"
            style: Text.Raised; styleColor: Qt.rgba(0, 0, 0, 0.5)
        }
        Text {
            // above the cards, clear of the title block in the corner
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height / 2 - parent.height * 0.04 - picker.cardH / 2 - 8 * picker.mm - height
            text: "←  →  browse     enter  keep     esc  back to " + (picker.themes.find(t => t.name === picker.original) || { label: "" }).label
            font.family: Fonts.mono; font.pixelSize: 3 * picker.mm
            color: Qt.rgba(0.95, 0.93, 0.9, 0.75)
        }
    }

    // a miniature desktop in theme `t`
    component Desktop: Canvas {
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
            // two windows: a terminal (focused, framed in its colour) and a file list
            const top = m + 3, left = m + 3, inner = h - 2 * m - 6
            const ww = (w - 2 * m - 9) * 0.55, wh = inner * 0.66
            const win = (x, width, focused, draw) => {
                c.fillStyle = t.windows.background; c.fillRect(x, top, width, wh)
                c.strokeStyle = focused ? t.windows.focused : t.windows.unfocused; c.lineWidth = focused ? Math.max(1.5, w / 700) : 1
                c.strokeRect(x, top, width, wh)
                c.save(); c.beginPath(); c.rect(x, top, width, wh); c.clip(); draw(x + width * 0.04, top + wh * 0.09); c.restore()
            }
            const lh = wh * 0.075, tf = font(lh * 0.72)
            win(left, ww, true, (x, y) => {
                const rows = [[t.windows.focused, "❯ ", t.windows.text, "cargo build --release"],
                              [t.windows.text_inactive, "   Compiling draftpaper-compositor v0.2.0", null, ""],
                              [t.windows.text_inactive, "    Finished release in 17.5s", null, ""],
                              [t.windows.focused, "❯ ", t.windows.text, "draftpaper theme " + t.name],
                              [t.windows.text, t.label + " applied.", null, ""],
                              [t.windows.focused, "❯ ", t.windows.text, "▌"]]
                c.font = tf
                rows.forEach((r, i) => {
                    c.fillStyle = r[0]; c.fillText(r[1], x, y + i * lh)
                    if (r[2]) { c.fillStyle = r[2]; c.fillText(r[3], x + c.measureText(r[1]).width, y + i * lh) }
                })
            })
            win(left + ww + 3, w - 2 * m - 9 - ww, false, (x, y) => {
                c.font = tf
                const files = ["compositor/", "docs/", "lib/", "qml/", "themes/", "README.md", "LICENSE"]
                files.forEach((f, i) => {
                    c.fillStyle = f.endsWith("/") ? t.windows.text : t.windows.text_inactive
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
}
