import QtQuick
import ".."

Item {
    id: root
    property real value: 0
    property real minValue: 0
    property real maxValue: 100
    property string icon: ""
    property string labelText: ""
    property color fillColor: Theme.accent
    signal moved(real newValue)
    signal iconClicked()

    height: 32

    readonly property real fraction: Math.max(0, Math.min(1, (value - minValue) / Math.max(1, maxValue - minValue)))

    Item {
        anchors.fill: parent

        // Left icon (clickable for mute/action)
        Text {
            id: iconTxt
            visible: root.icon.length > 0
            anchors.left: parent.left
            anchors.leftMargin: 4
            anchors.verticalCenter: parent.verticalCenter
            width: visible ? 22 : 0
            horizontalAlignment: Text.AlignHCenter
            text: root.icon
            font.family: Theme.fontFamily
            font.pixelSize: 15
            color: iconMa.containsMouse ? Theme.accent : Theme.textBright
            Behavior on color { ColorAnimation { duration: 120 } }

            MouseArea {
                id: iconMa
                anchors.fill: parent
                anchors.margins: -4
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.iconClicked()
            }
        }

        // Right value readout
        Text {
            id: valTxt
            visible: root.labelText.length > 0
            anchors.right: parent.right
            anchors.rightMargin: 4
            anchors.verticalCenter: parent.verticalCenter
            horizontalAlignment: Text.AlignRight
            text: root.labelText
            font.family: Theme.fontFamily
            font.pixelSize: 12
            font.weight: Font.DemiBold
            color: Theme.textBright
        }

        // Slim track + circular thumb container
        Item {
            id: trackHitArea
            anchors.left: iconTxt.visible ? iconTxt.right : parent.left
            anchors.leftMargin: iconTxt.visible ? 10 : 4
            anchors.right: valTxt.visible ? valTxt.left : parent.right
            anchors.rightMargin: valTxt.visible ? 10 : 4
            anchors.verticalCenter: parent.verticalCenter
            height: root.height

            readonly property real thumbSize: (dragMa.containsMouse || dragMa.pressed) ? 16 : 14
            readonly property real trackH: 6
            readonly property real usableW: Math.max(1, trackBar.width - thumbSize)
            readonly property real thumbCenterX: (thumbSize / 2) + usableW * root.fraction

            Rectangle {
                id: trackBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                height: trackHitArea.trackH
                radius: height / 2
                color: "#434c5e"

                // Subtle right-end stopper dot
                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: 3
                    anchors.verticalCenter: parent.verticalCenter
                    width: 4
                    height: 4
                    radius: 2
                    color: "#7b8496"
                    opacity: root.fraction < 0.94 ? 0.85 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                // Active filled segment
                Rectangle {
                    id: fillBar
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.max(trackBar.height, Math.min(trackBar.width, trackHitArea.thumbCenterX))
                    height: trackBar.height
                    radius: trackBar.radius
                    color: root.fillColor

                    Behavior on width {
                        enabled: !dragMa.pressed
                        NumberAnimation { duration: 80; easing.type: Easing.OutCubic }
                    }
                }
            }

            // Sleek ring thumb knob
            Rectangle {
                id: thumb
                width: trackHitArea.thumbSize
                height: trackHitArea.thumbSize
                radius: width / 2
                anchors.verticalCenter: trackBar.verticalCenter
                x: Math.max(0, Math.min(trackBar.width - width, trackHitArea.thumbCenterX - width / 2))
                color: root.fillColor
                border.width: 3
                border.color: Theme.popupBg

                Behavior on width { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
                Behavior on height { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
                Behavior on x {
                    enabled: !dragMa.pressed
                    NumberAnimation { duration: 80; easing.type: Easing.OutCubic }
                }
            }

            MouseArea {
                id: dragMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor

                function updateFromMouse(mx) {
                    var pad = trackHitArea.thumbSize / 2;
                    var span = Math.max(1, width - trackHitArea.thumbSize);
                    var frac = Math.max(0, Math.min(1, (mx - pad) / span));
                    var v = root.minValue + frac * (root.maxValue - root.minValue);
                    root.moved(Math.round(v));
                }

                onPressed: mouse => updateFromMouse(mouse.x)
                onPositionChanged: mouse => {
                    if (pressed) updateFromMouse(mouse.x);
                }
            }
        }
    }
}
