import QtQuick
import Quickshell
import ".."

Item {
    id: root
    property bool active: false
    property bool vertical: false
    readonly property alias hovered: clockMouse.containsMouse
    signal clicked()
    signal entered()
    signal exited()

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    width: vertical ? Theme.barHeight : (row.width + 16)
    height: vertical ? (col.height + 10) : Theme.barHeight

    Row {
        id: row
        visible: !root.vertical
        anchors.centerIn: parent
        spacing: 8

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatDateTime(clock.date, Services.lock24Hour ? "HH:mm" : "hh:mm AP")
            font.family: Theme.fontFamily
            font.pixelSize: 12
            font.weight: Font.Bold
            color: root.active ? Theme.accent : Theme.textBright
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatDateTime(clock.date, "ddd, MMM dd")
            font.family: Theme.fontFamily
            font.pixelSize: 11
            font.weight: Font.Medium
            color: Theme.muted
        }
    }

    Column {
        id: col
        visible: root.vertical
        anchors.centerIn: parent
        spacing: 0

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(clock.date, Services.lock24Hour ? "HH" : "hh")
            font.family: Theme.fontFamily
            font.pixelSize: 12
            font.weight: Font.Bold
            color: root.active ? Theme.accent : Theme.textBright
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(clock.date, "mm")
            font.family: Theme.fontFamily
            font.pixelSize: 11
            font.weight: Font.DemiBold
            color: Theme.muted
        }
    }

    MouseArea {
        id: clockMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onEntered: root.entered()
        onExited: root.exited()
        onClicked: root.clicked()
    }
}
