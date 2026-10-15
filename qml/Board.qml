// The board the settings (Settings.qml) lay their drawings on: the inside of the sheet's frame,
// cleared of windows. board.frag draws it: the sheet's own paper in theme `t` (worked out over the whole
// output, so it meets the margins without a seam), or with a wallpaper the picture blurred and tinted a
// little with the paper so lettering reads on it. Outside the frame's interior, and over the title block
// when no window covers it, the board is clear: the sheet's margins and title block show. Fills the
// layer surface it is in, which covers the whole output.
import QtQuick
import QtQuick.Effects
import Quickshell

Item {
    id: board
    required property var out                   // the output, from Compositor.qml
    property var t: null                        // the theme whose paper the board is
    property string wallpaper: ""               // a file URL, or "" for none
    property bool keepBlock: true               // leave the title block showing when no window covers it
    anchors.fill: parent

    readonly property real mm: out ? out.px_per_mm : 4.2
    readonly property real dpr: out ? out.scale : 1
    readonly property real margin: out ? out.margin : Math.round(6 * mm)
    readonly property color ink: t ? t.ink : "#62574C"
    // the frame's interior, in logical pixels (the frame's line is 2 physical pixels wide)
    readonly property real inX: margin + 2 / dpr
    readonly property real inY: margin + 2 / dpr
    readonly property real inW: width - 2 * inX
    readonly property real inH: height - 2 * inY
    // the title block (TitleBlock.qml: 130 by 36 mm, in the frame's bottom-right corner), and whether a
    // window on this screen covers it
    readonly property rect block: Qt.rect(width - Math.round(margin) - Math.round(130 * mm), height - Math.round(margin) - Math.round(36 * mm),
                                          Math.round(130 * mm), Math.round(36 * mm))
    readonly property bool blockCovered: out ? Compositor.windows.some(w => w.output === out.name && w.geometry && !w.parked
        && w.geometry.x - out.x < block.x + block.width && w.geometry.x - out.x + w.geometry.width > block.x
        && w.geometry.y - out.y < block.y + block.height && w.geometry.y - out.y + w.geometry.height > block.y) : true

    // the wallpaper over the interior, cropped as the compositor crops it (to fill, centred), blurred and
    // tinted; the shader samples it
    Item {
        id: blurred
        width: board.inW; height: board.inH
        visible: false
        Image {
            id: picture
            anchors.fill: parent
            source: board.wallpaper
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(Math.round(board.inW * board.dpr / 2), Math.round(board.inH * board.dpr / 2))
            asynchronous: true
            cache: false
            visible: false
        }
        MultiEffect {
            anchors.fill: parent
            source: picture
            blurEnabled: true
            blur: 1.0
            blurMax: 48
            autoPaddingEnabled: false
        }
        Rectangle { anchors.fill: parent; color: board.t ? board.t.paper : "#ffffff"; opacity: 0.35 }
    }

    ShaderEffect {
        anchors.fill: parent
        readonly property var th: board.t
        property color paper: th ? th.paper : "#ffffff"
        property color paperLight: th ? th.paper_light : "#ffffff"
        property color paperShade: th ? th.paper_shade : "#ffffff"
        property color glow: th ? th.glow : "#ffffff"
        property color ink: board.ink
        property vector4d shape: th ? Qt.vector4d(th.sheet.angle * Math.PI / 180, th.sheet.midpoint, th.sheet.blend, th.sheet.shading) : Qt.vector4d(1, 0.5, 1, 1)
        property vector4d strength: th ? Qt.vector4d(th.sheet.glow_strength, th.sheet.glow_size, th.sheet.pattern_strength, board.mm * board.dpr) : Qt.vector4d(0, 1, 0, 6)
        property vector4d physical: Qt.vector4d(board.width * board.dpr, board.height * board.dpr, picture.status === Image.Ready ? 1 : 0, 0)
        property vector4d interior: Qt.vector4d(board.margin * board.dpr + 2, board.margin * board.dpr + 2,
                                                (board.width - board.margin) * board.dpr - 2, (board.height - board.margin) * board.dpr - 2)
        property vector4d hole: board.blockCovered || !board.keepBlock ? Qt.vector4d(0, 0, 0, 0)
            : Qt.vector4d(board.block.x * board.dpr, board.block.y * board.dpr, (board.block.x + board.block.width) * board.dpr, (board.block.y + board.block.height) * board.dpr)
        property var picture: ShaderEffectSource {
            sourceItem: blurred
            textureSize: Qt.size(Math.round(board.inW * board.dpr), Math.round(board.inH * board.dpr))
            live: true
        }
        fragmentShader: Qt.resolvedUrl("board.frag.qsb")
    }
}
