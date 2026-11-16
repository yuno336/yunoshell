import QtQuick
import ".."

Column {
    id: root
    width: parent.width
    signal closeRequested()
    spacing: 12

    Text {
        leftPadding: 4
        text: "Power options"
        font.family: Theme.fontFamily
        font.pixelSize: 14
        font.weight: Font.Bold
        color: Theme.textBright
    }

    // 2x2 Grid of Pixel Quick-Settings Tiles + Full-Width Shut Down Pill
    Grid {
        width: parent.width
        columns: 2
        spacing: 8

        Repeater {
            model: [
                { id: "lock",    icon: "󰌾", label: "Lock" },
                { id: "suspend", icon: "󰤄", label: "Suspend" },
                { id: "logout",  icon: "󰍃", label: "Log Out" },
                { id: "reboot",  icon: "󰜉", label: "Restart" }
            ]

            Rectangle {
                required property var modelData
                width: Math.floor((parent.width - 8) / 2)
                height: 54
                radius: 12
                color: pMouse.containsMouse ? Theme.accent : Theme.surface

                Behavior on color { ColorAnimation { duration: 140 } }

                Row {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.icon
                        font.family: Theme.fontFamily
                        font.pixelSize: 16
                        color: pMouse.containsMouse ? Theme.accentText : Theme.text
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.label
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: Font.Bold
                        color: pMouse.containsMouse ? Theme.accentText : Theme.text
                    }
                }

                MouseArea {
                    id: pMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.closeRequested();
                        Services.powerAction(modelData.id);
                    }
                }
            }
        }
    }

    // Primary Pixel Pill for Shut Down
    Rectangle {
        width: parent.width
        height: 48
        radius: 12
        color: shutMouse.containsMouse ? Theme.accent : Theme.surfaceHover

        Behavior on color { ColorAnimation { duration: 140 } }

        Row {
            anchors.centerIn: parent
            spacing: 10

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "󰐥"
                font.family: Theme.fontFamily
                font.pixelSize: 16
                color: shutMouse.containsMouse ? Theme.accentText : Theme.text
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Power Off"
                font.family: Theme.fontFamily
                font.pixelSize: 13
                font.weight: Font.Bold
                color: shutMouse.containsMouse ? Theme.accentText : Theme.text
            }
        }

        MouseArea {
            id: shutMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.closeRequested();
                Services.powerAction("shutdown");
            }
        }
    }
}
