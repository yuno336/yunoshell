import QtQuick
import Quickshell
import ".."

Column {
    id: root
    width: parent.width
    spacing: 8

    signal closeRequested()

    property string query: ""
    readonly property var rawApps: DesktopEntries.applications.values
    readonly property var selectedApp: (appGrid.currentIndex >= 0 && appGrid.currentIndex < filteredApps.length)
                                     ? filteredApps[appGrid.currentIndex]
                                     : null

    function isSubsequence(q, text) {
        var qi = 0;
        for (var ti = 0; ti < text.length && qi < q.length; ti++) {
            if (text.charAt(ti) === q.charAt(qi)) qi++;
        }
        return qi === q.length;
    }

    function scoreEntry(entry, q) {
        var name = (entry.name || "").toLowerCase();
        var gen = (entry.genericName || "").toLowerCase();
        var comment = (entry.comment || "").toLowerCase();
        var idStr = (entry.id || "").toLowerCase();
        var kws = entry.keywords ? entry.keywords.join(" ").toLowerCase() : "";

        if (name === q) return 1000;
        if (name.indexOf(q) === 0) return 850 - name.length * 0.1;

        var words = name.split(/[\s\-_.]+/);
        for (var i = 0; i < words.length; i++) {
            if (words[i].indexOf(q) === 0) return 700 - name.length * 0.1;
        }
        if (name.indexOf(q) >= 0) return 550 - name.indexOf(q);
        if (gen.indexOf(q) >= 0) return 400;
        if (kws.indexOf(q) >= 0 || idStr.indexOf(q) >= 0) return 300;
        if (comment.indexOf(q) >= 0) return 220;
        if (isSubsequence(q, name)) return 140 - name.length * 0.1;
        return -1;
    }

    readonly property var filteredApps: {
        var q = root.query.trim().toLowerCase();
        var seen = {};
        var list = [];
        var src = root.rawApps || [];
        for (var i = 0; i < src.length; i++) {
            var e = src[i];
            if (!e || e.noDisplay || !e.name || e.name.length === 0) continue;
            var key = e.id || e.name;
            if (seen[key]) continue;
            seen[key] = true;

            if (q.length === 0) {
                list.push({ entry: e, score: 0 });
            } else {
                var sc = root.scoreEntry(e, q);
                if (sc > 0) {
                    list.push({ entry: e, score: sc });
                }
            }
        }
        list.sort(function(a, b) {
            if (b.score !== a.score) return b.score - a.score;
            return (a.entry.name || "").localeCompare(b.entry.name || "");
        });
        var out = [];
        for (var j = 0; j < list.length; j++) {
            out.push(list[j].entry);
        }
        return out;
    }

    function resetAndFocus() {
        if (searchInput.text !== "") searchInput.text = "";
        if (root.query !== "") root.query = "";
        if (appGrid.currentIndex !== 0) appGrid.currentIndex = 0;
        if (appGrid.contentY !== 0) appGrid.contentY = 0;
        searchInput.forceActiveFocus();
    }

    function launchApp(entry) {
        if (!entry) return;
        root.closeRequested();
        if (entry.runInTerminal && entry.command && entry.command.length > 0) {
            var cmd = ["kitty"];
            if (entry.workingDirectory && entry.workingDirectory.length > 0) {
                cmd.push("--directory");
                cmd.push(entry.workingDirectory);
            }
            cmd.push("-e");
            for (var i = 0; i < entry.command.length; i++) {
                cmd.push(entry.command[i]);
            }
            Quickshell.execDetached(cmd);
        } else {
            entry.execute();
        }
    }

    function moveSelection(delta) {
        var cnt = filteredApps.length;
        if (cnt === 0) return;
        var next = appGrid.currentIndex + delta;
        if (next < 0) next = cnt - 1;
        if (next >= cnt) next = 0;
        appGrid.currentIndex = next;
        appGrid.positionViewAtIndex(next, GridView.Contain);
    }

    Component.onCompleted: {
        Qt.callLater(resetAndFocus);
    }

    // Compact Search Bar (shows selected app pill badge on the right)
    Rectangle {
        width: parent.width
        height: 36
        radius: 11
        color: Theme.surface
        border.width: 1
        border.color: searchInput.activeFocus
                      ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.45)
                      : Qt.rgba(Theme.border.r, Theme.border.g, Theme.border.b, 0.55)

        Row {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 8
            spacing: 8

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "󰍉"
                font.family: Theme.fontFamily
                font.pixelSize: 15
                color: searchInput.activeFocus ? Theme.accent : Theme.muted
            }

            Item {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(120, parent.width - 28 - selBadge.width - (searchInput.text.length > 0 ? 32 : 8))
                height: 22
                clip: true

                Text {
                    anchors.fill: parent
                    verticalAlignment: Text.AlignVCenter
                    text: "Search apps..."
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    color: Theme.muted
                    visible: searchInput.text.length === 0
                }

                TextInput {
                    id: searchInput
                    anchors.fill: parent
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.textBright
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.accentText
                    clip: true
                    focus: true

                    onTextChanged: {
                        root.query = text;
                        appGrid.currentIndex = 0;
                        appGrid.contentY = 0;
                    }

                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Right || event.key === Qt.Key_Tab) {
                            root.moveSelection(1);
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Backtab) {
                            root.moveSelection(-1);
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Down || (event.modifiers & Qt.ControlModifier && (event.key === Qt.Key_J || event.key === Qt.Key_N))) {
                            root.moveSelection(appGrid.columns);
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Up || (event.modifiers & Qt.ControlModifier && (event.key === Qt.Key_K || event.key === Qt.Key_P))) {
                            root.moveSelection(-appGrid.columns);
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                            if (appGrid.currentIndex >= 0 && appGrid.currentIndex < root.filteredApps.length) {
                                root.launchApp(root.filteredApps[appGrid.currentIndex]);
                            }
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Escape) {
                            if (searchInput.text.length > 0) {
                                searchInput.text = "";
                            } else {
                                root.closeRequested();
                            }
                            event.accepted = true;
                        }
                    }
                }
            }

            Rectangle {
                id: selBadge
                anchors.verticalCenter: parent.verticalCenter
                height: 22
                width: Math.min(selLabel.implicitWidth + 16, 188)
                radius: 7
                color: root.selectedApp ? Theme.accentSoft : "transparent"
                visible: selLabel.text.length > 0

                Text {
                    id: selLabel
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, 172)
                    horizontalAlignment: Text.AlignCenter
                    text: root.selectedApp ? (root.selectedApp.name || "") : ""
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                    color: Theme.accent
                    elide: Text.ElideRight
                }
            }

            Rectangle {
                visible: searchInput.text.length > 0
                width: 20
                height: 20
                radius: 6
                anchors.verticalCenter: parent.verticalCenter
                color: clrMouse.containsMouse ? Theme.surfaceHover : "transparent"

                Text {
                    anchors.centerIn: parent
                    text: "󰅖"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.muted
                }

                MouseArea {
                    id: clrMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        searchInput.text = "";
                        searchInput.forceActiveFocus();
                    }
                }
            }
        }
    }

    // Empty search result
    Item {
        visible: root.filteredApps.length === 0
        width: parent.width
        height: 72

        Row {
            anchors.centerIn: parent
            spacing: 8

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "󰀻"
                font.family: Theme.fontFamily
                font.pixelSize: 18
                color: Theme.muted
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "No matching application"
                font.family: Theme.fontFamily
                font.pixelSize: 12
                color: Theme.muted
            }
        }
    }

    // Floating Launchpad Grid: auto-sizes 1..3 rows cleanly without dead vertical void
    Item {
        visible: root.filteredApps.length > 0
        width: parent.width
        readonly property int visibleRows: Math.min(3, Math.max(1, Math.ceil(root.filteredApps.length / 8)))
        height: visibleRows * 72
        Behavior on height {
            NumberAnimation {
                duration: 160
                easing.type: Easing.BezierSpline
                easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
            }
        }

        GridView {
            id: appGrid
            anchors.fill: parent
            anchors.rightMargin: 6
            readonly property int columns: 8
            cellWidth: Math.floor(width / columns)
            cellHeight: 72
            model: root.filteredApps
            clip: true
            interactive: false
            cacheBuffer: 600
            currentIndex: 0
            boundsBehavior: Flickable.StopAtBounds

            onContentHeightChanged: {
                var maxY = Math.max(0, contentHeight - height);
                if (contentY > maxY) contentY = maxY;
            }

            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                onWheel: event => {
                    var maxY = Math.max(0, appGrid.contentHeight - appGrid.height);
                    if (maxY <= 0) return;
                    var step = (event.pixelDelta && event.pixelDelta.y !== 0)
                               ? -event.pixelDelta.y
                               : -(event.angleDelta.y / 120.0) * 44.0;
                    appGrid.contentY = Math.max(0, Math.min(maxY, appGrid.contentY + step));
                    event.accepted = true;
                }
            }

            delegate: Item {
                id: appCell
                required property var modelData
                required property int index
                readonly property bool selected: appGrid.currentIndex === index
                readonly property bool active: selected || tileMouse.containsMouse
                readonly property string resolvedIcon: (modelData.icon && modelData.icon.length > 0)
                                                     ? Quickshell.iconPath(modelData.icon, true)
                                                     : ""

                width: appGrid.cellWidth
                height: appGrid.cellHeight

                Item {
                    anchors.centerIn: parent
                    width: 56
                    height: 62
                    scale: appCell.active ? 1.14 : 1.0
                    transform: Translate {
                        y: appCell.active ? -2 : 0
                        Behavior on y {
                            NumberAnimation {
                                duration: 150
                                easing.type: Easing.BezierSpline
                                easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                            }
                        }
                    }
                    Behavior on scale {
                        NumberAnimation {
                            duration: 150
                            easing.type: Easing.BezierSpline
                            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                        }
                    }

                    Image {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: -4
                        width: 44
                        height: 44
                        visible: appCell.resolvedIcon.length > 0 && status === Image.Ready
                        source: appCell.resolvedIcon
                        sourceSize.width: 64
                        sourceSize.height: 64
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                        mipmap: true
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: -4
                        visible: appCell.resolvedIcon.length === 0
                        text: "󰀻"
                        font.family: Theme.fontFamily
                        font.pixelSize: 26
                        color: Theme.accent
                    }

                    // Minimal Dock-style indicator pill beneath selected icon (no tile box/border)
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 3
                        width: appCell.selected ? 16 : 0
                        height: 3
                        radius: 1.5
                        color: Theme.accent
                        opacity: appCell.selected ? 1.0 : 0.0
                        Behavior on width {
                            NumberAnimation {
                                duration: 150
                                easing.type: Easing.BezierSpline
                                easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                            }
                        }
                        Behavior on opacity { NumberAnimation { duration: 120 } }
                    }

                    MouseArea {
                        id: tileMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onEntered: appGrid.currentIndex = appCell.index
                        onClicked: root.launchApp(appCell.modelData)
                    }
                }
            }
        }

        // Minimal 3px scroll indicator
        Rectangle {
            readonly property real maxY: Math.max(0, appGrid.contentHeight - appGrid.height)
            visible: maxY > 1
            anchors.right: parent.right
            anchors.rightMargin: 1
            width: 3
            radius: 1.5
            color: Theme.accent
            opacity: 0.45
            height: appGrid.contentHeight > 0
                    ? Math.max(24, Math.round(parent.height * (appGrid.height / appGrid.contentHeight)))
                    : 24
            y: maxY > 0
               ? Math.round((parent.height - height) * (appGrid.contentY / maxY))
               : 0
        }
    }
}
