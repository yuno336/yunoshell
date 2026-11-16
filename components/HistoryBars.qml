import QtQuick
import ".."

Row {
    id: root
    property var samples: []
    property color barColor: Theme.accent
    spacing: 2
    height: 28

    Repeater {
        model: root.samples || []
        delegate: Item {
            required property var modelData
            readonly property int totalSamples: Math.max(1, (root.samples || []).length)
            width: Math.max(3, Math.floor((root.width - Math.max(0, (totalSamples - 1) * root.spacing)) / totalSamples))
            height: root.height

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 2
                readonly property real norm: Math.max(0.05, Math.min(1.0, (Number(parent.modelData) || 0) / 100.0))
                height: Math.max(3, Math.round(parent.height * norm))
                color: Qt.rgba(root.barColor.r, root.barColor.g, root.barColor.b, 0.35 + 0.65 * norm)
                Behavior on height { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
            }
        }
    }
}
