// The lock screen (`draftpaper lock`: Mod+Shift+x, idle, before sleep), as the login screen looks: the
// sheet blurred inside its frame (the compositor draws it behind the lock surface, without the windows;
// the margins sharp) and one field in the middle (SignInField.qml), no words: the reader's glyph, lit
// while it listens, then the password's dots.
//
// PAM's conversation runs as soon as the lock is secure, through the service draftpaper-lock
// (/etc/pam.d/draftpaper-lock: the system's common-auth, so a fingerprint reader set up with
// pam_fprintd comes first). While the reader listens the field holds only its glyph, lit; when PAM asks
// for the password (the reader failed or gave up, or there is none), or as soon as something is typed,
// the dots follow the glyph; a password typed early is held until PAM asks. When the reader gives up
// (pam_fprintd: a failed read, or its timeout) and nothing is typed, the conversation starts over and
// the reader listens again, as often as it takes; a finger not recognised shakes the field. A refused
// password shakes it too and starts a new conversation. Enter gives the password, Escape clears it.
//
// An IPC handler "lock" tells `draftpaper lock` the state ("locked" once the compositor has confirmed);
// with DRAFTPAPER_LOCK_TEST=1 it also types and presses Enter, and DRAFTPAPER_LOCK_PAM_DIR names another
// directory for the PAM service (the headless test, compositor/tests/lock.sh).
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Pam

ShellRoot {
    id: root
    Theme { id: look }

    readonly property string user: Quickshell.env("USER") || ""
    property string text: ""
    property var pending: null          // a password given before PAM asked for it
    property bool asked: false          // PAM is asking for it
    property bool finger: false         // the fingerprint reader is listening
    property bool typing: false         // something was typed (the password row shown)
    property string note: ""            // the last failure
    property string hint: ""            // PAM's last word otherwise
    property bool busy: false
    property bool unlocking: false
    property bool reader: false         // the reader took part in this conversation
    property bool rearming: false       // the conversation ended on purpose, for the reader to listen again
    readonly property bool passwordRow: asked || typing || pending !== null
    // a password refused: the field shakes (no words on the lock screen)
    signal refusal()

    function submit() {
        if (busy || unlocking) return
        typing = true
        note = ""
        busy = true
        if (asked) {
            asked = false
            pam.respond(text)
        } else {
            // held until PAM asks (the reader first)
            pending = text
            if (!pam.active) pam.start()
        }
        text = ""
    }
    function clear() {
        if (unlocking) return
        text = ""
        note = ""
        if (asked) listenAgain()
    }
    // everything typed taken back: the reader again
    onTextChanged: if (asked && text === "") listenAgain()
    // the reader gave up (a failed read, or its timeout) and PAM asks for the password instead: unless one
    // is being typed, the conversation starts over and the reader listens again, as often as it takes
    function listenAgain() {
        if (!reader || text !== "" || pending !== null || unlocking) return false
        rearming = true
        asked = false
        pam.abort()
        again.restart()
        return true
    }

    PamContext {
        id: pam
        config: "draftpaper-lock"
        configDirectory: (Quickshell.env("DRAFTPAPER_LOCK_TEST") === "1" && Quickshell.env("DRAFTPAPER_LOCK_PAM_DIR")) || "/etc/pam.d"
        user: root.user
        onPamMessage: {
            if (responseRequired) {
                // the password (or a visible question, answered in the same row)
                root.finger = false
                if (responseVisible) root.hint = message
                if (!responseVisible && root.listenAgain()) return
                if (root.pending !== null) {
                    const p = root.pending
                    root.pending = null
                    root.busy = true
                    respond(p)
                } else {
                    root.asked = true
                    root.busy = false
                }
            } else if (messageIsError) {
                root.note = message
                // a finger not recognised: a shake (the reader listens again)
                if (root.reader && /match/i.test(message)) root.refusal()
            } else {
                if (/finger|swipe|touch|reader/i.test(message)) {
                    root.finger = true
                    root.reader = true
                }
                root.hint = message
            }
        }
        onCompleted: result => {
            // ended to listen again: not a refusal
            if (root.rearming) return
            root.busy = false
            root.finger = false
            root.asked = false
            root.pending = null
            if (result === PamResult.Success) {
                root.unlocking = true
                lock.locked = false
                quit.start()
            } else {
                root.note = "Not unlocked"
                root.typing = true
                root.refusal()
                again.restart()
            }
        }
        onError: error => {
            root.note = "Authentication unavailable"
            root.busy = false
            again.restart()
        }
    }
    // a new conversation after a failure (the reader listening again)
    Timer {
        id: again
        interval: 300
        onTriggered: {
            if (pam.active) { restart(); return }
            root.rearming = false
            root.reader = false
            if (!root.unlocking) pam.start()
        }
    }
    Timer { id: quit; interval: 200; onTriggered: Qt.quit() }

    WlSessionLock {
        id: lock
        locked: true
        onSecureChanged: if (secure && !pam.active) pam.start()
        surface: Component {
            WlSessionLockSurface {
                id: surface
                // the compositor draws the blurred sheet behind: only the card is drawn here
                color: "transparent"
                // the card on the largest screen
                readonly property bool main: screen && Quickshell.screens.reduce((a, s) => !a || s.width * s.height > a.width * a.height ? s : a, null) === screen

                Item {
                    anchors.fill: parent
                    focus: true
                    Component.onCompleted: forceActiveFocus()
                    Keys.onPressed: e => {
                        const k = e.key, ctrl = e.modifiers & Qt.ControlModifier
                        if (k === Qt.Key_Escape) root.clear()
                        else if (k === Qt.Key_Return || k === Qt.Key_Enter) root.submit()
                        else if (ctrl && k === Qt.Key_U) root.text = ""
                        else if (k === Qt.Key_Backspace) root.text = root.text.slice(0, -1)
                        else if (!ctrl && e.text && e.text >= " " && !root.busy && !root.unlocking) { root.typing = true; root.text += e.text }
                        else return
                        e.accepted = true
                    }

                    // the field alone (SignInField.qml): the reader's glyph, the password's dots once typed
                    SignInField {
                        id: signin
                        visible: surface.main
                        theme: look
                        screenName: surface.screen ? surface.screen.name : ""
                        text: "●".repeat(root.text.length)
                        finger: root.finger
                        busy: root.busy
                        caret: root.passwordRow && !root.busy && !root.unlocking
                        Connections { target: root; function onRefusal() { signin.refuse() } }
                    }
                }
            }
        }
    }

    IpcHandler {
        target: "lock"
        // "locking", "locked" (with " finger" while the reader listens, " password" with the password's
        // row, ": NOTE" after a failure), "unlocking"
        function state(): string {
            if (root.unlocking) return "unlocking"
            if (!lock.secure) return "locking"
            return "locked" + (root.finger ? " finger" : "") + (root.passwordRow ? " password" : "") + (root.note ? ": " + root.note : "")
        }
        function type(text: string): void { if (Quickshell.env("DRAFTPAPER_LOCK_TEST") === "1") { root.typing = true; root.text += text } }
        function enter(): void { if (Quickshell.env("DRAFTPAPER_LOCK_TEST") === "1") root.submit() }
    }
}
