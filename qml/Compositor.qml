pragma Singleton
// The compositor, over its IPC (DRAFTPAPER_SOCKET; newline-delimited JSON, compositor/src/ipc.rs): the
// outputs with their sheet's geometry (margin, px/mm, zones, in logical pixels), the workspaces and the
// windows, each replaced whole whenever the compositor sends it; action() runs any action a key binding
// can name ("workspace 3").
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: compositor
    property var outputs: []
    property var workspaces: []
    property var windows: []
    readonly property string path: Quickshell.env("DRAFTPAPER_SOCKET") || ""

    function output(name) { return outputs.find(o => o.name === name) || null }
    function action(text) { send({ request: "action", action: text }) }
    function send(request) {
        if (!socket.connected) return
        socket.write(JSON.stringify(request) + "\n")
        socket.flush()
    }

    Socket {
        id: socket
        path: compositor.path
        connected: compositor.path !== ""
        onConnectionStateChanged: {
            if (connected) compositor.send({ request: "subscribe", events: ["outputs", "workspaces", "windows"] })
            else retry.restart()
        }
        onError: retry.restart()
        parser: SplitParser {
            onRead: line => {
                let m
                try { m = JSON.parse(line) } catch (e) { return }
                if (m.event === "outputs") compositor.outputs = m.outputs
                else if (m.event === "workspaces") compositor.workspaces = m.workspaces
                else if (m.event === "windows") compositor.windows = m.windows
                else if (m.ok === false) console.warn("draftpaper:", m.error)
            }
        }
    }
    // not up yet, or gone: try again
    Timer { id: retry; interval: 1000; onTriggered: socket.connected = compositor.path !== "" }
}
