import QtQuick
import ".."

Item {
    id: root
    property bool vertical: false
    width: vertical ? Theme.barHeight : (labelText.width + 10)
    height: vertical ? (labelText.height + 8) : Theme.barHeight

    Text {
        id: labelText
        anchors.centerIn: parent
        text: root.vertical
              ? (Math.round(Services.ramPercent) + "%")
              : (Services.ramUsedGb.toFixed(1) + "/8")
        font.family: Theme.fontFamily
        font.pixelSize: root.vertical ? 10 : 11
        font.weight: Font.Medium
        color: Theme.text
    }
}
