import QtQuick
import ".."

Column {
    id: root
    width: parent.width
    spacing: 12

    // Battery Hero Card
    Rectangle {
        width: parent.width
        height: 68
        radius: 12
        color: Theme.surface
        clip: true

        Rectangle {
            width: Math.max(4, parent.width * (Services.batteryPercent / 100.0))
            height: parent.height
            radius: 12
            color: Theme.accent
            opacity: 0.16

            Behavior on width { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
        }

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 12

            Rectangle {
                width: 40
                height: 40
                radius: 10
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.accent

                Text {
                    anchors.centerIn: parent
                    text: Services.batteryCharging ? "󰂄" : "󰁹"
                    font.family: Theme.fontFamily
                    font.pixelSize: 18
                    color: Theme.accentText
                }
            }

            Column {
                width: parent.width - 52
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    width: parent.width
                    text: Services.batteryPercent + "% • " + Services.batteryStatus
                    font.family: Theme.fontFamily
                    font.pixelSize: 14
                    font.weight: Font.Bold
                    color: Theme.textBright
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: Services.batteryDetail
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.muted
                    elide: Text.ElideRight
                }
            }
        }
    }

    // Live CPU / auto-cpufreq telemetry card
    Rectangle {
        width: parent.width
        height: 54
        radius: 12
        color: Theme.surface
        clip: true

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 12

            Rectangle {
                width: 34
                height: 34
                radius: 9
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.accent

                Text {
                    anchors.centerIn: parent
                    text: "󰍛"
                    font.family: Theme.fontFamily
                    font.pixelSize: 16
                    color: Theme.accentText
                }
            }

            Column {
                width: parent.width - 46
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    width: parent.width
                    text: "auto-cpufreq • Active"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    font.weight: Font.Bold
                    color: Theme.textBright
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: Services.cpuFreqGhz + " • " + Services.cpuTemp + " • " + Services.powerProfile
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                    elide: Text.ElideRight
                }
            }
        }
    }
}
