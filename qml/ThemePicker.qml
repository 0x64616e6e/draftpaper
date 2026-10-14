// The theme picker (Mod+t, or a click on the theme cell): a card for every theme, each a miniature of
// the sheet drawn from that theme's own values (`draftpaper theme json`): the paper's gradient and
// glow, the frame and its ticks, two windows (one focused), the workspaces with the focused one in the
// theme's strong ink, an alert, the title block. Left and right (or the pointer) pick; Enter or a click
// applies; Escape closes. An overlay on the focused screen that has the keyboard while it is open.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: picker
    required property var shell
    visible: false
    // on the screen being worked on
    screen: Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0]
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: out ? out.px_per_mm : 4.2
    readonly property var theme: shell.theme
    property var themes: []
    property int selected: 0

    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-themes"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    readonly property real cardW: 62 * mm
    readonly property real cardH: cardW * 10 / 16
    readonly property real gap: 6 * mm
    implicitWidth: Math.max(1, themes.length) * (cardW + gap) + gap
    implicitHeight: cardH + 22 * mm

    function open() {
        load.running = true
        visible = true
    }
    function close() { visible = false }
    function apply(i) {
        const t = themes[i]
        if (!t) return
        shell.run([shell.bin, "theme", t.name])
        themes = themes.map(x => Object.assign({}, x, { current: x.name === t.name }))
    }

    // every theme's values; the current one picked first
    Process {
        id: load
        command: [picker.shell.bin, "theme", "json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    picker.themes = JSON.parse(this.text)
                    picker.selected = Math.max(0, picker.themes.findIndex(t => t.current))
                } catch (e) {}
            }
        }
    }

    // the panel: the current theme's paper, framed in its ink
    Rectangle {
        anchors.fill: parent
        color: picker.theme.data.paper || "#C9BDAE"
        border.color: picker.theme.ink
        border.width: 2
        focus: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Escape) picker.close()
            else if (e.key === Qt.Key_Left || e.key === Qt.Key_H) picker.selected = Math.max(0, picker.selected - 1)
            else if (e.key === Qt.Key_Right || e.key === Qt.Key_L) picker.selected = Math.min(picker.themes.length - 1, picker.selected + 1)
            else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) picker.apply(picker.selected)
            else return
            e.accepted = true
        }

        Text {
            x: picker.gap; y: 3 * picker.mm
            text: "THEMES"
            font.family: Fonts.mono; font.pixelSize: 3.2 * picker.mm; font.letterSpacing: 0.4 * picker.mm
            color: picker.theme.ink
        }
        Text {
            anchors { right: parent.right; rightMargin: picker.gap; top: parent.top; topMargin: 3 * picker.mm }
            text: "← →  enter  esc"
            font.family: Fonts.mono; font.pixelSize: 2.6 * picker.mm
            color: Qt.rgba(picker.theme.ink.r, picker.theme.ink.g, picker.theme.ink.b, 0.6)
        }

        Row {
            x: picker.gap; y: 10 * picker.mm
            spacing: picker.gap
            Repeater {
                model: picker.themes
                Column {
                    id: card
                    required property var modelData
                    required property int index
                    spacing: 1.6 * picker.mm
                    Rectangle {      // the selection mark: the theme's own focused-window colour
                        width: picker.cardW + 6; height: picker.cardH + 6
                        color: "transparent"
                        border.width: 3
                        border.color: card.index === picker.selected ? card.modelData.windows.focused : "transparent"
                        Sheet {
                            x: 3; y: 3
                            width: picker.cardW; height: picker.cardH
                            t: card.modelData
                        }
                        MouseArea {
                            anchors.fill: parent
                            hoverEnabled: true
                            onEntered: picker.selected = card.index
                            onClicked: picker.apply(card.index)
                        }
                    }
                    Text {
                        text: (card.modelData.current ? "● " : "") + card.modelData.label.toUpperCase()
                        font.family: Fonts.mono; font.pixelSize: 2.8 * picker.mm; font.bold: card.modelData.current
                        color: picker.theme.ink
                    }
                }
            }
        }
    }

    // one miniature sheet
    component Sheet: Canvas {
        id: sheet
        property var t
        onTChanged: requestPaint()
        onWidthChanged: requestPaint()
        onPaint: {
            const c = getContext("2d"), w = width, h = height, t = sheet.t
            if (!t) return
            const s = t.sheet, mono = Fonts.mono
            // the paper: light -> paper -> shade along the angle
            const a = s.angle * Math.PI / 180
            const g = c.createLinearGradient(0, 0, w * Math.cos(a) * 1.4, h * Math.sin(a) * 1.4)
            g.addColorStop(0, t.paper_light); g.addColorStop(s.midpoint, t.paper); g.addColorStop(1, t.paper_shade)
            c.fillStyle = g; c.fillRect(0, 0, w, h)
            // three blooms of glow, as the shader places them
            for (const [x, y, r, k] of [[0.16, 0.20, 0.55, 0.50], [0.80, 0.30, 0.40, 0.26], [0.42, 0.88, 0.48, 0.30]]) {
                const rr = r * w * 0.6 * s.glow_size
                const b = c.createRadialGradient(x * w, y * h, 0, x * w, y * h, rr)
                const col = Qt.color(t.glow)
                b.addColorStop(0, Qt.rgba(col.r, col.g, col.b, Math.min(1, k * s.glow_strength)))
                b.addColorStop(1, Qt.rgba(col.r, col.g, col.b, 0))
                c.fillStyle = b; c.fillRect(0, 0, w, h)
            }
            const ink = Qt.color(t.ink), m = w * 0.045
            const inkA = a => Qt.rgba(ink.r, ink.g, ink.b, a)
            // the frame and the zone ticks across the margins
            c.strokeStyle = inkA(0.85 * s.pattern_strength + 0.15); c.lineWidth = 1.2
            c.strokeRect(m, m, w - 2 * m, h - 2 * m)
            c.lineWidth = 0.7; c.strokeStyle = inkA(0.6 * s.pattern_strength + 0.1)
            for (let i = 1; i < 6; i++) {
                const x = m + i * (w - 2 * m) / 6
                c.beginPath(); c.moveTo(x, 0); c.lineTo(x, m); c.moveTo(x, h - m); c.lineTo(x, h); c.stroke()
            }
            for (let i = 1; i < 4; i++) {
                const y = m + i * (h - 2 * m) / 4
                c.beginPath(); c.moveTo(0, y); c.lineTo(m, y); c.moveTo(w - m, y); c.lineTo(w, y); c.stroke()
            }
            // two windows in the frame: the focused one framed in its colour
            const top = m + 2, inner = h - 2 * m - 4, half = (w - 2 * m - 6) / 2
            const win = (x, focused) => {
                c.fillStyle = t.windows.background; c.fillRect(x, top, half, inner * 0.62)
                c.strokeStyle = focused ? t.windows.focused : t.windows.unfocused; c.lineWidth = focused ? 1.5 : 1
                c.strokeRect(x, top, half, inner * 0.62)
                c.fillStyle = focused ? t.windows.text : t.windows.text_inactive
                c.font = (h * 0.045) + "px '" + mono + "'"
                c.fillText("$ draftpaper", x + 4, top + h * 0.08)
            }
            win(m + 2, true); win(m + 4 + half, false)
            // the workspaces, the focused one in strong ink; an alert in the last zone
            c.font = "bold " + (m * 0.55) + "px '" + mono + "'"; c.fillStyle = t.strong; c.fillText("2", m + w * 0.06, m * 0.7)
            c.font = (m * 0.55) + "px '" + mono + "'"; c.fillStyle = inkA(0.6)
            c.fillText("1", m + w * 0.02, m * 0.7); c.fillText("3", m + w * 0.10, m * 0.7)
            c.fillStyle = t.alert; c.fillText("89°", w - m - w * 0.09, m * 0.7)
            // the title block, bottom right
            const bw = w * 0.38, bh = h * 0.18, bx = w - m - bw - 2, by = h - m - bh - 2
            c.strokeStyle = inkA(0.85 * s.pattern_strength + 0.15); c.lineWidth = 1
            c.strokeRect(bx, by, bw, bh)
            c.beginPath(); c.moveTo(bx + bw * 0.28, by); c.lineTo(bx + bw * 0.28, by + bh)
            c.moveTo(bx + bw * 0.28, by + bh / 2); c.lineTo(bx + bw, by + bh / 2); c.stroke()
            c.fillStyle = t.ink; c.font = (bh * 0.42) + "px '" + mono + "'"; c.fillText("12:00", bx + 3, by + bh * 0.68)
            c.font = (bh * 0.26) + "px '" + mono + "'"; c.fillText(t.label.toUpperCase(), bx + bw * 0.32, by + bh * 0.38)
        }
    }
}
