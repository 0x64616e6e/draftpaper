// Read a file every `interval` ms and expose its trimmed text as `output`, as Poll.qml does a command's: for
// /proc and /sys, read in place, no process started (the CPU line, the memory, the backlight).
import QtQuick
import Quickshell.Io

Item {
    id: watch
    property string path: ""
    property int interval: 5000
    property bool running: true
    property string output: ""
    property bool ran: false            // read once (or found missing)
    FileView {
        id: file
        path: watch.path
        printErrors: false
        onLoaded: { watch.output = file.text().trim(); watch.ran = true }
        onLoadFailed: { watch.output = ""; watch.ran = true }
    }
    Timer {
        interval: watch.interval; running: watch.running && watch.path !== ""; repeat: true; triggeredOnStart: true
        onTriggered: file.reload()
    }
    // read again now (it reads every `interval` anyway)
    function rerun() { if (watch.path !== "") file.reload() }
}
