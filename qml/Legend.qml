// The keyboard's legend (docs/keyboard.md): hold Mod alone and the keymap is lettered onto the sheet,
// as a drawing's legend sits beside the drawing. Let Mod go and it is gone.
//
// The compositor decides when — Mod held alone past [keys] legend_after, any other key cancelling it, so
// a slow chord never raises it — and says so in the outputs it sends (`legend`). What is lettered is the
// compositor's own keymap, asked for once over the IPC ("keys"): the bindings as the table and the
// config write them, never a second copy kept here. It letters nothing it was not given.
//
// Everything in it sits at whole millimetres from the frame's corner and is snapped to physical pixels,
// as everything else on the sheet does (docs/wayland-design.md, "Invariants: drawing on the sheet").
import QtQuick
import Quickshell
import Quickshell.Wayland
import "sheet.js" as Sheet

Item {
    id: legend
    required property var shell
    required property var edge
    property bool wanted: false                // the compositor says Mod is being held
    property var keys: []                      // [{ keys: "Mod+h", action: "focus left" }]

    readonly property var theme: shell.theme
    readonly property var out: edge.output
    readonly property real mm: edge.mm
    readonly property real dpr: edge.dpr
    readonly property color ink: theme.ink
    readonly property color faint: Qt.rgba(ink.r, ink.g, ink.b, Math.min(1, 0.6 * theme.inkAlpha + 0.1))

    readonly property var frame: Sheet.frameMm(out)
    readonly property real frameX: out ? out.frame.x - out.x : 0
    readonly property real frameY: out ? out.frame.y - out.y : 0
    readonly property var titleRect: out && out.title && out.title.rect ? out.title.rect : null
    readonly property var titleMm: titleRect ? { x: Math.floor((titleRect.x - frameX) / mm), y: Math.floor((titleRect.y - frameY) / mm) } : null

    // ---- where ----
    // the sheet's free side: in from the frame's top-left corner, down to the title block or the frame.
    // The schedules keep to the title block's column on the right, so the two do not meet.
    readonly property int gap: mmFrom ? mmFrom.gap : 6
    readonly property int xMm: gap
    readonly property int yMm: gap
    // across: the whole free width of the sheet, stopping short of a schedule where one is out, since
    // the schedules keep to the title block's column and the legend would otherwise letter over them
    readonly property int rightMm: {
        let right = frame.w - gap
        if (avoid && avoid.shown) right = Math.min(right, Math.floor(avoid.xMm) - 4)
        return right
    }
    property var avoid: null
    readonly property int wMm: Math.max(0, rightMm - xMm)
    // down: never into the title block. The legend is bounded vertically, so it has no reason to be
    // bounded horizontally by the title block as well.
    readonly property int roomMm: Math.max(0, (titleMm ? titleMm.y - gap : frame.h - gap) - yMm)

    // ---- what fits ----
    // a drawing's legend is a table, and this one is as many columns of it as the sheet's free side will
    // take: each pair is a key and what it does. The tail that will not fit is counted, not dropped
    // silently — "a crowded sheet gets a shorter legend, not a broken overlay".
    property var mmFrom: null                       // the document's measurements (docs/keyboard.md)
    readonly property int headMm: mmFrom ? mmFrom.head : 10
    readonly property int rowMm: mmFrom ? mmFrom.row : 6
    readonly property int keyMm: mmFrom ? mmFrom.key : 28
    readonly property int actMm: mmFrom ? mmFrom.act : 35
    readonly property int innerMm: mmFrom ? mmFrom.inner : 2
    readonly property int pairMm: keyMm + innerMm + actMm + (mmFrom ? mmFrom.gutter : 3)
    readonly property int cols: Math.max(1, Math.floor(wMm / pairMm))
    readonly property int rowsPerCol: Math.max(0, Math.floor((roomMm - headMm) / rowMm))
    readonly property int room: cols * rowsPerCol
    readonly property var laid: {
        const out = []
        const n = Math.min(keys.length, room)
        for (let i = 0; i < n; i++)
            out.push({ keys: keys[i].keys, action: keys[i].action, col: Math.floor(i / rowsPerCol), row: i % rowsPerCol })
        return out
    }
    readonly property int over: Math.max(0, keys.length - laid.length)
    readonly property bool shown: wanted && !!out && laid.length > 0 && wMm >= 60

    // the drawn height: the heading and the rows that fit
    readonly property int hMm: headMm + Math.min(rowsPerCol, Math.ceil(laid.length / Math.max(1, cols))) * rowMm

    readonly property real x0: frameX + xMm * mm
    readonly property real y0: frameY + yMm * mm
    readonly property real sx: Sheet.surfaceAt(x0, dpr)
    readonly property real sy: Sheet.surfaceAt(y0, dpr)
    function ix(u) { return Math.round((x0 + u * mm) * dpr) / dpr - sx }
    function iy(v) { return Math.round((y0 + v * mm) * dpr) / dpr - sy }

    // it comes without animation and goes without one: it is an answer to a key being held, and the sheet
    // does not move to catch the eye (docs/keyboard.md)
    property bool windowUp: false
    onShownChanged: { if (shown) { windowUp = true; drop.stop() } else drop.restart() }
    Timer { id: drop; interval: 20000; onTriggered: legend.windowUp = false }

    LazyLoader {
        active: legend.windowUp
        component: Component {
            PanelWindow {
                screen: legend.edge.screen
                visible: legend.shown
                anchors { top: true; left: true }
                margins { top: legend.sy; left: legend.sx }
                implicitWidth: Math.ceil(legend.ix(legend.wMm)) + 1
                implicitHeight: Math.max(1, Math.ceil(legend.iy(legend.hMm)) + 1)
                exclusionMode: ExclusionMode.Ignore
                aboveWindows: true
                // it never takes the keyboard: Mod is held, and every key must go on reaching whatever
                // has the focus, or the gesture would eat the chord it is teaching
                focusable: false
                color: "transparent"
                WlrLayershell.namespace: "draftpaper-legend"

                Item {
                    anchors.fill: parent

                    Rectangle {     // over a wallpaper, and over windows: the paper under it, to be read
                        x: legend.ix(0); y: legend.iy(0)
                        width: legend.ix(legend.cols * legend.pairMm) - x; height: legend.iy(legend.hMm) - y
                        color: Qt.rgba(legend.theme.paper.r, legend.theme.paper.g, legend.theme.paper.b,
                                       Math.max(0.92, legend.theme.titleTint))
                    }

                    Text {          // the heading, as a schedule's
                        x: legend.ix(0); y: legend.iy(headRule.vMm - 7); height: legend.iy(headRule.vMm) - y
                        verticalAlignment: Text.AlignVCenter
                        text: "LEGEND"
                        font.family: Fonts.mono; font.pixelSize: 2.8 * legend.mm; font.bold: true
                        font.letterSpacing: 0.25 * legend.mm
                        color: legend.ink
                    }
                    Text {          // and what it is of
                        x: legend.ix(legend.cols * legend.pairMm) - width
                        y: legend.iy(headRule.vMm - 7); height: legend.iy(headRule.vMm) - y
                        verticalAlignment: Text.AlignVCenter
                        text: legend.over > 0 ? "MOD HELD · " + legend.over + " MORE" : "MOD HELD"
                        font.family: Fonts.mono; font.pixelSize: 2.0 * legend.mm; font.letterSpacing: 0.3 * legend.mm
                        color: legend.faint
                    }
                    Rectangle {     // the rule under the heading
                        id: headRule
                        readonly property int vMm: legend.headMm - 2
                        x: legend.ix(0); y: legend.iy(vMm)
                        width: legend.ix(legend.cols * legend.pairMm) - x
                        height: Math.max(1, Math.round(0.4 * legend.mm * legend.dpr) / legend.dpr)
                        color: legend.ink
                    }

                    Repeater {
                        model: legend.laid
                        delegate: Item {
                            required property var modelData
                            readonly property int atX: modelData.col * legend.pairMm
                            readonly property int atY: legend.headMm + modelData.row * legend.rowMm
                            Text {      // the key, as the table writes it
                                x: legend.ix(atX); y: legend.iy(atY)
                                width: legend.ix(atX + legend.keyMm) - x
                                height: legend.iy(atY + legend.rowMm) - y
                                verticalAlignment: Text.AlignVCenter
                                elide: Text.ElideRight      // XF86AudioRaiseVolume and its like
                                text: modelData.keys
                                // not bold: the client letters in one weight, and a hierarchy the two
                                // renderers cannot both draw is a hierarchy the sheet should not use.
                                // The key is in full ink and what it does is faint; that is the contrast.
                                font.family: Fonts.mono; font.pixelSize: 2.2 * legend.mm
                                color: legend.ink
                            }
                            Text {      // what it does
                                x: legend.ix(atX + legend.keyMm + legend.innerMm); y: legend.iy(atY)
                                width: legend.ix(atX + legend.keyMm + legend.innerMm + legend.actMm) - x
                                height: legend.iy(atY + legend.rowMm) - y
                                verticalAlignment: Text.AlignVCenter
                                elide: Text.ElideRight
                                text: modelData.action
                                font.family: Fonts.mono; font.pixelSize: 2.2 * legend.mm
                                color: legend.faint
                            }
                        }
                    }
                }
            }
        }
    }
}
