import QtQuick
import ".."

Column {
    id: root
    width: parent.width
    spacing: 10

    // Top Tile Header: Bluetooth + ON/OFF Switch
    Rectangle {
        width: parent.width
        height: 52
        radius: 12
        color: Theme.surface

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 12

            Rectangle {
                width: 34
                height: 34
                radius: 9
                anchors.verticalCenter: parent.verticalCenter
                color: Services.btPowered ? Theme.accent : Theme.surfaceHover

                Behavior on color { ColorAnimation { duration: 150 } }

                Text {
                    anchors.centerIn: parent
                    text: Services.btPowered ? "󰂯" : "󰂲"
                    font.family: Theme.fontFamily
                    font.pixelSize: 16
                    color: Services.btPowered ? Theme.accentText : Theme.text
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 1

                Text {
                    text: "Bluetooth"
                    font.family: Theme.fontFamily
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    color: Theme.textBright
                }

                Text {
                    text: !Services.btPowered ? "Off"
                        : Services.btConnectedCount > 0 ? (Services.btConnectedCount + " connected")
                        : "On"
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                }
            }
        }

        // Switch
        Rectangle {
            width: 50
            height: 28
            radius: 14
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            color: Services.btPowered ? Theme.accent : Theme.surfaceHover
            border.width: Services.btPowered ? 0 : 2
            border.color: Theme.muted

            Behavior on color { ColorAnimation { duration: 150 } }

            Rectangle {
                readonly property int sz: Services.btPowered ? 22 : 14
                width: sz
                height: sz
                radius: sz / 2
                anchors.verticalCenter: parent.verticalCenter
                x: Services.btPowered ? 25 : 7
                color: Services.btPowered ? Theme.accentText : Theme.muted

                Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

                Text {
                    visible: Services.btPowered
                    anchors.centerIn: parent
                    text: "󰄬"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    color: Theme.text
                }
            }

            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: Services.toggleBluetooth()
            }
        }
    }

    // "+ Pair new device" button (Google Pixel style)
    Rectangle {
        visible: Services.btPowered
        width: parent.width
        height: 42
        radius: 11
        color: Services.btScanning ? Theme.accent
             : addDevMouse.containsMouse ? Theme.surfaceHover : Theme.surface

        Behavior on color { ColorAnimation { duration: 140 } }

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Services.btScanning ? "󰑐" : "󰐕"
                font.family: Theme.fontFamily
                font.pixelSize: 15
                color: Services.btScanning ? Theme.accentText : Theme.text
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Services.btScanning ? "Scanning nearby devices... (Click to stop)" : "Pair new device"
                font.family: Theme.fontFamily
                font.pixelSize: 12
                font.weight: Font.Bold
                color: Services.btScanning ? Theme.accentText : Theme.textBright
            }
        }

        MouseArea {
            id: addDevMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Services.scanBluetooth()
        }
    }

    // Status message
    Text {
        visible: Services.btStatusMsg.length > 0
        width: parent.width
        leftPadding: 4
        text: Services.btStatusMsg
        font.family: Theme.fontFamily
        font.pixelSize: 11
        color: Theme.text
        elide: Text.ElideRight
    }

    // Fixed-height scrollable area for Saved Devices + Available Devices to Pair
    Flickable {
        id: btFlick
        visible: Services.btPowered
        width: parent.width
        height: 230
        contentWidth: width
        contentHeight: listsCol.height
        clip: true
        interactive: false
        boundsBehavior: Flickable.StopAtBounds
        property real savedY: 0

        Behavior on contentY {
            NumberAnimation {
                duration: 90
                easing.type: Easing.OutCubic
            }
        }

        WheelHandler {
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: event => {
                var maxScroll = Math.max(0, btFlick.contentHeight - btFlick.height);
                var step = event.pixelDelta.y !== 0
                    ? -event.pixelDelta.y * 1.35
                    : -(event.angleDelta.y / 120.0) * 42.0;
                btFlick.savedY = Math.max(0, Math.min(maxScroll, btFlick.savedY + step));
                btFlick.contentY = btFlick.savedY;
                event.accepted = true;
            }
        }

        Column {
            id: listsCol
            width: parent.width
            spacing: 8
            onHeightChanged: {
                if (height > 0) {
                    var maxScroll = Math.max(0, height - btFlick.height);
                    if (btFlick.savedY > maxScroll) {
                        btFlick.savedY = maxScroll;
                        btFlick.contentY = maxScroll;
                    }
                }
            }

            // SAVED / PAIRED DEVICES SECTION
            Text {
                visible: Services.btPairedDevices.length > 0
                leftPadding: 4
                text: "Saved devices"
                font.family: Theme.fontFamily
                font.pixelSize: 11
                font.weight: Font.DemiBold
                color: Theme.muted
            }

            Repeater {
                model: Services.btPairedDevices

                Rectangle {
                    required property var modelData
                    width: listsCol.width
                    height: 44
                    radius: 11
                    color: modelData.connected ? Theme.accent
                         : pairedMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.right: pairedActions.left
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10

                        Rectangle {
                            width: 28
                            height: 28
                            radius: 8
                            anchors.verticalCenter: parent.verticalCenter
                            color: modelData.connected ? Theme.accentText : Theme.surfaceHover

                            Text {
                                anchors.centerIn: parent
                                text: modelData.connected ? "󰂱" : "󰂯"
                                font.family: Theme.fontFamily
                                font.pixelSize: 14
                                color: Theme.text
                            }
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 38
                            text: modelData.name
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            font.weight: modelData.connected ? Font.Bold : Font.Medium
                            color: modelData.connected ? Theme.accentText : Theme.text
                            elide: Text.ElideRight
                        }
                    }

                    Row {
                        id: pairedActions
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 8

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.connected ? "Connected" : "Connect"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            font.weight: modelData.connected ? Font.Bold : Font.Normal
                            color: modelData.connected ? Theme.accentText : Theme.muted
                        }

                        // Remove / Forget button
                        Rectangle {
                            width: 24
                            height: 24
                            radius: 6
                            anchors.verticalCenter: parent.verticalCenter
                            color: forgetMouse.containsMouse ? Theme.surfaceActive : "transparent"

                            Text {
                                anchors.centerIn: parent
                                text: "󰅖"
                                font.family: Theme.fontFamily
                                font.pixelSize: 12
                                color: modelData.connected ? Theme.accentText : Theme.muted
                            }

                            MouseArea {
                                id: forgetMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: Services.removeBt(modelData.mac)
                            }
                        }
                    }

                    MouseArea {
                        id: pairedMouse
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        anchors.right: pairedActions.left
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (modelData.connected) {
                                Services.disconnectBt(modelData.mac);
                            } else {
                                Services.connectBt(modelData.mac, modelData.name);
                            }
                        }
                    }
                }
            }

            // AVAILABLE DEVICES TO PAIR SECTION
            Text {
                visible: Services.btAvailableDevices.length > 0 || Services.btScanning
                leftPadding: 4
                topPadding: Services.btPairedDevices.length > 0 ? 4 : 0
                text: "Available devices"
                font.family: Theme.fontFamily
                font.pixelSize: 11
                font.weight: Font.DemiBold
                color: Theme.muted
            }

            Repeater {
                model: Services.btAvailableDevices

                Rectangle {
                    required property var modelData
                    width: listsCol.width
                    height: 44
                    radius: 11
                    color: availMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.right: pairLabel.left
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10

                        Rectangle {
                            width: 28
                            height: 28
                            radius: 8
                            anchors.verticalCenter: parent.verticalCenter
                            color: Theme.surfaceHover

                            Text {
                                anchors.centerIn: parent
                                text: "󰂯"
                                font.family: Theme.fontFamily
                                font.pixelSize: 14
                                color: Theme.text
                            }
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 38
                            text: modelData.name
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            font.weight: Font.Medium
                            color: Theme.text
                            elide: Text.ElideRight
                        }
                    }

                    Text {
                        id: pairLabel
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Pair"
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        color: Theme.textBright
                    }

                    MouseArea {
                        id: availMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Services.pairAndConnectBt(modelData.mac, modelData.name)
                    }
                }
            }

            // Empty state when not scanning and no devices
            Column {
                visible: Services.btPairedDevices.length === 0 && Services.btAvailableDevices.length === 0 && !Services.btScanning
                width: parent.width
                topPadding: 36
                spacing: 8

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "No paired devices"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    color: Theme.muted
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Click 'Pair new device' above to scan"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.muted
                }
            }
        }
    }

    // Disabled state
    Item {
        visible: !Services.btPowered
        width: parent.width
        height: 160

        Column {
            anchors.centerIn: parent
            spacing: 10

            Rectangle {
                width: 48
                height: 48
                radius: 12
                anchors.horizontalCenter: parent.horizontalCenter
                color: Theme.surface

                Text {
                    anchors.centerIn: parent
                    text: "󰂲"
                    font.family: Theme.fontFamily
                    font.pixelSize: 22
                    color: Theme.muted
                }
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Bluetooth is turned off"
                font.family: Theme.fontFamily
                font.pixelSize: 12
                color: Theme.muted
            }
        }
    }
}
