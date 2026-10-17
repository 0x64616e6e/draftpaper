// The lock screen (`draftpaper lock`: Mod+Shift+x, idle, before sleep), as the login screen looks: the
// sheet blurred (the compositor draws it behind the lock surface, without the windows) and the sign-in
// card (SignInCard.qml) on whole zones in the middle of it.
//
// PAM's conversation runs as soon as the lock is secure, through the service draftpaper-lock
// (/etc/pam.d/draftpaper-lock: the system's common-auth, so a fingerprint reader set up with
// pam_fprintd comes first). While the reader listens the card shows only it, its glyph lit; when PAM
// asks for the password (the reader failed or gave up, or there is none), or as soon as something is
// typed, the row becomes the password's; a password typed early is held until PAM asks. A failure starts
// a new conversation (the reader listens again). Enter gives the password, Escape clears it.
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
    readonly property bool passwordRow: asked || typing || pending !== null

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
            } else {
                if (/finger|swipe|touch|reader/i.test(message)) root.finger = true
                root.hint = message
            }
        }
        onCompleted: result => {
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
    Timer { id: again; interval: 300; onTriggered: if (!pam.active && !root.unlocking) pam.start() }
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

                    SignInCard {
                        visible: surface.main
                        theme: look
                        screenName: surface.screen ? surface.screen.name : ""
                        heading: "LOCKED · " + root.host.toUpperCase()
                        finger: root.finger
                        caret: !root.busy && !root.unlocking
                        rows: [
                            root.passwordRow
                                ? { label: "PASSWORD", value: "●".repeat(root.text.length), here: !root.unlocking, reader: true }
                                : { label: "UNLOCK", value: root.hint || "Touch the reader", reader: true },
                            { label: "", value: root.unlocking ? "Unlocked" : root.note || (root.busy && !root.finger ? "Checking…" : root.passwordRow ? root.hint : ""),
                              note: true, alert: root.note !== "" && !root.unlocking }
                        ]
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

    FileView { id: hostFile; path: "/etc/hostname"; blockLoading: true; printErrors: false }
    readonly property string host: Quickshell.env("DRAFTPAPER_GREETER_HOST") || (hostFile.text() || "").trim()
}
