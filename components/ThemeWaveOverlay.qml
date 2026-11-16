import QtQuick
import Quickshell
import Quickshell.Wayland
import ".."

Item {
    id: root

    property bool active: false
    property real progress: 0.0
    property bool readyArmed: false

    Timer {
        interval: 250
        running: Services.settingsReady && !root.readyArmed
        repeat: false
        onTriggered: root.readyArmed = true
    }

    function triggerWave() {
        if (!root.readyArmed) return;
        waveAnim.stop();
        root.progress = 0.0;
        root.active = true;
        waveAnim.restart();
    }

    Connections {
        target: Services
        function onThemeNameChanged() { root.triggerWave(); }
        function onAccentPresetChanged() { root.triggerWave(); }
    }

    NumberAnimation {
        id: waveAnim
        target: root
        property: "progress"
        from: 0.0
        to: 1.0
        duration: 640
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0]
        onFinished: {
            root.active = false;
            root.progress = 0.0;
        }
    }

    PanelWindow {
        visible: root.active
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-theme-wave"
        WlrLayershell.layer: WlrLayer.Top
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        color: "transparent"
        mask: Region {
            item: emptyThemeMask
        }

        Item {
            id: emptyThemeMask
            width: 0
            height: 0
        }

        ShaderEffect {
            anchors.fill: parent
            property color accentColor: Theme.accent
            property real progress: root.progress
            property real aspect: height > 0 ? (width / height) : 1.6
            fragmentShader: Qt.resolvedUrl("../shaders/theme_diag.frag.qsb")
        }
    }
}
