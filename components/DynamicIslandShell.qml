import QtQuick
import QtQuick.Shapes
import ".."

Item {
    id: root
    property real bodyWidth: 420
    property real bodyHeight: 100
    property real earRadius: 16
    property real bottomRadius: 24
    property color topColor: Theme.barBg
    property color fillColor: Theme.popupBg
    property color borderColor: Theme.border
    property bool invertY: false
    property bool vertical: false
    property bool invertX: false

    transform: Scale {
        origin.x: root.width / 2
        origin.y: root.height / 2
        xScale: (root.vertical && root.invertX) ? -1 : 1
        yScale: (!root.vertical && root.invertY) ? -1 : 1
    }

    readonly property real effEar: Math.min(earRadius, Math.max(0.5, bodyHeight * 0.34))
    readonly property real effBottom: Math.min(bottomRadius, Math.max(0.5, bodyHeight * 0.62), Math.max(0.5, bodyWidth * 0.48))
    readonly property real bodyL: earRadius
    readonly property real bodyR: width - earRadius
    readonly property real earScale: effEar / 16.0

    readonly property real vEffEar: Math.min(earRadius, Math.max(0.5, width * 0.34))
    readonly property real vEffCorner: Math.min(bottomRadius, Math.max(0.5, width * 0.62), Math.max(0.5, bodyHeight * 0.48))
    readonly property real vBodyT: earRadius
    readonly property real vBodyB: height - earRadius
    readonly property real vEarScale: vEffEar / 16.0

    implicitWidth: vertical ? Math.round(bodyWidth) : Math.round(bodyWidth + 2 * earRadius)
    implicitHeight: vertical ? Math.round(bodyHeight + 2 * earRadius) : Math.round(bodyHeight)
    width: implicitWidth
    height: implicitHeight
    visible: vertical ? (bodyWidth >= 1) : (bodyHeight >= 1)

    // Horizontal Dynamic Island (zero per-frame Shape tessellation: GPU Rectangles + cached 16x16 ear sprites)
    Item {
        visible: !root.vertical
        anchors.fill: parent

        Rectangle {
            x: root.bodyL
            y: 0
            width: Math.max(1, root.bodyWidth)
            height: Math.max(1, root.height)
            radius: root.effBottom
            color: root.fillColor
        }

        // Square off top-left and top-right rounded corners cleanly in upper half (zero seam)
        Rectangle {
            x: root.bodyL
            y: 0
            width: Math.max(1, root.bodyWidth)
            height: Math.min(root.height * 0.5, root.effBottom)
            color: root.fillColor
        }

        // Subtle continuous top-edge bar color blend (only when topColor differs from fillColor)
        Rectangle {
            visible: root.topColor !== root.fillColor && root.bodyWidth > 2 && root.height > 2
            x: root.bodyL + 1
            y: 0
            width: Math.max(0, root.bodyWidth - 2)
            height: Math.min(28, root.height - 1)
            gradient: Gradient {
                GradientStop { position: 0.0; color: root.topColor }
                GradientStop { position: 1.0; color: Qt.rgba(root.fillColor.r, root.fillColor.g, root.fillColor.b, 0.0) }
            }
        }

        // Bottom rounded border clip strip
        Item {
            x: root.bodyL
            anchors.bottom: parent.bottom
            width: Math.max(1, root.bodyWidth)
            height: Math.min(root.height, root.effBottom + 1)
            clip: true

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: Math.max(root.height, root.effBottom * 2 + 2)
                radius: root.effBottom
                color: "transparent"
                border.width: 1
                border.color: root.borderColor
            }
        }

        // Left & right vertical straight borders
        Rectangle {
            x: root.bodyL
            y: root.effEar
            width: 1
            height: Math.max(0, root.height - root.effEar - root.effBottom)
            color: root.borderColor
        }
        Rectangle {
            x: root.bodyR - 1
            y: root.effEar
            width: 1
            height: Math.max(0, root.height - root.effEar - root.effBottom)
            color: root.borderColor
        }

        // Left static 16x16 concave ear sprite (cached once to GPU texture; never re-tessellates)
        Shape {
            x: root.bodyL - 16
            y: 0
            width: 16
            height: 16
            preferredRendererType: Shape.CurveRenderer
            antialiasing: true
            layer.enabled: true
            layer.smooth: true
            layer.textureSize: Qt.size(32, 32)
            transform: Scale {
                origin.x: 16
                origin.y: 0
                xScale: root.earScale
                yScale: root.earScale
            }
            ShapePath {
                strokeWidth: 0
                strokeColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0; y1: 0; x2: 0; y2: 16
                    GradientStop { position: 0.0; color: root.topColor }
                    GradientStop { position: 1.0; color: root.fillColor }
                }
                startX: 0; startY: 0
                PathCubic { x: 16; y: 16; control1X: 10.56; control1Y: 0; control2X: 16; control2Y: 5.44 }
                PathLine { x: 16; y: 0 }
                PathLine { x: 0; y: 0 }
            }
            ShapePath {
                fillColor: "transparent"
                strokeColor: root.borderColor
                strokeWidth: 1.0
                capStyle: ShapePath.RoundCap
                startX: 0; startY: 0.5
                PathCubic { x: 15.5; y: 16; control1X: 10.2; control1Y: 0.5; control2X: 15.5; control2Y: 5.7 }
            }
        }

        // Right static 16x16 concave ear sprite (x = bodyR + effEar, mirrored inward to bodyR)
        Shape {
            x: root.bodyR + root.effEar
            y: 0
            width: 16
            height: 16
            preferredRendererType: Shape.CurveRenderer
            antialiasing: true
            layer.enabled: true
            layer.smooth: true
            layer.textureSize: Qt.size(32, 32)
            transform: Scale {
                origin.x: 0
                origin.y: 0
                xScale: -root.earScale
                yScale: root.earScale
            }
            ShapePath {
                strokeWidth: 0
                strokeColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0; y1: 0; x2: 0; y2: 16
                    GradientStop { position: 0.0; color: root.topColor }
                    GradientStop { position: 1.0; color: root.fillColor }
                }
                startX: 0; startY: 0
                PathCubic { x: 16; y: 16; control1X: 10.56; control1Y: 0; control2X: 16; control2Y: 5.44 }
                PathLine { x: 16; y: 0 }
                PathLine { x: 0; y: 0 }
            }
            ShapePath {
                fillColor: "transparent"
                strokeColor: root.borderColor
                strokeWidth: 1.0
                capStyle: ShapePath.RoundCap
                startX: 0; startY: 0.5
                PathCubic { x: 15.5; y: 16; control1X: 10.2; control1Y: 0.5; control2X: 15.5; control2Y: 5.7 }
            }
        }
    }

    // Vertical-bar Dynamic Island (zero per-frame Shape tessellation)
    Item {
        visible: root.vertical
        anchors.fill: parent

        Rectangle {
            x: 0
            y: root.vBodyT
            width: Math.max(1, root.width)
            height: Math.max(1, root.bodyHeight)
            radius: root.vEffCorner
            color: root.fillColor
        }

        Rectangle {
            x: 0
            y: root.vBodyT
            width: Math.min(root.width * 0.5, root.vEffCorner)
            height: Math.max(1, root.bodyHeight)
            color: root.fillColor
        }

        Rectangle {
            visible: root.topColor !== root.fillColor && root.bodyHeight > 2 && root.width > 2
            x: 0
            y: root.vBodyT + 1
            width: Math.min(28, root.width - 1)
            height: Math.max(0, root.bodyHeight - 2)
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: root.topColor }
                GradientStop { position: 1.0; color: Qt.rgba(root.fillColor.r, root.fillColor.g, root.fillColor.b, 0.0) }
            }
        }

        // Right rounded outer border strip
        Item {
            anchors.right: parent.right
            y: root.vBodyT
            width: Math.min(root.width, root.vEffCorner + 1)
            height: Math.max(1, root.bodyHeight)
            clip: true

            Rectangle {
                anchors.right: parent.right
                width: Math.max(root.width, root.vEffCorner * 2 + 2)
                height: parent.height
                radius: root.vEffCorner
                color: "transparent"
                border.width: 1
                border.color: root.borderColor
            }
        }

        // Top & bottom horizontal straight borders
        Rectangle {
            x: root.vEffEar
            y: root.vBodyT
            width: Math.max(0, root.width - root.vEffEar - root.vEffCorner)
            height: 1
            color: root.borderColor
        }
        Rectangle {
            x: root.vEffEar
            y: root.vBodyB - 1
            width: Math.max(0, root.width - root.vEffEar - root.vEffCorner)
            height: 1
            color: root.borderColor
        }

        // Static 16x16 top & bottom concave ears along x=0
        Shape {
            x: 0
            y: root.vBodyT - 16
            width: 16
            height: 16
            preferredRendererType: Shape.CurveRenderer
            antialiasing: true
            layer.enabled: true
            layer.smooth: true
            layer.textureSize: Qt.size(32, 32)
            transform: Scale {
                origin.x: 0
                origin.y: 16
                xScale: root.vEarScale
                yScale: root.vEarScale
            }
            ShapePath {
                strokeWidth: 0
                strokeColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0; y1: 0; x2: 16; y2: 0
                    GradientStop { position: 0.0; color: root.topColor }
                    GradientStop { position: 1.0; color: root.fillColor }
                }
                startX: 0; startY: 0
                PathCubic { x: 16; y: 16; control1X: 0; control1Y: 10.56; control2X: 5.44; control2Y: 16 }
                PathLine { x: 0; y: 16 }
                PathLine { x: 0; y: 0 }
            }
            ShapePath {
                fillColor: "transparent"
                strokeColor: root.borderColor
                strokeWidth: 1.0
                capStyle: ShapePath.RoundCap
                startX: 0.5; startY: 0
                PathCubic { x: 16; y: 15.5; control1X: 0.5; control1Y: 10.2; control2X: 5.7; control2Y: 15.5 }
            }
        }

        Shape {
            x: 0
            y: root.vBodyB + root.vEffEar
            width: 16
            height: 16
            preferredRendererType: Shape.CurveRenderer
            antialiasing: true
            layer.enabled: true
            layer.smooth: true
            layer.textureSize: Qt.size(32, 32)
            transform: Scale {
                origin.x: 0
                origin.y: 0
                xScale: root.vEarScale
                yScale: -root.vEarScale
            }
            ShapePath {
                strokeWidth: 0
                strokeColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0; y1: 0; x2: 16; y2: 0
                    GradientStop { position: 0.0; color: root.topColor }
                    GradientStop { position: 1.0; color: root.fillColor }
                }
                startX: 0; startY: 0
                PathCubic { x: 16; y: 16; control1X: 0; control1Y: 10.56; control2X: 5.44; control2Y: 16 }
                PathLine { x: 0; y: 16 }
                PathLine { x: 0; y: 0 }
            }
            ShapePath {
                fillColor: "transparent"
                strokeColor: root.borderColor
                strokeWidth: 1.0
                capStyle: ShapePath.RoundCap
                startX: 0.5; startY: 0
                PathCubic { x: 16; y: 15.5; control1X: 0.5; control1Y: 10.2; control2X: 5.7; control2Y: 15.5 }
            }
        }
    }
}
