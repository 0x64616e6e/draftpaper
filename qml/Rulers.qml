// The rulers (issue #48; Mod+r, `draftpaper rulers`): a horizontal and a vertical ruler, each from the
// frame's line to the frame's line, crossing at a point that slides anywhere on the sheet, as clear
// acrylic on the drawing. Where they cross they are one piece: one outline round both. Both long edges of
// each are graduated in true millimetres (the sheet's own, from the display's measured size), counted out
// both ways from the ruler's own zero, which slides along it (a red line across it): at the crossing to
// start with, anywhere along the ruler to measure from (the crossing hides the graduations under it);
// a very thin line runs down the middle of each. Each turns about the crossing: free, or locked to the
// other at any angle; an inclined one shows its degrees on an arc from its level line, locked ones the angle
// between them. A small panel by the crossing says where it and the zeros are, takes the angles typed, and
// holds the lock and the pointer's steps.
//
// The crossing moves 1 mm a step (10 with Shift), the zeros whole millimetres, a ruler turns 1° (15° with
// Shift), unless config.toml's [rulers] says otherwise (step, step_shift, turn, turn_shift; numbers_h top|bottom,
// numbers_v left|right, centre_line, hold_pointer, width: the settings' Rulers page): by the
// keys, or dragging (a ruler moves the crossing, with Ctrl turns; a red line slides its zero; a double click
// puts the zero there). The zeros stay on whole millimetres from the crossing, so the compositor, holding
// the pointer to the rulers' millimetres while they are out (compositor/src/rulers.rs), counts from the
// crossing. On the overlay layer, above the windows, taking clicks only on the rulers and the panel; the
// keyboard once clicked.
//
// Keys: arrows move the crossing; Ctrl+left, right slide the horizontal ruler's zero, Ctrl+up, down the
// vertical one's; [ ] turn the horizontal ruler, , . the vertical one (with Shift by 15°); l locks and
// unlocks; s holds the pointer or lets it go; 0 levels both; p hides the panel (a tab by the crossing brings
// it back) or shows it; Escape puts them away.
import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Wayland

PanelWindow {
    id: rulers
    required property var shell
    visible: false
    screen: Quickshell.screens[0]
    function focusedScreen() { return Quickshell.screens.find(s => Compositor.output(s.name) && Compositor.output(s.name).focused) || Quickshell.screens[0] }
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "draftpaper-rulers"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    color: "transparent"

    // ---- the sheet's geometry (logical pixels, from the output's top-left corner) ----
    readonly property var out: Compositor.output(screen ? screen.name : "")
    readonly property real mm: out ? out.px_per_mm : 4.2
    readonly property real dpr: out ? out.scale : 1
    readonly property real hair: 1 / dpr
    readonly property real line: 2 / dpr
    // inside the frame's line
    readonly property real frameX: (out ? out.frame.x - out.x : 26) + line
    readonly property real frameY: (out ? out.frame.y - out.y : 26) + line
    readonly property real frameW: (out ? out.frame.width : width - 52) - 2 * line
    readonly property real frameH: (out ? out.frame.height : height - 52) - 2 * line

    // ---- the rulers ----
    property real zeroX: 0                // mm from the frame's top-left corner, whole millimetres
    property real zeroY: 0
    property real angleH: 0               // degrees counter-clockwise from the horizontal
    property real angleV: 90
    property bool isLocked: false
    property bool snapOn: true            // the pointer held to the millimetres
    property bool panelShown: true        // the panel by the zero, or only its tab (it may hide the angles)
    // each ruler's own zero, whole millimetres along it from the crossing (right, down when level): its
    // graduations count from there both ways, so the crossing does not hide what is measured near it
    property int zeroH: 0
    property int zeroV: 0
    // config.toml's [rulers] (the settings' Rulers page), through theme.json
    readonly property var conf: shell.theme.rulers
    function setting(k, fallback, lo, hi) { const v = Number(conf[k]); return isFinite(v) && v >= lo && v <= hi ? v : fallback }
    // the crossing's steps, in mm, without Shift and with it; the compositor holds the pointer to them
    readonly property real stepFine: setting("step", 1, 0.1, 50)
    readonly property real stepCoarse: setting("step_shift", 10, 0.1, 200)
    // the zeros' steps: whole millimetres (the graduations count from them)
    readonly property int zeroFine: Math.max(1, Math.round(stepFine))
    readonly property int zeroCoarse: Math.max(1, Math.round(stepCoarse))
    readonly property real turnFine: setting("turn", 1, 0.1, 90)
    readonly property real turnCoarse: setting("turn_shift", 15, 0.1, 90)
    // the edge each ruler is numbered by: the measuring is done in the top right quadrant, so the
    // horizontal ruler's top and the vertical one's right unless set otherwise
    readonly property bool numbersHFar: conf.numbers_h === "bottom"
    readonly property bool numbersVFar: conf.numbers_v !== "left"
    readonly property bool centreLine: conf.centre_line !== false
    readonly property real zx: frameX + zeroX * mm
    readonly property real zy: frameY + zeroY * mm
    readonly property real wide: setting("width", 16, 8, 40) * mm  // a ruler's width
    readonly property real reach: Math.hypot(frameW, frameH) + wide     // long enough at any angle
    readonly property real tiltH: angleH
    readonly property real tiltV: angleV - 90
    // where the rulers cross, the graduations stop: as far as the other one reaches along this one
    readonly property real across: Math.abs(Math.sin((angleV - angleH) * Math.PI / 180)) || 1
    readonly property real clearance: wide / 2 / across

    readonly property color ink: shell.theme.ink
    readonly property color acrylic: shell.theme.data.paper_light || shell.theme.data.paper || "#E1D8CC"
    readonly property color mark: shell.theme.alert

    function norm(a) { a = a % 360; return a > 180 ? a - 360 : (a <= -180 ? a + 360 : a) }
    function fmt(a) { return String(Math.round(a * 10) / 10) }

    // ---- opening and closing ----
    function open() {
        screen = focusedScreen()
        // the first time: the zero in the middle of the sheet, level
        if (zeroX === 0 && zeroY === 0) {
            zeroX = Math.round(frameW / mm / 2)
            zeroY = Math.round(frameH / mm / 2)
        }
        // the pointer held to the steps as they come out, unless [rulers] hold_pointer = false (s toggles)
        snapOn = conf.hold_pointer !== false
        visible = true
        report()
    }
    function close() {
        visible = false
        Compositor.send({ request: "rulers" })
    }
    // the compositor holds the pointer to the rulers' steps: tell it where they are
    function report() {
        if (!visible || !screen) return
        Compositor.send({ request: "rulers", output: screen.name, zero: [zx, zy], angles: [angleH, angleV], snap: snapOn && !dragging,
                          steps: [stepFine, stepCoarse] })
    }
    // any change: the compositor told, the input region made again (once, when the changes are done)
    function changed() { Qt.callLater(report); Qt.callLater(rebuildMask) }
    onZxChanged: changed()
    onZyChanged: changed()
    onAngleHChanged: changed()
    onAngleVChanged: changed()
    onSnapOnChanged: changed()
    onStepFineChanged: changed()
    onStepCoarseChanged: changed()
    onPanelShownChanged: changed()
    property bool dragging: false
    onDraggingChanged: changed()

    // ---- moving and turning ----
    function moveBy(dx, dy) {
        // on a tenth of a millimetre: a step of 0.5 mm stays on its grid
        zeroX = Math.max(0, Math.min(Math.round(frameW / mm), Math.round((zeroX + dx) * 10) / 10))
        zeroY = Math.max(0, Math.min(Math.round(frameH / mm), Math.round((zeroY + dy) * 10) / 10))
    }
    // turn one ruler to `to` degrees; locked, the other turns with it
    function turnH(to) { const d = norm(to - angleH); angleH = norm(angleH + d); if (isLocked) angleV = norm(angleV + d) }
    function turnV(to) { const d = norm(to - angleV); angleV = norm(angleV + d); if (isLocked) angleH = norm(angleH + d) }
    function level() { angleH = 0; angleV = 90 }

    Item {
        id: keys
        anchors.fill: parent
        focus: true
        Keys.onPressed: e => {
            const big = e.modifiers & Qt.ShiftModifier
            const step = big ? rulers.stepCoarse : rulers.stepFine, turn = big ? rulers.turnCoarse : rulers.turnFine
            const zstep = big ? rulers.zeroCoarse : rulers.zeroFine
            const k = e.key
            if (k === Qt.Key_Escape) rulers.close()
            else if ((e.modifiers & Qt.ControlModifier) && k === Qt.Key_Left) rulers.zeroH -= zstep
            else if ((e.modifiers & Qt.ControlModifier) && k === Qt.Key_Right) rulers.zeroH += zstep
            else if ((e.modifiers & Qt.ControlModifier) && k === Qt.Key_Up) rulers.zeroV -= zstep
            else if ((e.modifiers & Qt.ControlModifier) && k === Qt.Key_Down) rulers.zeroV += zstep
            else if (k === Qt.Key_Left) rulers.moveBy(-step, 0)
            else if (k === Qt.Key_Right) rulers.moveBy(step, 0)
            else if (k === Qt.Key_Up) rulers.moveBy(0, -step)
            else if (k === Qt.Key_Down) rulers.moveBy(0, step)
            else if (k === Qt.Key_BracketRight || k === Qt.Key_BraceRight) rulers.turnH(rulers.angleH + turn)
            else if (k === Qt.Key_BracketLeft || k === Qt.Key_BraceLeft) rulers.turnH(rulers.angleH - turn)
            else if (k === Qt.Key_Period || k === Qt.Key_Greater) rulers.turnV(rulers.angleV + turn)
            else if (k === Qt.Key_Comma || k === Qt.Key_Less) rulers.turnV(rulers.angleV - turn)
            else if (k === Qt.Key_L) rulers.isLocked = !rulers.isLocked
            else if (k === Qt.Key_S) rulers.snapOn = !rulers.snapOn
            else if (k === Qt.Key_0) rulers.level()
            else if (k === Qt.Key_P) rulers.panelShown = !rulers.panelShown
            else return
            e.accepted = true
        }
    }

    // ---- the rulers, inside the frame ----
    Item {
        id: sheet
        x: rulers.frameX; y: rulers.frameY; width: rulers.frameW; height: rulers.frameH
        clip: true
        readonly property real cx: rulers.zx - x
        readonly property real cy: rulers.zy - y

        // one piece where they cross: both outlines, then both bodies over them, inset by the outline,
        // so that each body hides the other's outline where they overlap
        Item {
            anchors.fill: parent
            opacity: 0.8
            Rectangle {
                x: sheet.cx - rulers.reach; y: sheet.cy - rulers.wide / 2; width: 2 * rulers.reach; height: rulers.wide
                rotation: -rulers.tiltH; transformOrigin: Item.Center
                color: "transparent"; border.color: rulers.ink; border.width: rulers.line
            }
            Rectangle {
                x: sheet.cx - rulers.wide / 2; y: sheet.cy - rulers.reach; width: rulers.wide; height: 2 * rulers.reach
                rotation: -rulers.tiltV; transformOrigin: Item.Center
                color: "transparent"; border.color: rulers.ink; border.width: rulers.line
            }
            Rectangle {
                x: sheet.cx - rulers.reach; y: sheet.cy - rulers.wide / 2 + rulers.line; width: 2 * rulers.reach; height: rulers.wide - 2 * rulers.line
                rotation: -rulers.tiltH; transformOrigin: Item.Center
                color: rulers.acrylic
            }
            Rectangle {
                x: sheet.cx - rulers.wide / 2 + rulers.line; y: sheet.cy - rulers.reach; width: rulers.wide - 2 * rulers.line; height: 2 * rulers.reach
                rotation: -rulers.tiltV; transformOrigin: Item.Center
                color: rulers.acrylic
            }
        }

        // the graduations, each ruler's along both its edges, and its centre line
        Graduations {
            x: sheet.cx - rulers.reach; y: sheet.cy - rulers.wide / 2; width: 2 * rulers.reach; height: rulers.wide
            rotation: -rulers.tiltH
            mm: rulers.mm; ink: rulers.ink; hair: rulers.hair; clearance: rulers.clearance; along: true; zero: rulers.zeroH; far: rulers.numbersHFar; centreLine: rulers.centreLine
        }
        Graduations {
            x: sheet.cx - rulers.wide / 2; y: sheet.cy - rulers.reach; width: rulers.wide; height: 2 * rulers.reach
            rotation: -rulers.tiltV
            mm: rulers.mm; ink: rulers.ink; hair: rulers.hair; clearance: rulers.clearance; along: false; zero: rulers.zeroV; far: rulers.numbersVFar; centreLine: rulers.centreLine
        }

        // dragging a ruler moves the zero; with Ctrl, turns that ruler about it
        MouseArea {
            id: hArea
            x: sheet.cx - rulers.reach; y: sheet.cy - rulers.wide / 2; width: 2 * rulers.reach; height: rulers.wide
            rotation: -rulers.tiltH
            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            onPressed: e => rulers.press(e, hArea)
            onPositionChanged: e => rulers.drag(e, hArea, "h")
            // a double click: this ruler's zero there
            onDoubleClicked: e => rulers.zeroH = Math.round((e.x - rulers.reach) / rulers.mm)
            onReleased: rulers.dragging = false
        }
        MouseArea {
            id: vArea
            x: sheet.cx - rulers.wide / 2; y: sheet.cy - rulers.reach; width: rulers.wide; height: 2 * rulers.reach
            rotation: -rulers.tiltV
            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            onPressed: e => rulers.press(e, vArea)
            onPositionChanged: e => rulers.drag(e, vArea, "v")
            onReleased: rulers.dragging = false
            onDoubleClicked: e => rulers.zeroV = Math.round((e.y - rulers.reach) / rulers.mm)
        }

        // each ruler's zero: a line across it in the alert colour, 0 by its numbered edge; dragged along it
        // in whole millimetres (ten with Shift)
        Repeater {
            model: [
                { along: true, tilt: rulers.tiltH },
                { along: false, tilt: rulers.tiltV },
            ]
            Item {
                id: zeroMark
                required property var modelData
                readonly property bool along: modelData.along
                readonly property int at: along ? rulers.zeroH : rulers.zeroV
                x: along ? sheet.cx - rulers.reach : sheet.cx - rulers.wide / 2
                y: along ? sheet.cy - rulers.wide / 2 : sheet.cy - rulers.reach
                width: along ? 2 * rulers.reach : rulers.wide
                height: along ? rulers.wide : 2 * rulers.reach
                rotation: -modelData.tilt
                // the line, and its handle: 4 mm across it to take hold of
                Rectangle {
                    x: zeroMark.along ? rulers.reach + zeroMark.at * rulers.mm - width / 2 : 0
                    y: zeroMark.along ? 0 : rulers.reach + zeroMark.at * rulers.mm - height / 2
                    width: zeroMark.along ? 2 * rulers.hair : rulers.wide
                    height: zeroMark.along ? rulers.wide : 2 * rulers.hair
                    color: rulers.mark
                }
                Text {          // 0, by the numbered edge
                    readonly property bool far: zeroMark.along ? rulers.numbersHFar : rulers.numbersVFar
                    readonly property real numberAt: Math.max(0.4 * rulers.wide, 5.6 * rulers.mm)
                    readonly property real edge: far ? rulers.wide - numberAt : numberAt
                    x: zeroMark.along ? rulers.reach + zeroMark.at * rulers.mm + 0.8 * rulers.mm : edge - width / 2
                    y: zeroMark.along ? edge - height / 2 : rulers.reach + zeroMark.at * rulers.mm + 0.8 * rulers.mm
                    text: "0"
                    font.family: Fonts.mono; font.pixelSize: 2.4 * rulers.mm; font.weight: Font.Bold
                    color: rulers.mark
                }
                MouseArea {
                    x: zeroMark.along ? rulers.reach + zeroMark.at * rulers.mm - width / 2 : 0
                    y: zeroMark.along ? 0 : rulers.reach + zeroMark.at * rulers.mm - height / 2
                    width: zeroMark.along ? 4 * rulers.mm : rulers.wide
                    height: zeroMark.along ? rulers.wide : 4 * rulers.mm
                    cursorShape: zeroMark.along ? Qt.SizeHorCursor : Qt.SizeVerCursor
                    preventStealing: true
                    onPressed: keys.forceActiveFocus()
                    onPositionChanged: e => {
                        if (!pressed) return
                        const p = mapToItem(zeroMark, e.x, e.y)
                        const unit = (e.modifiers & Qt.ShiftModifier) ? rulers.zeroCoarse : rulers.zeroFine
                        const v = Math.round(((zeroMark.along ? p.x : p.y) - rulers.reach) / rulers.mm / unit) * unit
                        if (zeroMark.along) rulers.zeroH = v
                        else rulers.zeroV = v
                    }
                }
            }
        }

        // an inclined ruler: its level line dashed, its degrees on an arc from it
        Repeater {
            model: [
                { from: 0, to: rulers.angleH, show: Math.abs(rulers.angleH) > 0.05 },
                { from: 90, to: rulers.angleV, show: Math.abs(rulers.angleV - 90) > 0.05 },
            ]
            Item {
                id: inclined
                required property var modelData
                visible: modelData.show
                anchors.fill: parent
                readonly property real r: 46 * rulers.mm
                readonly property real mid: (modelData.from + modelData.to) / 2 * Math.PI / 180
                Shape {
                    anchors.fill: parent
                    ShapePath {
                        strokeColor: rulers.mark; strokeWidth: 1.5 * rulers.hair; fillColor: "transparent"
                        PathAngleArc {
                            centerX: sheet.cx; centerY: sheet.cy; radiusX: inclined.r; radiusY: radiusX
                            startAngle: -modelData.from; sweepAngle: -(modelData.to - modelData.from)
                        }
                    }
                    ShapePath {
                        strokeColor: rulers.mark; strokeWidth: rulers.hair; fillColor: "transparent"
                        strokeStyle: ShapePath.DashLine; dashPattern: [4, 3]
                        startX: sheet.cx; startY: sheet.cy
                        PathLine {
                            x: sheet.cx + 60 * rulers.mm * Math.cos(modelData.from * Math.PI / 180)
                            y: sheet.cy - 60 * rulers.mm * Math.sin(modelData.from * Math.PI / 180)
                        }
                    }
                }
                Rectangle {
                    x: sheet.cx + (inclined.r + 7 * rulers.mm) * Math.cos(inclined.mid) - width / 2
                    y: sheet.cy - (inclined.r + 7 * rulers.mm) * Math.sin(inclined.mid) - height / 2
                    width: angleText.implicitWidth + 3 * rulers.mm; height: angleText.implicitHeight + 1.2 * rulers.mm
                    color: rulers.mark
                    Text {
                        id: angleText
                        anchors.centerIn: parent
                        text: rulers.fmt(Math.abs(rulers.norm(modelData.to - modelData.from))) + "°"
                        font.family: Fonts.mono; font.pixelSize: 2.6 * rulers.mm; font.weight: Font.Medium
                        color: "#FAF4EC"
                    }
                }
            }
        }

        // locked: the angle between them, on an arc, with a lock
        Item {
            visible: rulers.isLocked
            anchors.fill: parent
            readonly property real r: 22 * rulers.mm
            readonly property real mid: (rulers.angleH + rulers.angleV) / 2 * Math.PI / 180
            Shape {
                anchors.fill: parent
                ShapePath {
                    strokeColor: rulers.ink; strokeWidth: rulers.line; fillColor: "transparent"
                    PathAngleArc {
                        centerX: sheet.cx; centerY: sheet.cy; radiusX: 22 * rulers.mm; radiusY: radiusX
                        startAngle: -rulers.angleH; sweepAngle: -(rulers.angleV - rulers.angleH)
                    }
                }
            }
            Rectangle {
                x: sheet.cx + (parent.r + 8 * rulers.mm) * Math.cos(parent.mid) - width / 2
                y: sheet.cy - (parent.r + 8 * rulers.mm) * Math.sin(parent.mid) - height / 2
                width: lockText.implicitWidth + 3 * rulers.mm; height: lockText.implicitHeight + 1.2 * rulers.mm
                color: rulers.ink
                Text {
                    id: lockText
                    anchors.centerIn: parent
                    text: rulers.fmt(rulers.angleV - rulers.angleH) + "°"
                    font.family: Fonts.mono; font.pixelSize: 2.6 * rulers.mm; font.weight: Font.Medium
                    color: "#FAF4EC"
                }
            }
        }

        // the zero point
        Rectangle {
            x: sheet.cx - width / 2; y: sheet.cy - height / 2; width: 2.2 * rulers.mm; height: width; radius: width / 2
            color: rulers.acrylic; border.color: rulers.ink; border.width: 1.5 * rulers.hair
            Rectangle { anchors.centerIn: parent; width: 0.7 * rulers.mm; height: width; radius: width / 2; color: rulers.ink }
        }
    }

    // ---- the panel by the zero point ----
    Rectangle {
        id: panel
        visible: rulers.panelShown
        readonly property bool rightSide: rulers.zx + rulers.wide / 2 + 5 * rulers.mm + width < rulers.frameX + rulers.frameW
        x: rightSide ? rulers.zx + rulers.wide / 2 + 5 * rulers.mm : rulers.zx - rulers.wide / 2 - 5 * rulers.mm - width
        y: Math.min(rulers.zy + rulers.wide / 2 + 5 * rulers.mm, rulers.frameY + rulers.frameH - height)
        width: 50 * rulers.mm; height: column.implicitHeight + 4 * rulers.mm
        color: Qt.rgba(rulers.ink.r, rulers.ink.g, rulers.ink.b, 0.93)
        MouseArea { anchors.fill: parent; onPressed: keys.forceActiveFocus() }
        // hide the panel (p): its tab stays by the zero to bring it back
        Rectangle {
            anchors { top: parent.top; right: parent.right; margins: 1.2 * rulers.mm }
            width: 4.2 * rulers.mm; height: width
            color: hideArea.containsMouse ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
            Text {
                anchors.centerIn: parent
                text: "×"
                font.family: Fonts.mono; font.pixelSize: 3.2 * rulers.mm; color: "#EDE3D2"
            }
            MouseArea {
                id: hideArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: { rulers.panelShown = false; keys.forceActiveFocus() }
            }
        }
        Column {
            id: column
            x: 2.4 * rulers.mm; y: 2 * rulers.mm; spacing: 1.6 * rulers.mm
            Text {
                text: "crossing at " + rulers.zeroX + ", " + rulers.zeroY + " mm"
                font.family: Fonts.mono; font.pixelSize: 2.6 * rulers.mm; font.weight: Font.Medium; color: "#EDE3D2"
            }
            Text {
                // each ruler's zero from the crossing (right, down when level)
                function signed(v) { return v > 0 ? "+" + v : v < 0 ? "\u2212" + (-v) : "0" }
                text: "0 at H " + signed(rulers.zeroH) + ", V " + signed(rulers.zeroV) + " mm"
                font.family: Fonts.mono; font.pixelSize: 2.4 * rulers.mm; color: "#DAD0C5"
            }
            Row {
                spacing: 2.4 * rulers.mm
                Repeater {
                    model: [{ name: "H", which: "h" }, { name: "V", which: "v" }]
                    Row {
                        required property var modelData
                        spacing: 1 * rulers.mm
                        Text { anchors.verticalCenter: parent.verticalCenter; text: modelData.name; font.family: Fonts.mono; font.pixelSize: 2.6 * rulers.mm; color: "#EDE3D2" }
                        Rectangle {
                            width: 10 * rulers.mm; height: 5 * rulers.mm; color: rulers.acrylic
                            TextInput {
                                id: field
                                anchors { fill: parent; rightMargin: 1 * rulers.mm }
                                verticalAlignment: TextInput.AlignVCenter; horizontalAlignment: TextInput.AlignRight
                                font.family: Fonts.mono; font.pixelSize: 2.6 * rulers.mm; color: rulers.ink
                                validator: DoubleValidator { bottom: -360; top: 360; decimals: 1 }
                                text: rulers.fmt(modelData.which === "h" ? rulers.angleH : rulers.angleV)
                                selectByMouse: true
                                onAccepted: {
                                    const v = Number(text.replace(",", "."))
                                    if (Number.isFinite(v)) modelData.which === "h" ? rulers.turnH(v) : rulers.turnV(v)
                                    keys.forceActiveFocus()
                                }
                                Keys.onEscapePressed: keys.forceActiveFocus()
                            }
                        }
                        Text { anchors.verticalCenter: parent.verticalCenter; text: "°"; font.family: Fonts.mono; font.pixelSize: 2.6 * rulers.mm; color: "#EDE3D2" }
                    }
                }
            }
            Row {
                spacing: 1.6 * rulers.mm
                Repeater {
                    model: [
                        { label: rulers.isLocked ? "locked at " + rulers.fmt(rulers.angleV - rulers.angleH) + "°" : "lock the angle", on: rulers.isLocked, act: () => rulers.isLocked = !rulers.isLocked },
                        { label: rulers.snapOn ? "pointer held" : "pointer free", on: rulers.snapOn, act: () => rulers.snapOn = !rulers.snapOn },
                    ]
                    Rectangle {
                        required property var modelData
                        width: buttonText.implicitWidth + 2.4 * rulers.mm; height: 5 * rulers.mm
                        color: modelData.on ? rulers.shell.theme.accent : "transparent"
                        border.color: "#DAD0C5"; border.width: rulers.hair
                        Text {
                            id: buttonText
                            anchors.centerIn: parent
                            text: modelData.label
                            font.family: Fonts.mono; font.pixelSize: 2.4 * rulers.mm; font.weight: Font.Medium
                            color: modelData.on ? rulers.ink : "#EDE3D2"
                        }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { modelData.act(); keys.forceActiveFocus() } }
                    }
                }
            }
            Text {
                text: "move 1 mm · ⇧ 10 · turn 1° · ⇧ 15°"
                font.family: Fonts.mono; font.pixelSize: 2.2 * rulers.mm; color: "#DAD0C5"
            }
        }
    }

    // ---- the panel hidden: its tab, where its corner by the zero was ----
    Rectangle {
        id: tab
        visible: !rulers.panelShown
        width: 6 * rulers.mm; height: width
        x: panel.rightSide ? rulers.zx + rulers.wide / 2 + 5 * rulers.mm : rulers.zx - rulers.wide / 2 - 5 * rulers.mm - width
        y: Math.min(rulers.zy + rulers.wide / 2 + 5 * rulers.mm, rulers.frameY + rulers.frameH - height)
        color: Qt.rgba(rulers.ink.r, rulers.ink.g, rulers.ink.b, 0.93)
        Text {
            anchors.centerIn: parent
            text: "∠"
            font.family: Fonts.mono; font.pixelSize: 3.4 * rulers.mm; font.weight: Font.Medium; color: "#EDE3D2"
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: { rulers.panelShown = true; keys.forceActiveFocus() }
        }
    }

    // ---- dragging ----
    // where a drag began, on the sheet, and the zero then; and whether it turns (Ctrl) or moves
    property point pressAt: Qt.point(0, 0)
    property point pressZero: Qt.point(0, 0)
    property bool turning: false
    function press(e, area) {
        keys.forceActiveFocus()
        dragging = true
        turning = (e.modifiers & Qt.ControlModifier) !== 0
        pressAt = sheet.mapFromItem(area, e.x, e.y)
        pressZero = Qt.point(zeroX, zeroY)
    }
    function drag(e, area, which) {
        if (!dragging) return
        const p = sheet.mapFromItem(area, e.x, e.y)
        const big = (e.modifiers & Qt.ShiftModifier) !== 0
        if (turning) {
            // the pointer's direction from the zero, in whole degrees (15 with Shift); a ruler points both
            // ways, so the nearer of the two
            const unit = big ? turnCoarse : turnFine
            let a = Math.round(Math.atan2(-(p.y - sheet.cy), p.x - sheet.cx) * 180 / Math.PI / unit) * unit
            const now = which === "v" ? angleV : angleH
            if (Math.abs(norm(a - now)) > 90) a = norm(a + 180)
            which === "v" ? turnV(a) : turnH(a)
        } else {
            // the zero follows, in whole millimetres (10 with Shift)
            const unit = big ? stepCoarse : stepFine
            const dx = Math.round((p.x - pressAt.x) / mm / unit) * unit, dy = Math.round((p.y - pressAt.y) / mm / unit) * unit
            moveBy(pressZero.x + dx - zeroX, pressZero.y + dy - zeroY)
        }
    }

    // the input region: the rulers (a run of squares along an inclined one) and the panel
    Component { id: piece; Region {} }
    property var pieces: []
    function rebuildMask() {
        for (const p of pieces) p.destroy()
        const list = []
        const add = (x, y, w, h) => list.push(piece.createObject(rulers, { x: Math.round(x), y: Math.round(y), width: Math.round(w), height: Math.round(h) }))
        const band = (deg) => {
            const t = ((deg % 180) + 180) % 180
            if (t < 0.05 || t > 179.95) return add(frameX, zy - wide / 2, frameW, wide)
            if (Math.abs(t - 90) < 0.05) return add(zx - wide / 2, frameY, wide, frameH)
            const c = Math.cos(deg * Math.PI / 180), s = -Math.sin(deg * Math.PI / 180)
            for (let d = -reach; d <= reach; d += wide / 2) {
                const px = zx + d * c, py = zy + d * s
                if (px < frameX - wide || px > frameX + frameW + wide || py < frameY - wide || py > frameY + frameH + wide) continue
                add(px - wide * 0.55, py - wide * 0.55, wide * 1.1, wide * 1.1)
            }
        }
        band(angleH)
        band(angleV)
        if (panelShown) add(panel.x, panel.y, panel.width, panel.height)
        else add(tab.x, tab.y, tab.width, tab.height)
        pieces = list
    }
    mask: Region { regions: rulers.pieces }
    onVisibleChanged: if (visible) changed()
}
