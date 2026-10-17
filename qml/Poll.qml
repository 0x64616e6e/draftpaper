// Run a command every `interval` ms and expose its trimmed stdout as `output`; `running: false` pauses it.
import QtQuick
import Quickshell.Io

Item {
    id: poll
    property var command: []
    property int interval: 5000
    property bool running: true
    property string output: ""
    property bool ran: false            // the command has run once (its output, even empty, is in)
    Process {
        id: proc
        command: poll.command
        running: poll.running
        stdout: StdioCollector { onStreamFinished: { poll.output = this.text.trim(); poll.ran = true } }
    }
    Timer {
        interval: poll.interval; running: poll.running; repeat: true
        onTriggered: proc.running = true
    }
    // run it again now (it runs every `interval` anyway)
    function rerun() { if (poll.running) proc.running = true }
}
