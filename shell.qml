import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "components"
import "popups"

ShellRoot {
    id: root
    property string activePopup: ""
    property string renderedPopup: ""
    property real popupAnimProgress: 0.0
    property real islandWidthP: 0.0
    property real islandHeightP: 0.0
    property real islandContentP: 0.0
    property real morphP: 1.0
    property real morphFadeOpacity: 1.0
    property real morphFromBodyW: 164
    property real morphFromBodyH: 100
    property real morphFromX: 0
    property real morphFromY: 0
    property bool warmLauncher: false
    property bool warmNotifications: false
    property bool warmMedia: false
    property bool warmSettings: false
    property bool warmFiles: false
    property bool externalDragActive: false
    property bool allowMorph: false
    property real originX: 0
    property real originY: barBottom ? (overlayWindow.height - Theme.barHeight / 2) : (Theme.barHeight / 2)
    property int originWidth: 164
    property bool openedByHover: false
    property bool popupCardHovered: false
    property string settingsInitialTab: "time"
    readonly property string barPos: Services.barPosition
    readonly property bool barBottom: barPos === "bottom"
    readonly property bool barLeft: barPos === "left"
    readonly property bool barRight: barPos === "right"
    readonly property bool barVertical: barLeft || barRight
    readonly property int barSpan: Theme.barHeight + (Services.barFloating ? 10 : 0)
    readonly property bool barSwitching: barSwitchAnim.running || barRemapping
    property bool barRemapping: false
    property string pendingBarPos: ""
    property real barSlideOffset: 0
    property real barOpacity: 1.0
    property real barScale: 1.0

    Timer {
        id: warmupTimer
        property int stage: 0
        interval: 350
        running: Services.settingsReady
        repeat: false
        onTriggered: {
            if (stage === 0) {
                root.warmNotifications = true;
                root.warmMedia = true;
                stage = 1;
                interval = 180;
                restart();
            } else if (stage === 1) {
                root.warmLauncher = true;
                stage = 2;
                interval = 220;
                restart();
            } else if (stage === 2) {
                root.warmSettings = true;
                stage = 3;
                interval = 220;
                restart();
            } else if (stage === 3) {
                root.warmFiles = true;
                stage = 4;
            }
        }
    }

    onRenderedPopupChanged: {
        Services.sysMonitorActive = (renderedPopup === "settings");
        if (renderedPopup === "launcher" && launcherCache.item && launcherCache.item.resetAndFocus) {
            Qt.callLater(launcherCache.item.resetAndFocus);
        } else if (renderedPopup === "files" && filesCache.item && filesCache.item.focusDefault) {
            Qt.callLater(filesCache.item.focusDefault);
        }
    }

    readonly property bool anyHoverTriggerHovered: btIcon.hovered || brightIcon.hovered || volIcon.hovered
        || batIcon.hovered || wifiIcon.hovered || powerIcon.hovered || clockWidget.hovered

    function switchBarPosition(nextPos) {
        if (barSwitchAnim.running) return;
        if ((nextPos !== "top" && nextPos !== "bottom" && nextPos !== "left" && nextPos !== "right") || nextPos === Services.barPosition) return;
        pendingBarPos = nextPos;
        barSwitchAnim.restart();
    }

    SequentialAnimation {
        id: barSwitchAnim
        ParallelAnimation {
            NumberAnimation {
                target: root
                property: "barSlideOffset"
                to: (root.barBottom || root.barRight) ? 14 : -14
                duration: 160
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                target: root
                property: "barOpacity"
                to: 0.0
                duration: 150
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                target: root
                property: "barScale"
                to: 0.985
                duration: 160
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                target: root
                property: "islandContentP"
                to: 0.0
                duration: 80
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: root
                property: "islandHeightP"
                to: 0.0
                duration: 160
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                target: root
                property: "islandWidthP"
                to: 0.0
                duration: 160
                easing.type: Easing.InCubic
            }
        }
        ScriptAction {
            script: {
                if (root.pendingBarPos === "top" || root.pendingBarPos === "bottom" || root.pendingBarPos === "left" || root.pendingBarPos === "right") {
                    root.barRemapping = true;
                    Services.barPosition = root.pendingBarPos;
                    Services.saveSettings();
                    root.barSlideOffset = (root.pendingBarPos === "bottom" || root.pendingBarPos === "right") ? 12 : -12;
                    if (root.activePopup !== "") {
                        root.updatePopupOrigin(root.activePopup);
                    }
                }
            }
        }
        PauseAnimation { duration: 60 }
        ScriptAction {
            script: {
                root.barRemapping = false;
                if (root.activePopup !== "") {
                    root.updatePopupOrigin(root.activePopup);
                    openIslandAnim.restart();
                }
            }
        }
        ParallelAnimation {
            NumberAnimation {
                target: root
                property: "barSlideOffset"
                to: 0
                duration: 280
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: root
                property: "barOpacity"
                to: 1.0
                duration: 240
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: root
                property: "barScale"
                to: 1.0
                duration: 280
                easing.type: Easing.OutCubic
            }
        }
    }

    function isTopIslandName(name) {
        return name === "launcher" || name === "notifications" || name === "media" || name === "settings" || name === "clock" || name === "files";
    }

    function isIslandName(name) {
        return name !== "";
    }

    function isCenterIsland(name) {
        return isTopIslandName(name);
    }

    function iconItemForPopup(name) {
        if (name === "launcher" || name === "antigravity") return launcherIcon;
        if (name === "files") return filesIcon;
        if (name === "clock" || name === "settings") return clockWidget;
        if (name === "media") return mediaBarPill;
        if (name === "bluetooth") return btIcon;
        if (name === "brightness") return brightIcon;
        if (name === "volume") return volIcon;
        if (name === "battery") return batIcon;
        if (name === "wifi") return wifiIcon;
        if (name === "power") return powerIcon;
        return null;
    }

    function updatePopupOrigin(name) {
        var ovW = (overlayWindow.visible && overlayWindow.width > 200)
                  ? overlayWindow.width
                  : (barWindow.screen ? barWindow.screen.width : 1536);
        var ovH = (overlayWindow.visible && overlayWindow.height > 200)
                  ? overlayWindow.height
                  : (barWindow.screen ? barWindow.screen.height : 960);
        if (isTopIslandName(name)) {
            originX = ovW / 2;
            originY = (root.barPos === "top") ? (root.barSpan / 2) : 0;
            originWidth = (root.barPos === "top" && name === "media" && centerCluster)
                          ? Math.max(164, Math.round(centerCluster.width + 16))
                          : 164;
            return;
        }
        if (root.barVertical) {
            var edgeX = root.barLeft ? (root.barSpan / 2) : (ovW - root.barSpan / 2);
            originX = edgeX;
            if (isCenterIsland(name)) {
                originY = (barContent && barContent.height > 200) ? (barContent.height / 2) : (ovH / 2);
                originWidth = 164;
                return;
            }
            var vTarget = iconItemForPopup(name);
            var vOff = Services.barFloating ? 5 : 0;
            if (vTarget && barContent) {
                var vp = vTarget.mapToItem(barContent, vTarget.width / 2, vTarget.height / 2);
                originY = vOff + vp.y;
                originWidth = Math.max(48, Math.round((vTarget.height + 18) / 2) * 2);
            } else {
                originY = ovH / 2;
                originWidth = 120;
            }
            return;
        }
        var edgeY = root.barBottom ? (ovH - root.barSpan / 2) : (root.barSpan / 2);
        if (isCenterIsland(name)) {
            originX = (barContent && barContent.width > 200) ? (barContent.width / 2) : (ovW / 2);
            originY = edgeY;
            originWidth = (name === "media" || name === "clock") && centerCluster
                          ? Math.max(164, Math.round(centerCluster.width + 16))
                          : 164;
            return;
        }
        var target = iconItemForPopup(name);
        if (target && barContent) {
            var p = target.mapToItem(barContent, target.width / 2, target.height / 2);
            originX = p.x;
            originY = edgeY;
            originWidth = Math.max(52, Math.round((target.width + 20) / 2) * 2);
        } else {
            originX = barContent ? (barContent.width / 2) : (ovW / 2);
            originY = edgeY;
            originWidth = 164;
        }
    }

    Timer {
        id: deferredOpenTimer
        interval: 16
        repeat: false
        onTriggered: {
            if (root.activePopup !== "" && !root.isIslandName(root.activePopup)) {
                openPopupAnim.restart();
            }
        }
    }

    Timer {
        id: deferredIslandOpenTimer
        interval: 12
        repeat: false
        onTriggered: {
            if (root.activePopup !== "" && root.isIslandName(root.activePopup)) {
                openIslandAnim.restart();
            }
        }
    }

    onActivePopupChanged: {
        if (activePopup !== "") {
            closePopupAnim.stop();
            closeIslandAnim.stop();
            deferredOpenTimer.stop();
            deferredIslandOpenTimer.stop();
            morphAnim.stop();
            switchFadeAnim.stop();
            updatePopupOrigin(activePopup);
            var wasClosed = (renderedPopup === "");
            var nextIsIsland = isIslandName(activePopup);
            var prevIsTop = isTopIslandName(renderedPopup);
            var nextIsTop = isTopIslandName(activePopup);
            var sameAnchorSide = (root.barPos === "top") || (prevIsTop === nextIsTop);
            if (wasClosed || !sameAnchorSide) {
                allowMorph = false;
                morphP = 1.0;
                morphFadeOpacity = 1.0;
                popupAnimProgress = 0.0;
                islandWidthP = 0.0;
                islandHeightP = 0.0;
                islandContentP = 0.0;
                renderedPopup = activePopup;
                if (nextIsIsland) {
                    deferredIslandOpenTimer.restart();
                } else {
                    deferredOpenTimer.restart();
                }
            } else {
                // Smooth locked 1:1 Dynamic Island morph from previous popup size/pos to next popup
                var prevCard = isTopIslandName(renderedPopup) ? topPopupCard : popupCard;
                morphFromBodyW = prevCard.islandBodyW;
                morphFromBodyH = prevCard.islandBodyH;
                morphFromX = prevCard.x;
                morphFromY = prevCard.y;
                morphP = 0.0;
                morphFadeOpacity = 0.0;
                allowMorph = true;
                openIslandAnim.stop();
                openPopupAnim.stop();
                islandWidthP = 1.0;
                islandHeightP = 1.0;
                islandContentP = 1.0;
                popupAnimProgress = 1.0;
                renderedPopup = activePopup;
                morphAnim.restart();
                switchFadeAnim.restart();
            }
        } else if (renderedPopup !== "") {
            openedByHover = false;
            hoverCloseTimer.stop();
            deferredOpenTimer.stop();
            deferredIslandOpenTimer.stop();
            openPopupAnim.stop();
            openIslandAnim.stop();
            morphAnim.stop();
            switchFadeAnim.stop();
            allowMorph = false;
            if (morphP < 0.999) {
                var closeCard = isTopIslandName(renderedPopup) ? topPopupCard : popupCard;
                var curW = closeCard.islandBodyW;
                var curH = closeCard.islandBodyH;
                var span = closeCard.islandStartSpan;
                if (closeCard.verticalIsland) {
                    islandHeightP = Math.max(0.02, Math.min(1.0, curW / Math.max(1, closeCard.targetBodyW)));
                    islandWidthP = Math.max(0.0, Math.min(1.0, (curH - span) / Math.max(1, closeCard.targetBodyH - span)));
                } else {
                    islandWidthP = Math.max(0.0, Math.min(1.0, (curW - span) / Math.max(1, closeCard.targetBodyW - span)));
                    islandHeightP = Math.max(0.02, Math.min(1.0, curH / Math.max(1, closeCard.targetBodyH)));
                }
            }
            morphP = 1.0;
            morphFadeOpacity = 1.0;
            updatePopupOrigin(renderedPopup);
            if (isIslandName(renderedPopup)) {
                closeIslandAnim.restart();
            } else {
                closePopupAnim.restart();
            }
        }
    }

    NumberAnimation {
        id: morphAnim
        target: root
        property: "morphP"
        from: 0.0
        to: 1.0
        duration: 280
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
    }

    NumberAnimation {
        id: switchFadeAnim
        target: root
        property: "morphFadeOpacity"
        from: 0.0
        to: 1.0
        duration: 190
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0]
    }

    ParallelAnimation {
        id: openIslandAnim
        onStarted: root.allowMorph = true
        NumberAnimation {
            target: root
            property: "islandWidthP"
            to: 1.0
            duration: 270
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
        }
        NumberAnimation {
            target: root
            property: "islandHeightP"
            to: 1.0
            duration: 290
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
        }
        SequentialAnimation {
            PauseAnimation { duration: 15 }
            NumberAnimation {
                target: root
                property: "islandContentP"
                to: 1.0
                duration: 200
                easing.type: Easing.BezierSpline
                easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0]
            }
        }
        NumberAnimation {
            target: root
            property: "popupAnimProgress"
            to: 1.0
            duration: 270
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
        }
    }

    ParallelAnimation {
        id: closeIslandAnim
        NumberAnimation {
            target: root
            property: "islandContentP"
            to: 0.0
            duration: 75
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: root
            property: "islandHeightP"
            to: 0.0
            duration: 215
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.32, 0.0, 0.15, 1.0, 1.0, 1.0]
        }
        NumberAnimation {
            target: root
            property: "islandWidthP"
            to: 0.0
            duration: 215
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.32, 0.0, 0.15, 1.0, 1.0, 1.0]
        }
        NumberAnimation {
            target: root
            property: "popupAnimProgress"
            to: 0.0
            duration: 215
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.32, 0.0, 0.15, 1.0, 1.0, 1.0]
        }
        onFinished: {
            if (root.activePopup === "") {
                root.allowMorph = false;
                root.renderedPopup = "";
            }
        }
    }

    NumberAnimation {
        id: openPopupAnim
        target: root
        property: "popupAnimProgress"
        to: 1.0
        duration: 280
        easing.type: Easing.OutQuint
        onStarted: root.allowMorph = true
    }

    NumberAnimation {
        id: closePopupAnim
        target: root
        property: "popupAnimProgress"
        to: 0.0
        duration: 220
        easing.type: Easing.OutQuart
        onFinished: {
            if (root.activePopup === "") {
                root.allowMorph = false;
                root.renderedPopup = "";
            }
        }
    }

    IpcHandler {
        target: "settings"
        function toggle(): void {
            if (root.activePopup === "settings") {
                root.activePopup = "";
            } else {
                root.openSettingsTab("time", false);
            }
        }
        function open(): void {
            root.openSettingsTab(root.settingsInitialTab || "time", false);
        }
        function openTab(tab: string): void {
            root.openSettingsTab(tab || "time", false);
        }
        function close(): void {
            if (root.activePopup === "settings") root.activePopup = "";
        }
    }

    IpcHandler {
        target: "lock"
        function lock(): void {
            root.activePopup = "";
            Services.lockScreen();
        }
    }

    IpcHandler {
        target: "launcher"
        function toggle(): void {
            root.togglePopup("launcher");
        }
        function open(): void {
            root.activePopup = "launcher";
        }
        function close(): void {
            if (root.activePopup === "launcher") root.activePopup = "";
        }
    }

    IpcHandler {
        target: "files"
        function toggle(): void {
            root.togglePopup("files");
        }
        function open(): void {
            root.activePopup = "files";
        }
        function openDir(dir: string): void {
            if (dir && dir.length > 0 && filesCache.item && filesCache.item.navigateTo) {
                filesCache.item.navigateTo(dir, true);
            }
            root.activePopup = "files";
        }
        function close(): void {
            if (root.activePopup === "files") root.activePopup = "";
        }
    }

    IpcHandler {
        target: "notifications"
        function toggle(): void {
            root.togglePopup("notifications");
        }
        function open(): void {
            Services.markNotifsRead();
            root.activePopup = "notifications";
        }
        function close(): void {
            if (root.activePopup === "notifications") root.activePopup = "";
        }
        function clear(): void {
            Services.clearAllNotifs();
        }
        function toggleDnd(): void {
            Services.toggleDnd();
        }
    }

    IpcHandler {
        target: "antigravity"
        function toggle(): void {
            root.togglePopup("antigravity");
        }
        function open(): void {
            root.activePopup = "antigravity";
        }
        function close(): void {
            if (root.activePopup === "antigravity") root.activePopup = "";
        }
        function clear(): void {
            Services.agyNewChat();
        }
    }

    IpcHandler {
        target: "media"
        function toggle(): void {
            root.togglePopup("media");
        }
        function open(): void {
            root.activePopup = "media";
        }
        function close(): void {
            if (root.activePopup === "media") root.activePopup = "";
        }
        function playPause(): void {
            Services.mediaTogglePlay();
        }
        function next(): void {
            Services.mediaNext();
        }
        function previous(): void {
            Services.mediaPrev();
        }
    }

    IpcHandler {
        target: "popup"
        function toggle(name: string): void {
            if (name === "clock") {
                if (root.activePopup === "settings") root.activePopup = "";
                else root.openSettingsTab("time", false);
                return;
            }
            root.togglePopup(name);
        }
        function open(name: string): void {
            if (name === "clock") {
                root.openSettingsTab("time", false);
                return;
            }
            if (root.activePopup !== name) root.togglePopup(name);
        }
        function close(): void {
            root.activePopup = "";
        }
    }

    IpcHandler {
        target: "display"
        function cycle(): void {
            root.activePopup = "";
            desktopMenu.close();
            displaySwitcher.cycle();
        }
        function open(): void {
            root.activePopup = "";
            desktopMenu.close();
            displaySwitcher.openSwitcher();
        }
        function close(): void {
            displaySwitcher.dismiss();
        }
    }

    LockScreen {}
    WallpaperTransitionOverlay {}
    ThemeWaveOverlay {}
    PortalFileDialog {}
    DesktopContextMenu {
        id: desktopMenu
        barPos: root.barPos
        barBottom: root.barBottom
        popupOpen: root.renderedPopup !== "" || displaySwitcher.opened
        onOpenLauncherRequested: root.openPopup("launcher", false)
        onOpenSettingsTabRequested: tabId => root.openSettingsTab(tabId, false)
        onOpenDisplaySwitcherRequested: displaySwitcher.openSwitcher()
        onOpenPowerRequested: root.openPopup("power", false)
    }
    DisplaySwitcherOverlay {
        id: displaySwitcher
        onOpenDisplaySettingsRequested: root.openSettingsTab("monitors", false)
    }
    NotificationToastOverlay {
        suppressToasts: root.isCenterIsland(root.renderedPopup)
        barSwitching: root.barRemapping
        onOpenCenterRequested: root.togglePopup("notifications")
    }

    Timer {
        id: postOpenRefreshTimer
        interval: 300
        repeat: false
        onTriggered: {
            if (root.activePopup === "wifi") Services.refreshWifi();
            else if (root.activePopup === "settings") {
                Services.refreshSystemStats();
                Services.refreshBattery();
                Services.refreshAudioOutputs();
            }
        }
    }

    Timer {
        id: hoverCloseTimer
        interval: 260
        repeat: false
        onTriggered: {
            if (root.openedByHover && !root.anyHoverTriggerHovered && !root.popupCardHovered) {
                root.activePopup = "";
            }
        }
    }

    function openSettingsTab(tabId, byHover) {
        const nextTab = (tabId && tabId.length > 0) ? tabId : "time";
        root.settingsInitialTab = nextTab;
        if (settingsCache.item && settingsCache.item.switchTab) {
            settingsCache.item.switchTab(nextTab);
        }
        openPopup("settings", byHover);
    }

    function openPopup(name, byHover) {
        hoverCloseTimer.stop();
        postOpenRefreshTimer.stop();
        openedByHover = byHover === true;
        if (activePopup === name) return;
        if (name === "wifi") {
            if (Services.wifiNetworks.length === 0) {
                Services.refreshWifi();
            } else {
                postOpenRefreshTimer.restart();
            }
        }
        if (name === "bluetooth") Services.refreshBluetooth();
        if (name === "volume") Services.refreshAudioOutputs();
        if (name === "battery") Services.refreshBattery();
        if (name === "notifications") Services.markNotifsRead();
        if (name === "settings") postOpenRefreshTimer.restart();
        activePopup = name;
    }

    function scheduleHoverClose() {
        if (openedByHover && activePopup !== "" && !anyHoverTriggerHovered && !popupCardHovered) {
            hoverCloseTimer.restart();
        }
    }

    function togglePopup(name) {
        if (activePopup === name) {
            postOpenRefreshTimer.stop();
            activePopup = "";
        } else {
            openPopup(name, false);
        }
    }

    function batteryIcon(pct, charging) {
        if (charging) return "󰂄";
        if (pct >= 90) return "󰁹";
        if (pct >= 75) return "󰂁";
        if (pct >= 55) return "󰁿";
        if (pct >= 35) return "󰁽";
        if (pct >= 15) return "󰁻";
        return "󰁺";
    }

    PanelWindow {
        id: barWindow
        property bool anchorsReady: false
        function syncAnchors() {
            anchors.top = false;
            anchors.bottom = false;
            anchors.left = false;
            anchors.right = false;
            if (root.barPos === "bottom") {
                anchors.left = true;
                anchors.right = true;
                anchors.bottom = true;
            } else if (root.barPos === "left") {
                anchors.top = true;
                anchors.bottom = true;
                anchors.left = true;
            } else if (root.barPos === "right") {
                anchors.top = true;
                anchors.bottom = true;
                anchors.right = true;
            } else {
                anchors.left = true;
                anchors.right = true;
                anchors.top = true;
            }
            anchorsReady = true;
        }
        visible: Services.settingsReady && anchorsReady && !root.barRemapping
        anchors {
            top: false
            bottom: false
            left: false
            right: false
        }
        Component.onCompleted: syncAnchors()
        Connections {
            target: root
            function onBarPosChanged() { barWindow.syncAnchors(); }
        }
        margins {
            top: 0
            bottom: 0
            left: 0
            right: 0
        }
        implicitHeight: root.barVertical ? 0 : root.barSpan
        implicitWidth: root.barVertical ? root.barSpan : 0
        WlrLayershell.namespace: "qs-bar"
        color: "transparent"

        Rectangle {
            id: barContent
            anchors.fill: parent
            anchors.margins: Services.barFloating ? 5 : 0
            radius: Services.barFloating ? Math.max(11, Theme.radius) : Theme.radius
            color: Theme.barBg
            border.width: (Services.barFloating || Theme.radius > 0) ? 1 : 0
            border.color: Theme.border
            opacity: root.barOpacity
            transform: [
                Scale {
                    origin.x: barContent.width / 2
                    origin.y: barContent.height / 2
                    xScale: root.barScale
                    yScale: root.barScale
                },
                Translate {
                    x: root.barVertical ? root.barSlideOffset : 0
                    y: root.barVertical ? 0 : root.barSlideOffset
                }
            ]

            // Edge subtle 1px divider (docked square mode only; rounded/floating uses barContent.border)
            Rectangle {
                visible: !Services.barFloating && Theme.radius === 0
                x: root.barRight ? 0 : (root.barLeft ? (parent.width - 1) : 0)
                y: root.barBottom ? 0 : (root.barVertical ? 0 : (parent.height - 1))
                width: root.barVertical ? 1 : parent.width
                height: root.barVertical ? parent.height : 1
                color: Theme.border
            }

            // LEFT / TOP: Launcher + Workspaces + RAM Usage
            Grid {
                id: leftCluster
                x: root.barVertical ? Math.round((parent.width - width) / 2) : 8
                y: root.barVertical ? 8 : Math.round((parent.height - height) / 2)
                columns: root.barVertical ? 1 : 16
                horizontalItemAlignment: Grid.AlignHCenter
                verticalItemAlignment: Grid.AlignVCenter
                spacing: 6

                BarIcon {
                    id: launcherIcon
                    vertical: root.barVertical
                    iconSource: Qt.resolvedUrl("assets/yuno-gasai-icon.png")
                    iconImageSize: 23
                    active: root.activePopup === "antigravity" || Services.agyBusy
                    onClicked: root.togglePopup("antigravity")
                }

                BarIcon {
                    id: filesIcon
                    vertical: root.barVertical
                    icon: "󰉋"
                    iconColor: Theme.text
                    active: root.activePopup === "files"
                    onClicked: root.togglePopup("files")
                    onRightClicked: Services.openFiles()
                }

                WorkspaceBar {
                    vertical: root.barVertical
                }
                RamWidget {
                    visible: Services.showRamOnBar
                    vertical: root.barVertical
                }
            }

            // CENTER: Clock + Active Media Pill
            Grid {
                id: centerCluster
                x: Math.round((parent.width - width) / 2)
                y: Math.round((parent.height - height) / 2)
                columns: root.barVertical ? 1 : 16
                horizontalItemAlignment: Grid.AlignHCenter
                verticalItemAlignment: Grid.AlignVCenter
                spacing: 8

                ClockWidget {
                    id: clockWidget
                    vertical: root.barVertical
                    active: root.activePopup === "settings"
                    onEntered: {
                        if (root.activePopup === "" || root.openedByHover || root.activePopup !== "settings") {
                            root.openSettingsTab("time", true);
                        } else {
                            hoverCloseTimer.stop();
                        }
                    }
                    onExited: root.scheduleHoverClose()
                    onClicked: {
                        if (root.activePopup === "settings" && !root.openedByHover) {
                            root.activePopup = "";
                        } else {
                            root.openSettingsTab("time", false);
                        }
                    }
                }

                MediaBarPill {
                    id: mediaBarPill
                    visible: Services.showMediaOnBar && width > 1 && height > 1
                    vertical: root.barVertical
                    active: root.activePopup === "media"
                    onClicked: root.togglePopup("media")
                    onRightClicked: Services.mediaTogglePlay()
                }
            }

            // RIGHT / BOTTOM: Tray + Controls + Power
            Grid {
                id: rightRow
                x: root.barVertical ? Math.round((parent.width - width) / 2) : Math.round(parent.width - width - 6)
                y: root.barVertical ? Math.round(parent.height - height - 6) : Math.round((parent.height - height) / 2)
                columns: root.barVertical ? 1 : 24
                horizontalItemAlignment: Grid.AlignHCenter
                verticalItemAlignment: Grid.AlignVCenter
                spacing: 2

                TrayWidget {
                    vertical: root.barVertical
                    barWindow: barWindow
                }

                // Bluetooth
                BarIcon {
                    id: btIcon
                    vertical: root.barVertical
                    icon: Services.btPowered ? (Services.btConnectedCount > 0 ? "󰂱" : "󰂯") : "󰂲"
                    iconColor: Theme.text
                    active: root.activePopup === "bluetooth"
                    onEntered: root.openPopup("bluetooth", true)
                    onExited: root.scheduleHoverClose()
                    onClicked: root.openPopup("bluetooth", root.openedByHover)
                    onRightClicked: Services.toggleBluetooth()
                }

                // Brightness
                BarIcon {
                    id: brightIcon
                    vertical: root.barVertical
                    icon: "󰃠"
                    iconColor: Theme.text
                    active: root.activePopup === "brightness"
                    onEntered: root.openPopup("brightness", true)
                    onExited: root.scheduleHoverClose()
                    onClicked: root.openPopup("brightness", root.openedByHover)
                    onWheeled: delta => {
                        Services.setBrightness(Services.brightnessPercent + (delta > 0 ? 5 : -5));
                    }
                }

                // Volume
                BarIcon {
                    id: volIcon
                    vertical: root.barVertical
                    icon: Services.volumeMuted || Services.volumePercent === 0 ? "󰝟"
                        : Services.volumePercent >= 60 ? "󰕾" : "󰖀"
                    iconColor: Theme.text
                    active: root.activePopup === "volume"
                    onEntered: root.openPopup("volume", true)
                    onExited: root.scheduleHoverClose()
                    onClicked: root.openPopup("volume", root.openedByHover)
                    onRightClicked: Services.toggleMute()
                    onWheeled: delta => {
                        Services.setVolume(Services.volumePercent + (delta > 0 ? 5 : -5));
                    }
                }

                // Battery (between Volume and Wi-Fi)
                BarIcon {
                    id: batIcon
                    vertical: root.barVertical
                    icon: root.batteryIcon(Services.batteryPercent, Services.batteryCharging)
                    iconColor: Theme.text
                    active: root.activePopup === "battery"
                    onEntered: root.openPopup("battery", true)
                    onExited: root.scheduleHoverClose()
                    onClicked: root.openPopup("battery", root.openedByHover)
                }

                // Wi-Fi
                BarIcon {
                    id: wifiIcon
                    vertical: root.barVertical
                    icon: !Services.wifiEnabled ? "󰤭"
                        : Services.wifiCurrentSsid.length === 0 ? "󰤯"
                        : Services.wifiCurrentSignal >= 75 ? "󰤨"
                        : Services.wifiCurrentSignal >= 50 ? "󰤥"
                        : Services.wifiCurrentSignal >= 25 ? "󰤢" : "󰤟"
                    iconColor: Theme.text
                    active: root.activePopup === "wifi"
                    onEntered: root.openPopup("wifi", true)
                    onExited: root.scheduleHoverClose()
                    onClicked: root.openPopup("wifi", root.openedByHover)
                    onRightClicked: Services.toggleWifi()
                }

                // Power (rightmost / bottommost)
                BarIcon {
                    id: powerIcon
                    vertical: root.barVertical
                    icon: "󰐥"
                    iconColor: Theme.text
                    active: root.activePopup === "power"
                    onEntered: root.openPopup("power", true)
                    onExited: root.scheduleHoverClose()
                    onClicked: root.openPopup("power", root.openedByHover)
                }
            }
        }
    }

    // Persistent Overlay PanelWindow for outside-click backdrop + bar-attached icon popups
    PanelWindow {
        id: overlayWindow
        visible: Services.settingsReady && !root.barRemapping
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        mask: Region {
            item: (root.renderedPopup !== "" && !root.externalDragActive) ? overlayBackdropArea : emptyOverlayMask
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-overlay"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: (root.activePopup === "" || root.isTopIslandName(root.activePopup))
                                     ? WlrKeyboardFocus.None
                                     : (root.activePopup === "wifi" || root.activePopup === "antigravity")
                                       ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        color: "transparent"

        Item {
            id: emptyOverlayMask
            width: 0
            height: 0
        }

        Item {
            id: overlayBackdropArea
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.top: parent.top
            anchors.topMargin: root.barPos === "top" ? root.barSpan : 0
            anchors.bottomMargin: root.barPos === "bottom" ? root.barSpan : 0
            anchors.leftMargin: root.barPos === "left" ? root.barSpan : 0
            anchors.rightMargin: root.barPos === "right" ? root.barSpan : 0
            visible: root.renderedPopup !== ""

            // Keep backdrop transparent so rounded/floating bars never get a sharp dim cutoff line
            Rectangle {
                anchors.fill: parent
                color: "transparent"
            }

            // Click outside backdrop
            MouseArea {
                anchors.fill: parent
                onClicked: root.activePopup = ""
            }
        }

        readonly property int cardWidth: (root.renderedPopup === "antigravity") ? 360
                                       : (root.renderedPopup === "wifi" || root.renderedPopup === "bluetooth") ? 332
                                       : (root.renderedPopup === "volume" || root.renderedPopup === "battery") ? 312
                                       : (root.renderedPopup === "brightness") ? 292
                                       : (root.renderedPopup === "power") ? 260
                                       : 292

        Item {
            id: popupCard
            visible: root.renderedPopup !== "" && !root.isTopIslandName(root.renderedPopup)
            readonly property var activeItem: transientLoader.item
            readonly property bool animating: openPopupAnim.running || closePopupAnim.running || openIslandAnim.running || closeIslandAnim.running || morphAnim.running || switchFadeAnim.running || deferredIslandOpenTimer.running || deferredOpenTimer.running || root.barSwitching
            readonly property bool isMorphing: root.morphP < 0.999
            readonly property bool isTopIsland: false
            readonly property bool verticalIsland: root.barVertical
            readonly property bool islandInvertY: root.barBottom
            readonly property bool islandInvertX: verticalIsland && root.barRight
            readonly property bool useIslandShell: true
            readonly property bool isIsland: root.isIslandName(root.renderedPopup)
            readonly property bool isModal: false
            readonly property real zoomScale: isIsland ? 1.0 : (0.06 + 0.94 * root.popupAnimProgress)
            readonly property int islandEar: 16
            readonly property real edgeInset: Services.barFloating ? Math.max(8, Math.min(24, Theme.radius + 4)) : 6
            readonly property int targetBodyW: overlayWindow.cardWidth
            readonly property int targetBodyH: (activeItem ? activeItem.height : 0) + 34
            readonly property int islandTargetH: targetBodyH
            readonly property real islandStartSpan: verticalIsland
                                                    ? Math.min(targetBodyH, Math.max(44, root.originWidth))
                                                    : Math.min(targetBodyW, root.originWidth)
            readonly property real openBodyW: verticalIsland
                                              ? Math.max(2, targetBodyW * root.islandHeightP)
                                              : Math.max(40, islandStartSpan + (targetBodyW - islandStartSpan) * root.islandWidthP)
            readonly property real openBodyH: verticalIsland
                                              ? Math.max(40, islandStartSpan + (targetBodyH - islandStartSpan) * root.islandWidthP)
                                              : Math.max(2, targetBodyH * root.islandHeightP)
            readonly property real islandBodyW: isMorphing
                                                ? (root.morphFromBodyW + (targetBodyW - root.morphFromBodyW) * root.morphP)
                                                : openBodyW
            readonly property real islandBodyH: isMorphing
                                                ? (root.morphFromBodyH + (targetBodyH - root.morphFromBodyH) * root.morphP)
                                                : openBodyH

            readonly property real finalW: verticalIsland ? targetBodyW : (targetBodyW + 2 * islandEar)
            readonly property real startW: verticalIsland ? 2 : (islandStartSpan + 2 * islandEar)
            readonly property real startX: verticalIsland
                                           ? (root.barLeft
                                              ? (Services.barFloating ? (root.barSpan - 6) : (root.barSpan - 1))
                                              : (Services.barFloating ? (overlayWindow.width - root.barSpan + 6) : (overlayWindow.width - root.barSpan + 1)))
                                           : Math.max(edgeInset, Math.min(overlayWindow.width - startW - edgeInset, root.originX - startW / 2))
            readonly property real finalX: verticalIsland
                                           ? startX
                                           : Math.max(edgeInset, Math.min(overlayWindow.width - finalW - edgeInset, root.originX - finalW / 2))
            readonly property real startY: verticalIsland
                                           ? Math.max(edgeInset, Math.min(overlayWindow.height - (islandStartSpan + 2 * islandEar) - edgeInset, root.originY - (islandStartSpan + 2 * islandEar) / 2))
                                           : 0
            readonly property real finalY: verticalIsland
                                           ? Math.max(edgeInset, Math.min(overlayWindow.height - (targetBodyH + 2 * islandEar) - edgeInset, root.originY - (targetBodyH + 2 * islandEar) / 2))
                                           : 0

            readonly property real openX: verticalIsland
                                          ? (root.barRight ? (startX - width) : startX)
                                          : (startX + (finalX - startX) * root.islandWidthP)
            readonly property real openY: verticalIsland
                                          ? (startY + (finalY - startY) * root.islandWidthP)
                                          : (root.barBottom
                                             ? ((Services.barFloating ? (overlayWindow.height - root.barSpan + 6) : (overlayWindow.height - root.barSpan + 1)) - height)
                                             : (Services.barFloating ? (root.barSpan - 6) : (root.barSpan - 1)))
            readonly property real targetX: verticalIsland
                                            ? (root.barRight ? (startX - targetBodyW) : startX)
                                            : finalX
            readonly property real targetY: verticalIsland
                                            ? finalY
                                            : (root.barBottom
                                               ? ((Services.barFloating ? (overlayWindow.height - root.barSpan + 6) : (overlayWindow.height - root.barSpan + 1)) - targetBodyH)
                                               : (Services.barFloating ? (root.barSpan - 6) : (root.barSpan - 1)))

            width: verticalIsland ? islandBodyW : (islandBodyW + 2 * islandEar)
            height: verticalIsland ? (islandBodyH + 2 * islandEar) : islandBodyH
            x: isMorphing ? (root.morphFromX + (targetX - root.morphFromX) * root.morphP) : openX
            y: isMorphing ? (root.morphFromY + (targetY - root.morphFromY) * root.morphP) : openY
            opacity: root.islandHeightP > 0.002 ? 1.0 : 0.001

            DynamicIslandShell {
                visible: true
                anchors.fill: parent
                bodyWidth: popupCard.islandBodyW
                bodyHeight: popupCard.islandBodyH
                earRadius: popupCard.islandEar
                bottomRadius: Math.max(16, Theme.radius > 0 ? Theme.radius : 24)
                topColor: Theme.barBg
                fillColor: Theme.popupBg
                borderColor: Theme.border
                invertY: popupCard.islandInvertY
                vertical: popupCard.verticalIsland
                invertX: popupCard.islandInvertX
            }

            HoverHandler {
                onHoveredChanged: {
                    root.popupCardHovered = hovered;
                    if (hovered) hoverCloseTimer.stop();
                    else root.scheduleHoverClose();
                }
            }

            MouseArea { anchors.fill: parent }

            Item {
                anchors.fill: parent
                anchors.leftMargin: popupCard.verticalIsland ? (popupCard.islandInvertX ? 14 : 16) : popupCard.islandEar
                anchors.rightMargin: popupCard.verticalIsland ? (popupCard.islandInvertX ? 16 : 14) : popupCard.islandEar
                anchors.topMargin: popupCard.verticalIsland ? popupCard.islandEar : (popupCard.islandInvertY ? 14 : 0)
                anchors.bottomMargin: popupCard.verticalIsland ? popupCard.islandEar : (!popupCard.islandInvertY ? 14 : 0)
                clip: true

                Item {
                    width: popupCard.targetBodyW - 36
                    height: Math.max(1, popupCard.targetBodyH - 34)
                    x: popupCard.verticalIsland
                       ? (popupCard.islandInvertX ? 3 : (popupCard.islandBodyW - 30 - width - 3))
                       : 18
                    y: popupCard.verticalIsland
                       ? Math.round((popupCard.islandBodyH - height) / 2)
                       : (popupCard.islandInvertY
                          ? Math.round(popupCard.islandBodyH - popupCard.targetBodyH + 20)
                          : 14)
                    opacity: root.islandContentP * root.morphFadeOpacity
                    transform: Translate {
                        x: popupCard.verticalIsland ? ((1.0 - root.islandHeightP) * (popupCard.islandInvertX ? 10.0 : -10.0)) : 0.0
                        y: (popupCard.verticalIsland ? 0.0 : ((1.0 - root.islandHeightP) * (popupCard.islandInvertY ? 10.0 : -10.0)))
                           + (1.0 - root.morphFadeOpacity) * 6.0
                    }

                    Loader {
                        id: transientLoader
                        width: parent.width
                        focus: visible
                        visible: sourceComponent !== null
                        sourceComponent: {
                            if (root.renderedPopup === "volume") return volumeComp;
                            if (root.renderedPopup === "brightness") return brightnessComp;
                            if (root.renderedPopup === "battery") return batteryComp;
                            if (root.renderedPopup === "wifi") return wifiComp;
                            if (root.renderedPopup === "bluetooth") return bluetoothComp;
                            if (root.renderedPopup === "power") return powerComp;
                            if (root.renderedPopup === "antigravity") return antigravityComp;
                            return null;
                        }
                        onLoaded: {
                            if (root.renderedPopup === "antigravity" && item && item.focusInput) {
                                Qt.callLater(item.focusInput);
                            }
                        }
                    }
                }
            }
        }
    }

    // Tight 812x580 Dynamic Island PanelWindow for Launcher / Settings / Notifications / Media / Clock
    // Reduces Intel UHD 620 fillrate & swapchain bandwidth by ~5x vs fullscreen 1920x1200!
    PanelWindow {
        id: topIslandWindow
        visible: Services.settingsReady && !root.barRemapping
        anchors {
            top: true
        }
        margins.top: root.barPos === "top"
                     ? (Services.barFloating ? (root.barSpan - 6) : (root.barSpan - 1))
                     : 0
        implicitWidth: 932
        implicitHeight: 450
        mask: Region {
            item: (root.renderedPopup !== "" && root.isTopIslandName(root.renderedPopup)) ? topPopupCard : emptyTopMask
        }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.namespace: "qs-overlay"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: !root.isTopIslandName(root.activePopup)
                                     ? WlrKeyboardFocus.None
                                     : root.activePopup === "launcher"
                                       ? WlrKeyboardFocus.Exclusive
                                       : (root.activePopup === "settings" || root.activePopup === "notifications" || root.activePopup === "files")
                                         ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        color: "transparent"

        Item {
            id: emptyTopMask
            width: 0
            height: 0
        }

        Item {
            id: topPopupCard
            visible: root.renderedPopup !== "" && root.isTopIslandName(root.renderedPopup)
            readonly property bool verticalIsland: false
            readonly property var activeItem: {
                if (root.renderedPopup === "launcher") return launcherCache.item;
                if (root.renderedPopup === "notifications") return notificationsCache.item;
                if (root.renderedPopup === "media") return mediaCache.item;
                if (root.renderedPopup === "settings") return settingsCache.item;
                if (root.renderedPopup === "files") return filesCache.item;
                if (root.renderedPopup === "clock") return clockTopLoader.item;
                return null;
            }
            readonly property bool isMorphing: root.morphP < 0.999
            readonly property int islandEar: 16
            readonly property int targetBodyW: (root.renderedPopup === "settings" || root.renderedPopup === "files") ? 900
                                             : (root.renderedPopup === "launcher") ? 680
                                             : (root.renderedPopup === "notifications") ? 440
                                             : (root.renderedPopup === "media") ? 356
                                             : 292
            readonly property int targetBodyH: (activeItem ? activeItem.height : 0) + 34
            readonly property real islandStartSpan: Math.min(targetBodyW, root.originWidth)
            readonly property real openBodyW: Math.max(40, islandStartSpan + (targetBodyW - islandStartSpan) * root.islandWidthP)
            readonly property real openBodyH: Math.max(2, targetBodyH * root.islandHeightP)
            readonly property real islandBodyW: isMorphing
                                                ? (root.morphFromBodyW + (targetBodyW - root.morphFromBodyW) * root.morphP)
                                                : openBodyW
            readonly property real islandBodyH: isMorphing
                                                ? (root.morphFromBodyH + (targetBodyH - root.morphFromBodyH) * root.morphP)
                                                : openBodyH

            width: islandBodyW + 2 * islandEar
            height: islandBodyH
            x: (parent.width - width) / 2
            y: 0
            opacity: root.islandHeightP > 0.002 ? 1.0 : 0.001

            DynamicIslandShell {
                visible: true
                anchors.fill: parent
                bodyWidth: topPopupCard.islandBodyW
                bodyHeight: topPopupCard.islandBodyH
                earRadius: topPopupCard.islandEar
                bottomRadius: Math.max(16, Theme.radius > 0 ? Theme.radius : 24)
                topColor: (root.barPos !== "top") ? Theme.popupBg : Theme.barBg
                fillColor: Theme.popupBg
                borderColor: Theme.border
                invertY: false
                vertical: false
                invertX: false
            }

            HoverHandler {
                onHoveredChanged: {
                    root.popupCardHovered = hovered;
                    if (hovered) hoverCloseTimer.stop();
                    else root.scheduleHoverClose();
                }
            }

            MouseArea { anchors.fill: parent }

            Item {
                anchors.fill: parent
                anchors.leftMargin: topPopupCard.islandEar
                anchors.rightMargin: topPopupCard.islandEar
                anchors.bottomMargin: 14
                clip: true

                Item {
                    id: topContentStage
                    width: topPopupCard.targetBodyW - 36
                    height: Math.max(1, topPopupCard.targetBodyH - 34)
                    x: Math.round((parent.width - width) / 2)
                    y: 14
                    opacity: root.islandContentP * root.morphFadeOpacity
                    transform: Translate {
                        y: -(1.0 - root.islandHeightP) * 10.0 + (1.0 - root.morphFadeOpacity) * 6.0
                    }

                    Loader {
                        id: launcherCache
                        width: 644
                        active: root.warmLauncher || root.renderedPopup === "launcher"
                        visible: root.renderedPopup === "launcher"
                        focus: visible
                        sourceComponent: launcherComp
                        onLoaded: {
                            root.warmLauncher = true;
                            if (root.renderedPopup === "launcher" && item && item.resetAndFocus) {
                                Qt.callLater(item.resetAndFocus);
                            }
                        }
                    }

                    Loader {
                        id: notificationsCache
                        width: 404
                        active: root.warmNotifications || root.renderedPopup === "notifications"
                        visible: root.renderedPopup === "notifications"
                        focus: visible
                        sourceComponent: notificationsComp
                        onLoaded: root.warmNotifications = true
                    }

                    Loader {
                        id: mediaCache
                        width: 320
                        active: root.warmMedia || root.renderedPopup === "media"
                        visible: root.renderedPopup === "media"
                        focus: visible
                        sourceComponent: mediaComp
                        onLoaded: root.warmMedia = true
                    }

                    Loader {
                        id: settingsCache
                        width: 864
                        active: root.warmSettings || root.renderedPopup === "settings"
                        visible: root.renderedPopup === "settings"
                        focus: visible
                        sourceComponent: settingsComp
                        onLoaded: {
                            root.warmSettings = true;
                            if (root.settingsInitialTab && item && item.switchTab) {
                                item.switchTab(root.settingsInitialTab);
                            }
                        }
                    }

                    Loader {
                        id: filesCache
                        width: 864
                        active: root.warmFiles || root.renderedPopup === "files"
                        visible: root.renderedPopup === "files"
                        focus: visible
                        sourceComponent: filesComp
                        onLoaded: {
                            root.warmFiles = true;
                            if (root.renderedPopup === "files" && item && item.focusDefault) {
                                Qt.callLater(item.focusDefault);
                            }
                        }
                    }

                    Loader {
                        id: clockTopLoader
                        width: 256
                        focus: visible
                        visible: sourceComponent !== null
                        sourceComponent: root.renderedPopup === "clock" ? clockComp : null
                    }
                }
            }
        }
    }

    PolkitDialog {}

    Component { id: volumeComp; VolumePopup {} }
    Component { id: brightnessComp; BrightnessPopup {} }
    Component { id: batteryComp; BatteryPopup {} }
    Component { id: wifiComp; WifiPopup {} }
    Component { id: bluetoothComp; BluetoothPopup {} }
    Component {
        id: powerComp
        PowerPopup {
            onCloseRequested: root.activePopup = ""
        }
    }
    Component { id: clockComp; ClockPopup {} }
    Component { id: mediaComp; MediaPopup {} }
    Component {
        id: antigravityComp
        AntigravityMiniPopup {
            onCloseRequested: root.activePopup = ""
        }
    }
    Component {
        id: notificationsComp
        NotificationCenterPopup {
            onCloseRequested: root.activePopup = ""
        }
    }
    Component {
        id: launcherComp
        AppLauncherPopup {
            onCloseRequested: root.activePopup = ""
        }
    }
    Component {
        id: filesComp
        FilesPopup {
            onCloseRequested: root.activePopup = ""
            onExternalDragChanged: active => root.externalDragActive = active
        }
    }
    Component {
        id: settingsComp
        SettingsPopup {
            width: parent.width
            height: 354
            initialTab: root.settingsInitialTab
            onPinnedRequested: root.openedByHover = false
            onCloseRequested: root.activePopup = ""
            onOpenPowerRequested: root.activePopup = "power"
            onBarPositionRequested: pos => root.switchBarPosition(pos)
            onOpenDisplaySwitcherRequested: displaySwitcher.openSwitcher()
        }
    }
}
