// The process schedule (issue #49): what top tells, as a drawing's schedule: the busiest programs first,
// each one's share of a processor ruled as a bar (in the alert ink from 80 %), its memory and its
// processor time; a program's processes counted as one (`draftpaper processes`, polled by shell.qml every
// 2 s while it is shown). Drawn by Schedule.qml, on the sheet; Mod+i brings the equipment, then this,
// then neither.
import QtQuick

Schedule {
    readonly property var p: shell.processes
    title: "PROCESS SCHEDULE"
    meta: p ? "BY CPU · LOAD " + p.load + " · " + p.processes + " PROCESSES" : ""
    columns: [{ name: "NO.", mm: 8 }, { name: "PROCESS", mm: 0 }, { name: "PID", mm: 15, right: true },
              { name: "CPU %", mm: 24, right: true }, { name: "MEMORY", mm: 17, right: true }, { name: "TIME", mm: 16, right: true }]
    barCol: 3
    rows: !p ? [] : p.rows.map((r, i) => ({
        cells: [String(i + 1).padStart(2, "0"), r.name, r.pid, r.cpu.toFixed(1), r.memory, r.time],
        share: r.cpu / 100
    }))
}
