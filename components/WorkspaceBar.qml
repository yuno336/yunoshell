import QtQuick
import ".."

Rectangle {
    id: root
    property bool vertical: false
    height: vertical ? (grid.height + 10) : Theme.barHeight
    width: vertical ? Theme.barHeight : (grid.width + 10)
    color: "transparent"

    Grid {
        id: grid
        anchors.centerIn: parent
        columns: root.vertical ? 1 : Math.max(1, Services.tags.length)
        spacing: 6

        Repeater {
            model: Services.tags

            Item {
                required property var modelData
                readonly property bool isActive: modelData.is_active === true
                readonly property bool isUrgent: modelData.is_urgent === true
                readonly property bool hasClients: (modelData.client_count || 0) > 0

                width: root.vertical ? 24 : (isActive ? 24 : 12)
                height: root.vertical ? (isActive ? 24 : 12) : 24

                Behavior on width {
                    NumberAnimation { duration: 200; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0] }
                }
                Behavior on height {
                    NumberAnimation { duration: 200; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0] }
                }

                Rectangle {
                    anchors.centerIn: parent
                    width: root.vertical
                           ? (parent.isActive ? 10 : (parent.hasClients ? 7 : 5))
                           : (parent.isActive ? 24 : (parent.hasClients ? 7 : 5))
                    height: root.vertical
                            ? (parent.isActive ? 22 : (parent.hasClients ? 7 : 5))
                            : (parent.isActive ? 12 : (parent.hasClients ? 7 : 5))
                    radius: Math.min(width, height) / 2
                    color: Theme.accent
                    opacity: parent.isActive || parent.isUrgent ? 1.0
                           : wsMouse.containsMouse ? 0.9
                           : parent.hasClients ? 0.7
                           : 0.3

                    Behavior on width { NumberAnimation { duration: 200; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0] } }
                    Behavior on height { NumberAnimation { duration: 200; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0] } }
                    Behavior on opacity { NumberAnimation { duration: 160 } }
                }

                MouseArea {
                    id: wsMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Services.switchTag(modelData.index)
                }
            }
        }
    }
}
