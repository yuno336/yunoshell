import QtQuick
import Quickshell
import ".."

Column {
    id: root
    width: parent.width
    spacing: 14

    SystemClock {
        id: clock
        precision: SystemClock.Seconds
    }

    // Pixel Expressive Clock Pill Header
    Rectangle {
        width: parent.width
        height: 68
        radius: 12
        color: Theme.surface

        Column {
            anchors.centerIn: parent
            spacing: 2

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Qt.formatDateTime(clock.date, "HH:mm:ss")
                font.family: Theme.fontFamily
                font.pixelSize: 24
                font.weight: Font.Bold
                color: Theme.textBright
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Qt.formatDateTime(clock.date, "dddd, MMMM dd")
                font.family: Theme.fontFamily
                font.pixelSize: 11
                font.weight: Font.Medium
                color: Theme.muted
            }
        }
    }

    readonly property int currentYear: clock.date.getFullYear()
    readonly property int currentMonth: clock.date.getMonth()
    readonly property int todayDate: clock.date.getDate()
    readonly property int firstDayOffset: (new Date(currentYear, currentMonth, 1).getDay() + 6) % 7
    readonly property int daysInMonth: new Date(currentYear, currentMonth + 1, 0).getDate()
    readonly property real cellWidth: Math.floor((width - 6 * 4) / 7)

    Grid {
        width: parent.width
        columns: 7
        spacing: 4

        Repeater {
            model: ["M", "T", "W", "T", "F", "S", "S"]
            Text {
                required property string modelData
                width: root.cellWidth
                height: 22
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: modelData
                font.family: Theme.fontFamily
                font.pixelSize: 10
                font.weight: Font.Bold
                color: Theme.muted
            }
        }

        Repeater {
            model: root.firstDayOffset + root.daysInMonth
            Rectangle {
                required property int index
                readonly property int dayNum: index - root.firstDayOffset + 1
                readonly property bool isValid: dayNum >= 1
                readonly property bool isToday: isValid && dayNum === root.todayDate

                width: root.cellWidth
                height: root.cellWidth
                radius: 8
                color: isToday ? Theme.accent : "transparent"

                Text {
                    visible: isValid
                    anchors.centerIn: parent
                    text: dayNum
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    font.weight: isToday ? Font.Bold : Font.Medium
                    color: isToday ? Theme.accentText : Theme.text
                }
            }
        }
    }
}
