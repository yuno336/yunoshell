import QtQuick
import ".."

Item {
    id: root
    property string icon: ""
    property url iconSource: ""
    property int iconImageSize: 20
    property string label: ""
    property color iconColor: Theme.text
    property bool active: false
    property bool vertical: false
    readonly property alias hovered: mouseArea.containsMouse

    signal clicked()
    signal rightClicked()
    signal wheeled(int delta)
    signal entered()
    signal exited()

    width: vertical ? Theme.barHeight : Math.max(28, row.width + 12)
    height: vertical ? Math.max(28, row.height + 10) : Theme.barHeight

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 5

        Image {
            visible: root.iconSource.toString().length > 0
            anchors.verticalCenter: parent.verticalCenter
            source: root.iconSource
            width: root.iconImageSize
            height: root.iconImageSize
            fillMode: Image.PreserveAspectFit
            smooth: true
            mipmap: true
            cache: false
            opacity: root.active || mouseArea.containsMouse ? 1.0 : 0.88
        }

        Text {
            visible: root.iconSource.toString().length === 0
            anchors.verticalCenter: parent.verticalCenter
            text: root.icon
            font.family: Theme.fontFamily
            font.pixelSize: 14
            color: root.iconColor
            opacity: root.active || mouseArea.containsMouse ? 1.0 : 0.88
        }

        Text {
            visible: root.label.length > 0
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            font.family: Theme.fontFamily
            font.pixelSize: 11
            font.weight: Font.Medium
            color: Theme.text
            opacity: root.active || mouseArea.containsMouse ? 1.0 : 0.88
        }
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        property real wheelAccum: 0
        onEntered: root.entered()
        onExited: root.exited()
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) {
                root.rightClicked();
            } else {
                root.clicked();
            }
        }
        onWheel: wheel => {
            wheelAccum += wheel.angleDelta.y;
            if (Math.abs(wheelAccum) >= 60) {
                root.wheeled(wheelAccum > 0 ? 120 : -120);
                wheelAccum = 0;
            }
        }
    }
}
