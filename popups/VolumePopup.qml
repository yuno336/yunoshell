import QtQuick
import ".."
import "../components"

Column {
    id: root
    width: parent.width
    spacing: 14

    // Pixel Header: Title + Circular Mute Pill Button
    Item {
        width: parent.width
        height: 36

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 4
            anchors.verticalCenter: parent.verticalCenter
            text: "Sound & Output"
            font.family: Theme.fontFamily
            font.pixelSize: 14
            font.weight: Font.Bold
            color: Theme.textBright
        }

        Rectangle {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 68
            height: 32
            radius: 10
            color: Services.volumeMuted ? Theme.accent
                 : muteMouse.containsMouse ? Theme.surfaceHover : Theme.surface

            Behavior on color { ColorAnimation { duration: 140 } }

            Row {
                anchors.centerIn: parent
                spacing: 6

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Services.volumeMuted ? "󰝟" : "󰕾"
                    font.family: Theme.fontFamily
                    font.pixelSize: 13
                    color: Services.volumeMuted ? Theme.accentText : Theme.text
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Services.volumeMuted ? "Muted" : "Mute"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    color: Services.volumeMuted ? Theme.accentText : Theme.text
                }
            }

            MouseArea {
                id: muteMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Services.toggleMute()
            }
        }
    }

    // Pixel Expressive Thick Pill Slider with embedded icon & percentage
    SmoothSlider {
        width: parent.width
        minValue: 0
        maxValue: 100
        value: Services.volumeMuted ? 0 : Services.volumePercent
        icon: Services.volumeMuted ? "󰝟" : (Services.volumePercent >= 50 ? "󰕾" : "󰖀")
        labelText: Services.volumePercent + "%"
        fillColor: Theme.accent
        onMoved: val => Services.setVolume(val)
    }

    Text {
        leftPadding: 4
        text: "Media output"
        font.family: Theme.fontFamily
        font.pixelSize: 11
        font.weight: Font.DemiBold
        color: Theme.muted
    }

    // Pixel Morphing Tile List for Output Devices
    Column {
        width: parent.width
        spacing: 6

        Repeater {
            model: Services.audioOutputs

            Rectangle {
                required property var modelData
                width: parent.width
                height: 48
                radius: 11
                color: modelData.active ? Theme.accent
                     : outMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                Behavior on color { ColorAnimation { duration: 150 } }

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 12

                    Rectangle {
                        width: 32
                        height: 32
                        radius: 8
                        anchors.verticalCenter: parent.verticalCenter
                        color: modelData.active ? Theme.accentText : Theme.surfaceHover

                        Text {
                            anchors.centerIn: parent
                            text: modelData.label.toLowerCase().indexOf("headphone") !== -1 ? "󰋋" : "󰓃"
                            font.family: Theme.fontFamily
                            font.pixelSize: 15
                            color: Theme.text
                        }
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.label
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: modelData.active ? Font.Bold : Font.Medium
                        color: modelData.active ? Theme.accentText : Theme.text
                    }
                }

                Text {
                    visible: modelData.active
                    anchors.right: parent.right
                    anchors.rightMargin: 16
                    anchors.verticalCenter: parent.verticalCenter
                    text: "󰄬"
                    font.family: Theme.fontFamily
                    font.pixelSize: 15
                    color: Theme.accentText
                }

                MouseArea {
                    id: outMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Services.selectAudioOutput(modelData.sink, modelData.port)
                }
            }
        }
    }
}
