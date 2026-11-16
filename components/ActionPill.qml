import QtQuick
import ".."

Rectangle {
    id: root
    property string icon: ""
    property string label: ""
    property bool active: false
    property bool danger: false
    property bool busy: false
    signal clicked()

    implicitWidth: pillRow.implicitWidth + 24
    height: 32
    radius: 10
    opacity: busy ? 0.65 : 1.0
    color: {
        if (root.danger) return pillMa.containsMouse ? "#49313a" : "#3a2930";
        if (root.active) return Theme.accent;
        return pillMa.containsMouse ? Theme.surfaceHover : Theme.surface;
    }
    border.width: 1
    border.color: root.danger ? "#bf616a" : (root.active ? Theme.accent : "#434c5e")
    Behavior on color { ColorAnimation { duration: 140 } }

    Row {
        id: pillRow
        anchors.centerIn: parent
        spacing: 6

        Text {
            visible: root.icon.length > 0
            anchors.verticalCenter: parent.verticalCenter
            text: root.icon
            color: root.active ? Theme.accentText : (root.danger ? "#f28fad" : Theme.accent)
            font.family: Theme.fontFamily
            font.pixelSize: 13
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.active ? Theme.accentText : (root.danger ? "#f28fad" : Theme.textBright)
            font.family: Theme.fontFamily
            font.pixelSize: 12
            font.weight: root.active ? Font.DemiBold : Font.Medium
        }
    }

    MouseArea {
        id: pillMa
        anchors.fill: parent
        hoverEnabled: true
        enabled: !root.busy
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
