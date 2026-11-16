import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Services.Pam
import ".."

Item {
    id: root

    property string pendingPassword: ""
    property bool checking: false
    property string statusText: "Enter password to unlock"
    property bool statusError: false

    function submitPassword(pw) {
        if (root.checking || !pw || pw.length === 0) return;
        root.checking = true;
        root.statusError = false;
        root.statusText = "Authenticating...";
        root.pendingPassword = pw;
        if (!pam.start()) {
            root.checking = false;
            root.pendingPassword = "";
            root.statusError = true;
            root.statusText = "Unable to start PAM";
        }
    }

    PamContext {
        id: pam
        config: "system-auth"
        user: "huyle"

        onPamMessage: {
            if (responseRequired) {
                pam.respond(root.pendingPassword);
            }
        }

        onCompleted: result => {
            root.checking = false;
            root.pendingPassword = "";
            if (result === PamResult.Success) {
                root.statusError = false;
                root.statusText = "Enter password to unlock";
                Services.unlockScreen();
            } else {
                root.statusError = true;
                root.statusText = "Incorrect password";
            }
        }

        onError: err => {
            root.checking = false;
            root.pendingPassword = "";
            root.statusError = true;
            root.statusText = "Auth error: " + PamError.toString(err);
        }
    }

    WlSessionLock {
        id: sessionLock
        locked: Services.screenLocked

        onLockStateChanged: {
            if (locked) {
                root.checking = false;
                root.pendingPassword = "";
                root.statusError = false;
                root.statusText = "Enter password to unlock";
            }
        }

        WlSessionLockSurface {
            id: lockSurface

            Rectangle {
                anchors.fill: parent
                color: "#2e3440"

                Image {
                    anchors.fill: parent
                    source: {
                        var p = Services.lockUseSameWallpaper
                            ? Services.wallpaperPath
                            : (Services.lockWallpaperPath.length > 0 ? Services.lockWallpaperPath : Services.wallpaperPath);
                        return p.length > 0 ? ("file://" + p) : "";
                    }
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    smooth: true
                }

                Rectangle {
                    anchors.fill: parent
                    color: "#000000"
                    opacity: Services.lockDimOpacity
                }

                Timer {
                    interval: 1000
                    running: sessionLock.locked
                    repeat: true
                    triggeredOnStart: true
                    onTriggered: {
                        var now = new Date();
                        clockText.text = Services.lock24Hour
                            ? Qt.formatTime(now, "HH:mm")
                            : Qt.formatTime(now, "hh:mm AP");
                        dateText.text = Qt.formatDate(now, "dddd, dd MMM yyyy");
                    }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 22
                    width: 380

                    // Clock Header
                    Column {
                        visible: Services.lockShowClock
                        width: parent.width
                        spacing: 4

                        Text {
                            id: clockText
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "00:00"
                            font.family: Theme.fontFamily
                            font.pixelSize: 64
                            font.weight: Font.Bold
                            color: "#eceff4"
                        }

                        Text {
                            id: dateText
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: ""
                            font.family: Theme.fontFamily
                            font.pixelSize: 13
                            color: "#d8dee9"
                        }
                    }

                    // Nord Auth Card
                    Rectangle {
                        width: parent.width
                        height: cardCol.height + 36
                        color: "#eb2e3440"
                        border.width: 1
                        border.color: root.statusError ? "#bf616a" : Theme.border

                        Behavior on border.color { ColorAnimation { duration: 150 } }

                        Column {
                            id: cardCol
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 18
                            spacing: 12

                            Row {
                                anchors.horizontalCenter: parent.horizontalCenter
                                spacing: 8

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "󰌾"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 15
                                    color: Theme.accent
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "huyle@archbtw"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 13
                                    font.weight: Font.Bold
                                    color: Theme.textBright
                                }
                            }

                            Rectangle {
                                width: parent.width
                                height: 42
                                color: Theme.surface
                                border.width: 1
                                border.color: pwInput.activeFocus ? Theme.accent : Theme.border

                                TextInput {
                                    id: pwInput
                                    anchors.left: parent.left
                                    anchors.right: unlockBtn.left
                                    anchors.top: parent.top
                                    anchors.bottom: parent.bottom
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 8
                                    verticalAlignment: TextInput.AlignVCenter
                                    echoMode: TextInput.Password
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 13
                                    color: Theme.textBright
                                    selectionColor: Theme.accent
                                    selectedTextColor: Theme.accentText
                                    focus: true
                                    enabled: !root.checking

                                    Connections {
                                        target: root
                                        function onStatusErrorChanged() {
                                            if (root.statusError) {
                                                pwInput.text = "";
                                                pwInput.forceActiveFocus();
                                            }
                                        }
                                    }

                                    Connections {
                                        target: sessionLock
                                        function onLockStateChanged() {
                                            if (sessionLock.locked) {
                                                pwInput.text = "";
                                                pwInput.forceActiveFocus();
                                            }
                                        }
                                    }

                                    Text {
                                        anchors.fill: parent
                                        verticalAlignment: Text.AlignVCenter
                                        text: "Password..."
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 12
                                        color: Theme.muted
                                        visible: pwInput.text.length === 0
                                    }

                                    Keys.onReturnPressed: root.submitPassword(pwInput.text)
                                    Keys.onEnterPressed: root.submitPassword(pwInput.text)
                                }

                                Rectangle {
                                    id: unlockBtn
                                    width: 34
                                    height: 34
                                    anchors.right: parent.right
                                    anchors.rightMargin: 4
                                    anchors.verticalCenter: parent.verticalCenter
                                    color: unlockMouse.containsMouse ? Theme.accent : Theme.surfaceHover

                                    Text {
                                        anchors.centerIn: parent
                                        text: root.checking ? "󰔟" : "󰁔"
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 15
                                        color: unlockMouse.containsMouse ? Theme.accentText : Theme.text
                                    }

                                    MouseArea {
                                        id: unlockMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.submitPassword(pwInput.text)
                                    }
                                }
                            }

                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: root.statusText
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                color: root.statusError ? "#bf616a" : Theme.muted
                            }
                        }
                    }
                }
            }
        }
    }
}
