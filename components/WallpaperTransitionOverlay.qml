import QtQuick
import Quickshell
import Quickshell.Wayland
import ".."

Item {
    id: root

    property bool active: false
    property real progress: 0.0
    property string oldPath: ""
    property string oldMode: "fill"
    property string newPath: ""
    property string newMode: "fill"

    function fillModeFor(m) {
        if (m === "fit") return Image.PreserveAspectFit;
        if (m === "stretch") return Image.Stretch;
        if (m === "center") return Image.Pad;
        if (m === "tile") return Image.Tile;
        return Image.PreserveAspectCrop;
    }

    function fileUrl(p) {
        if (!p || p.length === 0) return "";
        return p.startsWith("file://") ? p : ("file://" + p);
    }

    property bool revealStarted: false

    function tryStartReveal() {
        if (!root.active || root.revealStarted) return;
        if ((newImg.status === Image.Ready || newImg.status === Image.Error)
            && (oldImg.status === Image.Ready || oldImg.status === Image.Error)) {
            root.revealStarted = true;
            readyPollTimer.stop();
            Services.commitSwaybg(root.newPath, root.newMode);
            revealAnim.restart();
        }
    }

    Connections {
        target: Services
        function onWallpaperTransitionRequested(prevPath, prevMode, nextPath, nextMode) {
            if (!nextPath || nextPath.length === 0) return;
            if (!prevPath || prevPath.length === 0 || (prevPath === nextPath && prevMode === nextMode)) {
                Services.commitSwaybg(nextPath, nextMode);
                return;
            }
            revealAnim.stop();
            settleTimer.stop();
            readyPollTimer.stop();

            root.revealStarted = false;
            root.oldPath = (root.active && root.progress > 0.5 && root.newPath.length > 0) ? root.newPath : prevPath;
            root.oldMode = (root.active && root.progress > 0.5 && root.newPath.length > 0) ? root.newMode : prevMode;
            root.newPath = nextPath;
            root.newMode = nextMode;
            root.progress = 0.0;
            root.active = true;
            readyPollTimer.restart();
            Qt.callLater(root.tryStartReveal);
        }
    }

    Timer {
        id: readyPollTimer
        interval: 16
        repeat: false
        onTriggered: {
            if (!root.active || root.revealStarted) return;
            root.tryStartReveal();
            if (!root.revealStarted) readyPollTimer.restart();
        }
    }

    NumberAnimation {
        id: revealAnim
        target: root
        property: "progress"
        from: 0.0
        to: 1.0
        duration: 740
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0]
        onFinished: settleTimer.restart()
    }

    Timer {
        id: settleTimer
        interval: 120
        repeat: false
        onTriggered: {
            root.active = false;
            root.progress = 0.0;
            root.oldPath = "";
            root.newPath = "";
        }
    }

    PanelWindow {
        id: wallFxWindow
        visible: root.active
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-wallpaper-fx"
        WlrLayershell.layer: WlrLayer.Bottom
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        color: "transparent"
        mask: Region {
            item: emptyWallMask
        }

        Item {
            id: emptyWallMask
            width: 0
            height: 0
        }

        Rectangle {
            anchors.fill: parent
            color: Theme.bgBase
        }

        Image {
            id: oldImg
            anchors.fill: parent
            source: root.fileUrl(root.oldPath)
            fillMode: root.fillModeFor(root.oldMode)
            asynchronous: false
            cache: false
            smooth: true
            onStatusChanged: root.tryStartReveal()
        }

        Image {
            id: newImg
            anchors.fill: parent
            source: root.fileUrl(root.newPath)
            fillMode: root.fillModeFor(root.newMode)
            asynchronous: true
            cache: false
            smooth: true
            onStatusChanged: root.tryStartReveal()
        }

        ShaderEffectSource {
            id: oldTexSrc
            anchors.fill: parent
            sourceItem: oldImg
            hideSource: true
            live: true
            smooth: true
        }

        ShaderEffectSource {
            id: newTexSrc
            anchors.fill: parent
            sourceItem: newImg
            hideSource: true
            live: true
            smooth: true
        }

        ShaderEffect {
            anchors.fill: parent
            property variant oldTex: oldTexSrc
            property variant newTex: newTexSrc
            property color accentColor: Theme.accent
            property real progress: root.progress
            property real aspect: height > 0 ? (width / height) : 1.6
            fragmentShader: Qt.resolvedUrl("../shaders/wallpaper_diag.frag.qsb")
        }
    }
}
