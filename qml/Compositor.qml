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
    // Mod is being held and the keymap belongs on the sheet (docs/keyboard.md). The compositor decides
    // it, being the only thing that sees every key, and says so with the outputs.
    readonly property bool legend: outputs.some(o => o.legend === true)
    // and whether it was tapped rather than held: the next key composes, and the heading says so
    readonly property bool leader: outputs.some(o => o.leader === true)
    // the passing word being said on the legend, where one is (docs/keyboard.md): the compositor names
    // what happened ({ say, about }) and the document says what that reads as. Nothing is phrased here.
    readonly property var word: (outputs.find(o => o.word) || {}).word || null
    // its keymap, asked for once and kept: the bindings as its own table and the config write them.
    // Nothing here keeps a second copy, so the legend cannot letter a key the compositor would not act on.
    property var keys: []
    readonly property string path: Quickshell.env("DRAFTPAPER_SOCKET") || ""

    function output(name) { return outputs.find(o => o.name === name) || null }
    // the cells merged on each screen ({top: [[from, to]], ...}): the compositor leaves out their inner
    // ticks; sent when they change and again whenever the connection is made
    property var merged: ({})
    function setMerged(name, cells) {
        if (JSON.stringify(merged[name]) === JSON.stringify(cells)) return
        merged[name] = cells
        send({ request: "sheet", output: name, merged: cells })
    }
    // where the shell letters each screen's side margins ({left: [[y0, y1]], right: ...}, logical pixels down
    // the output): the hatching of parked cells leaves them clear; sent as merged is
    property var clear: ({})
    function setClear(name, spans) {
        if (JSON.stringify(clear[name]) === JSON.stringify(spans)) return
        clear[name] = spans
        send({ request: "sheet", output: name, clear: spans })
    }
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
            if (connected) {
                compositor.send({ request: "subscribe", events: ["outputs", "workspaces", "windows"] })
                compositor.send({ request: "keys" })
                for (const name in compositor.merged) compositor.send({ request: "sheet", output: name, merged: compositor.merged[name] })
                for (const name in compositor.clear) compositor.send({ request: "sheet", output: name, clear: compositor.clear[name] })
            }
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
                else if (m.ok === true && m.for === "keys") compositor.keys = m.reply
                else if (m.ok === false) console.warn("draftpaper:", m.error)
            }
        }
    }
    // not up yet, or gone: try again
    Timer { id: retry; interval: 1000; onTriggered: socket.connected = compositor.path !== "" }
}
