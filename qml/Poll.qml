// Run a command every `interval` ms and expose its trimmed stdout as `output`.
import QtQuick
import Quickshell.Io

Item {
    id: poll
    property var command: []
    property int interval: 5000
    property string output: ""
    Process {
        id: proc
        command: poll.command
        running: true
        stdout: StdioCollector { onStreamFinished: poll.output = this.text.trim() }
    }
    Timer {
        interval: poll.interval; running: true; repeat: true
        onTriggered: proc.running = true
    }
}
