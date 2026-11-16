import QtQuick
import Quickshell
import Quickshell.Wayland
import ".."

Item {
    id: root
    property bool opened: false
    property int selectedIndex: 1
    property real animP: 0.0

    signal openDisplaySettingsRequested()

    readonly property var modeItems: [
        { id: "primary",   title: "Primary Only",  sub: "Internal screen" },
        { id: "extend",    title: "Extend",        sub: "Dual workspace" },
        { id: "mirror",    title: "Mirror",        sub: "Duplicate view" },
        { id: "secondary", title: "External Only", sub: "Second screen" }
    ]

    function indexForMode(modeId) {
        for (var i = 0; i < modeItems.length; i++) {
            if (modeItems[i].id === modeId) return i;
        }
        return 1;
    }

    function openSwitcher() {
        Services.refreshMonitors();
        selectedIndex = indexForMode(Services.displayMode);
        opened = true;
        autoApplyTimer.stop();
        enterAnim.restart();
    }

    function cycle() {
        if (!opened) {
            Services.refreshMonitors();
            var cur = indexForMode(Services.displayMode);
            selectedIndex = (cur + 1) % modeItems.length;
            opened = true;
            enterAnim.restart();
        } else {
            selectedIndex = (selectedIndex + 1) % modeItems.length;
        }
        autoApplyTimer.restart();
    }

    function commitSelection() {
        autoApplyTimer.stop();
        var item = modeItems[selectedIndex];
        if (item) {
            Services.applyDisplayMode(item.id);
        }
        opened = false;
        animP = 0.0;
    }

    function dismiss() {
        autoApplyTimer.stop();
        opened = false;
        animP = 0.0;
    }

    Timer {
        id: autoApplyTimer
        interval: 1050
        repeat: false
        onTriggered: root.commitSelection()
    }

    NumberAnimation {
        id: enterAnim
        target: root
        property: "animP"
        from: 0.0
        to: 1.0
        duration: 180
        easing.type: Easing.OutCubic
    }

    PanelWindow {
        id: osdWindow
        visible: root.opened
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-overlay"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: root.opened ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        color: "transparent"

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0, 0, 0, 0.24 * root.animP)

            MouseArea {
                anchors.fill: parent
                onClicked: root.dismiss()
            }
        }

        Rectangle {
            id: osdCard
            anchors.centerIn: parent
            width: 632
            height: osdCol.implicitHeight + 36
            radius: 20
            color: "#fa2e3440"
            border.width: 1
            border.color: "#434c5e"
            opacity: root.animP
            scale: 0.94 + 0.06 * root.animP
            focus: root.opened

            Keys.onEscapePressed: root.dismiss()
            Keys.onReturnPressed: root.commitSelection()
            Keys.onEnterPressed: root.commitSelection()
            Keys.onSpacePressed: root.commitSelection()
            Keys.onLeftPressed: {
                autoApplyTimer.stop();
                root.selectedIndex = (root.selectedIndex + root.modeItems.length - 1) % root.modeItems.length;
            }
            Keys.onRightPressed: {
                autoApplyTimer.stop();
                root.selectedIndex = (root.selectedIndex + 1) % root.modeItems.length;
            }

            // Swallow clicks inside card
            MouseArea {
                anchors.fill: parent
            }

            Column {
                id: osdCol
                anchors.fill: parent
                anchors.margins: 18
                spacing: 14

                Item {
                    width: parent.width
                    height: 26

                    Row {
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 9

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "󰍺"
                            color: Theme.accent
                            font.family: Theme.fontFamily
                            font.pixelSize: 16
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Display Mode"
                            color: Theme.textBright
                            font.family: Theme.fontFamily
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                        }
                    }
                }

                Row {
                    width: parent.width
                    spacing: 10

                    Repeater {
                        model: root.modeItems

                        delegate: Rectangle {
                            required property var modelData
                            required property int index
                            readonly property bool isSel: root.selectedIndex === index
                            readonly property bool isCur: Services.displayMode === modelData.id

                            width: (parent.width - 30) / 4
                            height: 112
                            radius: 15
                            color: isSel ? "#3b4457" : (tileMa.containsMouse ? "#353c4c" : "#2b303c")
                            border.width: isSel ? 2 : 1
                            border.color: isSel ? Theme.accent : (isCur ? "#5e6a82" : "#3f4758")
                            Behavior on color { ColorAnimation { duration: 120 } }

                            Column {
                                anchors.fill: parent
                                anchors.margins: 12
                                spacing: 10

                                // Visual monitor diagram
                                Item {
                                    width: parent.width
                                    height: 54

                                    // Primary only diagram
                                    Row {
                                        visible: modelData.id === "primary"
                                        anchors.centerIn: parent
                                        spacing: 6

                                        Rectangle {
                                            width: 44; height: 30; radius: 5
                                            color: isSel ? Theme.accent : "#5b657a"
                                            border.width: 1
                                            border.color: isSel ? Theme.accent : "#707b94"
                                            Text {
                                                anchors.centerIn: parent
                                                text: "1"
                                                color: isSel ? Theme.accentText : Theme.textBright
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 11
                                                font.weight: Font.Bold
                                            }
                                        }
                                        Rectangle {
                                            anchors.verticalCenter: parent.verticalCenter
                                            width: 28; height: 22; radius: 4
                                            color: "transparent"
                                            border.width: 1
                                            border.color: "#434c5e"
                                            opacity: 0.55
                                        }
                                    }

                                    // Extend diagram
                                    Row {
                                        visible: modelData.id === "extend"
                                        anchors.centerIn: parent
                                        spacing: 5

                                        Rectangle {
                                            width: 38; height: 28; radius: 5
                                            color: isSel ? Theme.accent : "#5b657a"
                                            Text {
                                                anchors.centerIn: parent
                                                text: "1"
                                                color: isSel ? Theme.accentText : Theme.textBright
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 10
                                                font.weight: Font.Bold
                                            }
                                        }
                                        Rectangle {
                                            width: 38; height: 28; radius: 5
                                            color: isSel ? "#8ea0c9" : "#4c566a"
                                            border.width: 1
                                            border.color: isSel ? Theme.accent : "#66728a"
                                            Text {
                                                anchors.centerIn: parent
                                                text: "2"
                                                color: isSel ? Theme.accentText : Theme.textBright
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 10
                                                font.weight: Font.Bold
                                            }
                                        }
                                    }

                                    // Mirror diagram
                                    Item {
                                        visible: modelData.id === "mirror"
                                        anchors.centerIn: parent
                                        width: 56; height: 36

                                        Rectangle {
                                            x: 12; y: 0
                                            width: 38; height: 26; radius: 5
                                            color: isSel ? "#7d8fb3" : "#434c5e"
                                            border.width: 1
                                            border.color: "#5d687f"
                                        }
                                        Rectangle {
                                            x: 4; y: 8
                                            width: 38; height: 26; radius: 5
                                            color: isSel ? Theme.accent : "#5b657a"
                                            border.width: 1
                                            border.color: "#2e3440"
                                            Text {
                                                anchors.centerIn: parent
                                                text: "1=2"
                                                color: isSel ? Theme.accentText : Theme.textBright
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 9
                                                font.weight: Font.Bold
                                            }
                                        }
                                    }

                                    // External only diagram
                                    Row {
                                        visible: modelData.id === "secondary"
                                        anchors.centerIn: parent
                                        spacing: 6

                                        Rectangle {
                                            anchors.verticalCenter: parent.verticalCenter
                                            width: 28; height: 22; radius: 4
                                            color: "transparent"
                                            border.width: 1
                                            border.color: "#434c5e"
                                            opacity: 0.55
                                        }
                                        Rectangle {
                                            width: 44; height: 30; radius: 5
                                            color: isSel ? Theme.accent : "#5b657a"
                                            border.width: 1
                                            border.color: isSel ? Theme.accent : "#707b94"
                                            Text {
                                                anchors.centerIn: parent
                                                text: "2"
                                                color: isSel ? Theme.accentText : Theme.textBright
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 11
                                                font.weight: Font.Bold
                                            }
                                        }
                                    }
                                }

                                Text {
                                    width: parent.width
                                    horizontalAlignment: Text.AlignHCenter
                                    text: modelData.title
                                    color: Theme.textBright
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 12
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }
                            }

                            MouseArea {
                                id: tileMa
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onEntered: {
                                    autoApplyTimer.stop();
                                    root.selectedIndex = index;
                                }
                                onClicked: {
                                    root.selectedIndex = index;
                                    root.commitSelection();
                                }
                            }
                        }
                    }
                }

                Item {
                    width: parent.width
                    height: 28

                    Rectangle {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: cfgRow.implicitWidth + 18
                        height: 28
                        radius: 10
                        color: cfgMa.containsMouse ? "#3e4658" : "#323846"
                        border.width: 1
                        border.color: "#434c5e"

                        Row {
                            id: cfgRow
                            anchors.centerIn: parent
                            spacing: 6

                            Text {
                                text: "󰍹"
                                color: Theme.accent
                                font.family: Theme.fontFamily
                                font.pixelSize: 12
                            }
                            Text {
                                text: "Display Settings"
                                color: Theme.textBright
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                font.weight: Font.DemiBold
                            }
                        }

                        MouseArea {
                            id: cfgMa
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.dismiss();
                                root.openDisplaySettingsRequested();
                            }
                        }
                    }
                }
            }
        }
    }
}
