// The login screen (draftpaper-greeter, under greetd): a sign-in card on the sheet, whole zones wide in
// the middle of the frame, ruled like the title block: the name, the password, and what greetd says.
// Enter goes on (the name to the password, the password to greetd); Escape clears. Once greetd accepts,
// the card fades from the sheet and the session (draftpaper-session) is started; Quickshell quits, and the
// compositor leaves the bare sheet on screen for the session to take over (compositor/src/handoff.rs).
// The sheet is the user's own (draftpaper-greeter runs the compositor on their copy of it), so nothing
// but the card changes when the session comes.
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

    property string user: Quickshell.env("DRAFTPAPER_GREETER_USER") || ""
    readonly property var session: (Quickshell.env("DRAFTPAPER_GREETER_SESSION") || "draftpaper-session").split(" ")
    // what is being typed: the name, the password, or the answer to a further question greetd asks
    property string field: user ? "password" : "name"
    property string text: ""
    property string password: ""        // kept until greetd asks for it
    property string prompt: ""          // a further question (a one-time code...)
    property bool echo: false
    property string note: ""            // greetd's last word: an error, a hint
    property bool busy: false
    property bool leaving: false

    function submit() {
        if (busy || leaving) return
        if (field === "name") {
            if (!text.trim()) return
            user = text.trim()
            text = ""
            field = "password"
            note = ""
        } else if (field === "password") {
            password = text
            text = ""
            busy = true
            note = ""
            Greetd.createSession(user)
        } else {
            const answer = text
            text = ""
            busy = true
            Greetd.respond(answer)
        }
    }
    function clear() {
        if (leaving) return
        if (Greetd.state !== GreetdState.Inactive) Greetd.cancelSession()
        busy = false
        prompt = ""
        password = ""
        if (text) text = ""
        else if (field !== "name") { field = "name"; user = "" }
    }

    Connections {
        target: Greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (responseRequired && root.field === "password" && !root.prompt && !echoResponse) {
                // the password, asked for as greetd's first secret
                const p = root.password
                root.password = ""
                root.prompt = "-"
                Greetd.respond(p)
            } else if (responseRequired) {
                // a further question: ask it in the card
                root.prompt = message.replace(/:\s*$/, "")
                root.echo = echoResponse
                root.field = "answer"
                root.busy = false
            } else {
                root.note = message
            }
        }
        function onAuthFailure(message) {
            root.note = message || "Not signed in"
            root.busy = false
            root.prompt = ""
            root.password = ""
            root.field = "password"
        }
        function onError(error) {
            root.note = error
            root.busy = false
            root.prompt = ""
            root.field = root.user ? "password" : "name"
        }
        function onReadyToLaunch() {
            root.leaving = true
            root.busy = false
            fade.start()
        }
    }

    // the card fades from the sheet, then the session starts (greetd wants the greeter gone quickly
    // after that, so the fading comes first)
    NumberAnimation {
        id: fade
        target: card; property: "opacity"; to: 0
        duration: theme.animations ? 240 : 0
        onFinished: Greetd.launch(root.session, [], true)
    }

    IpcHandler {
        target: "greeter"
        enabled: Quickshell.env("DRAFTPAPER_GREETER_TEST") === "1"
        function type(text: string): void { root.text += text }
        function enter(): void { root.submit() }
        function state(): string { return root.field + (root.busy ? " busy" : "") + (root.note ? ": " + root.note : "") }
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
                else if (!ctrl && e.text && e.text >= " " && !root.busy) root.text += e.text
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
                        { label: "NAME", value: root.field === "name" ? root.text : root.user, here: root.field === "name" },
                        { label: root.field === "answer" ? root.prompt.toUpperCase() : "PASSWORD",
                          value: root.field === "name" ? "" : root.field === "answer" && root.echo ? root.text : "●".repeat(root.text.length),
                          here: root.field !== "name" },
                        { label: "", value: root.leaving ? "Signed in" : root.busy ? "Checking…" : root.note, here: false, note: true }
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
                            x: row.modelData.note ? 2 * win.mm : win.colLabel + 2 * win.mm
                            width: row.width - x - 2 * win.mm
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideLeft; textFormat: Text.PlainText
                            text: row.modelData.value + (row.modelData.here && !root.busy && !root.leaving ? "▏" : "")
                            font.family: Fonts.mono; font.pixelSize: row.modelData.note ? win.font * 0.85 : win.font
                            font.bold: row.modelData.here
                            color: row.modelData.note ? win.inkA(0.7) : win.ink
                        }
                    }
                }
            }
        }
    }

    // the date and time in the heading, and the machine's name
    SystemClock { id: time; precision: SystemClock.Minutes }
    QtObject { id: clock; readonly property string text: Qt.formatDateTime(time.date, "yyyy-MM-dd · HH:mm") }
    FileView { id: hostFile; path: "/etc/hostname"; blockLoading: true; printErrors: false }
    readonly property string host: (hostFile.text() || "").trim()
}
