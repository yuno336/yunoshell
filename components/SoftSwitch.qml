import QtQuick
import ".."

Rectangle {
    id: root
    property bool checked: false
    signal toggled()

    width: 44
    height: 24
    radius: 12
    color: checked ? Theme.accent : "#323846"
    border.width: 1
    border.color: checked ? Theme.accent : "#4c566a"
    Behavior on color { ColorAnimation { duration: 160 } }

    Rectangle {
        width: 18
        height: 18
        radius: 9
        anchors.verticalCenter: parent.verticalCenter
        x: root.checked ? root.width - width - 3 : 3
        color: root.checked ? Theme.accentText : Theme.textBright
        Behavior on x { NumberAnimation { duration: 190; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0] } }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
