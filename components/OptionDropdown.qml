import QtQuick
import ".."

Column {
    id: root
    property string label: ""
    property string currentText: ""
    property var options: []
    property var selectedValue: ""
    property bool expanded: false
    property bool interactiveEnabled: true

    signal headerClicked()
    signal optionPicked(var value, string label)

    width: parent ? parent.width : 300
    spacing: 5
    opacity: interactiveEnabled ? 1.0 : 0.45

    Item {
        id: headerRow
        width: parent.width
        height: 28

        Text {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 12
            font.weight: Font.Medium
        }

        Rectangle {
            id: triggerBox
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(headerRow.width * 0.56, Math.max(76, valText.implicitWidth + arrowTxt.implicitWidth + 28))
            height: 27
            radius: 9
            color: root.expanded ? Theme.surfaceActive : (trigMa.containsMouse && root.interactiveEnabled ? Theme.surfaceHover : Theme.bgDeep)
            border.width: 1
            border.color: root.expanded ? Theme.accent : Theme.border
            Behavior on color { ColorAnimation { duration: 120 } }

            Row {
                id: trigContentRow
                anchors.centerIn: parent
                spacing: 6

                Text {
                    id: valText
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, Math.max(24, triggerBox.width - arrowTxt.implicitWidth - 24))
                    text: root.currentText
                    color: Theme.textBright
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: Font.Medium
                    elide: Text.ElideRight
                    horizontalAlignment: Text.AlignHCenter
                }

                Text {
                    id: arrowTxt
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.expanded ? "󰅃" : "󰅀"
                    color: root.expanded ? Theme.accent : Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                }
            }

            MouseArea {
                id: trigMa
                anchors.fill: parent
                enabled: root.interactiveEnabled
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.headerClicked()
            }
        }
    }

    Item {
        visible: root.expanded && root.options && root.options.length > 0
        width: parent.width
        height: optCol.implicitHeight + 8

        Rectangle {
            anchors.right: parent.right
            width: Math.min(parent.width, Math.max(triggerBox.width, 168))
            height: parent.height
            radius: 10
            color: Theme.bgDeep
            border.width: 1
            border.color: Theme.border

            Column {
                id: optCol
                anchors.fill: parent
                anchors.margins: 4
                spacing: 2

                Repeater {
                    model: root.options

                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool isSel: modelData.value === root.selectedValue

                        width: parent.width
                        height: 26
                        radius: 7
                        color: isSel ? Theme.accent : (optMa.containsMouse ? Theme.surfaceHover : "transparent")
                        Behavior on color { ColorAnimation { duration: 100 } }

                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 9
                            anchors.right: checkTxt.left
                            anchors.rightMargin: 6
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                            color: isSel ? Theme.accentText : Theme.textBright
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            font.weight: isSel ? Font.DemiBold : Font.Normal
                            elide: Text.ElideRight
                        }

                        Text {
                            id: checkTxt
                            visible: isSel
                            anchors.right: parent.right
                            anchors.rightMargin: 8
                            anchors.verticalCenter: parent.verticalCenter
                            text: "󰄬"
                            color: Theme.accentText
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                        }

                        MouseArea {
                            id: optMa
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.optionPicked(modelData.value, modelData.label)
                        }
                    }
                }
            }
        }
    }
}
