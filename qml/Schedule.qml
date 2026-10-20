// A schedule on the sheet (issue #49): a drawing's ruled table, as part of the sheet: no outline, no panel,
// no margin of its own, only its heading, its rules and its lettering, on the paper under every window
// (over a wallpaper, on the theme's paper tinted as the title block is, to be read). The equipment
// and the processes take turns in one place: Mod+i, or
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
    property var other: null              // the plotted load (LoadGraph.qml): never on top of it (rows stop above it)
    property bool wanted: false           // this one is the schedule shown, on this screen
    property string title: ""
    property string meta: ""
    // the columns: their heads, widths in millimetres (0: what is left), and alignment
    property var columns: []              // [{ name, mm, right }]
    // the rows: their cells, a note under one of them, a heavier rule above where a group starts, and a
    // share to rule as a bar (0..1) in the column `barCol`, in the alert ink from `alertAt`
    property var rows: []                 // [{ cells: [...], note, heavyAbove, share, action }]
    property int noteCol: -1
    property int barCol: -1
    property real alertAt: 0.8

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))
    // the ink a row asks for, if it asks for one: a schedule of the user's own may letter a row in the
    // theme's alert, faint or strong ink (docs/sheet-content.md); anything else is the sheet's own
    function rowInk(row, fallback) {
        if (!row || !row.ink) return fallback
        if (row.ink === "alert") return theme.alert
        if (row.ink === "faint") return faint
        if (row.ink === "strong") return edge.strong
        return fallback
    }
    readonly property color rule: Qt.rgba(ink.r, ink.g, ink.b, 0.55)
    readonly property real hair: 1 / dpr
    // no margin of its own: the lettering starts at its edges
    readonly property real pad: 0
    readonly property real heavy: Math.max(2, Math.round(0.35 * mm * dpr)) / dpr

    // shown, and on the sheet while it fades out
    readonly property bool shown: wanted && !!out && laid.length > 0 && wMm >= 70
    property real fade: shown ? 1 : 0
    Behavior on fade { enabled: sched.theme.animations; NumberAnimation { duration: 240; easing.type: Easing.InOutQuad } }

    readonly property var frame: Sheet.frameMm(out)
    readonly property real frameX: out ? out.frame.x - out.x : 0
    readonly property real frameY: out ? out.frame.y - out.y : 0
    readonly property var titleRect: out && out.title && out.title.rect ? out.title.rect : null
    readonly property var titleMm: titleRect ? { x: Math.floor((titleRect.x - frameX) / mm), y: Math.floor((titleRect.y - frameY) / mm) } : null

    // ---- where ----
    // without [sheet] schedule_at (and not being dragged): the title block's column, from the top: its left
    // edge on the title block's left rule, its top and right `gap` in from the frame's lines (not on them),
    // as many rows as fit above the revision block, or the title block, `gap` clear: the plotted load's
    // distance from the title block
    readonly property int gap: 6
    property var dragged: null
    // the title block's column, unless config.toml puts it elsewhere and that place has room on this display
    // (70 mm across, its heading and a row down): a place set on one display may not fit another
    readonly property bool placedFits: place.x >= 2 && place.y >= 2 && place.x + 70 <= frame.w - 2 && place.y + 18 <= frame.h - 2
    readonly property bool column: dragged === null && titleRect !== null && (theme.sheet.schedule_at === undefined || !placedFits)
    readonly property var place: {
        const at = theme.sheet.schedule_at
        if (Array.isArray(at) && at.length === 2) return { x: Math.round(at[0]), y: Math.round(at[1]) }
        return Sheet.zoneCorner(out, at, 7) || { x: 10, y: 10 }
    }
    // the schedule's left and top edges (the output's logical pixels, on physical pixels), and its width
    // in millimetres (whole away from the title block's column)
    readonly property real x0: column ? Math.round(titleRect.x * dpr) / dpr : frameX + (dragged || place).x * mm
    readonly property real y0: column ? Math.round((frameY + gap * mm) * dpr) / dpr : frameY + (dragged || place).y * mm
    readonly property real xMm: (x0 - frameX) / mm
    readonly property real yMm: (y0 - frameY) / mm
    readonly property real wMm: {
        if (column) return (Math.round((titleRect.x + titleRect.width - gap * mm) * dpr) / dpr - x0) / mm
        let right = frame.w - 2
        if (titleMm && yMm + 12 > titleMm.y - 2) right = Math.min(right, titleMm.x - 6)
        return Math.max(0, Math.min(112, right - xMm))
    }
    // up to the title block, `gap` clear (the revisions take their turn in this place, never above the
    // title block); elsewhere to the frame, 2 mm clear, or to the graph (4 mm clear) where it is out below
    readonly property int roomMm: column ? Math.floor((titleRect.y - y0) / mm) - gap : Math.floor(limit(xMm) - yMm)
    // how far down a schedule at x (mm) may reach: the graph's top line, 4 mm clear, where it is out under
    // it (its place is the title block's, LoadGraph.qml), else the frame's, 2 mm clear
    function limit(x) {
        if (other && other.shown && x < other.xMm + other.wMm + 4 && other.xMm < x + 112 + 4) return other.yMm - 4
        return frame.h - 2
    }

    // each row's top and height in whole millimetres: the heading 7 (more, below), the columns' heads 5, a
    // row 6 (9 with a note); the rows that fit. One with more rows than fit fills its place: the millimetres
    // left over go to its heading, so its last row ends `gap` above the title block, as the plotted load
    // keeps from it
    readonly property var fit: {
        let y = 12
        const out = []
        for (const row of rows) {
            const h = row.note ? 9 : 6
            if (y + h > roomMm) break
            out.push({ row: row, yMm: y, hMm: h })
            y += h
        }
        return { rows: out, end: y, full: out.length < rows.length }
    }
    readonly property int headMm: 7 + (column && fit.full ? Math.max(0, roomMm - fit.end) : 0)
    readonly property var laid: fit.rows.map(r => ({ row: r.row, yMm: r.yMm + headMm - 7, hMm: r.hMm }))
    // the row with an action under the pointer (a local y, in pixels), or null
    property var hoverRow: null
    function rowAt(y) {
        const l = laid.find(l => !!l.row.action && y >= iy(l.yMm) && y < iy(l.yMm + l.hMm))
        return l ? l.row : null
    }
    readonly property int hMm: laid.length ? laid[laid.length - 1].yMm + laid[laid.length - 1].hMm : headMm + 5

    function clear(x, y) {
        if (x < 2 || y < 2 || x + 70 > frame.w - 2 || y + hMm > frame.h - 2) return false
        if (titleMm && x + 70 > titleMm.x - 6 && y + hMm > titleMm.y - 2) return false
        // its heading and a row above the graph: the rows that do not fit stay off
        if (y + 18 > limit(x)) return false
        return true
    }
    // the columns' left edges, in millimetres from the schedule's
    readonly property var colX: {
        const fixed = columns.reduce((a, c) => a + (c.mm || 0), 0)
        let x = 0
        return columns.map(c => { const at = x; x += c.mm || Math.max(10, wMm - fixed); return at })
    }
    function colW(i) { return (i + 1 < colX.length ? colX[i + 1] : wMm) - colX[i] }

    readonly property int sx: Sheet.surfaceAt(x0, dpr)
    readonly property int sy: Sheet.surfaceAt(y0, dpr)
    // u, v millimetres into it, on the nearest physical pixel (from its edges: whole millimetres from the
    // frame's corner where they are)
    function ix(u) { return Math.round((x0 + u * mm) * dpr) / dpr - sx }
    function iy(v) { return Math.round((y0 + v * mm) * dpr) / dpr - sy }

    // its window is made when it is called up, and let go a while after it goes: a window of its own, with
    // its scene graph and its drawing context, is some 25 MB a screen, held for nothing while it is away
    property bool windowUp: false
    onShownChanged: { if (shown) { windowUp = true; drop.stop() } else drop.restart() }
    Timer { id: drop; interval: 20000; onTriggered: sched.windowUp = false }

    LazyLoader {
        active: sched.windowUp
        component: Component {
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
                        x: sched.ix(sched.pad); y: sched.iy(sched.headMm - 7); height: sched.iy(sched.headMm) - y
                        verticalAlignment: Text.AlignVCenter
                        text: sched.title
                        font.family: Fonts.mono; font.pixelSize: 2.8 * sched.mm; font.bold: true; font.letterSpacing: 0.25 * sched.mm
                        color: sched.ink
                    }
                    Text {
                        x: sched.ix(sched.wMm - sched.pad) - width; y: sched.iy(sched.headMm - 7); height: sched.iy(sched.headMm) - y
                        verticalAlignment: Text.AlignVCenter
                        text: sched.meta
                        font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                        color: sched.faint
                    }
                    Rectangle { x: sched.ix(0); y: sched.iy(sched.headMm); width: sched.ix(sched.wMm) - x; height: sched.heavy; color: sched.ink }
                    // the columns' heads
                    Repeater {
                        model: sched.columns.length
                        Text {
                            required property int index
                            readonly property var c: sched.columns[index]
                            x: c.right ? sched.ix(sched.colX[index] + sched.colW(index) - 1.6) - width : sched.ix(sched.colX[index] + (index === 0 ? sched.pad : 1.6))
                            y: sched.iy(sched.headMm); height: sched.iy(sched.headMm + 5) - y
                            verticalAlignment: Text.AlignVCenter
                            text: c.name
                            font.family: Fonts.mono; font.pixelSize: 2.0 * sched.mm; font.letterSpacing: 0.3 * sched.mm
                            color: sched.faint
                        }
                    }
                    Rectangle { x: sched.ix(0); y: sched.iy(sched.headMm + 5); width: sched.ix(sched.wMm) - x; height: sched.hair; color: sched.ink }
                    Repeater {
                        model: sched.laid
                        Item {
                            id: line
                            required property var modelData
                            readonly property var row: modelData.row
                            // a row that does something (its `action`): a faint tint under the pointer, as the title block's cells
                            Rectangle {
                                visible: !!line.row.action && sched.hoverRow === line.row
                                x: sched.ix(0); y: sched.iy(line.modelData.yMm); width: sched.ix(sched.wMm) - x; height: sched.iy(line.modelData.yMm + line.modelData.hMm) - y
                                color: Qt.rgba(sched.ink.r, sched.ink.g, sched.ink.b, 0.07)
                            }
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
                                    readonly property real fromMm: sched.colX[index] + (index === 0 ? sched.pad : 1.6)
                                    readonly property real toMm: sched.colX[index] + sched.colW(index) - 1.6
                                    readonly property bool bar: index === sched.barCol && line.row.share !== undefined
                                    Text {
                                        x: cell.c.right ? sched.ix(cell.toMm) - width : sched.ix(cell.fromMm)
                                        y: sched.iy(line.modelData.yMm + 1.4)
                                        width: cell.c.right ? implicitWidth : sched.ix(cell.toMm) - sched.ix(cell.fromMm)
                                        text: line.row.cells[cell.index] || ""
                                        elide: cell.c.right ? Text.ElideNone : Text.ElideRight
                                        font.family: Fonts.mono; font.pixelSize: (cell.index === 0 ? 2.5 : 2.6) * sched.mm; font.bold: cell.index === 0
                                        color: sched.rowInk(line.row, cell.bar && line.row.share >= sched.alertAt ? sched.theme.alert : sched.ink)
                                    }
                                    // the share as a bar ruled to the millimetre, 10 mm long, before the figure
                                    Rectangle {
                                        visible: cell.bar
                                        x: sched.ix(cell.fromMm); y: sched.iy(line.modelData.yMm + 2); width: sched.ix(cell.fromMm + 10) - x; height: sched.iy(line.modelData.yMm + 4) - y
                                        color: "transparent"; border.width: sched.hair; border.color: sched.rule
                                        Rectangle {
                                            width: parent.width * Math.max(0, Math.min(1, line.row.share || 0)); height: parent.height
                                            color: sched.rowInk(line.row, (line.row.share || 0) >= sched.alertAt ? sched.theme.alert : sched.ink)
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
                            x: sched.ix(sched.colX[index + 1]); y: sched.iy(sched.headMm); width: sched.hair; height: sched.iy(sched.hMm) - y; color: sched.rule
                        }
                    }
                }

                // dragged a whole millimetre at a time to a clear place; config.toml keeps it on release; a row with an
                // action clicked (pressed and let go without moving it) does that
                MouseArea {
                    anchors.fill: parent
                    enabled: sched.shown
                    hoverEnabled: true
                    cursorShape: pressed ? Qt.ClosedHandCursor : sched.hoverRow ? Qt.PointingHandCursor : Qt.OpenHandCursor
                    onExited: sched.hoverRow = null
                    property point from
                    property var start: null
                    property bool moved: false
                    onPressed: e => { from = mapToGlobal(e.x, e.y); moved = false; start = { x: Math.round(sched.xMm), y: Math.round(sched.yMm) }; keep.stop(); sched.dragged = start }
                    onPositionChanged: e => {
                        if (!pressed || !start) { sched.hoverRow = sched.rowAt(e.y); return }
                        const p = mapToGlobal(e.x, e.y)
                        if (Math.abs(p.x - from.x) + Math.abs(p.y - from.y) > 0.5 * sched.mm) moved = true
                        const x = start.x + Math.round((p.x - from.x) / sched.mm), y = start.y + Math.round((p.y - from.y) / sched.mm)
                        if (sched.clear(x, y)) sched.dragged = { x: x, y: y }
                    }
                    onReleased: {
                        const d = sched.dragged
                        if (d && start && (d.x !== start.x || d.y !== start.y)) sched.shell.setConfig(["sheet.schedule_at=[" + d.x + ", " + d.y + "]"])
                        keep.restart()
                    }
                    onClicked: e => { const r = sched.rowAt(e.y); if (r && !moved) shell.act(r.action, edge.screen.name) }
                }
                Timer { id: keep; interval: 1500; onTriggered: sched.dragged = null }
            }
        }
    }
}
