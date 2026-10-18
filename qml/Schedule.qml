// A schedule on the sheet (issue #49): a drawing's ruled table, as part of the sheet: no outline, no panel,
// no margin of its own, only its heading, its rules and its lettering, on the paper under every window
// (over a wallpaper, on the theme's paper tinted as the title block is, to be read). The equipment
// (MachineSchedule.qml) and the processes (ProcessSchedule.qml) take turns in one place: Mod+i, or
// `draftpaper schedule next|equipment|processes|hide`; each fades in and out.
//
// Where: [sheet] schedule_at, millimetres from the frame's corner ([10, 10], the default) or a zone
// ("B2": 7 mm into it), as dragging it leaves it, a whole millimetre at a time, clear of the title block
// and the plotted load. Everything in it whole millimetres from the frame's corner (sheet.js;
// docs/wayland-design.md, "Invariants: drawing on the sheet").
import QtQuick
import Quickshell
import Quickshell.Wayland
import "sheet.js" as Sheet

Item {
    id: sched
    required property var shell
    required property var edge
    property var other: null              // the plotted load (LoadGraph.qml): never on top of it
    property bool wanted: false           // this one is the schedule shown, on this screen
    property string title: ""
    property string meta: ""
    // the columns: their heads, widths in millimetres (0: what is left), and alignment
    property var columns: []              // [{ name, mm, right }]
    // the rows: their cells, a note under one of them, a heavier rule above where a group starts, and a
    // share to rule as a bar (0..1) in the column `barCol`, in the alert ink from `alertAt`
    property var rows: []                 // [{ cells: [...], note, heavyAbove, share }]
    property int noteCol: -1
    property int barCol: -1
    property real alertAt: 0.8

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    readonly property color rule: Qt.rgba(ink.r, ink.g, ink.b, 0.55)
    readonly property real hair: 1 / dpr
    readonly property real heavy: Math.max(2, Math.round(0.35 * mm * dpr)) / dpr

    // shown, and on the sheet while it fades out
    readonly property bool shown: wanted && !!out && rows.length > 0 && wMm >= 70
    property real fade: shown ? 1 : 0
    Behavior on fade { enabled: sched.theme.animations; NumberAnimation { duration: 240; easing.type: Easing.InOutQuad } }

    // each row's top and height in whole millimetres: the heading 7, the columns' heads 5, a row 6 (9 with
    // a note)
    readonly property var laid: {
        let y = 12
        return rows.map(row => {
            const h = row.note ? 9 : 6
            const o = { row: row, yMm: y, hMm: h }
            y += h
            return o
        })
    }
    readonly property int hMm: laid.length ? laid[laid.length - 1].yMm + laid[laid.length - 1].hMm : 12
    readonly property var frame: Sheet.frameMm(out)

    // ---- where: whole millimetres from the frame's corner ----
    readonly property var place: {
        const at = theme.sheet.schedule_at !== undefined ? theme.sheet.schedule_at : theme.sheet.machine_at
        if (Array.isArray(at) && at.length === 2) return { x: Math.round(at[0]), y: Math.round(at[1]) }
        return Sheet.zoneCorner(out, at, 7) || { x: 10, y: 10 }
    }
    property var dragged: null
    readonly property int xMm: (dragged || place).x
    readonly property int yMm: (dragged || place).y
    readonly property var titleMm: out && out.title && out.title.rect
        ? { x: Math.floor((out.title.rect.x - (out.frame.x - out.x)) / mm), y: Math.floor((out.title.rect.y - (out.frame.y - out.y)) / mm) } : null
    // 112 mm wide; narrower where the frame, or the title block beside it (6 mm clear), ends sooner
    readonly property int wMm: {
        let right = frame.w - 2
        if (titleMm && yMm + hMm > titleMm.y - 2) right = Math.min(right, titleMm.x - 6)
        return Math.max(0, Math.min(112, right - xMm))
    }
    function clear(x, y) {
        if (x < 2 || y < 2 || x + 70 > frame.w - 2 || y + hMm > frame.h - 2) return false
        if (titleMm && x + 70 > titleMm.x - 6 && y + hMm > titleMm.y - 2) return false
        if (other && other.shown && x < other.xMm + other.wMm + 4 && other.xMm < x + 112 + 4
            && y < other.yMm + other.hMm + 4 && other.yMm < y + hMm + 4) return false
        return true
    }
    // the columns' left edges, in millimetres from the schedule's
    readonly property var colX: {
        const fixed = columns.reduce((a, c) => a + (c.mm || 0), 0)
        let x = 0
        return columns.map(c => { const at = x; x += c.mm || Math.max(10, wMm - fixed); return at })
    }
    function colW(i) { return (i + 1 < colX.length ? colX[i + 1] : wMm) - colX[i] }

    readonly property int sx: out ? Sheet.surfaceAt(Sheet.at(out, dpr, xMm, true), dpr) : 0
    readonly property int sy: out ? Sheet.surfaceAt(Sheet.at(out, dpr, yMm, false), dpr) : 0
    function ix(u) { return out ? Sheet.at(out, dpr, xMm + u, true) - sx : 0 }
    function iy(v) { return out ? Sheet.at(out, dpr, yMm + v, false) - sy : 0 }

    PanelWindow {
        screen: sched.edge.screen
        visible: sched.fade > 0
        anchors { top: true; left: true }
        margins { top: sched.sy; left: sched.sx }
        implicitWidth: Math.ceil(sched.ix(sched.wMm)) + 1
        implicitHeight: Math.max(1, Math.ceil(sched.iy(sched.hMm)) + 1)
        exclusionMode: ExclusionMode.Ignore
        aboveWindows: false
        focusable: false
        color: "transparent"
        WlrLayershell.namespace: "draftpaper-schedule"

        Item {
            anchors.fill: parent
            opacity: sched.fade

            Rectangle {     // over a wallpaper: the paper under it, to be read
                x: sched.ix(0); y: sched.iy(0); width: sched.ix(sched.wMm) - x; height: sched.iy(sched.hMm) - y
                visible: sched.theme.pictured
                color: Qt.rgba(sched.theme.paper.r, sched.theme.paper.g, sched.theme.paper.b, Math.max(0.8, sched.theme.titleTint))
            }
            // the heading: what it is, and of what
            Text {
                x: sched.ix(0); y: sched.iy(0); height: sched.iy(7) - y
                verticalAlignment: Text.AlignVCenter
                text: sched.title
                font.family: Fonts.mono; font.pixelSize: 2.8 * sched.mm; font.bold: true; font.letterSpacing: 0.25 * sched.mm
                color: sched.ink
            }
            Text {
                x: sched.ix(sched.wMm) - width; y: sched.iy(0); height: sched.iy(7) - y
                verticalAlignment: Text.AlignVCenter
                text: sched.meta
                font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                color: sched.faint
            }
            Rectangle { x: sched.ix(0); y: sched.iy(7); width: sched.ix(sched.wMm) - x; height: sched.heavy; color: sched.ink }
            // the columns' heads
            Repeater {
                model: sched.columns.length
                Text {
                    required property int index
                    readonly property var c: sched.columns[index]
                    x: c.right ? sched.ix(sched.colX[index] + sched.colW(index) - 1.6) - width : sched.ix(sched.colX[index] + (index === 0 ? 0 : 1.6))
                    y: sched.iy(7); height: sched.iy(12) - y
                    verticalAlignment: Text.AlignVCenter
                    text: c.name
                    font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                    color: sched.faint
                }
            }
            Rectangle { x: sched.ix(0); y: sched.iy(12); width: sched.ix(sched.wMm) - x; height: sched.hair; color: sched.ink }
            Repeater {
                model: sched.laid
                Item {
                    id: line
                    required property var modelData
                    readonly property var row: modelData.row
                    // the rule under it (none under the last: no outline); a heavier one above a group
                    Rectangle {
                        visible: line.modelData !== sched.laid[sched.laid.length - 1]
                        x: sched.ix(0); y: sched.iy(line.modelData.yMm + line.modelData.hMm)
                        width: sched.ix(sched.wMm) - x; height: sched.hair; color: sched.rule
                    }
                    Rectangle {
                        visible: !!line.row.heavyAbove
                        x: sched.ix(0); y: sched.iy(line.modelData.yMm); width: sched.ix(sched.wMm) - x; height: sched.heavy; color: sched.ink
                    }
                    Repeater {
                        model: sched.columns.length
                        Item {
                            id: cell
                            required property int index
                            readonly property var c: sched.columns[index]
                            readonly property real fromMm: sched.colX[index] + (index === 0 ? 0 : 1.6)
                            readonly property real toMm: sched.colX[index] + sched.colW(index) - 1.6
                            readonly property bool bar: index === sched.barCol && line.row.share !== undefined
                            Text {
                                x: cell.c.right ? sched.ix(cell.toMm) - width : sched.ix(cell.fromMm)
                                y: sched.iy(line.modelData.yMm + 1.4)
                                width: cell.c.right ? implicitWidth : sched.ix(cell.toMm) - sched.ix(cell.fromMm)
                                text: line.row.cells[cell.index] || ""
                                elide: cell.c.right ? Text.ElideNone : Text.ElideRight
                                font.family: Fonts.mono; font.pixelSize: (cell.index === 0 ? 2.5 : 2.6) * sched.mm; font.bold: cell.index === 0
                                color: cell.bar && line.row.share >= sched.alertAt ? sched.theme.alert : sched.ink
                            }
                            // the share as a bar ruled to the millimetre, 10 mm long, before the figure
                            Rectangle {
                                visible: cell.bar
                                x: sched.ix(cell.fromMm); y: sched.iy(line.modelData.yMm + 2); width: sched.ix(cell.fromMm + 10) - x; height: sched.iy(line.modelData.yMm + 4) - y
                                color: "transparent"; border.width: sched.hair; border.color: sched.rule
                                Rectangle {
                                    width: parent.width * Math.max(0, Math.min(1, line.row.share || 0)); height: parent.height
                                    color: (line.row.share || 0) >= sched.alertAt ? sched.theme.alert : sched.ink
                                }
                            }
                            Text {
                                visible: cell.index === sched.noteCol && !!line.row.note
                                x: sched.ix(cell.fromMm); y: sched.iy(line.modelData.yMm + 5); width: sched.ix(cell.toMm) - x
                                text: line.row.note || ""
                                elide: Text.ElideRight
                                font.family: Fonts.mono; font.pixelSize: 2.1 * sched.mm
                                color: sched.faint
                            }
                        }
                    }
                }
            }
            // the columns' rules, between them only, from the heads down
            Repeater {
                model: Math.max(0, sched.columns.length - 1)
                Rectangle {
                    required property int index
                    x: sched.ix(sched.colX[index + 1]); y: sched.iy(7); width: sched.hair; height: sched.iy(sched.hMm) - y; color: sched.rule
                }
            }
        }

        // dragged a whole millimetre at a time to a clear place; config.toml keeps it on release
        MouseArea {
            anchors.fill: parent
            enabled: sched.shown
            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            property point from
            property var start: null
            onPressed: e => { from = mapToGlobal(e.x, e.y); start = { x: sched.xMm, y: sched.yMm }; keep.stop(); sched.dragged = start }
            onPositionChanged: e => {
                if (!pressed || !start) return
                const p = mapToGlobal(e.x, e.y)
                const x = start.x + Math.round((p.x - from.x) / sched.mm), y = start.y + Math.round((p.y - from.y) / sched.mm)
                if (sched.clear(x, y)) sched.dragged = { x: x, y: y }
            }
            onReleased: {
                const d = sched.dragged
                if (d && start && (d.x !== start.x || d.y !== start.y)) sched.shell.setConfig(["sheet.schedule_at=[" + d.x + ", " + d.y + "]"])
                keep.restart()
            }
        }
        Timer { id: keep; interval: 1500; onTriggered: sched.dragged = null }
    }
}
