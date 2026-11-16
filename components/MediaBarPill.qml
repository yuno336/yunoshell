import QtQuick
import Quickshell.Widgets
import ".."

Item {
    id: root
    property bool active: false
    property bool vertical: false
    signal clicked()
    signal rightClicked()

    readonly property var player: Services.activeMediaPlayer
    readonly property bool hasMedia: player !== null
    readonly property bool isPlaying: player ? player.isPlaying : false
    readonly property string trackTitle: player && player.trackTitle ? player.trackTitle : ""
    readonly property string artUrl: player && player.trackArtUrl ? player.trackArtUrl : ""
    readonly property var sourceInfo: Services.mediaSourceInfo(player)

    implicitWidth: vertical ? (hasMedia ? 26 : 0) : (hasMedia ? Math.min(206, pillRow.implicitWidth + 16) : 0)
    width: implicitWidth
    height: vertical ? (hasMedia ? 26 : 0) : 24
    visible: width > 1 && height > 1
    clip: true

    Behavior on width {
        NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
    }

    Rectangle {
        anchors.fill: parent
        radius: 12
        color: root.active ? Theme.surfaceActive
             : pillMouse.containsMouse ? Theme.surfaceHover
             : Theme.surface
        border.width: 1
        border.color: root.active ? Theme.accent : Theme.border

        Behavior on color { ColorAnimation { duration: 140 } }
    }

    Row {
        id: pillRow
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: root.vertical ? undefined : parent.left
        anchors.leftMargin: root.vertical ? 0 : 6
        anchors.horizontalCenter: root.vertical ? parent.horizontalCenter : undefined
        spacing: root.vertical ? 0 : 6

        // Mini rounded thumbnail / fallback glyph
        ClippingRectangle {
            width: 16
            height: 16
            radius: 4
            color: Theme.surfaceHover
            anchors.verticalCenter: parent.verticalCenter

            Image {
                id: miniThumb
                anchors.fill: parent
                visible: root.artUrl.length > 0 && status === Image.Ready
                source: root.artUrl
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                smooth: true
                mipmap: true
            }

            Text {
                anchors.centerIn: parent
                visible: !miniThumb.visible
                text: root.sourceInfo.icon
                font.family: Theme.fontFamily
                font.pixelSize: 10
                color: Theme.text
            }
        }

        Text {
            visible: !root.vertical
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(138, implicitWidth)
            text: root.trackTitle.length > 0 ? root.trackTitle : root.sourceInfo.label
            font.family: Theme.fontFamily
            font.pixelSize: 11
            font.weight: Font.Medium
            color: Theme.textBright
            elide: Text.ElideRight
        }

        // Animated 3-bar mini equalizer
        Row {
            visible: !root.vertical
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2
            height: 12

            Repeater {
                model: [
                    { lo: 4, hi: 11, dur: 320 },
                    { lo: 5, hi: 12, dur: 260 },
                    { lo: 3, hi: 9,  dur: 380 }
                ]

                Rectangle {
                    required property var modelData
                    width: 2
                    height: root.isPlaying ? modelData.hi : 4
                    radius: 1
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 1
                    color: root.isPlaying ? Theme.accent : Theme.muted

                    SequentialAnimation on height {
                        running: root.isPlaying && root.visible
                        loops: Animation.Infinite
                        NumberAnimation {
                            to: modelData.lo
                            duration: modelData.dur
                            easing.type: Easing.InOutSine
                        }
                        NumberAnimation {
                            to: modelData.hi
                            duration: modelData.dur
                            easing.type: Easing.InOutSine
                        }
                    }
                }
            }
        }
    }

    MouseArea {
        id: pillMouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) {
                root.rightClicked();
            } else {
                root.clicked();
            }
        }
    }
}
