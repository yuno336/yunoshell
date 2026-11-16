import QtQuick
import Quickshell.Widgets
import ".."

Column {
    id: root
    width: parent.width
    spacing: 8

    readonly property var player: Services.activeMediaPlayer
    readonly property var allPlayers: Services.mediaPlayers
    readonly property var sourceInfo: Services.mediaSourceInfo(player)
    readonly property real totalLen: player && player.length > 0 ? player.length : 0
    readonly property bool isPlaying: player ? player.isPlaying : false

    property real anchorPos: 0
    property double anchorTimeMs: 0
    property real displayPos: 0
    property bool scrubbing: false
    property real scrubRatio: 0

    function syncPosition() {
        anchorPos = (player && player.position > 0) ? player.position : 0;
        anchorTimeMs = Date.now();
        displayPos = totalLen > 0 ? Math.min(totalLen, anchorPos) : anchorPos;
    }

    function formatTime(sec) {
        if (!sec || sec < 0 || isNaN(sec)) return "0:00";
        var total = Math.floor(sec);
        var h = Math.floor(total / 3600);
        var m = Math.floor((total % 3600) / 60);
        var s = total % 60;
        var sStr = (s < 10 ? "0" : "") + s;
        if (h > 0) {
            return h + ":" + (m < 10 ? "0" : "") + m + ":" + sStr;
        }
        return m + ":" + sStr;
    }

    Component.onCompleted: syncPosition()
    onPlayerChanged: syncPosition()

    Connections {
        target: root.player
        function onPositionChanged() { root.syncPosition(); }
        function onTrackTitleChanged() { root.syncPosition(); }
        function onIsPlayingChanged() { root.syncPosition(); }
        function onLengthChanged() { root.syncPosition(); }
    }

    Timer {
        interval: 250
        repeat: true
        running: root.visible && root.isPlaying && !root.scrubbing && root.totalLen > 0
        onTriggered: {
            var elapsed = (Date.now() - root.anchorTimeMs) / 1000.0;
            root.displayPos = Math.min(root.totalLen, root.anchorPos + Math.max(0, elapsed));
        }
    }

    // Compact player switcher chips when multiple sources are active
    Row {
        visible: root.allPlayers.length > 1
        width: parent.width
        spacing: 5

        Repeater {
            model: root.allPlayers

            Rectangle {
                required property var modelData
                readonly property bool isCurrent: root.player && modelData.dbusName === root.player.dbusName
                readonly property var info: Services.mediaSourceInfo(modelData)

                height: 21
                width: chipRow.implicitWidth + 12
                radius: 7
                color: isCurrent ? Theme.accent
                     : chipMouse.containsMouse ? Theme.surfaceHover : Theme.surface

                Behavior on color { ColorAnimation { duration: 140 } }

                Row {
                    id: chipRow
                    anchors.centerIn: parent
                    spacing: 4

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: info.icon
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        color: isCurrent ? Theme.accentText : Theme.text
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: info.label
                        font.family: Theme.fontFamily
                        font.pixelSize: 9
                        font.weight: isCurrent ? Font.Bold : Font.Medium
                        color: isCurrent ? Theme.accentText : Theme.text
                    }
                }

                MouseArea {
                    id: chipMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Services.selectMediaPlayer(modelData.dbusName)
                }
            }
        }
    }

    // Compact Active Player Content (no double-nested card wrapper)
    Column {
        visible: root.player !== null
        width: parent.width
        spacing: 8

        // Track Art + Title + Artist/Source
        Row {
            width: parent.width
            spacing: 10

            ClippingRectangle {
                width: 52
                height: 52
                radius: 10
                color: Theme.surface

                Image {
                    id: coverImg
                    anchors.fill: parent
                    visible: root.player && root.player.trackArtUrl && root.player.trackArtUrl.length > 0 && status === Image.Ready
                    source: (root.player && root.player.trackArtUrl) ? root.player.trackArtUrl : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    smooth: true
                    mipmap: true
                }

                Text {
                    anchors.centerIn: parent
                    visible: !coverImg.visible
                    text: root.sourceInfo.icon
                    font.family: Theme.fontFamily
                    font.pixelSize: 22
                    color: Theme.text
                }
            }

            Column {
                width: parent.width - 62
                anchors.verticalCenter: parent.verticalCenter
                spacing: 3

                Row {
                    width: parent.width
                    spacing: 6

                    Text {
                        width: parent.width - ((raiseBtn.visible ? 22 : 0) + 6)
                        anchors.verticalCenter: parent.verticalCenter
                        text: (root.player && root.player.trackTitle && root.player.trackTitle.length > 0)
                              ? root.player.trackTitle : "Unknown Title"
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: Font.Bold
                        color: Theme.textBright
                        elide: Text.ElideRight
                    }

                    Rectangle {
                        id: raiseBtn
                        visible: root.player && root.player.canRaise
                        width: 20
                        height: 20
                        radius: 6
                        anchors.verticalCenter: parent.verticalCenter
                        color: raiseMouse.containsMouse ? Theme.surfaceActive : Theme.surface

                        Text {
                            anchors.centerIn: parent
                            text: "󰖯"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.text
                        }

                        MouseArea {
                            id: raiseMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: if (root.player) root.player.raise()
                        }
                    }
                }

                Row {
                    width: parent.width
                    spacing: 6

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, parent.width - srcBadge.width - 6)
                        text: (root.player && root.player.trackArtist && root.player.trackArtist.length > 0)
                              ? root.player.trackArtist
                              : (root.player && root.player.trackAlbum && root.player.trackAlbum.length > 0)
                                ? root.player.trackAlbum
                                : root.sourceInfo.label
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: Font.Medium
                        color: Theme.muted
                        elide: Text.ElideRight
                    }

                    Rectangle {
                        id: srcBadge
                        height: 16
                        width: srcBadgeTxt.implicitWidth + 10
                        radius: 5
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.surface

                        Text {
                            id: srcBadgeTxt
                            anchors.centerIn: parent
                            text: root.sourceInfo.icon + " " + root.sourceInfo.label
                            font.family: Theme.fontFamily
                            font.pixelSize: 9
                            font.weight: Font.DemiBold
                            color: Theme.accent
                        }
                    }
                }
            }
        }

        // Inline Seek Bar: elapsed | track | total
        Row {
            visible: root.totalLen > 0
            width: parent.width
            spacing: 8

            Text {
                id: elapsedTxt
                anchors.verticalCenter: parent.verticalCenter
                text: root.formatTime(root.scrubbing ? (root.scrubRatio * root.totalLen) : root.displayPos)
                font.family: Theme.fontFamily
                font.pixelSize: 9
                color: Theme.muted
            }

            Item {
                id: seekTrack
                width: Math.max(40, parent.width - elapsedTxt.implicitWidth - totalTxt.implicitWidth - 16)
                height: 14
                anchors.verticalCenter: parent.verticalCenter
                readonly property real ratio: root.scrubbing
                                              ? root.scrubRatio
                                              : Math.max(0.0, Math.min(1.0, root.displayPos / Math.max(1.0, root.totalLen)))

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: seekMouse.containsMouse || root.scrubbing ? 5 : 3
                    radius: height / 2
                    color: Theme.surfaceHover

                    Behavior on height { NumberAnimation { duration: 120 } }

                    Rectangle {
                        width: parent.width * seekTrack.ratio
                        height: parent.height
                        radius: height / 2
                        color: Theme.accent
                    }
                }

                Rectangle {
                    visible: seekMouse.containsMouse || root.scrubbing
                    width: 10
                    height: 10
                    radius: 5
                    color: Theme.textBright
                    anchors.verticalCenter: parent.verticalCenter
                    x: Math.max(0, Math.min(parent.width - width, parent.width * seekTrack.ratio - width / 2))
                }

                MouseArea {
                    id: seekMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    enabled: root.player && root.player.canSeek && root.totalLen > 0
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor

                    function updateScrub(mx) {
                        root.scrubRatio = Math.max(0.0, Math.min(1.0, mx / seekTrack.width));
                    }

                    onPressed: mouse => {
                        root.scrubbing = true;
                        updateScrub(mouse.x);
                    }
                    onPositionChanged: mouse => {
                        if (root.scrubbing) updateScrub(mouse.x);
                    }
                    onReleased: mouse => {
                        updateScrub(mouse.x);
                        if (root.player && root.player.canSeek && root.totalLen > 0) {
                            root.player.position = root.scrubRatio * root.totalLen;
                            root.anchorPos = root.scrubRatio * root.totalLen;
                            root.anchorTimeMs = Date.now();
                            root.displayPos = root.anchorPos;
                        }
                        root.scrubbing = false;
                    }
                }
            }

            Text {
                id: totalTxt
                anchors.verticalCenter: parent.verticalCenter
                text: root.formatTime(root.totalLen)
                font.family: Theme.fontFamily
                font.pixelSize: 9
                color: Theme.muted
            }
        }

        // Compact Transport Controls
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 8

            Rectangle {
                width: 30
                height: 28
                radius: 8
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.player && root.player.canSeek
                opacity: enabled ? 1.0 : 0.35
                color: rewMouse.containsMouse && enabled ? Theme.surfaceActive : Theme.surface

                Text {
                    anchors.centerIn: parent
                    text: "-10"
                    font.family: Theme.fontFamily
                    font.pixelSize: 9
                    font.weight: Font.Bold
                    color: Theme.text
                }

                MouseArea {
                    id: rewMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: parent.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: {
                        if (root.player && root.player.canSeek) {
                            root.player.seek(-10);
                            root.syncPosition();
                        }
                    }
                }
            }

            Rectangle {
                width: 32
                height: 30
                radius: 9
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.player && root.player.canGoPrevious
                opacity: enabled ? 1.0 : 0.35
                color: prevMouse.containsMouse && enabled ? Theme.surfaceActive : Theme.surface

                Text {
                    anchors.centerIn: parent
                    text: "󰒮"
                    font.family: Theme.fontFamily
                    font.pixelSize: 15
                    color: Theme.textBright
                }

                MouseArea {
                    id: prevMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: parent.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Services.mediaPrev()
                }
            }

            Rectangle {
                width: 38
                height: 38
                radius: 19
                anchors.verticalCenter: parent.verticalCenter
                color: playMouse.containsMouse ? Theme.textBright : Theme.accent

                Behavior on color { ColorAnimation { duration: 140 } }

                Text {
                    anchors.centerIn: parent
                    text: root.isPlaying ? "󰏤" : "󰐊"
                    font.family: Theme.fontFamily
                    font.pixelSize: 18
                    color: Theme.accentText
                }

                MouseArea {
                    id: playMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Services.mediaTogglePlay()
                }
            }

            Rectangle {
                width: 32
                height: 30
                radius: 9
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.player && root.player.canGoNext
                opacity: enabled ? 1.0 : 0.35
                color: nextMouse.containsMouse && enabled ? Theme.surfaceActive : Theme.surface

                Text {
                    anchors.centerIn: parent
                    text: "󰒭"
                    font.family: Theme.fontFamily
                    font.pixelSize: 15
                    color: Theme.textBright
                }

                MouseArea {
                    id: nextMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: parent.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Services.mediaNext()
                }
            }

            Rectangle {
                width: 30
                height: 28
                radius: 8
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.player && root.player.canSeek
                opacity: enabled ? 1.0 : 0.35
                color: fwdMouse.containsMouse && enabled ? Theme.surfaceActive : Theme.surface

                Text {
                    anchors.centerIn: parent
                    text: "+10"
                    font.family: Theme.fontFamily
                    font.pixelSize: 9
                    font.weight: Font.Bold
                    color: Theme.text
                }

                MouseArea {
                    id: fwdMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: parent.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: {
                        if (root.player && root.player.canSeek) {
                            root.player.seek(10);
                            root.syncPosition();
                        }
                    }
                }
            }
        }
    }

    // Compact Empty State
    Row {
        visible: root.player === null
        width: parent.width
        height: 44
        spacing: 10

        Rectangle {
            width: 36
            height: 36
            radius: 10
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.surface

            Text {
                anchors.centerIn: parent
                text: "󰝚"
                font.family: Theme.fontFamily
                font.pixelSize: 17
                color: Theme.muted
            }
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 46
            spacing: 1

            Text {
                text: "Nothing playing"
                font.family: Theme.fontFamily
                font.pixelSize: 12
                font.weight: Font.Bold
                color: Theme.textBright
            }

            Text {
                width: parent.width
                text: "Play music or video in browser"
                font.family: Theme.fontFamily
                font.pixelSize: 10
                color: Theme.muted
                elide: Text.ElideRight
            }
        }
    }
}
