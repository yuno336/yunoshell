import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import ".."

Column {
    id: root
    width: parent.width
    spacing: 8

    signal closeRequested()
    signal externalDragChanged(bool active)

    property string currentDir: "/home/huyle"
    property var backStack: []
    property var forwardStack: []
    property bool showHidden: false
    property string filterQuery: ""
    property int selectedIndex: -1
    property string selectedName: ""
    property string selectedPath: ""
    property bool selectedIsDir: false
    property real selectedSize: 0
    property string statusToast: ""

    readonly property var defaultPlaces: [
        { label: "Home",       dir: "/home/huyle",                  icon: "󰋜" },
        { label: "Downloads",  dir: "/home/huyle/Downloads",        icon: "󰇚" },
        { label: "Pictures",   dir: "/home/huyle/Pictures",         icon: "󰸉" },
        { label: "Mango",      dir: "/home/huyle/.config/mango",    icon: "󰒓" },
        { label: "Quickshell", dir: "/home/huyle/.config/quickshell", icon: "󰕮" },
        { label: "Root /",     dir: "/",                            icon: "󰋊" }
    ]

    readonly property bool isDefaultPlace: {
        for (var i = 0; i < defaultPlaces.length; i++) {
            if (defaultPlaces[i].dir === root.currentDir) return true;
        }
        return false;
    }

    readonly property bool isPinned: Services.hasFileBookmark(root.currentDir)

    readonly property var breadcrumbs: {
        var d = root.currentDir || "/";
        var home = "/home/huyle";
        var segs = [];
        if (d === home || d.indexOf(home + "/") === 0) {
            segs.push({ label: "󰋜 ~", path: home });
            var rel = d.slice(home.length).replace(/^\/+/, "");
            if (rel.length > 0) {
                var parts = rel.split("/").filter(function(p) { return p.length > 0; });
                var acc = home;
                for (var i = 0; i < parts.length; i++) {
                    acc += "/" + parts[i];
                    segs.push({ label: parts[i], path: acc });
                }
            }
        } else {
            segs.push({ label: "/", path: "/" });
            var parts2 = d.replace(/^\/+/, "").split("/").filter(function(p) { return p.length > 0; });
            var acc2 = "";
            for (var j = 0; j < parts2.length; j++) {
                acc2 += "/" + parts2[j];
                segs.push({ label: parts2[j], path: acc2 });
            }
        }
        if (segs.length > 4) {
            return [
                segs[0],
                { label: "…", path: "" },
                segs[segs.length - 2],
                segs[segs.length - 1]
            ];
        }
        return segs;
    }

    function basename(path) {
        if (!path || path === "/") return "/";
        var clean = path.endsWith("/") ? path.slice(0, -1) : path;
        var idx = clean.lastIndexOf("/");
        return idx >= 0 ? clean.slice(idx + 1) : clean;
    }

    function isImageFile(name) {
        return /\.(png|jpe?g|webp|bmp|gif|svg|avif|ico)$/i.test(name || "");
    }

    function fileIcon(name) {
        var n = (name || "").toLowerCase();
        if (isImageFile(n)) return "󰸉";
        if (/\.(mp4|mkv|webm|avi|mov|flv)$/.test(n)) return "󰕧";
        if (/\.(mp3|flac|wav|ogg|m4a|opus)$/.test(n)) return "󰎆";
        if (/\.(zip|tar|gz|xz|zst|7z|rar)$/.test(n)) return "󰛫";
        if (/\.pdf$/.test(n)) return "󰈦";
        if (/\.(qml|js|ts|py|rs|c|cpp|h|sh)$/.test(n)) return "󰈮";
        if (/\.(json|conf|toml|yaml|yml|ini)$/.test(n)) return "󰒓";
        if (/\.(md|txt)$/.test(n)) return "󰈙";
        return "󰈔";
    }

    function fileIconColor(name, isDir) {
        if (isDir) return Theme.frostBlue;
        var n = (name || "").toLowerCase();
        if (isImageFile(n)) return Theme.purple;
        if (/\.(mp4|mkv|webm|avi|mov|mp3|flac|wav|ogg)$/.test(n)) return Theme.yellow;
        if (/\.(zip|tar|gz|xz|zst|7z|rar)$/.test(n)) return Theme.orange;
        if (/\.(qml|js|ts|py|rs|c|cpp|h|sh)$/.test(n)) return Theme.cyan;
        if (/\.(json|conf|toml|yaml|yml|ini)$/.test(n)) return Theme.green;
        if (/\.(pdf|md|txt)$/.test(n)) return Theme.frostTeal;
        return Theme.muted;
    }

    function formatSize(bytes) {
        if (bytes === undefined || bytes === null || bytes < 0) return "";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
        if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
        return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
    }

    function showToast(msg) {
        root.statusToast = msg;
        toastTimer.restart();
    }

    function clearSelection() {
        root.selectedIndex = -1;
        root.selectedName = "";
        root.selectedPath = "";
        root.selectedIsDir = false;
        root.selectedSize = 0;
    }

    function navigateTo(dir, recordHistory) {
        if (!dir || dir.length === 0) return;
        var clean = (dir.length > 1 && dir.endsWith("/")) ? dir.slice(0, -1) : dir;
        if (clean === root.currentDir) return;
        if (recordHistory !== false) {
            var b = root.backStack.slice();
            b.push(root.currentDir);
            root.backStack = b;
            root.forwardStack = [];
        }
        root.currentDir = clean;
        root.clearSelection();
        fileListView.contentY = 0;
    }

    function goBack() {
        if (root.backStack.length === 0) return;
        var b = root.backStack.slice();
        var prev = b.pop();
        var f = root.forwardStack.slice();
        f.push(root.currentDir);
        root.backStack = b;
        root.forwardStack = f;
        root.navigateTo(prev, false);
    }

    function goForward() {
        if (root.forwardStack.length === 0) return;
        var f = root.forwardStack.slice();
        var next = f.pop();
        var b = root.backStack.slice();
        b.push(root.currentDir);
        root.forwardStack = f;
        root.backStack = b;
        root.navigateTo(next, false);
    }

    function goUp() {
        if (root.currentDir === "/") return;
        var idx = root.currentDir.lastIndexOf("/");
        var parentDir = idx <= 0 ? "/" : root.currentDir.slice(0, idx);
        root.navigateTo(parentDir, true);
    }

    function selectEntry(idx, name, path, isDir, size) {
        root.selectedIndex = idx;
        root.selectedName = name;
        root.selectedPath = path;
        root.selectedIsDir = isDir;
        root.selectedSize = size;
    }

    function activateSelectedOrOpen() {
        if (root.selectedPath.length === 0) return;
        if (root.selectedIsDir) {
            root.navigateTo(root.selectedPath, true);
        } else {
            root.closeRequested();
            Quickshell.execDetached(["xdg-open", root.selectedPath]);
        }
    }

    function popOutToFilesR() {
        var args = ["/home/huyle/.local/bin/filesR"];
        if (root.selectedPath.length > 0 && !root.selectedIsDir) {
            args.push("--preselect");
            args.push(root.selectedPath);
            args.push(root.currentDir);
        } else if (root.selectedPath.length > 0 && root.selectedIsDir) {
            args.push(root.selectedPath);
        } else {
            args.push(root.currentDir);
        }
        root.closeRequested();
        Quickshell.execDetached(args);
    }

    function focusDefault() {
        root.forceActiveFocus();
    }

    Timer {
        id: toastTimer
        interval: 2200
        repeat: false
        onTriggered: root.statusToast = ""
    }

    FolderListModel {
        id: folderModel
        folder: "file://" + root.currentDir
        nameFilters: root.filterQuery.trim().length > 0 ? ["*" + root.filterQuery.trim() + "*"] : ["*"]
        showDirs: true
        showDirsFirst: true
        showFiles: true
        showHidden: root.showHidden
        sortField: FolderListModel.Name
    }

    Keys.onPressed: event => {
        if (filterInput.activeFocus) return;
        if (event.key === Qt.Key_Down || event.key === Qt.Key_J) {
            if (folderModel.count > 0) {
                var next = Math.min(folderModel.count - 1, Math.max(0, root.selectedIndex + 1));
                var fp = folderModel.get(next, "filePath");
                root.selectEntry(next, folderModel.get(next, "fileName"), fp, folderModel.get(next, "fileIsDir"), folderModel.get(next, "fileSize"));
                fileListView.positionViewAtIndex(next, ListView.Contain);
            }
            event.accepted = true;
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_K) {
            if (folderModel.count > 0) {
                var prev = Math.max(0, root.selectedIndex <= 0 ? 0 : root.selectedIndex - 1);
                var fp2 = folderModel.get(prev, "filePath");
                root.selectEntry(prev, folderModel.get(prev, "fileName"), fp2, folderModel.get(prev, "fileIsDir"), folderModel.get(prev, "fileSize"));
                fileListView.positionViewAtIndex(prev, ListView.Contain);
            }
            event.accepted = true;
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.activateSelectedOrOpen();
            event.accepted = true;
        } else if (event.key === Qt.Key_Backspace) {
            root.goUp();
            event.accepted = true;
        } else if (event.key === Qt.Key_Slash || (event.modifiers & Qt.ControlModifier && event.key === Qt.Key_F)) {
            filterInput.forceActiveFocus();
            event.accepted = true;
        } else if (event.key === Qt.Key_Escape) {
            if (root.filterQuery.length > 0) {
                filterInput.text = "";
            } else {
                root.closeRequested();
            }
            event.accepted = true;
        }
    }

    // ==================== TOP NAVIGATION & BREADCRUMB BAR ====================
    Row {
        width: parent.width
        height: 32
        spacing: 6

        // Nav Segmented Pill
        Rectangle {
            height: 32
            width: navRow.implicitWidth + 8
            radius: 9
            color: Theme.surface
            border.width: 1
            border.color: Theme.border

            Row {
                id: navRow
                anchors.centerIn: parent
                spacing: 2

                Repeater {
                    model: [
                        { icon: "󰁍", enabled: root.backStack.length > 0, act: "back" },
                        { icon: "󰁔", enabled: root.forwardStack.length > 0, act: "fwd" },
                        { icon: "󰁞", enabled: root.currentDir !== "/", act: "up" },
                        { icon: "󰑐", enabled: true, act: "refresh" }
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 26
                        height: 24
                        radius: 7
                        opacity: modelData.enabled ? 1.0 : 0.35
                        color: navMouse.containsMouse && modelData.enabled ? Theme.surfaceHover : "transparent"

                        Text {
                            anchors.centerIn: parent
                            text: modelData.icon
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            color: Theme.text
                        }

                        MouseArea {
                            id: navMouse
                            anchors.fill: parent
                            enabled: modelData.enabled
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (modelData.act === "back") root.goBack();
                                else if (modelData.act === "fwd") root.goForward();
                                else if (modelData.act === "up") root.goUp();
                                else {
                                    var cur = root.currentDir;
                                    folderModel.folder = "";
                                    folderModel.folder = "file://" + cur;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Clickable Breadcrumbs Pill
        Rectangle {
            height: 32
            width: Math.max(180, parent.width - 116 - 140 - 66 - 92 - 24)
            radius: 9
            color: Theme.surface
            border.width: 1
            border.color: Theme.border
            clip: true

            Row {
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: 8
                spacing: 2

                Repeater {
                    model: root.breadcrumbs
                    delegate: Row {
                        required property var modelData
                        required property int index
                        spacing: 2

                        Text {
                            visible: index > 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: "›"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.muted
                        }

                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            height: 24
                            width: Math.min(crumbText.implicitWidth + 12, 140)
                            radius: 6
                            color: (crumbMouse.containsMouse && modelData.path.length > 0) ? Theme.surfaceHover : "transparent"

                            Text {
                                id: crumbText
                                anchors.centerIn: parent
                                width: Math.min(implicitWidth, 128)
                                text: modelData.label
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                font.weight: (index === root.breadcrumbs.length - 1) ? Font.DemiBold : Font.Normal
                                color: (index === root.breadcrumbs.length - 1) ? Theme.textBright : Theme.muted
                                elide: Text.ElideRight
                            }

                            MouseArea {
                                id: crumbMouse
                                anchors.fill: parent
                                enabled: modelData.path.length > 0
                                hoverEnabled: true
                                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                                onClicked: root.navigateTo(modelData.path, true)
                            }
                        }
                    }
                }
            }
        }

        // Quick Filter Box
        Rectangle {
            width: 140
            height: 32
            radius: 9
            color: Theme.surface
            border.width: 1
            border.color: filterInput.activeFocus ? Theme.accent : Theme.border

            Row {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 6
                spacing: 5

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "󰍉"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    color: filterInput.activeFocus ? Theme.accent : Theme.muted
                }

                Item {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - (filterInput.text.length > 0 ? 38 : 20)
                    height: 20
                    clip: true

                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        text: "Filter..."
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        color: Theme.muted
                        visible: filterInput.text.length === 0
                    }

                    TextInput {
                        id: filterInput
                        anchors.fill: parent
                        verticalAlignment: TextInput.AlignVCenter
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        color: Theme.textBright
                        selectionColor: Theme.accent
                        selectedTextColor: Theme.accentText
                        clip: true
                        onTextChanged: root.filterQuery = text
                        Keys.onEscapePressed: event => {
                            if (text.length > 0) text = "";
                            else root.focusDefault();
                            event.accepted = true;
                        }
                    }
                }

                Text {
                    visible: filterInput.text.length > 0
                    anchors.verticalCenter: parent.verticalCenter
                    text: "󰅖"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.muted
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: filterInput.text = ""
                    }
                }
            }
        }

        // Hidden + Pin Toggle Group
        Rectangle {
            width: 66
            height: 32
            radius: 9
            color: Theme.surface
            border.width: 1
            border.color: Theme.border

            Row {
                anchors.centerIn: parent
                spacing: 4

                Rectangle {
                    width: 26
                    height: 24
                    radius: 7
                    color: root.showHidden ? Theme.accentSoft : (hidMouse.containsMouse ? Theme.surfaceHover : "transparent")
                    border.width: root.showHidden ? 1 : 0
                    border.color: Theme.accent

                    Text {
                        anchors.centerIn: parent
                        text: ".*"
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: Font.Bold
                        color: root.showHidden ? Theme.accent : Theme.muted
                    }

                    MouseArea {
                        id: hidMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.showHidden = !root.showHidden
                    }
                }

                Rectangle {
                    width: 26
                    height: 24
                    radius: 7
                    opacity: root.isDefaultPlace ? 0.35 : 1.0
                    color: root.isPinned ? Theme.accentSoft : (pinMouse.containsMouse && !root.isDefaultPlace ? Theme.surfaceHover : "transparent")

                    Text {
                        anchors.centerIn: parent
                        text: "󰐃"
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        color: root.isPinned ? Theme.accent : Theme.muted
                    }

                    MouseArea {
                        id: pinMouse
                        anchors.fill: parent
                        enabled: !root.isDefaultPlace
                        hoverEnabled: true
                        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: {
                            if (root.isPinned) {
                                Services.removeFileBookmark(root.currentDir);
                                root.showToast("Unpinned folder");
                            } else {
                                Services.addFileBookmark(root.currentDir);
                                root.showToast("Pinned to Places");
                            }
                        }
                    }
                }
            }
        }

        // Pop out to full GTK3 filesR window
        Rectangle {
            width: 92
            height: 32
            radius: 9
            color: popMouse.containsMouse ? Theme.accent : Theme.accentSoft
            border.width: 1
            border.color: Theme.accent

            Row {
                anchors.centerIn: parent
                spacing: 5

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "󰏋"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    color: popMouse.containsMouse ? Theme.accentText : Theme.accent
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "filesR"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                    color: popMouse.containsMouse ? Theme.accentText : Theme.accent
                }
            }

            MouseArea {
                id: popMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.popOutToFilesR()
            }
        }
    }

    // ==================== 3-PANE WORKSPACE ====================
    Row {
        width: parent.width
        height: 324
        spacing: 8

        // Left Places + Pinned Card
        Rectangle {
            width: 158
            height: parent.height
            radius: 12
            color: Theme.surface
            border.width: 1
            border.color: Theme.border
            clip: true

            Flickable {
                anchors.fill: parent
                anchors.margins: 6
                contentHeight: placesCol.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: placesCol
                    width: parent.width
                    spacing: 2

                    Repeater {
                        model: root.defaultPlaces
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool active: root.currentDir === modelData.dir
                            width: parent.width
                            height: 28
                            radius: 7
                            color: active ? Theme.accentSoft : (pMouse.containsMouse ? Theme.surfaceHover : "transparent")
                            border.width: active ? 1 : 0
                            border.color: Theme.accent

                            Row {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                spacing: 7

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.icon
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 13
                                    color: active ? Theme.accent : Theme.frostBlue
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - 22
                                    text: modelData.label
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.weight: active ? Font.DemiBold : Font.Normal
                                    color: active ? Theme.accent : Theme.text
                                    elide: Text.ElideRight
                                }
                            }

                            MouseArea {
                                id: pMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.navigateTo(modelData.dir, true)
                            }
                        }
                    }

                    Rectangle {
                        width: parent.width - 8
                        height: 1
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: Theme.border
                        visible: Services.fileBookmarks.length > 0
                    }

                    Text {
                        visible: Services.fileBookmarks.length > 0
                        text: "PINNED"
                        font.family: Theme.fontFamily
                        font.pixelSize: 9
                        font.weight: Font.Bold
                        color: Theme.muted
                        leftPadding: 8
                        topPadding: 4
                        bottomPadding: 2
                    }

                    Repeater {
                        model: Services.fileBookmarks
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool active: root.currentDir === modelData.dir
                            width: parent.width
                            height: 28
                            radius: 7
                            color: active ? Theme.accentSoft : (bmMouse.containsMouse ? Theme.surfaceHover : "transparent")
                            border.width: active ? 1 : 0
                            border.color: Theme.accent

                            Row {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 6
                                spacing: 6

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.icon || "󰉋"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 12
                                    color: Theme.green
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - 36
                                    text: modelData.label
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.weight: active ? Font.DemiBold : Font.Normal
                                    color: active ? Theme.accent : Theme.text
                                    elide: Text.ElideRight
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "󰅖"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 10
                                    color: unpinMouse.containsMouse ? Theme.red : Theme.muted
                                    MouseArea {
                                        id: unpinMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: Services.removeFileBookmark(modelData.dir)
                                    }
                                }
                            }

                            MouseArea {
                                id: bmMouse
                                anchors.fill: parent
                                anchors.rightMargin: 18
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.navigateTo(modelData.dir, true)
                            }
                        }
                    }
                }
            }
        }

        // Center File List Card
        Rectangle {
            width: parent.width - 158 - 224 - 16
            height: parent.height
            radius: 12
            color: Theme.surface
            border.width: 1
            border.color: Theme.border
            clip: true

            Column {
                anchors.fill: parent

                // Header
                Rectangle {
                    width: parent.width
                    height: 26
                    color: "transparent"

                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 14
                        spacing: 8

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 68
                            text: "NAME"
                            font.family: Theme.fontFamily
                            font.pixelSize: 9
                            font.weight: Font.Bold
                            color: Theme.muted
                            leftPadding: 24
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: 60
                            horizontalAlignment: Text.AlignRight
                            text: "SIZE"
                            font.family: Theme.fontFamily
                            font.pixelSize: 9
                            font.weight: Font.Bold
                            color: Theme.muted
                        }
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: Theme.border
                        opacity: 0.6
                    }
                }

                // File List
                Item {
                    width: parent.width
                    height: parent.height - 26

                    Text {
                        anchors.centerIn: parent
                        visible: folderModel.count === 0
                        text: root.filterQuery.length > 0 ? "No matching files" : "Empty folder"
                        font.family: Theme.fontFamily
                        font.pixelSize: 11
                        color: Theme.muted
                    }

                    ListView {
                        id: fileListView
                        anchors.fill: parent
                        anchors.margins: 4
                        anchors.rightMargin: 8
                        model: folderModel
                        clip: true
                        spacing: 2
                        boundsBehavior: Flickable.StopAtBounds

                        delegate: Rectangle {
                            id: fRow
                            required property int index
                            required property string fileName
                            required property string filePath
                            required property bool fileIsDir
                            required property real fileSize
                            readonly property bool selected: root.selectedPath === filePath

                            width: fileListView.width
                            height: 30
                            radius: 7
                            color: selected ? Theme.accentSoft : (rowMouse.containsMouse ? Theme.surfaceHover : "transparent")
                            border.width: selected ? 1 : 0
                            border.color: Theme.accent

                            Row {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                spacing: 8

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 18
                                    horizontalAlignment: Text.AlignHCenter
                                    text: fileIsDir ? "󰉋" : root.fileIcon(fileName)
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 14
                                    color: root.fileIconColor(fileName, fileIsDir)
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - 92
                                    text: fileName
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.weight: fileIsDir ? Font.DemiBold : Font.Normal
                                    color: fileIsDir ? Theme.textBright : Theme.text
                                    elide: Text.ElideRight
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 58
                                    horizontalAlignment: Text.AlignRight
                                    text: fileIsDir ? "" : root.formatSize(fileSize)
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 10
                                    color: Theme.muted
                                }
                            }

                            Item {
                                id: dragProxy
                                width: 1
                                height: 1
                                Drag.active: rowMouse.drag.active
                                Drag.dragType: Drag.Automatic
                                Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
                                Drag.mimeData: ({
                                    "text/uri-list": "file://" + encodeURI(fRow.filePath) + "\r\n",
                                    "text/plain": fRow.filePath
                                })
                                Drag.onDragStarted: {
                                    root.selectEntry(fRow.index, fRow.fileName, fRow.filePath, fRow.fileIsDir, fRow.fileSize);
                                    root.externalDragChanged(true);
                                    root.showToast("Dragging " + fRow.fileName);
                                }
                                Drag.onDragFinished: dropAction => {
                                    dragProxy.x = 0;
                                    dragProxy.y = 0;
                                    root.externalDragChanged(false);
                                }
                            }

                            MouseArea {
                                id: rowMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                drag.target: dragProxy
                                drag.threshold: 8
                                onClicked: root.selectEntry(index, fileName, filePath, fileIsDir, fileSize)
                                onDoubleClicked: {
                                    root.selectEntry(index, fileName, filePath, fileIsDir, fileSize);
                                    root.activateSelectedOrOpen();
                                }
                            }
                        }
                    }

                    // Minimal 3px Scroll Indicator
                    Rectangle {
                        readonly property real maxY: Math.max(0, fileListView.contentHeight - fileListView.height)
                        visible: maxY > 2
                        anchors.right: parent.right
                        anchors.rightMargin: 2
                        width: 3
                        radius: 1.5
                        color: Theme.accent
                        opacity: 0.45
                        height: fileListView.contentHeight > 0
                                ? Math.max(20, Math.round(parent.height * (fileListView.height / fileListView.contentHeight)))
                                : 20
                        y: maxY > 0
                           ? Math.round((parent.height - height) * (fileListView.contentY / maxY))
                           : 0
                    }
                }
            }
        }

        // Right Preview & Quick Actions Card
        Rectangle {
            width: 224
            height: parent.height
            radius: 12
            color: Theme.surface
            border.width: 1
            border.color: Theme.border
            clip: true

            Column {
                anchors.fill: parent
                anchors.margins: 10
                spacing: 8

                // Preview Box
                Rectangle {
                    width: parent.width
                    height: 128
                    radius: 10
                    color: Theme.bgBase
                    border.width: 1
                    border.color: Theme.border
                    clip: true

                    readonly property bool showImg: root.selectedPath.length > 0 && !root.selectedIsDir && root.isImageFile(root.selectedName)

                    Image {
                        anchors.fill: parent
                        anchors.margins: 6
                        visible: parent.showImg
                        source: parent.showImg ? ("file://" + root.selectedPath) : ""
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                    }

                    Rectangle {
                        anchors.centerIn: parent
                        visible: !parent.showImg
                        width: 62
                        height: 54
                        radius: 14
                        color: Theme.accentSoft
                        border.width: 1
                        border.color: Theme.accent

                        Text {
                            anchors.centerIn: parent
                            text: root.selectedPath.length === 0 || root.selectedIsDir
                                  ? "󰉋"
                                  : root.fileIcon(root.selectedName)
                            font.family: Theme.fontFamily
                            font.pixelSize: 26
                            color: root.selectedPath.length === 0
                                   ? Theme.frostBlue
                                   : root.fileIconColor(root.selectedName, root.selectedIsDir)
                        }
                    }
                }

                // Selected Name + Meta
                Column {
                    width: parent.width
                    spacing: 2

                    Text {
                        width: parent.width
                        text: root.selectedName.length > 0 ? root.selectedName : root.basename(root.currentDir)
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: Font.Bold
                        color: Theme.textBright
                        elide: Text.ElideMiddle
                    }

                    Text {
                        width: parent.width
                        text: root.statusToast.length > 0
                              ? root.statusToast
                              : (root.selectedPath.length === 0
                                 ? (folderModel.count + " items in folder")
                                 : (root.selectedIsDir ? "Folder" : root.formatSize(root.selectedSize)))
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        color: root.statusToast.length > 0 ? Theme.green : Theme.muted
                        elide: Text.ElideRight
                    }
                }

                // Quick Actions 2x2 + Wallpaper
                Grid {
                    width: parent.width
                    columns: 2
                    rowSpacing: 6
                    columnSpacing: 6

                    Repeater {
                        model: [
                            { icon: "󰏌", label: "Open",     act: "open",  en: root.selectedPath.length > 0, accent: true },
                            { icon: "󰨞", label: "Codium",   act: "code",  en: true, accent: false },
                            { icon: "󰆍", label: "Terminal", act: "term",  en: true, accent: false },
                            { icon: "󰅌", label: "CopyPath", act: "copy",  en: true, accent: false }
                        ]
                        delegate: Rectangle {
                            required property var modelData
                            width: (parent.width - 6) / 2
                            height: 28
                            radius: 7
                            opacity: modelData.en ? 1.0 : 0.4
                            color: modelData.accent
                                   ? Theme.accentSoft
                                   : (actMouse.containsMouse && modelData.en ? Theme.surfaceHover : Theme.bgBase)
                            border.width: 1
                            border.color: modelData.accent ? Theme.accent : Theme.border

                            Row {
                                anchors.centerIn: parent
                                spacing: 5

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.icon
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    color: modelData.accent ? Theme.accent : Theme.text
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.label
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 10
                                    font.weight: modelData.accent ? Font.DemiBold : Font.Normal
                                    color: modelData.accent ? Theme.accent : Theme.text
                                }
                            }

                            MouseArea {
                                id: actMouse
                                anchors.fill: parent
                                enabled: modelData.en
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    var target = root.selectedPath.length > 0 ? root.selectedPath : root.currentDir;
                                    if (modelData.act === "open") {
                                        root.activateSelectedOrOpen();
                                    } else if (modelData.act === "code") {
                                        root.closeRequested();
                                        Quickshell.execDetached(["codium", target]);
                                    } else if (modelData.act === "term") {
                                        var termDir = (root.selectedPath.length > 0 && root.selectedIsDir) ? root.selectedPath : root.currentDir;
                                        root.closeRequested();
                                        Quickshell.execDetached(["kitty", "--directory", termDir]);
                                    } else if (modelData.act === "copy") {
                                        Quickshell.execDetached(["wl-copy", target]);
                                        root.showToast("Copied path");
                                    }
                                }
                            }
                        }
                    }
                }

                // Set Wallpaper CTA (full width, highlighted cleanly when an image is selected)
                Rectangle {
                    readonly property bool isImg: root.selectedPath.length > 0 && !root.selectedIsDir && root.isImageFile(root.selectedName)
                    width: parent.width
                    height: 28
                    radius: 7
                    opacity: isImg ? 1.0 : 0.38
                    color: wallMouse.containsMouse && isImg ? Theme.accent : Theme.bgBase
                    border.width: 1
                    border.color: isImg ? Theme.accent : Theme.border

                    Row {
                        anchors.centerIn: parent
                        spacing: 6

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "󰸉"
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            color: wallMouse.containsMouse && parent.parent.isImg ? Theme.accentText : (parent.parent.isImg ? Theme.accent : Theme.muted)
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Set as Wallpaper"
                            font.family: Theme.fontFamily
                            font.pixelSize: 10
                            font.weight: Font.DemiBold
                            color: wallMouse.containsMouse && parent.parent.isImg ? Theme.accentText : (parent.parent.isImg ? Theme.accent : Theme.muted)
                        }
                    }

                    MouseArea {
                        id: wallMouse
                        anchors.fill: parent
                        enabled: parent.isImg
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Services.applyWallpaper(root.selectedPath, Services.wallpaperMode);
                            root.showToast("Wallpaper updated");
                        }
                    }
                }
            }
        }
    }
}
