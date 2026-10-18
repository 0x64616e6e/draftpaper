// The revision block (issue #45), as a schedule (Schedule.qml): the latest commits of the git repository the
// focused window works in (`draftpaper revisions`, polled by shell.qml), a row each of REV, DESCRIPTION, DATE
// and BY, the repository's name in the heading; as many as fit. It takes its turn with the equipment and
// the processes in the title block's column (Mod+i), and with [title] revisions = true comes by itself
// when neither is chosen and the focused window is in a repository; off by default, since commit subjects
// are then on screen and in every screenshot.
import QtQuick

Schedule {
    readonly property var revs: shell.revisions
    title: "REVISIONS" + (revs && revs.repo ? " · " + String(revs.repo).toUpperCase() : "")
    meta: "SHEET " + (edge.activeSpace ? edge.activeSpace.number : 1)
    columns: [{ name: "REV", mm: 16 }, { name: "DESCRIPTION", mm: 0 }, { name: "DATE", mm: 21, right: true }, { name: "BY", mm: 22, right: true }]
    rows: revs && revs.rows ? revs.rows.map(r => ({ cells: [r.rev, r.description, r.date, r.by] })) : []
}
