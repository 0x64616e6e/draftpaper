// The login screen (draftpaper-greeter, under greetd): a sign-in card on the sheet, whole zones wide in
// the middle of the frame, ruled like the title block: the name, the password (with the fingerprint
// reader's glyph), and what greetd says. The sheet is the user's own (draftpaper-greeter runs the
// compositor on their copy of it), blurred behind the card once the name is known.
//
// With the name known, greetd is asked for a session at once, so a fingerprint reader (pam_fprintd) is
// listening from the start; a password typed meanwhile is held and given when PAM asks for it (after
// the reader, which gives up after its timeout). Enter goes on (the name to the password, the password
// to greetd); Escape clears.
//
// Once greetd accepts, the name slides from the card into its place in the title block (the DRAWN cell,
// lettered there as the session's shell letters it) while the card fades; the session is started and
// the compositor told to leave (IPC "leave"): it exits with that frame on screen, and the session takes
// it over as its opening picture, revealing the desktop under it as the blur goes
// (compositor/src/handoff.rs, opening.rs).
//
// Started by the compositor (--greeter) with DRAFTPAPER_GREETER_USER (the name to fill in) and
// DRAFTPAPER_GREETER_SESSION (the command; draftpaper-session). With DRAFTPAPER_GREETER_TEST=1 the
// IPC handler "greeter" types and presses Enter, for the headless test (compositor/tests/greeter.sh).
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Greetd

ShellRoot {
    id: root
    Theme { id: theme }

    // the copy's user: their sheet, their name in the title block
    readonly property string owner: Quickshell.env("DRAFTPAPER_GREETER_USER") || ""
    property string user: owner
    readonly property var session: (Quickshell.env("DRAFTPAPER_GREETER_SESSION") || "draftpaper-session").split(" ")
    // as blurred as the session's opening starts (compositor/src/opening.rs, BLUR_MM)
    readonly property real blurMm: 6
    // what is being typed: the name, the password, or the answer to a further question greetd asks
    property string field: user ? "password" : "name"
    property string text: ""
    property var pending: null          // a password given before PAM asked for it
    property bool asked: false          // PAM is asking for the password
    property bool finger: false         // the fingerprint reader is listening
    property string prompt: ""          // a further question (a one-time code...)
    property bool echo: false
    property string note: ""            // greetd's last error
    property string hint: ""            // and its last word otherwise
    property bool busy: false
    property bool leaving: false

    function blur(on) { Compositor.send({ request: "blur", mm: on ? blurMm : 0 }) }
    // a session for the name: PAM starts (the reader listens)
    function begin() {
        if (!user || leaving || Greetd.state !== GreetdState.Inactive) return
        asked = false
        finger = false
        Greetd.createSession(user)
    }
    function submit() {
        if (leaving) return
        if (field === "name") {
            if (!text.trim()) return
            user = text.trim()
            text = ""
            field = "password"
            note = ""
            blur(true)
            begin()
        } else if (field === "password") {
            if (busy) return
            note = ""
            busy = true
            if (asked) {
                asked = false
                Greetd.respond(text)
            } else {
                // held until PAM asks (the reader first)
                pending = text
                begin()
            }
            text = ""
        } else {
            const answer = text
            text = ""
            busy = true
            Greetd.respond(answer)
        }
    }
    function clear() {
        if (leaving) return
        if (text) { text = ""; return }
        if (Greetd.state !== GreetdState.Inactive) Greetd.cancelSession()
        busy = false; pending = null; asked = false; finger = false; prompt = ""; note = ""; hint = ""
        field = "name"
        user = ""
        blur(false)
    }

    Component.onCompleted: if (user) { blur(true); begin() }
    // after a failure, a new session (the reader listening again)
    Timer { id: again; interval: 300; onTriggered: root.begin() }

    Connections {
        target: Greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (responseRequired && !echoResponse && root.field === "password") {
                // the password
                root.finger = false
                if (root.pending !== null) {
                    const p = root.pending
                    root.pending = null
                    root.busy = true
                    Greetd.respond(p)
                } else {
                    root.asked = true
                    root.busy = false
                }
            } else if (responseRequired) {
                // a further question: ask it in the card
                root.prompt = message.replace(/:\s*$/, "")
                root.echo = echoResponse
                root.field = "answer"
                root.busy = false
            } else if (error) {
                root.note = message
            } else {
                if (/finger|swipe|touch|reader/i.test(message)) root.finger = true
                root.hint = message
            }
        }
        function onAuthFailure(message) {
            root.note = message || "Not signed in"
            root.busy = false; root.pending = null; root.asked = false; root.finger = false; root.prompt = ""
            if (root.field === "answer") root.field = "password"
            again.restart()
        }
        function onError(error) {
            root.note = error
            root.busy = false; root.pending = null; root.asked = false; root.finger = false; root.prompt = ""
            root.field = root.user ? "password" : "name"
        }
        function onReadyToLaunch() {
            root.leaving = true
            root.busy = false
            root.finger = false
            slide.start()
        }
        function onLaunched() {
            // the compositor leaves this frame on screen and exits; Quickshell goes with it
            Compositor.send({ request: "leave" })
            quit.start()
        }
    }
    // without the compositor's IPC: quit, and the compositor goes when its card does
    Timer { id: quit; interval: 1500; onTriggered: Qt.quit() }

    IpcHandler {
        target: "greeter"
        enabled: Quickshell.env("DRAFTPAPER_GREETER_TEST") === "1"
        function type(text: string): void { root.text += text }
        function enter(): void { root.submit() }
        function state(): string {
            return root.field + (root.busy ? " busy" : "") + (root.finger ? " finger" : "") + (root.note ? ": " + root.note : "")
        }
    }

    PanelWindow {
        id: win
        // the largest screen
        screen: Quickshell.screens.reduce((a, s) => !a || s.width * s.height > a.width * a.height ? s : a, null)
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "draftpaper-greeter"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
        color: "transparent"

        // ---- the sheet's geometry (logical pixels, from the output's top-left corner) ----
        readonly property var out: Compositor.output(screen ? screen.name : "")
        readonly property real mm: out ? out.px_per_mm : 4.2
        readonly property real dpr: out ? out.scale : 1
        readonly property real zoneW: out ? out.zones.width : 50 * mm
        readonly property real zoneH: out ? out.zones.height : 50 * mm
        readonly property real frameX: out ? out.frame.x - out.x : 6 * mm
        readonly property real frameY: out ? out.frame.y - out.y : 6 * mm
        readonly property int across: out ? out.zones.across : 4
        readonly property int down: out ? out.zones.down : 3
        // whole zones, about 130 mm across (the title block's width) and 45 mm down, in the middle
        readonly property int spanX: Math.max(2, Math.min(across, Math.round(130 * mm / zoneW)))
        readonly property int spanY: Math.max(1, Math.min(down, Math.round(45 * mm / zoneH)))
        readonly property int fromX: Math.floor((across - spanX) / 2)
        readonly property int fromY: Math.floor((down - spanY) / 2)
        function px(v) { return Math.round(v * dpr) / dpr }     // on a physical pixel
        readonly property real line: 2 / dpr                    // the frame's line, as the sheet draws it
        readonly property real hair: 1 / dpr
        // its rules start on the ticks, as the title block's do
        readonly property real cardX: px(frameX + fromX * zoneW)
        readonly property real cardY: px(frameY + fromY * zoneH)
        readonly property real cardW: px(frameX + (fromX + spanX) * zoneW) + line - cardX
        readonly property real cardH: px(frameY + (fromY + spanY) * zoneH) + line - cardY
        readonly property real headH: px(Math.min(10 * mm, cardH / 4))
        readonly property real rowH: (cardH - headH - 2 * line) / 3
        readonly property real font: 2.8 * mm
        readonly property real colLabel: px(Math.min(34 * mm, cardW * 0.3))
        readonly property color ink: theme.ink
        readonly property color paper: theme.data.paper || "#C9BDAE"
        function inkA(a) { return Qt.rgba(ink.r, ink.g, ink.b, a) }

        // the DRAWN cell of the title block (the last; the compositor's geometry, title.rs), when the
        // shell letters the name there (no cell of the user's own in its place)
        readonly property var drawnCell: out && out.title && theme.data.drawnCell !== false ? out.title.cells[out.title.cells.length - 1] : null
        // the name as the title block letters it: the copy's (config.toml's [title] drawn), for its user
        readonly property string drawnName: root.user === root.owner && theme.data.drawn ? theme.data.drawn : root.user

        Item {
            id: keys
            anchors.fill: parent
            focus: true
            Component.onCompleted: forceActiveFocus()
            Keys.onPressed: e => {
                const k = e.key, ctrl = e.modifiers & Qt.ControlModifier
                if (k === Qt.Key_Escape) root.clear()
                else if (k === Qt.Key_Return || k === Qt.Key_Enter) root.submit()
                else if (ctrl && k === Qt.Key_U) root.text = ""
                else if (k === Qt.Key_Backspace) root.text = root.text.slice(0, -1)
                else if (!ctrl && e.text && e.text >= " " && !root.busy && !root.leaving) root.text += e.text
                else return
                e.accepted = true
            }

            Rectangle {
                id: card
                x: win.cardX; y: win.cardY
                width: win.cardW; height: win.cardH
                color: win.paper
                // the outer rule, as heavy as the frame's line
                border.width: win.line; border.color: win.ink
                Rectangle { y: win.headH; width: card.width; height: win.hair; color: win.ink }

                // the heading, like a title block cell
                Text {
                    x: 2 * win.mm
                    y: (win.headH - height) / 2
                    text: "SIGN IN · " + root.host.toUpperCase()
                    font.family: Fonts.mono; font.pixelSize: win.font * 0.75; font.letterSpacing: 0.3 * win.mm
                    color: win.inkA(0.6)
                }
                Text {
                    anchors.right: parent.right; anchors.rightMargin: 2 * win.mm
                    y: (win.headH - height) / 2
                    text: clock.text
                    font.family: Fonts.mono; font.pixelSize: win.font * 0.75; font.letterSpacing: 0.3 * win.mm
                    color: win.inkA(0.6)
                }

                // the rows: the name, the password (or greetd's further question), what greetd says
                Repeater {
                    model: [
                        { label: "NAME", value: root.field === "name" ? root.text : root.user, here: root.field === "name", name: true },
                        { label: root.field === "answer" ? root.prompt.toUpperCase() : "PASSWORD",
                          value: root.field === "name" ? "" : root.field === "answer" && root.echo ? root.text : "●".repeat(root.text.length),
                          here: root.field !== "name", password: root.field === "password" },
                        { label: "", value: root.leaving ? "Signed in" : root.note || (root.busy && !root.finger ? "Checking…" : root.hint),
                          here: false, note: true }
                    ]
                    Item {
                        id: row
                        required property var modelData
                        required property int index
                        x: win.line; y: win.headH + win.hair + index * win.rowH
                        width: card.width - 2 * win.line; height: win.rowH
                        Rectangle { visible: row.index > 0; width: parent.width; height: win.hair; color: win.inkA(0.35) }
                        Rectangle { visible: !row.modelData.note; x: win.colLabel; width: win.hair; height: row.height; color: win.inkA(0.35) }
                        Rectangle {
                            visible: row.modelData.here && !root.leaving
                            anchors.fill: parent
                            color: win.inkA(0.12)
                            border.width: Math.max(win.line, 0.5 * win.mm); border.color: win.ink
                        }
                        Text {
                            x: 2 * win.mm; width: win.colLabel - 4 * win.mm
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            text: row.modelData.label
                            font.family: Fonts.mono; font.pixelSize: win.font * 0.7; font.letterSpacing: 0.3 * win.mm
                            color: win.inkA(0.6)
                        }
                        Text {
                            id: value
                            // the name leaves its row when it slides into the title block
                            visible: !(row.modelData.name && root.leaving && slider.visible)
                            x: row.modelData.note ? 2 * win.mm : win.colLabel + 2 * win.mm
                            width: row.width - x - (row.modelData.password ? 12 : 2) * win.mm
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideLeft; textFormat: Text.PlainText
                            text: row.modelData.value + (row.modelData.here && !root.busy && !root.leaving ? "▏" : "")
                            font.family: Fonts.mono; font.pixelSize: row.modelData.note ? win.font * 0.85 : win.font
                            font.bold: row.modelData.here
                            color: row.modelData.note ? (root.note ? theme.alert : win.inkA(0.7)) : win.ink
                            Component.onCompleted: if (row.modelData.name) nameText.target = value
                        }
                        // the fingerprint reader, drawn (the login screen has no fonts of the user's): ridges as
                        // open arcs about a core, faint, lit (and breathing) while the reader listens
                        Canvas {
                            id: reader
                            visible: row.modelData.password === true
                            anchors.right: parent.right; anchors.rightMargin: 3 * win.mm
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(5 * win.mm); height: Math.round(6 * win.mm)
                            readonly property color colour: root.finger ? win.ink : win.inkA(0.3)
                            onColourChanged: requestPaint()
                            onWidthChanged: requestPaint()
                            onPaint: {
                                const c = getContext("2d")
                                c.reset()
                                c.strokeStyle = colour
                                c.lineWidth = Math.max(1, width / 14)
                                c.lineCap = "round"
                                const cx = width / 2, cy = height * 0.55, step = width / 9
                                // the ridges, ellipses about the core
                                for (let i = 1; i <= 4; i++) {
                                    const r = i * step
                                    c.beginPath()
                                    c.ellipse(cx - r, cy - r * 1.2, 2 * r, 2.4 * r)
                                    c.stroke()
                                }
                                // the ridges open below the core, cut away
                                c.globalCompositeOperation = "destination-out"
                                c.fillStyle = "black"
                                c.beginPath()
                                c.moveTo(cx, cy)
                                c.lineTo(cx - width * 0.28, height)
                                c.lineTo(cx + width * 0.05, height)
                                c.closePath()
                                c.fill()
                                c.globalCompositeOperation = "source-over"
                                // the core
                                c.beginPath()
                                c.moveTo(cx, cy - step * 0.6)
                                c.lineTo(cx, cy + step * 0.6)
                                c.stroke()
                            }
                            SequentialAnimation on opacity {
                                running: root.finger
                                loops: Animation.Infinite
                                NumberAnimation { to: 0.45; duration: 700; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                                onRunningChanged: if (!running) parent.opacity = 1
                            }
                        }
                    }
                }
            }

            // the name, sliding from the card into the title block's DRAWN cell, lettered there at the
            // size and place the session's shell letters it (TitleBlock.qml), so the session takes over
            // the very same letters
            QtObject { id: nameText; property var target: null }
            // the cell, its letters set exactly as TitleBlock.qml sets a reading; it moves (and grows) from
            // where the name is on the card to the cell
            Item {
                id: slider
                visible: false
                width: win.drawnCell ? win.drawnCell.width : 0
                height: win.drawnCell ? win.drawnCell.height : 0
                transformOrigin: Item.TopLeft
                readonly property real fromScale: win.font / (3.6 * win.mm)
                readonly property real toX: win.drawnCell ? win.drawnCell.x : 0
                readonly property real toY: win.drawnCell ? win.drawnCell.y : 0
                property alias letters: letters
                Text {
                    id: letters
                    x: 2 * win.mm; width: parent.width - 4 * win.mm
                    anchors.bottom: parent.bottom; anchors.bottomMargin: 1.6 * win.mm
                    text: win.drawnName
                    textFormat: Text.StyledText
                    elide: Text.ElideRight
                    fontSizeMode: Text.HorizontalFit
                    minimumPixelSize: 2.4 * win.mm
                    font.family: Fonts.mono
                    font.pixelSize: 3.6 * win.mm
                    color: win.ink
                }
            }
        }
    }

    // the card fades and the name slides (or, with no DRAWN cell, only the fading); then the session
    SequentialAnimation {
        id: slide
        readonly property int duration: theme.animations ? 450 : 0
        ScriptAction {
            script: {
                if (!win.drawnCell || !nameText.target) return
                // the letters over the name on the card: the cell placed so, scaled down to the card.s size
                const p = nameText.target.mapToItem(keys, 0, 0)
                const s = slider.fromScale, l = slider.letters
                slider.x = p.x - l.x * s
                slider.y = p.y + (nameText.target.height - l.height * s) / 2 - l.y * s
                slider.scale = slider.fromScale
                slider.visible = true
            }
        }
        ParallelAnimation {
            NumberAnimation { target: card; property: "opacity"; to: 0; duration: slide.duration * 0.6; easing.type: Easing.OutCubic }
            NumberAnimation { target: slider; property: "x"; to: slider.toX; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "y"; to: slider.toY; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "scale"; to: 1; duration: slide.duration; easing.type: Easing.InOutCubic }
        }
        // a few frames more: the last one, the name in its place, committed before the compositor keeps it
        PauseAnimation { duration: 120 }
        onFinished: Greetd.launch(root.session, [], false)
    }

    // the date and time in the heading, and the machine's name
    SystemClock { id: time; precision: SystemClock.Minutes }
    QtObject { id: clock; readonly property string text: Qt.formatDateTime(time.date, "yyyy-MM-dd · HH:mm") }
    FileView { id: hostFile; path: "/etc/hostname"; blockLoading: true; printErrors: false }
    readonly property string host: (hostFile.text() || "").trim()
}
