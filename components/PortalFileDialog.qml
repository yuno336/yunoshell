import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import ".."

Item {
    id: root

    property bool active: false
    property string activeReqId: ""
    property string activeResFile: ""
    property var pendingFiles: []
    property real animProgress: 0.0

    function writeResult(accepted, paths, filterIndex) {
        if (!root.activeResFile || root.activeResFile.length === 0) return;
        var outPaths = paths || [];
        if (accepted && root.pendingFiles && root.pendingFiles.length > 0 && outPaths.length > 0) {
            var dir = outPaths[0];
            outPaths = root.pendingFiles.map(fn => dir.endsWith("/") ? (dir + fn) : (dir + "/" + fn));
        }
        var payload = JSON.stringify({
            accepted: !!accepted,
            paths: outPaths,
            filter_index: filterIndex !== undefined ? filterIndex : -1
        });
        var resPath = root.activeResFile;
        root.activeResFile = "";
        root.activeReqId = "";
        root.pendingFiles = [];
        Quickshell.execDetached([
            "python3", "-c",
            "import os, sys; p, d = sys.argv[1], sys.argv[2]; t = p + '.tmp'; open(t, 'w').write(d); os.replace(t, p)",
            resPath, payload
        ]);
    }

    function beginClose() {
        root.active = false;
        openAnim.stop();
        closeAnim.restart();
    }

    NumberAnimation {
        id: openAnim
        target: root
        property: "animProgress"
        to: 1.0
        duration: 200
        easing.type: Easing.OutCubic
    }

    NumberAnimation {
        id: closeAnim
        target: root
        property: "animProgress"
        to: 0.0
        duration: 140
        easing.type: Easing.InCubic
    }

    Process {
        id: readReqProc
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                if (!text || text.trim().length === 0) return;
                try {
                    var req = JSON.parse(text);
                    root.activeReqId = req.id || "";
                    root.activeResFile = req.res_file || "";
                    root.pendingFiles = req.files || [];
                    browser.openPortalRequest(req);
                    root.active = true;
                    closeAnim.stop();
                    openAnim.restart();
                } catch (e) {
                    console.log("Portal req parse error:", e);
                }
            }
        }
    }

    IpcHandler {
        target: "portal"
        function open(reqFilePath: string): void {
            if (root.activeResFile.length > 0) {
                root.writeResult(false, [], -1);
            }
            readReqProc.command = ["cat", reqFilePath];
            readReqProc.running = true;
        }
        function cancel(reqId: string): void {
            if (root.activeReqId === reqId) {
                root.activeResFile = "";
                root.activeReqId = "";
                root.beginClose();
            }
        }
    }

    PanelWindow {
        id: portalWindow
        visible: root.active || root.animProgress > 0.01
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: root.active ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
        color: Qt.rgba(0, 0, 0, 0.42 * root.animProgress)

        MouseArea {
            anchors.fill: parent
            onClicked: {
                root.writeResult(false, [], -1);
                root.beginClose();
            }
        }

        Item {
            id: cardHost
            width: 724
            height: 490
            x: Math.round((portalWindow.width - width) / 2)
            y: Math.round((portalWindow.height - height) / 2) + Math.round((1.0 - root.animProgress) * 14)
            opacity: root.animProgress
            scale: 0.93 + 0.07 * root.animProgress

            FileBrowserDialog {
                id: browser
                isStandalonePortal: true
                onAcceptedPaths: (paths, filterIndex) => {
                    root.writeResult(true, paths, filterIndex);
                    root.beginClose();
                }
                onCancelled: {
                    root.writeResult(false, [], -1);
                    root.beginClose();
                }
            }
        }
    }
}
