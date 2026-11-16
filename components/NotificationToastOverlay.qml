import QtQuick
import Quickshell
import Quickshell.Wayland
import ".."

Item {
    id: root
    property bool suppressToasts: false
    property bool barSwitching: false
    signal openCenterRequested()

    readonly property bool barBottom: Services.barPosition === "bottom"
    readonly property bool hasToasts: !root.suppressToasts && !Services.screenLocked && Services.popupNotifs.length > 0

    onHasToastsChanged: {
        if (hasToasts) {
            toastIsland.islandClosing = false;
            toastExitAnim.stop();
            if (!toastEnterAnim.running && toastIsland.heightP < 1.0) {
                toastIsland.widthP = 0.0;
                toastIsland.heightP = 0.0;
                toastIsland.contentP = 0.0;
                toastEnterAnim.restart();
            }
        } else {
            toastEnterAnim.stop();
            if (root.suppressToasts || Services.screenLocked) {
                toastExitAnim.stop();
                toastIsland.islandClosing = false;
                toastIsland.widthP = 0.0;
                toastIsland.heightP = 0.0;
                toastIsland.contentP = 0.0;
                if (Services.popupNotifs.length > 0) Services.clearPopupToasts();
            } else if (toastIsland.heightP > 0.005 && !toastExitAnim.running) {
                toastIsland.islandClosing = true;
                toastExitAnim.restart();
            }
        }
    }

    // Keep toastWindow always mapped with empty Region when idle so compositor
    // layer open/close slide+fade animations never fight QML island physics.
    PanelWindow {
        id: toastWindow
        property bool anchorsReady: false
        function syncAnchors() {
            if (root.barBottom) {
                anchors.top = false;
                anchors.bottom = true;
            } else {
                anchors.bottom = false;
                anchors.top = true;
            }
            anchorsReady = true;
        }
        visible: Services.settingsReady && anchorsReady && !root.barSwitching
                 && (root.hasToasts || toastIsland.islandClosing || toastEnterAnim.running || toastExitAnim.running)
        anchors {
            top: false
            bottom: false
        }
        Component.onCompleted: syncAnchors()
        Connections {
            target: root
            function onBarBottomChanged() { toastWindow.syncAnchors(); }
        }
        margins {
            top: root.barBottom ? 0 : (Services.barPosition === "top" ? (Theme.barHeight + (Services.barFloating ? 4 : -1)) : 0)
            bottom: root.barBottom ? (Theme.barHeight + (Services.barFloating ? 4 : -1)) : 0
        }
        implicitWidth: 468
        implicitHeight: 580
        mask: Region {
            item: toastIsland.visible ? toastIsland : emptyToastMask
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-toast"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        color: "transparent"

        Item {
            id: emptyToastMask
            width: 0
            height: 0
        }

        Item {
            id: toastIsland
            readonly property int earR: 16
            readonly property int pillBodyW: 164
            readonly property int fullBodyW: 420
            readonly property int targetBodyH: Math.max(68, toastCol.implicitHeight + 28)
            readonly property bool anyCritical: {
                var arr = Services.popupNotifs;
                for (var i = 0; i < arr.length; i++) {
                    if (arr[i].isCritical) return true;
                }
                return false;
            }
            property real widthP: 0.0
            property real heightP: 0.0
            property real contentP: 0.0
            property bool islandClosing: false
            readonly property real curBodyW: Math.max(120, pillBodyW + (fullBodyW - pillBodyW) * widthP)
            readonly property real curBodyH: Math.max(2, targetBodyH * heightP)

            anchors.horizontalCenter: parent.horizontalCenter
            y: root.barBottom ? (toastWindow.height - height) : 0
            width: curBodyW + 2 * earR
            height: curBodyH
            visible: heightP > 0.005 && (root.hasToasts || islandClosing)

            Behavior on height {
                enabled: !toastEnterAnim.running && !toastExitAnim.running
                NumberAnimation { duration: 240; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0] }
            }

            ParallelAnimation {
                id: toastEnterAnim
                NumberAnimation {
                    target: toastIsland
                    property: "widthP"
                    to: 1.0
                    duration: 280
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                }
                NumberAnimation {
                    target: toastIsland
                    property: "heightP"
                    to: 1.0
                    duration: 300
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                }
                NumberAnimation {
                    target: toastIsland
                    property: "contentP"
                    to: 1.0
                    duration: 220
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0]
                }
            }

            ParallelAnimation {
                id: toastExitAnim
                NumberAnimation {
                    target: toastIsland
                    property: "contentP"
                    to: 0.0
                    duration: 75
                    easing.type: Easing.OutCubic
                }
                NumberAnimation {
                    target: toastIsland
                    property: "heightP"
                    to: 0.0
                    duration: 220
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: [0.32, 0.0, 0.15, 1.0, 1.0, 1.0]
                }
                NumberAnimation {
                    target: toastIsland
                    property: "widthP"
                    to: 0.0
                    duration: 220
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: [0.32, 0.0, 0.15, 1.0, 1.0, 1.0]
                }
                onFinished: {
                    toastIsland.islandClosing = false;
                    var arr = Services.popupNotifs.slice();
                    for (var i = 0; i < arr.length; i++) {
                        Services.dismissPopupToast(arr[i].uid);
                    }
                }
            }

            function dismissUid(uid) {
                if (Services.popupNotifs.length <= 1) {
                    if (!islandClosing) {
                        islandClosing = true;
                        toastEnterAnim.stop();
                        toastExitAnim.restart();
                    }
                } else {
                    Services.dismissPopupToast(uid);
                }
            }

            function dismissAll() {
                if (!islandClosing) {
                    islandClosing = true;
                    toastEnterAnim.stop();
                    toastExitAnim.restart();
                }
            }

            DynamicIslandShell {
                anchors.fill: parent
                bodyWidth: toastIsland.curBodyW
                bodyHeight: toastIsland.height
                earRadius: toastIsland.earR
                bottomRadius: 22
                topColor: (root.barBottom || Services.barPosition === "top") ? Theme.barBg : Theme.popupBg
                fillColor: Theme.popupBg
                borderColor: toastIsland.anyCritical ? Theme.accent : Theme.border
                invertY: root.barBottom
            }

            MouseArea {
                id: islandHover
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.NoButton
            }

            Item {
                anchors.fill: parent
                anchors.leftMargin: toastIsland.earR
                anchors.rightMargin: toastIsland.earR
                anchors.topMargin: root.barBottom ? 12 : 0
                anchors.bottomMargin: root.barBottom ? 0 : 12
                clip: true

                Column {
                    id: toastCol
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    y: root.barBottom ? (parent.height - height - 10) : 10
                    spacing: 8
                    opacity: toastIsland.contentP
                    transform: Translate {
                        y: (1.0 - toastIsland.heightP) * (root.barBottom ? 12.0 : -12.0)
                    }

                    Repeater {
                        model: Services.popupNotifs

                        Rectangle {
                            id: toastCard
                            required property var modelData
                            readonly property int uid: modelData.uid

                            width: toastCol.width
                            height: Math.max(60, cardRow.implicitHeight + 20)
                            radius: 11
                            color: hoverArea.containsMouse ? Theme.surfaceHover : Theme.surface
                            border.width: 1
                            border.color: modelData.isCritical ? Theme.accent : Theme.border

                            Behavior on color { ColorAnimation { duration: 120 } }

                            Timer {
                                id: autoExpireTimer
                                interval: toastCard.modelData.expireMs || 5000
                                running: !islandHover.containsMouse && !hoverArea.containsMouse && !toastIsland.islandClosing
                                repeat: false
                                onTriggered: toastIsland.dismissUid(toastCard.uid)
                            }

                            Rectangle {
                                visible: toastCard.modelData.isCritical
                                anchors.left: parent.left
                                anchors.top: parent.top
                                anchors.bottom: parent.bottom
                                width: 3
                                color: Theme.accent
                            }

                            MouseArea {
                                id: hoverArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    var acts = toastCard.modelData.actions || [];
                                    for (var i = 0; i < acts.length; i++) {
                                        if (acts[i].identifier === "default") {
                                            try { acts[i].invoke(); } catch (e) {}
                                            toastIsland.dismissUid(toastCard.uid);
                                            return;
                                        }
                                    }
                                    toastIsland.dismissAll();
                                    root.openCenterRequested();
                                }
                            }

                            Row {
                                id: cardRow
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 11

                                // Left prominent app icon
                                Rectangle {
                                    width: 38
                                    height: 38
                                    radius: 8
                                    anchors.top: parent.top
                                    anchors.topMargin: 1
                                    color: Theme.popupBg
                                    border.width: 1
                                    border.color: Theme.border

                                    Image {
                                        anchors.centerIn: parent
                                        width: 24
                                        height: 24
                                        visible: toastCard.modelData.iconSource.length > 0 && status === Image.Ready
                                        source: toastCard.modelData.iconSource
                                        sourceSize.width: 48
                                        sourceSize.height: 48
                                        fillMode: Image.PreserveAspectFit
                                        asynchronous: false
                                        smooth: true
                                    }

                                    Text {
                                        anchors.centerIn: parent
                                        visible: toastCard.modelData.iconSource.length === 0
                                        text: toastCard.modelData.isCritical ? "󰀦" : "󰂚"
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 18
                                        color: Theme.accent
                                    }
                                }

                                // Right content column
                                Column {
                                    width: parent.width - 49
                                    spacing: 3

                                    Item {
                                        width: parent.width
                                        height: 17

                                        Text {
                                            anchors.left: parent.left
                                            anchors.right: rightMetaRow.left
                                            anchors.rightMargin: 8
                                            anchors.verticalCenter: parent.verticalCenter
                                            text: toastCard.modelData.appName
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            font.weight: Font.DemiBold
                                            color: Theme.muted
                                            elide: Text.ElideRight
                                        }

                                        Row {
                                            id: rightMetaRow
                                            anchors.right: parent.right
                                            anchors.verticalCenter: parent.verticalCenter
                                            spacing: 6

                                            Text {
                                                anchors.verticalCenter: parent.verticalCenter
                                                text: toastCard.modelData.timeStr
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 10
                                                color: Theme.muted
                                            }

                                            Rectangle {
                                                width: 16
                                                height: 16
                                                radius: 4
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: closeToastMouse.containsMouse ? Theme.popupBg : "transparent"

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: "󰅖"
                                                    font.family: Theme.fontFamily
                                                    font.pixelSize: 10
                                                    color: closeToastMouse.containsMouse ? Theme.textBright : Theme.muted
                                                }

                                                MouseArea {
                                                    id: closeToastMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: toastIsland.dismissUid(toastCard.uid)
                                                }
                                            }
                                        }
                                    }

                                    Text {
                                        visible: toastCard.modelData.summary.length > 0
                                        width: parent.width
                                        text: toastCard.modelData.summary
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 12
                                        font.weight: Font.Bold
                                        color: "#eceff4"
                                        wrapMode: Text.Wrap
                                        maximumLineCount: 2
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        visible: toastCard.modelData.body.length > 0
                                        width: parent.width
                                        text: toastCard.modelData.body
                                        textFormat: Text.PlainText
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 11
                                        color: Theme.text
                                        opacity: 0.82
                                        lineHeight: 1.25
                                        wrapMode: Text.Wrap
                                        maximumLineCount: 4
                                        elide: Text.ElideRight
                                    }

                                    Flow {
                                        visible: (toastCard.modelData.actions || []).length > 0
                                        width: parent.width
                                        spacing: 6
                                        topPadding: 5

                                        Repeater {
                                            model: toastCard.modelData.actions || []

                                            Rectangle {
                                                required property var modelData
                                                width: Math.min(toastCol.width - 70, actLbl.implicitWidth + 18)
                                                height: 24
                                                radius: 6
                                                color: actMouse.containsMouse ? Theme.surfaceActive : Theme.popupBg
                                                border.width: 1
                                                border.color: Theme.border

                                                Text {
                                                    id: actLbl
                                                    anchors.centerIn: parent
                                                    text: modelData.text
                                                    font.family: Theme.fontFamily
                                                    font.pixelSize: 10
                                                    font.weight: Font.Medium
                                                    color: Theme.textBright
                                                    elide: Text.ElideRight
                                                }

                                                MouseArea {
                                                    id: actMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        try { modelData.invoke(); } catch (e) {}
                                                        toastIsland.dismissUid(toastCard.uid);
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
    }
}
