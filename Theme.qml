pragma Singleton
import QtQuick
import "."

QtObject {
    property color bgBase: Services.currentTheme.bg
    Behavior on bgBase { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    readonly property color bgDeep: Qt.darker(bgBase, 1.18)
    readonly property color barBg: Qt.rgba(bgBase.r, bgBase.g, bgBase.b, Services.barOpacity)
    readonly property color popupBg: Qt.rgba(bgBase.r, bgBase.g, bgBase.b, Services.popupOpacity)

    property color surface: Services.currentTheme.surface
    Behavior on surface { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color surfaceHover: Services.currentTheme.surfaceHover
    Behavior on surfaceHover { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color surfaceActive: Services.currentTheme.surfaceActive
    Behavior on surfaceActive { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color border: Services.currentTheme.border
    Behavior on border { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color text: Services.currentTheme.text
    Behavior on text { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color textBright: Services.currentTheme.textBright
    Behavior on textBright { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color muted: Services.currentTheme.muted
    Behavior on muted { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color dim: Services.currentTheme.dim
    Behavior on dim { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color accent: Services.accentColor
    Behavior on accent { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    readonly property color accentSoft: Qt.rgba(accent.r, accent.g, accent.b, 0.2)

    property color accentText: Services.currentTheme.accentText
    Behavior on accentText { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color frostTeal: Services.currentTheme.cyan
    Behavior on frostTeal { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color frostBlue: Services.currentTheme.accent
    Behavior on frostBlue { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color frostDeep: Services.currentTheme.purple
    Behavior on frostDeep { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color red: Services.currentTheme.red
    Behavior on red { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color orange: Services.currentTheme.orange
    Behavior on orange { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color yellow: Services.currentTheme.yellow
    Behavior on yellow { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color green: Services.currentTheme.green
    Behavior on green { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color purple: Services.currentTheme.purple
    Behavior on purple { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    property color cyan: Services.currentTheme.cyan
    Behavior on cyan { enabled: Services.settingsReady; ColorAnimation { duration: 420; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0] } }

    readonly property string fontFamily: "JetBrainsMono Nerd Font"
    readonly property int barHeight: Services.barSize
    readonly property int radius: Services.cornerRadius
    readonly property int pillRadius: Services.cornerRadius
}
