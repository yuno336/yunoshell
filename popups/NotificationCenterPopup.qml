import QtQuick
import Quickshell
import ".."

Column {
    id: root
    width: parent.width
    spacing: 10

    signal closeRequested()

    // Header
    Row {
        width: parent.width
        spacing: 10

        Rectangle {
            width: 34
            height: 34
            radius: 10
            color: Theme.surface
            anchors.verticalCenter: parent.verticalCenter

            Text {
                anchors.centerIn: parent
                text: Services.dndEnabled ? "󰂛" : "󰂚"
                font.family: Theme.fontFamily
                font.pixelSize: 17
                color: Services.dndEnabled ? Theme.muted : Theme.accent
            }
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(80, parent.width - 178)
            spacing: 1

            Text {
                text: "Notifications"
                font.family: Theme.fontFamily
                font.pixelSize: 13
                font.weight: Font.Bold
                color: Theme.textBright
            }

            Text {
                text: Services.dndEnabled
                      ? "Do Not Disturb"
                      : (Services.notifHistory.length === 0
                         ? "All caught up"
                         : (Services.notifHistory.length + " notification(s)"))
                font.family: Theme.fontFamily
                font.pixelSize: 10
                color: Theme.muted
                elide: Text.ElideRight
                width: parent.width
            }
        }

        // Clear All button
        Rectangle {
            visible: Services.notifHistory.length > 0
            width: 58
            height: 28
            radius: 8
            anchors.verticalCenter: parent.verticalCenter
            color: clearMouse.containsMouse ? Theme.surfaceHover : Theme.surface
            border.width: 1
            border.color: Theme.border

            Behavior on color { ColorAnimation { duration: 120 } }

            Text {
                anchors.centerIn: parent
                text: "Clear"
                font.family: Theme.fontFamily
                font.pixelSize: 11
                font.weight: Font.Medium
                color: Theme.text
            }

            MouseArea {
                id: clearMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Services.clearAllNotifs()
            }
        }

        // DND Switch
        Rectangle {
            width: 50
            height: 28
            radius: 14
            anchors.verticalCenter: parent.verticalCenter
            color: Services.dndEnabled ? Theme.accent : Theme.surfaceHover
            border.width: Services.dndEnabled ? 0 : 2
            border.color: Theme.muted

            Behavior on color { ColorAnimation { duration: 150 } }

            Rectangle {
                readonly property int sz: Services.dndEnabled ? 22 : 14
                width: sz
                height: sz
                radius: sz / 2
                anchors.verticalCenter: parent.verticalCenter
                x: Services.dndEnabled ? 25 : 7
                color: Services.dndEnabled ? Theme.accentText : Theme.muted

                Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                Behavior on width { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

                Text {
                    visible: Services.dndEnabled
                    anchors.centerIn: parent
                    text: "󰂛"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.text
                }
            }

            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: Services.toggleDnd()
            }
        }
    }

    Rectangle {
        width: parent.width
        height: 1
        color: Theme.border
    }

    // Empty state
    Item {
        visible: Services.notifHistory.length === 0
        width: parent.width
        height: 136

        Column {
            anchors.centerIn: parent
            spacing: 8

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Services.dndEnabled ? "󰂛" : "󰂚"
                font.family: Theme.fontFamily
                font.pixelSize: 28
                color: Theme.muted
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Services.dndEnabled ? "Notifications muted (DND)" : "No notifications"
                font.family: Theme.fontFamily
                font.pixelSize: 12
                color: Theme.muted
            }
        }
    }

    // Notification List
    Flickable {
        id: notifFlick
        visible: Services.notifHistory.length > 0
        width: parent.width
        height: Math.min(360, notifCol.height)
        contentWidth: width
        contentHeight: notifCol.height
        clip: true
        interactive: false
        boundsBehavior: Flickable.StopAtBounds

        Behavior on contentY {
            NumberAnimation {
                duration: 90
                easing.type: Easing.OutCubic
            }
        }

        WheelHandler {
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: event => {
                var maxScroll = Math.max(0, notifFlick.contentHeight - notifFlick.height);
                var step = event.pixelDelta.y !== 0
                    ? -event.pixelDelta.y * 1.35
                    : -(event.angleDelta.y / 120.0) * 42.0;
                notifFlick.contentY = Math.max(0, Math.min(maxScroll, notifFlick.contentY + step));
            }
        }

        Column {
            id: notifCol
            width: parent.width
            spacing: 8

            Repeater {
                model: Services.notifHistory

                Rectangle {
                    id: histCard
                    required property var modelData
                    width: notifCol.width
                    height: Math.max(60, histRow.implicitHeight + 22)
                    radius: 11
                    color: histHover.containsMouse ? Theme.surfaceHover : Theme.surface
                    border.width: 1
                    border.color: modelData.isCritical ? Theme.accent : Theme.border

                    Behavior on color { ColorAnimation { duration: 120 } }

                    Rectangle {
                        visible: histCard.modelData.isCritical
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 3
                        radius: 2
                        color: Theme.accent
                    }

                    MouseArea {
                        id: histHover
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.NoButton
                    }

                    Row {
                        id: histRow
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 11
                        spacing: 11

                        // Left prominent icon
                        Rectangle {
                            width: 36
                            height: 36
                            radius: 8
                            anchors.top: parent.top
                            anchors.topMargin: 2
                            color: Theme.popupBg
                            border.width: 1
                            border.color: Theme.border

                            Image {
                                anchors.centerIn: parent
                                width: 22
                                height: 22
                                visible: histCard.modelData.iconSource.length > 0 && status === Image.Ready
                                source: histCard.modelData.iconSource
                                sourceSize.width: 44
                                sourceSize.height: 44
                                fillMode: Image.PreserveAspectFit
                                asynchronous: false
                                smooth: true
                            }

                            Text {
                                anchors.centerIn: parent
                                visible: histCard.modelData.iconSource.length === 0
                                text: histCard.modelData.isCritical ? "󰀦" : "󰂚"
                                font.family: Theme.fontFamily
                                font.pixelSize: 16
                                color: Theme.accent
                            }
                        }

                        // Right content
                        Column {
                            width: parent.width - 47
                            spacing: 3

                            Item {
                                width: parent.width
                                height: 18

                                Text {
                                    anchors.left: parent.left
                                    anchors.right: histRightMeta.left
                                    anchors.rightMargin: 8
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: histCard.modelData.appName
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 10
                                    font.weight: Font.DemiBold
                                    color: Theme.muted
                                    elide: Text.ElideRight
                                }

                                Row {
                                    id: histRightMeta
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 6

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: histCard.modelData.timeStr
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 10
                                        color: Theme.muted
                                    }

                                    Rectangle {
                                        width: 18
                                        height: 18
                                        radius: 5
                                        anchors.verticalCenter: parent.verticalCenter
                                        color: delMouse.containsMouse ? Theme.surfaceActive : "transparent"

                                        Text {
                                            anchors.centerIn: parent
                                            text: "󰅖"
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 11
                                            color: delMouse.containsMouse ? Theme.textBright : Theme.muted
                                        }

                                        MouseArea {
                                            id: delMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: Services.removeNotif(histCard.modelData.uid)
                                        }
                                    }
                                }
                            }

                            Text {
                                visible: histCard.modelData.summary.length > 0
                                width: parent.width
                                text: histCard.modelData.summary
                                font.family: Theme.fontFamily
                                font.pixelSize: 12
                                font.weight: Font.Bold
                                color: "#eceff4"
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                            }

                            Text {
                                visible: histCard.modelData.body.length > 0
                                width: parent.width
                                text: histCard.modelData.body
                                textFormat: Text.PlainText
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                color: Theme.text
                                opacity: 0.80
                                lineHeight: 1.25
                                wrapMode: Text.Wrap
                                maximumLineCount: 4
                                elide: Text.ElideRight
                            }

                            Flow {
                                visible: (histCard.modelData.actions || []).length > 0
                                width: parent.width
                                spacing: 6
                                topPadding: 5

                                Repeater {
                                    model: histCard.modelData.actions || []

                                    Rectangle {
                                        required property var modelData
                                        width: Math.min(notifCol.width - 72, histActLbl.implicitWidth + 16)
                                        height: 22
                                        radius: 6
                                        color: histActMouse.containsMouse ? Theme.surfaceActive : Theme.popupBg
                                        border.width: 1
                                        border.color: Theme.border

                                        Text {
                                            id: histActLbl
                                            anchors.centerIn: parent
                                            text: modelData.text
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            color: Theme.textBright
                                            elide: Text.ElideRight
                                        }

                                        MouseArea {
                                            id: histActMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                try { modelData.invoke(); } catch (e) {}
                                                root.closeRequested();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
