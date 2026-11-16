import QtQuick
import Qt.labs.folderlistmodel
import Quickshell.Widgets
import ".."

Column {
    id: root
    width: parent ? parent.width : 500
    spacing: 12

    property string selectedPath: ""
    property string filterText: ""
    signal imageSelected(string filePath)

    FolderListModel {
        id: wallModel
        folder: "file://" + Services.wallpaperDir
        nameFilters: ["*.png", "*.jpg", "*.jpeg", "*.webp", "*.bmp", "*.PNG", "*.JPG", "*.JPEG"]
        showDirs: false
        sortField: FolderListModel.Name
    }

    // Single clean row: folder chips left + search filter right
    Item {
        id: dirBar
        width: parent.width
        height: 30

        Row {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            Repeater {
                model: [
                    { label: "Pictures",  dir: "/home/huyle/Pictures" },
                    { label: "Downloads", dir: "/home/huyle/Downloads" },
                    { label: "Mango",     dir: "/home/huyle/.config/mango" }
                ]

                Rectangle {
                    required property var modelData
                    readonly property bool isDir: Services.wallpaperDir === modelData.dir
                    width: dirChipTxt.implicitWidth + 18
                    height: 30
                    radius: 10
                    color: isDir ? Theme.accent : (dirChipMouse.containsMouse ? Theme.surfaceHover : Theme.surface)
                    border.width: 1
                    border.color: isDir ? Theme.accent : Theme.border

                    Behavior on color { ColorAnimation { duration: 130 } }

                    Text {
                        id: dirChipTxt
                        anchors.centerIn: parent
                        text: modelData.label
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: isDir ? Font.Bold : Font.Medium
                        color: isDir ? Theme.accentText : Theme.text
                    }

                    MouseArea {
                        id: dirChipMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Services.wallpaperDir = modelData.dir;
                            Services.saveSettings();
                        }
                    }
                }
            }
        }

        Rectangle {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 164
            height: 30
            radius: 10
            color: Theme.surface
            border.width: 1
            border.color: searchInput.activeFocus ? Theme.accent : Theme.border

            Behavior on border.color { ColorAnimation { duration: 120 } }

            Text {
                anchors.left: parent.left
                anchors.leftMargin: 9
                anchors.verticalCenter: parent.verticalCenter
                text: "󰍉"
                font.family: Theme.fontFamily
                font.pixelSize: 11
                color: Theme.muted
            }

            TextInput {
                id: searchInput
                anchors.fill: parent
                anchors.leftMargin: 26
                anchors.rightMargin: 9
                verticalAlignment: TextInput.AlignVCenter
                text: root.filterText
                font.family: Theme.fontFamily
                font.pixelSize: 10
                color: Theme.text
                clip: true
                onTextChanged: root.filterText = text
            }

            Text {
                visible: searchInput.text.length === 0
                anchors.left: parent.left
                anchors.leftMargin: 26
                anchors.verticalCenter: parent.verticalCenter
                text: "Filter..."
                font.family: Theme.fontFamily
                font.pixelSize: 10
                color: Theme.muted
            }
        }
    }

    // Soft Rounded Thumbnail Grid (ClippingRectangle clips image corners smoothly)
    Grid {
        width: parent.width
        columns: 3
        spacing: 10

        Repeater {
            model: wallModel

            ClippingRectangle {
                required property string fileName
                required property string filePath
                readonly property bool matchesFilter: root.filterText.trim().length === 0
                    || fileName.toLowerCase().indexOf(root.filterText.trim().toLowerCase()) >= 0
                readonly property bool isCurrent: root.selectedPath === filePath
                visible: matchesFilter
                width: Math.floor((parent.width - 20) / 3)
                height: 112
                radius: 12
                color: Theme.surface
                border.width: isCurrent ? 2 : 1
                border.color: isCurrent ? Theme.accent : (thumbMouse.containsMouse ? Theme.surfaceActive : Theme.border)

                Behavior on border.color { ColorAnimation { duration: 140 } }

                Image {
                    anchors.fill: parent
                    source: "file://" + filePath
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    smooth: true
                    scale: thumbMouse.containsMouse ? 1.04 : 1.0

                    Behavior on scale {
                        NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
                    }
                }

                // Active checkmark pill
                Rectangle {
                    visible: isCurrent
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.margins: 7
                    width: 22
                    height: 22
                    radius: 11
                    color: Theme.accent

                    Text {
                        anchors.centerIn: parent
                        text: "󰄬"
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: Font.Bold
                        color: Theme.accentText
                    }
                }

                // Hover filename overlay
                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 22
                    color: "#c82e3440"
                    opacity: thumbMouse.containsMouse ? 1.0 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 140 } }

                    Text {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.margins: 8
                        anchors.verticalCenter: parent.verticalCenter
                        text: fileName
                        elide: Text.ElideRight
                        font.family: Theme.fontFamily
                        font.pixelSize: 9
                        font.weight: isCurrent ? Font.Bold : Font.Normal
                        color: isCurrent ? Theme.accent : Theme.textBright
                    }
                }

                MouseArea {
                    id: thumbMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.imageSelected(filePath)
                }
            }
        }
    }
}
