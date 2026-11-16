import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Services.Polkit
import ".."

Item {
    id: root

    PolkitAgent {
        id: agent
    }

    readonly property bool active: agent.isActive && agent.flow !== null
    readonly property var flow: agent.flow

    onActiveChanged: {
        passInput.text = "";
        if (active) passInput.forceActiveFocus();
    }

    Connections {
        target: root.flow
        function onFailedChanged() {
            if (root.flow && root.flow.failed) {
                passInput.text = "";
                passInput.forceActiveFocus();
            }
        }
        function onIsResponseRequiredChanged() {
            if (root.flow && root.flow.isResponseRequired) {
                passInput.forceActiveFocus();
            }
        }
    }

    PanelWindow {
        id: polkitOverlay
        visible: root.active
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: root.active ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
        color: "#88000000"

        Rectangle {
            id: card
            width: 416
            height: cardCol.implicitHeight + 40
            anchors.centerIn: parent
            radius: 18
            color: Theme.popupBg
            border.width: 1
            border.color: (root.flow && root.flow.supplementaryIsError) ? "#bf616a" : "#434c5e"
            scale: root.active ? 1.0 : 0.94
            opacity: root.active ? 1.0 : 0.0

            Behavior on border.color { ColorAnimation { duration: 150 } }
            Behavior on scale { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
            Behavior on opacity { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

            Keys.onEscapePressed: {
                if (root.flow) root.flow.cancelAuthenticationRequest();
            }

            Column {
                id: cardCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 20
                spacing: 14

                // Header
                Row {
                    spacing: 12
                    width: parent.width

                    Rectangle {
                        width: 38
                        height: 38
                        radius: 11
                        color: "#323846"
                        border.width: 1
                        border.color: "#434c5e"
                        anchors.verticalCenter: parent.verticalCenter

                        Text {
                            anchors.centerIn: parent
                            text: "󰦝"
                            font.family: Theme.fontFamily
                            font.pixelSize: 18
                            color: Theme.accent
                        }
                    }

                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2

                        Text {
                            text: "Authentication Required"
                            font.family: Theme.fontFamily
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                            color: Theme.textBright
                        }

                        Text {
                            text: (root.flow && root.flow.actionId) ? root.flow.actionId : "Polkit Authorization"
                            font.family: Theme.fontFamily
                            font.pixelSize: 10
                            color: Theme.muted
                            elide: Text.ElideRight
                            width: 320
                        }
                    }
                }

                // Request message
                Text {
                    width: parent.width
                    text: root.flow ? root.flow.message : ""
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.text
                }

                // Supplementary message / error
                Rectangle {
                    visible: root.flow && root.flow.supplementaryMessage && root.flow.supplementaryMessage.length > 0
                    width: parent.width
                    height: suppTxt.implicitHeight + 14
                    radius: 10
                    color: (root.flow && root.flow.supplementaryIsError) ? "#3a2930" : Theme.surface
                    border.width: 1
                    border.color: (root.flow && root.flow.supplementaryIsError) ? "#bf616a" : "#434c5e"

                    Text {
                        id: suppTxt
                        anchors.fill: parent
                        anchors.margins: 7
                        wrapMode: Text.WordWrap
                        text: root.flow ? root.flow.supplementaryMessage : ""
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        color: (root.flow && root.flow.supplementaryIsError) ? "#f28fad" : Theme.text
                    }
                }

                // Password input
                Rectangle {
                    width: parent.width
                    height: 40
                    radius: 12
                    color: "#272c36"
                    border.width: 1
                    border.color: passInput.activeFocus ? Theme.accent : "#434c5e"
                    Behavior on border.color { ColorAnimation { duration: 150 } }

                    TextInput {
                        id: passInput
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 14
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: (root.flow && root.flow.responseVisible) ? TextInput.Normal : TextInput.Password
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        color: Theme.textBright
                        focus: true
                        clip: true

                        onAccepted: {
                            if (root.flow && root.flow.isResponseRequired && text.length > 0) {
                                root.flow.submit(text);
                            }
                        }

                        Keys.onEscapePressed: {
                            if (root.flow) root.flow.cancelAuthenticationRequest();
                        }
                    }

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        visible: passInput.text.length === 0
                        text: (root.flow && root.flow.inputPrompt && root.flow.inputPrompt.length > 0)
                              ? root.flow.inputPrompt
                              : "Password..."
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        color: Theme.muted
                    }
                }

                // Action Buttons
                Row {
                    anchors.right: parent.right
                    spacing: 8

                    Rectangle {
                        width: 92
                        height: 34
                        radius: 10
                        color: cancelMouse.containsMouse ? Theme.surfaceHover : Theme.surface
                        border.width: 1
                        border.color: "#434c5e"
                        Behavior on color { ColorAnimation { duration: 140 } }

                        Text {
                            anchors.centerIn: parent
                            text: "Cancel"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            font.weight: Font.Medium
                            color: Theme.textBright
                        }

                        MouseArea {
                            id: cancelMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (root.flow) root.flow.cancelAuthenticationRequest();
                            }
                        }
                    }

                    Rectangle {
                        readonly property bool canSubmit: passInput.text.length > 0
                        width: 120
                        height: 34
                        radius: 10
                        opacity: canSubmit ? 1.0 : 0.72
                        color: authMouse.containsMouse ? "#d8dee9" : Theme.accent
                        Behavior on color { ColorAnimation { duration: 140 } }
                        Behavior on opacity { NumberAnimation { duration: 140 } }

                        Text {
                            anchors.centerIn: parent
                            text: "Authenticate"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            color: Theme.accentText
                        }

                        MouseArea {
                            id: authMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (root.flow && root.flow.isResponseRequired && passInput.text.length > 0) {
                                    root.flow.submit(passInput.text);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
