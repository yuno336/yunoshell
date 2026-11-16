import QtQuick
import ".."

Rectangle {
    id: root
    property string icon: ""
    property string title: ""
    property string subtitle: ""
    property bool active: false
    signal clicked()

    height: 54
    radius: 13
    color: tcMa.containsMouse
        ? (active ? "#485266" : Theme.surfaceHover)
        : (active ? "#3f485b" : Theme.surface)
    border.width: 1
    border.color: active ? Theme.accent : "#3e4657"
    Behavior on color { ColorAnimation { duration: 150 } }

    Row {
        anchors.fill: parent
        anchors.leftMargin: 13
        anchors.rightMargin: 13
        spacing: 11

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 34
            height: 34
            radius: 10
            color: root.active ? Theme.accent : "#303644"
            Behavior on color { ColorAnimation { duration: 150 } }

            Text {
                anchors.centerIn: parent
                text: root.icon
                color: root.active ? Theme.accentText : Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 15
            }
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 58
            spacing: 2

            Text {
                width: parent.width
                text: root.title
                color: Theme.textBright
                font.family: Theme.fontFamily
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            Text {
                visible: root.subtitle.length > 0
                width: parent.width
                text: root.subtitle
                color: Theme.muted
                font.family: Theme.fontFamily
                font.pixelSize: 11
                elide: Text.ElideRight
            }
        }
    }

    MouseArea {
        id: tcMa
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
