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
    Theme { id: look }

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
    // the card placed (the outputs known) and drawn: the compositor's opening may reveal it (opening.rs)
    readonly property bool placed: Compositor.outputs.length > 0
    onPlacedChanged: if (placed) readyTimer.start()
    Timer { id: readyTimer; interval: 150; onTriggered: Compositor.send({ request: "ready" }) }
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

        // the name as the title block letters it: the copy's (config.toml's [title] drawn), for its user
        readonly property string drawnName: root.user === root.owner && look.data.drawn ? look.data.drawn : root.user

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

            // the card (SignInCard.qml): the name, the password (or greetd's further question), what
            // greetd says
            SignInCard {
                id: card
                theme: look
                screenName: win.screen ? win.screen.name : ""
                heading: "SIGN IN · " + root.host.toUpperCase()
                finger: root.finger
                caret: !root.busy && !root.leaving
                hideName: root.leaving && slider.visible
                rows: [
                    { label: "NAME", value: root.field === "name" ? root.text : root.user, here: root.field === "name" && !root.leaving, name: true },
                    { label: root.field === "answer" ? root.prompt.toUpperCase() : "PASSWORD",
                      value: root.field === "name" ? "" : root.field === "answer" && root.echo ? root.text : "●".repeat(root.text.length),
                      here: root.field !== "name" && !root.leaving, reader: root.field === "password" },
                    { label: "", value: root.leaving ? "Signed in" : root.note || (root.busy && !root.finger ? "Checking…" : root.hint),
                      note: true, alert: root.note !== "" && !root.leaving }
                ]
            }

            // the name, sliding from the card into the title block's DRAWN cell, lettered there at the
            // size and place the session's shell letters it (TitleBlock.qml), so the session takes over
            // the very same letters. The cell, its letters set exactly as TitleBlock.qml sets a reading;
            // it moves (and grows) from where the name is on the card to the cell
            Item {
                id: slider
                visible: false
                width: card.drawnCell ? card.drawnCell.width : 0
                height: card.drawnCell ? card.drawnCell.height : 0
                transformOrigin: Item.TopLeft
                readonly property real fromScale: card.font / (3.6 * card.mm)
                readonly property real toX: card.drawnCell ? card.drawnCell.x : 0
                readonly property real toY: card.drawnCell ? card.drawnCell.y : 0
                property alias letters: letters
                Text {
                    id: letters
                    x: 2 * card.mm; width: parent.width - 4 * card.mm
                    anchors.bottom: parent.bottom; anchors.bottomMargin: 1.6 * card.mm
                    text: win.drawnName
                    textFormat: Text.StyledText
                    elide: Text.ElideRight
                    fontSizeMode: Text.HorizontalFit
                    minimumPixelSize: 2.4 * card.mm
                    font.family: Fonts.mono
                    font.pixelSize: 3.6 * card.mm
                    color: card.ink
                }
            }
        }
    }

    // the card fades and the name slides (or, with no DRAWN cell, only the fading); then the session
    SequentialAnimation {
        id: slide
        readonly property int duration: look.animations ? 450 : 0
        ScriptAction {
            script: {
                const name = card.nameItem
                if (!card.drawnCell || !name) return
                // the letters over the name on the card: the cell placed so, scaled down to the card's size
                const p = name.mapToItem(keys, 0, 0)
                const s = slider.fromScale, l = slider.letters
                slider.x = p.x - l.x * s
                slider.y = p.y + (name.height - l.height * s) / 2 - l.y * s
                slider.scale = slider.fromScale
                slider.visible = true
            }
        }
        ParallelAnimation {
            NumberAnimation { target: card.box; property: "opacity"; to: 0; duration: slide.duration * 0.6; easing.type: Easing.OutCubic }
            NumberAnimation { target: slider; property: "x"; to: slider.toX; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "y"; to: slider.toY; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "scale"; to: 1; duration: slide.duration; easing.type: Easing.InOutCubic }
        }
        // a few frames more: the last one, the name in its place, committed before the compositor keeps it
        PauseAnimation { duration: 120 }
        onFinished: Greetd.launch(root.session, [], false)
    }

    // the machine's name in the heading (DRAFTPAPER_GREETER_HOST, for screenshots; else /etc/hostname)
    FileView { id: hostFile; path: "/etc/hostname"; blockLoading: true; printErrors: false }
    readonly property string host: Quickshell.env("DRAFTPAPER_GREETER_HOST") || (hostFile.text() || "").trim()
}
