// Run a command every `interval` ms and expose its trimmed stdout as `output`.
import QtQuick
import Quickshell.Io

Item {
    id: poll
    property var command: []
    property int interval: 5000
    property string output: ""
    property bool ran: false            // the command has run once (its output, even empty, is in)
    Process {
        id: proc
        command: poll.command
        running: true
        stdout: StdioCollector { onStreamFinished: { poll.output = this.text.trim(); poll.ran = true } }
    }
    Timer {
        interval: poll.interval; running: true; repeat: true
        onTriggered: proc.running = true
    }
    // run it again now (it runs every `interval` anyway)
    function rerun() { proc.running = true }
}
