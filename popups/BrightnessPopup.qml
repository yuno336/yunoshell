import QtQuick
import ".."
import "../components"

Column {
    id: root
    width: parent.width
    spacing: 12

    Text {
        leftPadding: 4
        text: "Display brightness"
        font.family: Theme.fontFamily
        font.pixelSize: 14
        font.weight: Font.Bold
        color: Theme.textBright
    }

    SmoothSlider {
        width: parent.width
        minValue: 5
        maxValue: 100
        value: Services.brightnessPercent
        icon: "󰃠"
        labelText: Services.brightnessPercent + "%"
        fillColor: Theme.accent
        onMoved: val => Services.setBrightness(val)
    }
}
