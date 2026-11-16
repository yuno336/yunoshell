import QtQuick
import Quickshell
import Quickshell.Services.SystemTray
import ".."

Grid {
    id: root
    property var barWindow
    property bool vertical: false
    columns: vertical ? 1 : Math.max(1, SystemTray.items.values.length)
    spacing: 4
    visible: Services.showTrayOnBar && SystemTray.items.values.length > 0

    Repeater {
        model: SystemTray.items

        Rectangle {
            id: trayItem
            required property SystemTrayItem modelData

            width: 24
            height: 24
            color: "transparent"

            Image {
                anchors.centerIn: parent
                width: 15
                height: 15
                source: modelData.icon
                smooth: true
                asynchronous: true
            }

            MouseArea {
                id: trayMouse
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                cursorShape: Qt.PointingHandCursor
                onClicked: mouse => {
                    if (mouse.button === Qt.LeftButton) {
                        if (modelData.onlyMenu && modelData.hasMenu) {
                            var pos = trayItem.mapToItem(null, 0, trayItem.height);
                            modelData.display(root.barWindow, pos.x, pos.y);
                        } else {
                            modelData.activate();
                        }
                    } else if (mouse.button === Qt.RightButton) {
                        if (modelData.hasMenu) {
                            var posR = trayItem.mapToItem(null, 0, trayItem.height);
                            modelData.display(root.barWindow, posR.x, posR.y);
                        }
                    } else if (mouse.button === Qt.MiddleButton) {
                        modelData.secondaryActivate();
                    }
                }
            }
        }
    }
}
