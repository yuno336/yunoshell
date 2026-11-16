import QtQuick
import Quickshell
import Quickshell.Wayland
import ".."

Item {
    id: root
    property string barPos: Services.barPosition
    property bool barBottom: barPos === "bottom"
    property bool popupOpen: false
    property bool opened: false
    property real menuX: 120
    property real menuY: 120
    property real animP: 0.0

    signal openLauncherRequested()
    signal openSettingsTabRequested(string tabId)
    signal openDisplaySwitcherRequested()
    signal openPowerRequested()

    readonly property var menuItems: [
        { type: "item", id: "terminal",   icon: "󰆍", label: "Open Terminal" },
        { type: "item", id: "files",      icon: "󰉋", label: "Open Files" },
        { type: "item", id: "launcher",   icon: "󰀻", label: "App Launcher" },
        { type: "sep" },
        { type: "item", id: "background", icon: "󰸉", label: "Personalize" },
        { type: "item", id: "monitors",   icon: "󰍹", label: "Display Configuration" },
        { type: "item", id: "settings",   icon: "󰒓", label: "Settings" }
    ]

    function triggerAction(actionId) {
        close();
        if (actionId === "terminal") {
            Services.openTerminal();
        } else if (actionId === "files") {
            Services.openFiles();
        } else if (actionId === "launcher") {
            root.openLauncherRequested();
        } else if (actionId === "background") {
            root.openSettingsTabRequested("background");
        } else if (actionId === "monitors") {
            root.openSettingsTabRequested("monitors");
        } else if (actionId === "settings") {
            root.openSettingsTabRequested("time");
        }
    }

    function openAt(px, py) {
        if (popupOpen || Services.screenLocked) return;
        var sw = catcherWindow.width > 0 ? catcherWindow.width : (catcherWindow.screen ? catcherWindow.screen.width : 1536);
        var sh = catcherWindow.height > 0 ? catcherWindow.height : (catcherWindow.screen ? catcherWindow.screen.height : 960);
        var mw = 214;
        var mh = (menuCol.implicitHeight > 0 ? menuCol.implicitHeight : 350) + 16;
        var barPad = Theme.barHeight + (Services.barFloating ? 10 : 0) + 8;
        var minX = (root.barPos === "left") ? barPad : 10;
        var maxX = Math.max(minX, sw - mw - ((root.barPos === "right") ? barPad : 10));
        var minY = (root.barPos === "top") ? barPad : 10;
        var maxY = Math.max(minY, sh - mh - ((root.barPos === "bottom") ? barPad : 10));
        menuX = Math.max(minX, Math.min(maxX, Math.round(px)));
        menuY = Math.max(minY, Math.min(maxY, Math.round(py)));
        opened = true;
        openAnim.restart();
    }

    function close() {
        opened = false;
        animP = 0.0;
    }

    onPopupOpenChanged: {
        if (popupOpen && opened) close();
    }

    NumberAnimation {
        id: openAnim
        target: root
        property: "animP"
        from: 0.0
        to: 1.0
        duration: 185
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
    }

    // Desktop layer (below all windows, above wallpaper) to catch empty-desktop right-clicks
    PanelWindow {
        id: catcherWindow
        visible: Services.settingsReady && !Services.screenLocked && !root.popupOpen
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-desktop-catcher"
        WlrLayershell.layer: WlrLayer.Bottom
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        color: "transparent"

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            onPressed: mouse => {
                if (mouse.button === Qt.RightButton) {
                    root.openAt(mouse.x, mouse.y);
                } else if (root.opened) {
                    root.close();
                }
            }
        }
    }

    // Menu popup window (reuses qs-overlay namespace so no_animation rule applies)
    PanelWindow {
        id: menuWindow
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

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            onPressed: mouse => {
                if (mouse.button === Qt.RightButton) {
                    root.openAt(mouse.x, mouse.y);
                } else {
                    root.close();
                }
            }
        }

        Rectangle {
            id: menuCard
            x: root.menuX
            y: root.menuY
            width: 214
            height: menuCol.implicitHeight + 16
            radius: 14
            color: Theme.popupBg
            border.width: 1
            border.color: Theme.border
            opacity: root.animP
            scale: 0.94 + 0.06 * root.animP
            transformOrigin: Item.TopLeft
            focus: root.opened

            Keys.onEscapePressed: root.close()

            // Swallow clicks inside the menu surface
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
            }

            Column {
                id: menuCol
                anchors.fill: parent
                anchors.margins: 8
                spacing: 2

                Repeater {
                    model: root.menuItems

                    delegate: Item {
                        required property var modelData
                        width: parent.width
                        height: modelData.type === "sep" ? 9 : 31

                        Rectangle {
                            visible: modelData.type === "sep"
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.leftMargin: 6
                            anchors.rightMargin: 6
                            height: 1
                            color: Theme.border
                        }

                        Rectangle {
                            visible: modelData.type === "item"
                            anchors.fill: parent
                            radius: 9
                            color: itemMa.containsMouse ? Theme.surfaceHover : "transparent"
                            Behavior on color { ColorAnimation { duration: 100 } }

                            Row {
                                anchors.left: parent.left
                                anchors.leftMargin: 10
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 9

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 16
                                    horizontalAlignment: Text.AlignHCenter
                                    text: modelData.icon || ""
                                    color: modelData.id === "power" ? "#bf616a" : Theme.accent
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 13
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.label || ""
                                    color: itemMa.containsMouse ? Theme.textBright : Theme.text
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 12
                                    font.weight: itemMa.containsMouse ? Font.DemiBold : Font.Normal
                                }
                            }

                            MouseArea {
                                id: itemMa
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.triggerAction(modelData.id)
                            }
                        }
                    }
                }
            }
        }
    }
}
