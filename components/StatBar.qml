import QtQuick
import ".."

Rectangle {
    id: root
    property real ratio: 0.0
    property color fillColor: Theme.accent
    height: 7
    radius: 3.5
    color: "#2b303c"

    Rectangle {
        height: parent.height
        radius: parent.radius
        width: Math.max(parent.height, parent.width * Math.max(0, Math.min(1, root.ratio)))
        color: root.fillColor
        Behavior on width { NumberAnimation { duration: 320; easing.type: Easing.OutCubic } }
    }
}
