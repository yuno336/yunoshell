import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import ".."

Item {
    id: root
    anchors.fill: parent
    visible: isStandalonePortal ? true : (opened || animProgress > 0.01)

    property bool isStandalonePortal: false
    property bool opened: false
    property string title: "Select Image"
    property string appBadge: ""
    property string mode: "open" // "open" | "directory" | "save" | "save_files"
    property bool multiple: false
    property string currentDir: Services.wallpaperDir
    property string selectedFile: ""
    property string selectedFolder: ""
    property var selectedFiles: []
    property string saveFileName: ""
    property string acceptLabel: ""
    property bool showHidden: false
    property bool showNewFolderBar: false
    property real animProgress: 0.0

    property var filters: [{ label: "Images", globs: ["*.png", "*.jpg", "*.jpeg", "*.webp", "*.bmp", "*.PNG", "*.JPG", "*.JPEG", "*.WEBP", "*.BMP"] }]
    property int activeFilterIdx: 0

    readonly property var activeGlobs: (filters && activeFilterIdx >= 0 && activeFilterIdx < filters.length)
                                     ? filters[activeFilterIdx].globs
                                     : ["*"]

    signal acceptedPaths(var paths, int filterIndex)
    signal accepted(string filePath)
    signal cancelled()

    function open(startDir, dialogTitle) {
        root.mode = "open";
        root.multiple = false;
        root.appBadge = "";
        root.acceptLabel = "Select";
        root.filters = [
            { label: "Images", globs: ["*.png", "*.jpg", "*.jpeg", "*.webp", "*.bmp", "*.PNG", "*.JPG", "*.JPEG", "*.WEBP", "*.BMP"] },
            { label: "All Files", globs: ["*"] }
        ];
        root.activeFilterIdx = 0;
        root.saveFileName = "";
        root.selectedFile = "";
        root.selectedFolder = "";
        root.selectedFiles = [];
        root.showNewFolderBar = false;
        if (dialogTitle && dialogTitle.length > 0) root.title = dialogTitle;
        if (startDir && startDir.length > 0) root.currentDir = startDir;
        root.opened = true;
        closeAnim.stop();
        openAnim.restart();
        root.forceActiveFocus();
    }

    function openPortalRequest(req) {
        root.mode = req.mode || "open";
        root.multiple = !!req.multiple;
        root.appBadge = req.app_id || "";
        root.title = req.title || (root.mode === "directory" ? "Select Folder" : (root.mode === "save" ? "Save File" : "Select File"));
        root.currentDir = req.current_dir || "/home/huyle";
        root.saveFileName = req.current_name || "";
        root.acceptLabel = req.accept_label || (root.mode === "save" ? "Save" : (root.mode === "directory" || root.mode === "save_files" ? "Select Folder" : "Open"));
        root.filters = (req.filters && req.filters.length > 0) ? req.filters : [{ label: "All Files", globs: ["*"] }];
        root.activeFilterIdx = (req.active_filter_idx !== undefined && req.active_filter_idx < root.filters.length) ? req.active_filter_idx : 0;
        root.selectedFile = "";
        root.selectedFolder = "";
        root.selectedFiles = [];
        root.showNewFolderBar = false;
        root.opened = true;
        if (root.mode === "save" && saveNameInput) {
            saveNameInput.forceActiveFocus();
        } else {
            root.forceActiveFocus();
        }
    }

    function close() {
        root.opened = false;
        root.showNewFolderBar = false;
        if (!isStandalonePortal) {
            openAnim.stop();
            closeAnim.restart();
        }
    }

    function goUp() {
        var d = root.currentDir;
        if (d.length > 1 && d.endsWith("/")) d = d.slice(0, -1);
        var idx = d.lastIndexOf("/");
        root.currentDir = (idx <= 0) ? "/" : d.slice(0, idx);
        root.selectedFolder = "";
        fileList.contentY = 0;
    }

    function basename(path) {
        if (!path) return "";
        var idx = path.lastIndexOf("/");
        return idx >= 0 ? path.slice(idx + 1) : path;
    }

    function isImageFile(name) {
        return /\.(png|jpe?g|webp|bmp|gif|svg|avif|ico)$/i.test(name || "");
    }

    function fileIcon(name) {
        var n = (name || "").toLowerCase();
        if (/\.(mp4|mkv|webm|avi|mov|flv|ts)$/.test(n)) return "󰕧";
        if (/\.(mp3|flac|wav|ogg|m4a|opus)$/.test(n)) return "󰎆";
        if (/\.pdf$/.test(n)) return "󰈦";
        if (/\.(zip|tar|gz|xz|zst|7z|rar)$/.test(n)) return "󰗄";
        if (/\.(json|yaml|yml|toml|conf|ini|sh|py|qml|js|ts|rs|c|cpp|h|md|txt|html|css)$/.test(n)) return "󰈙";
        return "󰈔";
    }

    function formatSize(bytes) {
        if (bytes === undefined || bytes === null || bytes < 0) return "";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
        if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
        return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
    }

    function joinPath(dir, name) {
        if (!dir || dir === "/") return "/" + name;
        return dir.endsWith("/") ? (dir + name) : (dir + "/" + name);
    }

    function canConfirm() {
        if (root.mode === "directory" || root.mode === "save_files") return root.currentDir.length > 0;
        if (root.mode === "save") return root.saveFileName.trim().length > 0;
        if (root.multiple) return root.selectedFiles.length > 0 || root.selectedFile.length > 0;
        return root.selectedFile.length > 0;
    }

    function confirmSelection() {
        if (!canConfirm()) return;
        var out = [];
        if (root.mode === "directory" || root.mode === "save_files") {
            out = [root.selectedFolder.length > 0 ? root.selectedFolder : root.currentDir];
        } else if (root.mode === "save") {
            out = [joinPath(root.currentDir, root.saveFileName.trim())];
        } else if (root.multiple) {
            out = root.selectedFiles.length > 0 ? root.selectedFiles.slice() : [root.selectedFile];
        } else {
            out = [root.selectedFile];
        }
        var fIdx = root.activeFilterIdx;
        root.close();
        if (out.length === 1) root.accepted(out[0]);
        root.acceptedPaths(out, fIdx);
    }

    function createFolder(name) {
        var trimmed = name.trim();
        if (trimmed.length === 0 || trimmed.indexOf("/") >= 0) return;
        var newPath = joinPath(root.currentDir, trimmed);
        Quickshell.execDetached(["mkdir", "-p", newPath]);
        root.showNewFolderBar = false;
        newFolderInput.text = "";
        newDirDelay.targetDir = newPath;
        newDirDelay.restart();
    }

    Timer {
        id: newDirDelay
        property string targetDir: ""
        interval: 120
        repeat: false
        onTriggered: {
            if (targetDir.length > 0) {
                root.currentDir = targetDir;
                root.selectedFolder = "";
                fileList.contentY = 0;
            }
        }
    }

    NumberAnimation {
        id: openAnim
        target: root
        property: "animProgress"
        to: 1.0
        duration: 160
        easing.type: Easing.OutCubic
    }

    NumberAnimation {
        id: closeAnim
        target: root
        property: "animProgress"
        to: 0.0
        duration: 120
        easing.type: Easing.InCubic
    }

    FolderListModel {
        id: browserModel
        folder: "file://" + root.currentDir
        nameFilters: root.activeGlobs
        showDirs: true
        showDirsFirst: true
        showFiles: root.mode !== "directory" && root.mode !== "save_files"
        showHidden: root.showHidden
        sortField: FolderListModel.Name
    }

    Keys.onEscapePressed: event => {
        event.accepted = true;
        if (root.showNewFolderBar) {
            root.showNewFolderBar = false;
            return;
        }
        root.close();
        root.cancelled();
    }

    // Inline Scrim (disabled when hosted in PortalFileDialog's PanelWindow)
    Rectangle {
        visible: !root.isStandalonePortal
        anchors.fill: parent
        color: Qt.rgba(0.11, 0.13, 0.16, 0.72 * root.animProgress)

        MouseArea {
            anchors.fill: parent
            onClicked: {
                root.close();
                root.cancelled();
            }
        }
    }

    // Modal Card
    Rectangle {
        anchors.fill: root.isStandalonePortal ? parent : undefined
        width: root.isStandalonePortal ? parent.width : parent.width - 36
        height: root.isStandalonePortal ? parent.height : parent.height - 36
        anchors.centerIn: root.isStandalonePortal ? undefined : parent
        color: Theme.popupBg
        border.width: 1
        border.color: Theme.border
        opacity: root.isStandalonePortal ? 1.0 : root.animProgress
        scale: root.isStandalonePortal ? 1.0 : (0.95 + 0.05 * root.animProgress)

        MouseArea {
            anchors.fill: parent
        }

        // Top Bar
        Item {
            id: pickerHeader
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 44

            Row {
                anchors.left: parent.left
                anchors.leftMargin: 14
                anchors.right: headerBtns.left
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.mode === "save" ? "󰆓" : "󰉋"
                    font.family: Theme.fontFamily
                    font.pixelSize: 16
                    color: Theme.accent
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, parent.width - 120)
                    elide: Text.ElideRight
                    text: root.title
                    font.family: Theme.fontFamily
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    color: Theme.textBright
                }

                Rectangle {
                    visible: root.appBadge.length > 0
                    anchors.verticalCenter: parent.verticalCenter
                    width: appBadgeTxt.implicitWidth + 10
                    height: 18
                    color: Theme.surfaceHover

                    Text {
                        id: appBadgeTxt
                        anchors.centerIn: parent
                        text: root.appBadge
                        font.family: Theme.fontFamily
                        font.pixelSize: 9
                        color: Theme.muted
                    }
                }
            }

            Row {
                id: headerBtns
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6

                // New folder
                Rectangle {
                    width: 30
                    height: 26
                    color: root.showNewFolderBar ? Theme.accent : (mkMouse.containsMouse ? Theme.surfaceHover : Theme.surface)

                    Text {
                        anchors.centerIn: parent
                        text: "󰝰"
                        font.family: Theme.fontFamily
                        font.pixelSize: 13
                        color: root.showNewFolderBar ? Theme.accentText : Theme.text
                    }

                    MouseArea {
                        id: mkMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.showNewFolderBar = !root.showNewFolderBar;
                            if (root.showNewFolderBar) newFolderInput.forceActiveFocus();
                        }
                    }
                }

                // Up directory
                Rectangle {
                    width: 60
                    height: 26
                    color: upMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                    Text {
                        anchors.centerIn: parent
                        text: "󰁞 Up"
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        color: Theme.text
                    }

                    MouseArea {
                        id: upMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.goUp()
                    }
                }

                // Hidden toggle
                Rectangle {
                    width: 32
                    height: 26
                    color: root.showHidden ? Theme.accent : (hidMouse.containsMouse ? Theme.surfaceHover : Theme.surface)

                    Text {
                        anchors.centerIn: parent
                        text: ".*"
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: Font.Bold
                        color: root.showHidden ? Theme.accentText : Theme.muted
                    }

                    MouseArea {
                        id: hidMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.showHidden = !root.showHidden
                    }
                }

                // Close
                Rectangle {
                    width: 26
                    height: 26
                    color: closePickerMouse.containsMouse ? Theme.surfaceHover : "transparent"

                    Text {
                        anchors.centerIn: parent
                        text: "󰅖"
                        font.family: Theme.fontFamily
                        font.pixelSize: 13
                        color: Theme.muted
                    }

                    MouseArea {
                        id: closePickerMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.close();
                            root.cancelled();
                        }
                    }
                }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 1
                color: Theme.border
            }
        }

        // Quick Places + Path Bar
        Row {
            id: pathRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pickerHeader.bottom
            anchors.margins: 12
            height: 28
            spacing: 6

            Repeater {
                model: [
                    { label: "~", dir: "/home/huyle" },
                    { label: "Pictures", dir: "/home/huyle/Pictures" },
                    { label: "Downloads", dir: "/home/huyle/Downloads" },
                    { label: "Mango", dir: "/home/huyle/.config/mango" }
                ]

                Rectangle {
                    required property var modelData
                    readonly property bool isHere: root.currentDir === modelData.dir
                    width: chipLbl.implicitWidth + 14
                    height: 28
                    color: isHere ? Theme.accent : (chipMouse.containsMouse ? Theme.surfaceHover : Theme.surface)

                    Text {
                        id: chipLbl
                        anchors.centerIn: parent
                        text: modelData.label
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: isHere ? Font.Bold : Font.Normal
                        color: isHere ? Theme.accentText : Theme.text
                    }

                    MouseArea {
                        id: chipMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.currentDir = modelData.dir;
                            root.selectedFolder = "";
                            fileList.contentY = 0;
                        }
                    }
                }
            }

            Rectangle {
                width: parent.width - 232
                height: 28
                color: Theme.surface
                border.width: 1
                border.color: pathInput.activeFocus ? Theme.accent : Theme.border

                TextInput {
                    id: pathInput
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    verticalAlignment: TextInput.AlignVCenter
                    text: root.currentDir
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.text
                    clip: true
                    onAccepted: {
                        var t = text.trim();
                        if (t.length > 0) {
                            root.currentDir = t;
                            root.selectedFolder = "";
                            fileList.contentY = 0;
                            root.forceActiveFocus();
                        }
                    }
                }
            }
        }

        // Optional New Folder Bar
        Row {
            id: newFolderRow
            visible: root.showNewFolderBar
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pathRow.bottom
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            height: visible ? 28 : 0
            spacing: 6

            Rectangle {
                width: parent.width - 96
                height: 28
                color: Theme.surface
                border.width: 1
                border.color: Theme.accent

                TextInput {
                    id: newFolderInput
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    verticalAlignment: TextInput.AlignVCenter
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.textBright
                    clip: true
                    onAccepted: root.createFolder(text)
                }

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    visible: newFolderInput.text.length === 0
                    text: "New folder name..."
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                }
            }

            Rectangle {
                width: 90
                height: 28
                color: createBtnMouse.containsMouse ? Theme.accent : Theme.surfaceHover

                Text {
                    anchors.centerIn: parent
                    text: "Create"
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    font.weight: Font.Bold
                    color: createBtnMouse.containsMouse ? Theme.accentText : Theme.text
                }

                MouseArea {
                    id: createBtnMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.createFolder(newFolderInput.text)
                }
            }
        }

        // Optional Save File Name Bar
        Row {
            id: saveNameRow
            visible: root.mode === "save"
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: root.showNewFolderBar ? newFolderRow.bottom : pathRow.bottom
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            height: visible ? 30 : 0
            spacing: 8

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Save as:"
                font.family: Theme.fontFamily
                font.pixelSize: 10
                color: Theme.muted
            }

            Rectangle {
                width: parent.width - 66
                height: 30
                color: Theme.surface
                border.width: 1
                border.color: saveNameInput.activeFocus ? Theme.accent : Theme.border

                TextInput {
                    id: saveNameInput
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    verticalAlignment: TextInput.AlignVCenter
                    text: root.saveFileName
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    color: Theme.textBright
                    clip: true
                    onTextChanged: root.saveFileName = text
                    onAccepted: root.confirmSelection()
                }
            }
        }

        // Split Body: Folder/File List (left) + Preview & Actions (right)
        Item {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: root.mode === "save"
                         ? saveNameRow.bottom
                         : (root.showNewFolderBar ? newFolderRow.bottom : pathRow.bottom)
            anchors.bottom: parent.bottom
            anchors.margins: 12
            anchors.topMargin: 10

            // Left File List
            Rectangle {
                id: listWrap
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.right: previewPane.left
                anchors.rightMargin: 12
                color: "#282d38"
                border.width: 1
                border.color: Theme.border
                clip: true

                ListView {
                    id: fileList
                    anchors.fill: parent
                    anchors.margins: 4
                    model: browserModel
                    spacing: 2
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    WheelHandler {
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onWheel: event => {
                            var maxY = Math.max(0, fileList.contentHeight - fileList.height);
                            var step = event.pixelDelta.y !== 0
                                ? -event.pixelDelta.y * 1.35
                                : -(event.angleDelta.y / 120.0) * 44.0;
                            fileList.contentY = Math.max(0, Math.min(maxY, fileList.contentY + step));
                        }
                    }

                    delegate: Rectangle {
                        required property string fileName
                        required property string filePath
                        required property bool fileIsDir
                        required property var fileSize

                        readonly property bool isMultiSel: !fileIsDir && root.multiple && root.selectedFiles.indexOf(filePath) >= 0
                        readonly property bool isDirSel: fileIsDir && (root.mode === "directory" || root.mode === "save_files") && root.selectedFolder === filePath
                        readonly property bool isSingleSel: !fileIsDir && !root.multiple && root.selectedFile === filePath
                        readonly property bool isSel: isMultiSel || isDirSel || isSingleSel

                        width: fileList.width
                        height: 34
                        color: isSel ? Theme.accent : (rowMouse.containsMouse ? Theme.surfaceHover : "transparent")

                        Row {
                            anchors.fill: parent
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8
                            spacing: 10

                            // Icon or thumbnail
                            Item {
                                width: 24
                                height: 24
                                anchors.verticalCenter: parent.verticalCenter

                                Text {
                                    visible: fileIsDir || !root.isImageFile(fileName)
                                    anchors.centerIn: parent
                                    text: fileIsDir ? "󰉋" : root.fileIcon(fileName)
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 16
                                    color: isSel ? Theme.accentText : (fileIsDir ? "#88c0d0" : Theme.muted)
                                }

                                Image {
                                    visible: !fileIsDir && root.isImageFile(fileName)
                                    anchors.fill: parent
                                    source: (!fileIsDir && root.isImageFile(fileName)) ? ("file://" + filePath) : ""
                                    sourceSize.width: 48
                                    sourceSize.height: 48
                                    fillMode: Image.PreserveAspectCrop
                                    asynchronous: true
                                }
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - (fileIsDir ? 42 : 96)
                                elide: Text.ElideRight
                                text: fileName
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                font.weight: (fileIsDir || isSel) ? Font.Bold : Font.Normal
                                color: isSel ? Theme.accentText : (fileIsDir ? Theme.textBright : Theme.text)
                            }

                            Text {
                                visible: !fileIsDir
                                anchors.verticalCenter: parent.verticalCenter
                                width: 52
                                horizontalAlignment: Text.AlignRight
                                text: root.formatSize(fileSize)
                                font.family: Theme.fontFamily
                                font.pixelSize: 9
                                color: isSel ? Theme.accentText : Theme.muted
                            }
                        }

                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: mouse => {
                                if (fileIsDir) {
                                    if (root.mode === "directory" || root.mode === "save_files") {
                                        root.selectedFolder = filePath;
                                    } else {
                                        root.currentDir = filePath;
                                        fileList.contentY = 0;
                                    }
                                    return;
                                }

                                if (root.mode === "save") {
                                    root.selectedFile = filePath;
                                    root.saveFileName = fileName;
                                    return;
                                }

                                if (root.multiple && (mouse.modifiers & Qt.ControlModifier)) {
                                    var arr = root.selectedFiles.slice();
                                    var idx = arr.indexOf(filePath);
                                    if (idx >= 0) arr.splice(idx, 1);
                                    else arr.push(filePath);
                                    root.selectedFiles = arr;
                                    root.selectedFile = filePath;
                                } else {
                                    root.selectedFile = filePath;
                                    if (root.multiple) root.selectedFiles = [filePath];
                                }
                            }
                            onDoubleClicked: {
                                if (fileIsDir) {
                                    root.currentDir = filePath;
                                    root.selectedFolder = "";
                                    fileList.contentY = 0;
                                } else if (root.mode === "save") {
                                    root.selectedFile = filePath;
                                    root.saveFileName = fileName;
                                    root.confirmSelection();
                                } else {
                                    root.selectedFile = filePath;
                                    if (root.multiple) root.selectedFiles = [filePath];
                                    root.confirmSelection();
                                }
                            }
                        }
                    }
                }

                Text {
                    visible: browserModel.count === 0
                    anchors.centerIn: parent
                    text: (root.mode === "directory" || root.mode === "save_files")
                        ? "No subfolders — click Select Folder"
                        : "Empty folder"
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.muted
                }
            }

            // Right Preview + Filter + Actions
            Item {
                id: previewPane
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 214

                Rectangle {
                    id: previewBox
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    height: 152
                    color: Theme.surface
                    border.width: 1
                    border.color: Theme.border
                    clip: true

                    Image {
                        anchors.fill: parent
                        visible: root.selectedFile.length > 0 && root.isImageFile(root.selectedFile)
                        source: (root.selectedFile.length > 0 && root.isImageFile(root.selectedFile))
                                ? ("file://" + root.selectedFile) : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        smooth: true
                    }

                    Column {
                        visible: !(root.selectedFile.length > 0 && root.isImageFile(root.selectedFile))
                        anchors.centerIn: parent
                        spacing: 6

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: (root.mode === "directory" || root.mode === "save_files") ? "󰉋"
                                : root.selectedFile.length > 0 ? root.fileIcon(root.selectedFile) : "󰈔"
                            font.family: Theme.fontFamily
                            font.pixelSize: 32
                            color: Theme.accent
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: (root.mode === "directory" || root.mode === "save_files")
                                ? "Folder Mode"
                                : (root.selectedFile.length > 0 ? root.basename(root.selectedFile) : "No file selected")
                            width: 190
                            horizontalAlignment: Text.AlignHCenter
                            elide: Text.ElideRight
                            font.family: Theme.fontFamily
                            font.pixelSize: 10
                            color: Theme.muted
                        }
                    }
                }

                Text {
                    id: selTitleTxt
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: previewBox.bottom
                    anchors.topMargin: 8
                    text: (root.mode === "directory" || root.mode === "save_files")
                        ? root.basename(root.selectedFolder.length > 0 ? root.selectedFolder : root.currentDir)
                        : (root.multiple && root.selectedFiles.length > 1)
                            ? (root.selectedFiles.length + " files selected")
                            : (root.selectedFile.length > 0 ? root.basename(root.selectedFile) : (root.mode === "save" ? root.saveFileName : "No file selected"))
                    elide: Text.ElideRight
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    color: Theme.textBright
                }

                Text {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selTitleTxt.bottom
                    anchors.topMargin: 3
                    text: root.mode === "directory" ? "Double-click folder to open"
                        : root.multiple ? "Ctrl+Click for multiple files"
                        : "Double-click to confirm"
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontFamily
                    font.pixelSize: 9
                    color: Theme.muted
                }

                // Filter switcher button (when filters available)
                Rectangle {
                    visible: root.filters && root.filters.length > 1
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: actionRow.top
                    anchors.bottomMargin: 8
                    height: 26
                    color: filterMouse.containsMouse ? Theme.surfaceHover : Theme.surface
                    border.width: 1
                    border.color: Theme.border

                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                        text: "Filter: " + ((root.filters && root.filters[root.activeFilterIdx]) ? root.filters[root.activeFilterIdx].label : "All Files")
                        font.family: Theme.fontFamily
                        font.pixelSize: 9
                        color: Theme.text
                    }

                    MouseArea {
                        id: filterMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.activeFilterIdx = (root.activeFilterIdx + 1) % root.filters.length;
                        }
                    }
                }

                Row {
                    id: actionRow
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    spacing: 8

                    Rectangle {
                        width: Math.floor((parent.width - 8) / 2)
                        height: 34
                        color: cancelBtnMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                        Text {
                            anchors.centerIn: parent
                            text: "Cancel"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.text
                        }

                        MouseArea {
                            id: cancelBtnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.close();
                                root.cancelled();
                            }
                        }
                    }

                    Rectangle {
                        width: Math.floor((parent.width - 8) / 2)
                        height: 34
                        readonly property bool canSelect: root.canConfirm()
                        color: canSelect
                            ? (selectBtnMouse.containsMouse ? Theme.accent : Theme.surfaceActive)
                            : Theme.surface
                        opacity: canSelect ? 1.0 : 0.45

                        Text {
                            anchors.centerIn: parent
                            text: root.acceptLabel.length > 0 ? root.acceptLabel : "Select"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            font.weight: Font.Bold
                            color: (parent.canSelect && selectBtnMouse.containsMouse) ? Theme.accentText : Theme.textBright
                        }

                        MouseArea {
                            id: selectBtnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            enabled: parent.canSelect
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.confirmSelection()
                        }
                    }
                }
            }
        }
    }
}
