// The login screen (draftpaper-greeter, under greetd): one field in the middle of the sheet
// (SignInField.qml), no words: the name while it is typed, then the fingerprint reader's glyph and the
// password's dots; a refusal shakes it. The sheet is the user's own (draftpaper-greeter runs the
// compositor on their copy of it), blurred inside its frame (the margins sharp) once the name is known.
//
// With the name known, greetd is asked for a session at once, so a fingerprint reader (pam_fprintd) is
// listening from the start; a password typed meanwhile is held and given when PAM asks for it (after
// the reader, which gives up after its timeout). When the reader gives up (a failed read, or its
// timeout) and nothing is typed, the session starts over and the reader listens again, as often as it
// takes. Enter goes on (the name to the password, the password to greetd); Escape clears (back to the
// name once the reader gives up, while it listens).
//
// Once greetd accepts, the name appears in the field and slides into its place in the title block (the
// DRAWN cell, lettered there as the session's shell letters it) while the field fades; the session is
// started and the compositor told to leave (IPC "leave"): it exits with that frame on screen, and the
// session takes it over as its opening picture, revealing the desktop under it as the blur goes
// (compositor/src/handoff.rs, opening.rs).
//
// When more than one session is installed (wayland-sessions: Draftpaper, and another desktop beside it),
// the one that will start is lettered under the field, and Tab (Shift+Tab, a click) goes to the next;
// with Draftpaper's alone, nothing more shows. The choice is kept per user, in the greeter's own state
// (DRAFTPAPER_GREETER_STATE: draftpaper-greeter puts it in greetd's home, so it outlives a restart).
//
// Started by the compositor (--greeter) with DRAFTPAPER_GREETER_USER (the name to fill in) and
// DRAFTPAPER_GREETER_SESSION (the command; draftpaper-session, the default and the first). With
// DRAFTPAPER_GREETER_TEST=1 the IPC handler "greeter" types, presses Enter and goes to the next session,
// for the headless test (compositor/tests/greeter.sh).
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
    // the session to start: the one chosen of those installed (the default first), or the default alone
    readonly property var defaultSession: (Quickshell.env("DRAFTPAPER_GREETER_SESSION") || "draftpaper-session").split(" ")
    property var sessions: []           // [{id, name, command}], the default first
    property int sessionIndex: 0
    readonly property var session: sessions.length ? sessions[sessionIndex].command : defaultSession
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
    property bool reader: false         // the reader took part in this conversation
    property bool rearming: false       // the session cancelled on purpose, for the reader to listen again
    property bool toName: false         // Escape pressed while greetd could not be asked: the name, when it can

    // the reader gave up (a failed read, or its timeout) and PAM asks for the password instead: unless one
    // is being typed, the session starts over and the reader listens again, as often as it takes. greetd
    // waits on us then (the question is ours to answer), so the cancel is answered at once
    function listenAgain() {
        // Escape pressed while the reader listened: back to the name instead
        if (toName && field === "password" && text === "") { toName = false; asked = true; clear(); return true }
        if (!reader || field !== "password" || text !== "" || pending !== null || leaving) return false
        rearming = true
        asked = false
        Greetd.cancelSession()
        again.restart()
        return true
    }
    onTextChanged: if (asked && text === "") listenAgain()

    function blur(on) { Compositor.send({ request: "blur", mm: on ? blurMm : 0 }) }
    // the installed sessions, from the compositor: the default first (named as its entry names it, or
    // after its program when it has none), then the others in the order found
    function takeSessions(list) {
        const all = (list || []).slice()
        const same = s => s.command.join(" ") === defaultSession.join(" ")
        const i = all.findIndex(same)
        const first = i >= 0 ? all.splice(i, 1)[0] : { id: "", name: defaultSession[0] === "draftpaper-session" ? "Draftpaper" : defaultSession[0], command: defaultSession }
        sessions = [first].concat(all)
        recallSession()
    }
    // the one this user chose last, if it is still installed
    function recallSession() {
        let chosen = {}
        try { chosen = JSON.parse(chosenFile.text()) || {} } catch (e) {}
        const i = sessions.findIndex(s => s.id && s.id === chosen[user])
        sessionIndex = i >= 0 ? i : 0
    }
    function nextSession(step) {
        if (sessions.length < 2 || leaving) return
        sessionIndex = (sessionIndex + step + sessions.length) % sessions.length
    }
    // kept once it is used: the session about to start, for this name
    function keepSession() {
        if (!user || sessions.length < 2) return
        let chosen = {}
        try { chosen = JSON.parse(chosenFile.text()) || {} } catch (e) {}
        chosen[user] = sessions[sessionIndex].id
        chosenFile.setText(JSON.stringify(chosen))
    }
    onUserChanged: if (sessions.length) recallSession()
    FileView {
        id: chosenFile
        // the greeter's own lasting state (draftpaper-greeter: in greetd's home), else its runtime state
        path: (Quickshell.env("DRAFTPAPER_GREETER_STATE") || Quickshell.env("XDG_STATE_HOME") || "/tmp") +
            "/draftpaper-greeter-sessions.json"
        blockLoading: true
        printErrors: false
    }
    Connections {
        target: Compositor
        function onSessionsChanged() { root.takeSessions(Compositor.sessions) }
    }
    // a session for the name: PAM starts (the reader listens)
    function begin() {
        if (!user || leaving || Greetd.state !== GreetdState.Inactive) return
        asked = false
        finger = false
        reader = false
        rearming = false
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
        // back to the name only while greetd waits on us (no conversation, or a question unanswered):
        // greetd answers one request at a time, and while PAM waits on the reader it answers none, so a
        // cancel then would queue behind it and the replies come out of order: then it is done as soon as
        // the reader gives up
        if (Greetd.state !== GreetdState.Inactive && !asked && field !== "answer") { toName = true; return }
        toName = false
        if (Greetd.state !== GreetdState.Inactive) Greetd.cancelSession()
        busy = false; pending = null; asked = false; finger = false; prompt = ""; note = ""; hint = ""
        field = "name"
        user = ""
        blur(false)
    }

    Component.onCompleted: {
        Compositor.wantSessions = true
        if (user) { blur(true); begin() }
    }
    // the card placed (the outputs known) and drawn: the compositor's opening may reveal it (opening.rs)
    readonly property bool placed: Compositor.outputs.length > 0
    onPlacedChanged: if (placed) readyTimer.start()
    Timer { id: readyTimer; interval: 150; onTriggered: Compositor.send({ request: "ready" }) }
    // after a failure, a new session (the reader listening again), once greetd's side is idle again
    Timer {
        id: again
        interval: 300
        property int tries: 0
        onTriggered: {
            if (Greetd.state === GreetdState.Inactive) { tries = 0; root.begin() }
            else if (++tries < 20) restart()
        }
    }
    // something went wrong once the name began to slide (an error, a session greetd would not start):
    // the field back, shaken, and a new conversation; never a screen with nothing to type into
    function recover() {
        slide.stop()
        stuck.stop()
        leaving = false
        slider.visible = false
        card.box.opacity = 1
        busy = false; pending = null; asked = false; finger = false; prompt = ""
        field = user ? "password" : "name"
        card.refuse()
        again.restart()
    }
    // no word from greetd after asking it to start the session
    Timer { id: stuck; interval: 4000; onTriggered: root.recover() }

    Connections {
        target: Greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (responseRequired && !echoResponse && root.field === "password") {
                // the password
                root.finger = false
                if (root.listenAgain()) return
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
                // a finger not recognised: a shake (the reader listens again)
                if (root.reader && /match/i.test(message)) card.refuse()
            } else {
                if (/finger|swipe|touch|reader/i.test(message)) {
                    root.finger = true
                    root.reader = true
                }
                root.hint = message
            }
        }
        function onAuthFailure(message) {
            // cancelled to listen again: not a refusal
            if (root.rearming) return
            root.note = message || "Not signed in"
            card.refuse()
            root.busy = false; root.pending = null; root.asked = false; root.finger = false; root.prompt = ""
            if (root.field === "answer") root.field = "password"
            again.restart()
        }
        function onError(error) {
            root.note = error
            root.recover()
        }
        function onReadyToLaunch() {
            root.leaving = true
            root.busy = false
            root.finger = false
            slide.start()
        }
        function onLaunched() {
            // the compositor leaves this frame on screen and exits; Quickshell goes with it
            stuck.stop()
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
        function back(): void { root.clear() }
        function nextSession(): void { root.nextSession(1) }
        // the sessions offered, the one that would start in brackets: "[Draftpaper] Tom's WM"
        function sessions(): string {
            return root.sessions.map((s, i) => i === root.sessionIndex ? "[" + s.name + "]" : s.name).join(" ")
        }
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
                else if (k === Qt.Key_Tab) root.nextSession(1)
                else if (k === Qt.Key_Backtab) root.nextSession(-1)
                else if (k === Qt.Key_Backspace) root.text = root.text.slice(0, -1)
                else if (!ctrl && e.text && e.text >= " " && !root.busy && !root.leaving) root.text += e.text
                else return
                e.accepted = true
            }

            // the field alone (SignInField.qml), no words: the name while it is typed, then the reader's
            // glyph and the password's dots (or the answer to greetd's further question)
            SignInField {
                id: card
                theme: look
                screenName: win.screen ? win.screen.name : ""
                text: root.field === "name" || (root.field === "answer" && root.echo) ? root.text : "●".repeat(root.text.length)
                reader: root.field !== "name"
                finger: root.finger
                busy: root.busy
                caret: !root.busy && !root.leaving && (root.field !== "password" || root.text !== "" || !root.finger)
            }

            // the session that will start, under the field, when there is a choice: lettered small and
            // faint, as the sheet letters its notes, with the way to the next on either side. It fades with
            // the card, so the frame the session takes over does not carry it
            Text {
                id: sessionLine
                visible: root.sessions.length > 1
                x: card.fieldX; width: card.fieldW
                y: card.fieldY + card.fieldH + 2.4 * card.mm
                horizontalAlignment: Text.AlignHCenter
                textFormat: Text.PlainText
                elide: Text.ElideMiddle
                text: visible ? "\u2039  " + root.sessions[root.sessionIndex].name.toUpperCase() + "  \u203a" : ""
                font.family: Fonts.mono
                font.pixelSize: 2.2 * card.mm
                font.letterSpacing: 0.25 * card.mm
                color: card.inkA(0.6)
                opacity: card.box.opacity
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: e => root.nextSession(e.button === Qt.RightButton ? -1 : 1)
                }
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
                const name = card.letters
                if (!card.drawnCell) return
                // the name appears where the typed letters are (the field shows no name), at their size, and
                // slides into the cell, fading in on the way
                const p = name.mapToItem(keys, 0, 0)
                const s = slider.fromScale, l = slider.letters
                slider.x = p.x - l.x * s
                slider.y = p.y + (name.height - l.height * s) / 2 - l.y * s
                slider.scale = slider.fromScale
                slider.opacity = 0
                slider.visible = true
            }
        }
        ParallelAnimation {
            NumberAnimation { target: card.box; property: "opacity"; to: 0; duration: slide.duration * 0.6; easing.type: Easing.OutCubic }
            NumberAnimation { target: slider; property: "x"; to: slider.toX; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "y"; to: slider.toY; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "scale"; to: 1; duration: slide.duration; easing.type: Easing.InOutCubic }
            NumberAnimation { target: slider; property: "opacity"; to: 1; duration: slide.duration * 0.4; easing.type: Easing.OutCubic }
        }
        // a few frames more: the last one, the name in its place, committed before the compositor keeps it
        PauseAnimation { duration: 120 }
        onFinished: {
            if (Greetd.state !== GreetdState.ReadyToLaunch) { root.recover(); return }
            root.keepSession()
            Greetd.launch(root.session, [], false)
            stuck.start()
        }
    }
}
