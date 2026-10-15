// The theme picker (Mod+t, or a click on the theme cell): the themes laid out as drawings on the board.
// The inside of the sheet's frame is cleared of windows (board.frag: the sheet's own paper, or the
// wallpaper blurred), and the themes lie on it in a grid, all the same size, each a miniature desktop in
// that theme drawn from its own values (`draftpaper theme json`): the sheet's paper, glow, frame and
// ticks, the margins lettered, two windows with a terminal and a file list, the title block filled in.
// Each carries a drawing label below; the picked one is outlined in heavier ink. The margins stay the
// sheet's, the keys are lettered in the top margin (shell.qml, the notice zone), and the title block
// shows when no window covers it. Browsing previews live: the sheet takes the picked theme a moment
// after it is picked; Enter keeps it (and runs config.toml's [hooks] theme: the applications), Escape
// goes back to the theme it was. The arrows (or h j k l) and the wheel browse; a click on a drawing
// picks it, on the picked one keeps it, anywhere else goes back.
import QtQuick
import QtQuick.Effects
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
    readonly property real dpr: out ? out.scale : 1
    readonly property real margin: out ? out.margin : Math.round(6 * mm)
    property var themes: []
    property int selected: 0
    property string original: ""        // the theme when the picker opened: Escape goes back to it
    property string shown: ""           // the theme applied for the preview
    property string wallpaper: ""       // config.toml's, as a file URL ("" for none)
    // the keys, lettered in the top margin while the picker is open
    readonly property string hint: visible ? "←→↑↓ browse · enter keep · esc back to "
        + (themes.find(t => t.name === original) || { label: "" }).label : ""

    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-themes"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"

    // the board's theme: the one the sheet shows
    readonly property var t: themes.find(x => x.name === shown) || themes[selected] || null
    readonly property color ink: t ? t.ink : "#62574C"

    // the frame's interior, in logical pixels (the frame's line is 2 physical pixels wide)
    readonly property real inX: margin + 2 / dpr
    readonly property real inY: margin + 2 / dpr
    readonly property real inW: width - 2 * inX
    readonly property real inH: height - 2 * inY
    // the title block (TitleBlock.qml: 130 by 36 mm, in the frame's bottom-right corner), and whether a
    // window on this screen covers it
    readonly property rect block: Qt.rect(width - Math.round(margin) - Math.round(130 * mm), height - Math.round(margin) - Math.round(36 * mm),
                                          Math.round(130 * mm), Math.round(36 * mm))
    readonly property bool blockCovered: out ? Compositor.windows.some(w => w.output === out.name && w.geometry && !w.parked
        && w.geometry.x - out.x < block.x + block.width && w.geometry.x - out.x + w.geometry.width > block.x
        && w.geometry.y - out.y < block.y + block.height && w.geometry.y - out.y + w.geometry.height > block.y) : true

    // the drawings: a grid of columns enough for a square-ish layout, clear of the title block's height
    // when it shows
    readonly property int cols: Math.max(1, Math.ceil(Math.sqrt(themes.length)))
    readonly property int rows: Math.max(1, Math.ceil(themes.length / cols))
    readonly property real gutter: 8 * mm
    readonly property real labelH: 8 * mm
    readonly property real areaH: inH - gutter - (blockCovered ? 0 : 36 * mm + gutter)
    readonly property real cardH: Math.max(10, Math.min((areaH - gutter - (rows - 1) * gutter - rows * labelH) / rows,
                                                        ((inW - 2 * gutter - (cols - 1) * gutter) / cols) * height / Math.max(1, width)))
    readonly property real cardW: cardH * width / Math.max(1, height)
    readonly property real gridX: inX + (inW - cols * cardW - (cols - 1) * gutter) / 2
    readonly property real gridY: inY + gutter + Math.max(0, (areaH - gutter - rows * (cardH + labelH) - (rows - 1) * gutter) / 2)

    function open() {
        load.running = true
        wall.running = true
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
        settle.restart()
    }
    // the sheet takes the picked theme once the picking pauses
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
                    picker.selected = i
                } catch (e) {}
            }
        }
    }
    Process {
        id: wall
        command: [picker.shell.bin, "wallpaper"]
        stdout: StdioCollector {
            onStreamFinished: {
                const p = this.text.trim().replace(/^~(?=\/)/, Quickshell.env("HOME"))
                picker.wallpaper = p && p !== "none" ? "file://" + p : ""
            }
        }
    }

    // the wallpaper over the interior, cropped as the compositor crops it (to fill, centred), blurred and
    // tinted a little with the paper so the labels read on it; the board samples it
    Item {
        id: blurred
        width: picker.inW; height: picker.inH
        visible: false
        Image {
            id: picture
            anchors.fill: parent
            source: picker.wallpaper
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(Math.round(picker.inW * picker.dpr / 2), Math.round(picker.inH * picker.dpr / 2))
            asynchronous: true
            cache: false
            visible: false
        }
        MultiEffect {
            anchors.fill: parent
            source: picture
            blurEnabled: true
            blur: 1.0
            blurMax: 48
            autoPaddingEnabled: false
        }
        Rectangle { anchors.fill: parent; color: picker.t ? picker.t.paper : "#ffffff"; opacity: 0.35 }
    }

    ShaderEffect {
        id: board
        anchors.fill: parent
        readonly property var th: picker.t
        property color paper: th ? th.paper : "#ffffff"
        property color paperLight: th ? th.paper_light : "#ffffff"
        property color paperShade: th ? th.paper_shade : "#ffffff"
        property color glow: th ? th.glow : "#ffffff"
        property color ink: picker.ink
        property vector4d shape: th ? Qt.vector4d(th.sheet.angle * Math.PI / 180, th.sheet.midpoint, th.sheet.blend, th.sheet.shading) : Qt.vector4d(1, 0.5, 1, 1)
        property vector4d strength: th ? Qt.vector4d(th.sheet.glow_strength, th.sheet.glow_size, th.sheet.pattern_strength, picker.mm * picker.dpr) : Qt.vector4d(0, 1, 0, 6)
        property vector4d physical: Qt.vector4d(picker.width * picker.dpr, picker.height * picker.dpr, picture.status === Image.Ready ? 1 : 0, 0)
        property vector4d interior: Qt.vector4d(picker.margin * picker.dpr + 2, picker.margin * picker.dpr + 2,
                                                (picker.width - picker.margin) * picker.dpr - 2, (picker.height - picker.margin) * picker.dpr - 2)
        property vector4d hole: picker.blockCovered ? Qt.vector4d(0, 0, 0, 0)
            : Qt.vector4d(picker.block.x * picker.dpr, picker.block.y * picker.dpr, (picker.block.x + picker.block.width) * picker.dpr, (picker.block.y + picker.block.height) * picker.dpr)
        property var picture: ShaderEffectSource {
            sourceItem: blurred
            textureSize: Qt.size(Math.round(picker.inW * picker.dpr), Math.round(picker.inH * picker.dpr))
            live: true
        }
        fragmentShader: Qt.resolvedUrl("board.frag.qsb")
    }

    Item {
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const k = e.key
            if (k === Qt.Key_Escape) picker.cancel()
            else if (k === Qt.Key_Left || k === Qt.Key_H) picker.pick(picker.selected - 1)
            else if (k === Qt.Key_Right || k === Qt.Key_L) picker.pick(picker.selected + 1)
            else if (k === Qt.Key_Up || k === Qt.Key_K) picker.pick(picker.selected - picker.cols)
            else if (k === Qt.Key_Down || k === Qt.Key_J) picker.pick(picker.selected + picker.cols)
            else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) picker.keep()
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
                readonly property bool picked: index === picker.selected
                x: picker.gridX + (index % picker.cols) * (picker.cardW + picker.gutter)
                y: picker.gridY + Math.floor(index / picker.cols) * (picker.cardH + picker.labelH + picker.gutter)
                width: picker.cardW; height: picker.cardH

                Desktop { anchors.fill: parent; t: card.modelData }
                Rectangle {          // the outline: a hairline of ink, heavier around the picked one
                    readonly property real w: card.picked ? Math.max(2, 0.7 * picker.mm) : Math.max(1 / picker.dpr, 0.2 * picker.mm)
                    anchors.fill: parent
                    anchors.margins: -w
                    color: "transparent"
                    border.width: w
                    border.color: card.picked ? picker.ink : Qt.rgba(picker.ink.r, picker.ink.g, picker.ink.b, 0.7)
                }
                Text {               // the drawing's label
                    anchors { left: parent.left; top: parent.bottom; topMargin: 2.2 * picker.mm }
                    text: "DWG " + String(card.index + 1).padStart(2, "0") + "    " + card.modelData.label.toUpperCase()
                    font.family: Fonts.mono; font.pixelSize: 3 * picker.mm; font.letterSpacing: 0.3 * picker.mm
                    font.bold: card.picked
                    color: card.picked ? picker.ink : Qt.rgba(picker.ink.r, picker.ink.g, picker.ink.b, 0.7)
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: card.picked ? picker.keep() : picker.pick(card.index)
                }
            }
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
