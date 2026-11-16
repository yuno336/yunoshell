import QtQuick
import ".."

Column {
    id: root
    width: parent.width
    property string selectedSsid: ""
    spacing: 12

    function signalIcon(sig) {
        if (sig >= 75) return "󰤨";
        if (sig >= 50) return "󰤥";
        if (sig >= 25) return "󰤢";
        return "󰤟";
    }

    // Pixel Expressive Quick-Settings Tile Header
    Rectangle {
        width: parent.width
        height: 54
        radius: 12
        color: Theme.surface

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 9
            anchors.verticalCenter: parent.verticalCenter
            spacing: 12

            Rectangle {
                width: 36
                height: 36
                radius: 9
                anchors.verticalCenter: parent.verticalCenter
                color: Services.wifiEnabled ? Theme.accent : Theme.surfaceHover

                Behavior on color { ColorAnimation { duration: 150 } }

                Text {
                    anchors.centerIn: parent
                    text: Services.wifiEnabled ? "󰤨" : "󰤭"
                    font.family: Theme.fontFamily
                    font.pixelSize: 16
                    color: Services.wifiEnabled ? Theme.accentText : Theme.text
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(80, parent.parent.width - 170)
                spacing: 1

                Text {
                    text: "Internet"
                    font.family: Theme.fontFamily
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    color: Theme.textBright
                }

                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: !Services.wifiEnabled ? "Wi-Fi is off"
                        : Services.wifiCurrentSsid.length > 0 ? Services.wifiCurrentSsid
                        : "Select a network"
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                }
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            // Circular Rescan Button
            Rectangle {
                visible: Services.wifiEnabled
                width: 32
                height: 32
                radius: 10
                anchors.verticalCenter: parent.verticalCenter
                color: scanMouse.containsMouse ? Theme.surfaceActive : Theme.surfaceHover

                Text {
                    anchors.centerIn: parent
                    text: Services.wifiScanning ? "󰑐" : "󰚰"
                    font.family: Theme.fontFamily
                    font.pixelSize: 14
                    color: Theme.text
                }

                MouseArea {
                    id: scanMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Services.rescanWifi()
                }
            }

            // Material 3 Expressive Switch (morphing thumb + checkmark)
            Rectangle {
                width: 50
                height: 28
                radius: 14
                anchors.verticalCenter: parent.verticalCenter
                color: Services.wifiEnabled ? Theme.accent : Theme.surfaceHover
                border.width: Services.wifiEnabled ? 0 : 2
                border.color: Theme.muted

                Behavior on color { ColorAnimation { duration: 150 } }

                Rectangle {
                    readonly property int sz: Services.wifiEnabled ? 22 : 14
                    width: sz
                    height: sz
                    radius: sz / 2
                    anchors.verticalCenter: parent.verticalCenter
                    x: Services.wifiEnabled ? 25 : 7
                    color: Services.wifiEnabled ? Theme.accentText : Theme.muted

                    Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                    Behavior on width { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                    Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

                    Text {
                        visible: Services.wifiEnabled
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
                    onClicked: {
                        root.selectedSsid = "";
                        Services.toggleWifi();
                    }
                }
            }
        }
    }

    // Status message bar
    Text {
        visible: Services.wifiStatusMsg.length > 0
        leftPadding: 6
        width: parent.width
        text: Services.wifiStatusMsg
        font.family: Theme.fontFamily
        font.pixelSize: 11
        color: Theme.text
        elide: Text.ElideRight
    }

    // Pixel Pill Password Dialog Box
    Rectangle {
        visible: root.selectedSsid.length > 0 && Services.wifiEnabled
        width: parent.width
        height: 76
        radius: 12
        color: Theme.surface

        Column {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            Item {
                width: parent.width
                height: 16

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 4
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 28
                    text: "Password for " + root.selectedSsid
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    color: Theme.textBright
                    elide: Text.ElideRight
                }

                Text {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    text: "󰅖"
                    font.family: Theme.fontFamily
                    font.pixelSize: 13
                    color: Theme.muted

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            passInput.text = "";
                            root.selectedSsid = "";
                        }
                    }
                }
            }

            Row {
                width: parent.width
                spacing: 8

                Rectangle {
                    width: parent.width - 82
                    height: 30
                    radius: 8
                    color: Theme.barBg

                    TextInput {
                        id: passInput
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: TextInput.AlignVCenter
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        color: Theme.textBright
                        echoMode: TextInput.Password
                        clip: true
                        onAccepted: {
                            if (passInput.text.length > 0) {
                                Services.connectWifi(root.selectedSsid, passInput.text, false);
                                passInput.text = "";
                                root.selectedSsid = "";
                            }
                        }
                    }
                }

                Rectangle {
                    width: 74
                    height: 30
                    radius: 8
                    color: Theme.accent

                    Text {
                        anchors.centerIn: parent
                        text: "Connect"
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        color: Theme.accentText
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Services.connectWifi(root.selectedSsid, passInput.text, false);
                            passInput.text = "";
                            root.selectedSsid = "";
                        }
                    }
                }
            }
        }
    }

    // Fixed-height container so the popup never stretches/shrinks
    Item {
        width: parent.width
        height: 244

        ListView {
            id: wifiList
            visible: Services.wifiEnabled
            anchors.fill: parent
            clip: true
            spacing: 6
            cacheBuffer: 120
            interactive: false
            boundsBehavior: Flickable.StopAtBounds
            model: Services.wifiNetworks
            property real savedY: 0

            Behavior on contentY {
                NumberAnimation {
                    duration: 90
                    easing.type: Easing.OutCubic
                }
            }

            onContentHeightChanged: {
                if (contentHeight > 0) {
                    var maxScroll = Math.max(0, contentHeight - height);
                    if (savedY > maxScroll) {
                        savedY = maxScroll;
                        contentY = maxScroll;
                    }
                }
            }

            WheelHandler {
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onWheel: event => {
                    var maxScroll = Math.max(0, wifiList.contentHeight - wifiList.height);
                    var step = event.pixelDelta.y !== 0
                        ? -event.pixelDelta.y * 1.35
                        : -(event.angleDelta.y / 120.0) * 42.0;
                    wifiList.savedY = Math.max(0, Math.min(maxScroll, wifiList.savedY + step));
                    wifiList.contentY = wifiList.savedY;
                    event.accepted = true;
                }
            }

            delegate: Rectangle {
                required property var modelData
                width: wifiList.width
                height: 46
                radius: 11
                color: modelData.active ? Theme.accent
                     : netMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                Behavior on color { ColorAnimation { duration: 150 } }

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: 8
                    anchors.right: actionRow.left
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 10

                    Rectangle {
                        width: 30
                        height: 30
                        radius: 8
                        anchors.verticalCenter: parent.verticalCenter
                        color: modelData.active ? Theme.accentText : Theme.surfaceHover

                        Text {
                            anchors.centerIn: parent
                            text: root.signalIcon(modelData.signal)
                            font.family: Theme.fontFamily
                            font.pixelSize: 14
                            color: Theme.text
                        }
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 44
                        text: modelData.ssid
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: modelData.active ? Font.Bold : Font.Medium
                        color: modelData.active ? Theme.accentText : Theme.text
                        elide: Text.ElideRight
                    }
                }

                Row {
                    id: actionRow
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6

                    Text {
                        visible: !modelData.active && modelData.saved
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Saved"
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: Font.DemiBold
                        color: Theme.text
                    }

                    Text {
                        visible: !modelData.active && !modelData.saved && modelData.security && modelData.security !== "--"
                        anchors.verticalCenter: parent.verticalCenter
                        text: "󰌾"
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        color: Theme.muted
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.active ? "Connected" : (modelData.signal + "%")
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        font.weight: modelData.active ? Font.Bold : Font.Normal
                        color: modelData.active ? Theme.accentText : Theme.muted
                    }
                }

                MouseArea {
                    id: netMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    onClicked: mouse => {
                        if (mouse.button === Qt.RightButton) {
                            root.selectedSsid = modelData.ssid;
                            passInput.forceActiveFocus();
                            return;
                        }
                        if (modelData.active) {
                            Services.disconnectWifi(modelData.ssid);
                        } else if (modelData.saved || !modelData.security || modelData.security === "--") {
                            root.selectedSsid = "";
                            Services.connectWifi(modelData.ssid, "", modelData.saved);
                        } else {
                            if (root.selectedSsid === modelData.ssid) {
                                root.selectedSsid = "";
                            } else {
                                root.selectedSsid = modelData.ssid;
                                passInput.forceActiveFocus();
                            }
                        }
                    }
                }
            }
        }

        Column {
            visible: !Services.wifiEnabled
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
                    text: "󰤭"
                    font.family: Theme.fontFamily
                    font.pixelSize: 22
                    color: Theme.muted
                }
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Wi-Fi is turned off"
                font.family: Theme.fontFamily
                font.pixelSize: 12
                color: Theme.muted
            }
        }
    }
}
