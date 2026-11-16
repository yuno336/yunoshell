import QtQuick
import Quickshell
import Quickshell.Widgets
import ".."
import "../components"

Item {
    id: root
    focus: true

    signal closeRequested()
    signal openPowerRequested()
    signal barPositionRequested(string pos)
    signal pinnedRequested()
    signal openDisplaySwitcherRequested()

    property string initialTab: "time"
    property string activeTab: initialTab
    property string bgTarget: "desktop"
    property string pickerTarget: "wallpaper"
    property int calOffsetMonths: 0
    property var draftMonitors: []
    property int selectedMonitorIdx: 0
    property string draftModeSpec: ""
    property real draftScale: 1.0
    property string draftTransform: "normal"
    property bool draftEnabled: true
    property bool layoutDirty: false
    property string openMonitorDropdown: ""
    property real tabSlideX: 0
    property bool navPillReady: false
    readonly property var tabOrder: ["time", "personalize", "monitors", "system", "controls"]
    readonly property int activeTabIdx: Math.max(0, tabOrder.indexOf(activeTab))

    function toggleMonitorDropdown(name) {
        root.openMonitorDropdown = (root.openMonitorDropdown === name) ? "" : name;
    }

    function activeFlickable() {
        if (root.activeTab === "time") return timeFlick;
        if (root.activeTab === "personalize") return bgFlick;
        if (root.activeTab === "monitors") return monFlick;
        if (root.activeTab === "system") return sysFlick;
        if (root.activeTab === "controls") return ctrlFlick;
        return null;
    }

    Keys.onEscapePressed: {
        if (fileBrowser.opened) {
            fileBrowser.opened = false;
            fileBrowser.cancelled();
        } else {
            root.closeRequested();
        }
    }

    SystemClock {
        id: liveClock
        precision: SystemClock.Seconds
    }

    function switchTab(tabId) {
        if (!tabId) return;
        let nextTab = tabId;
        if (tabId === "wallpaper" || tabId === "background") {
            root.bgTarget = "desktop";
            nextTab = "personalize";
        } else if (tabId === "lockscreen") {
            root.bgTarget = "lockscreen";
            nextTab = "personalize";
        } else if (tabId === "sddm") {
            root.bgTarget = "sddm";
            nextTab = "personalize";
        } else if (tabId === "bar" || tabId === "theme") {
            root.bgTarget = "theme";
            nextTab = "personalize";
        } else if (tabId === "mango" || tabId === "mangowm") {
            root.bgTarget = "mango";
            nextTab = "personalize";
            Services.loadMangoConfig();
        } else if (tabId === "display" || tabId === "monitor") {
            nextTab = "monitors";
        }
        if (root.activeTab === nextTab) {
            if (nextTab === "monitors") Services.refreshMonitors();
            return;
        }
        var prevIdx = Math.max(0, root.tabOrder.indexOf(root.activeTab));
        var nextIdx = Math.max(0, root.tabOrder.indexOf(nextTab));
        var dir = nextIdx >= prevIdx ? 1 : -1;
        root.navPillReady = true;
        root.activeTab = nextTab;
        root.openMonitorDropdown = "";
        tabSwitchAnim.stop();
        root.tabSlideX = dir * 16;
        tabSwitchAnim.restart();
        if (nextTab === "system") {
            Services.refreshSystemStats();
        } else if (nextTab === "monitors") {
            Services.refreshMonitors();
        } else if (nextTab === "personalize" && root.bgTarget === "mango") {
            Services.loadMangoConfig();
        }
    }

    function syncDraftMonitors() {
        var src = Services.monitors || [];
        var copy = [];
        for (var i = 0; i < src.length; i++) {
            var m = src[i];
            var curSpec = "";
            if (Array.isArray(m.modes)) {
                for (var k = 0; k < m.modes.length; k++) {
                    if (m.modes[k].current) {
                        curSpec = m.modes[k].spec;
                        break;
                    }
                }
                if (!curSpec && m.modes.length > 0) curSpec = m.modes[0].spec;
            }
            copy.push({
                name: m.name,
                description: m.description,
                enabled: m.enabled,
                x: m.x,
                y: m.y,
                width: m.width,
                height: m.height,
                refresh: m.refresh,
                scale: m.scale,
                transform: m.transform,
                logicalWidth: m.logicalWidth,
                logicalHeight: m.logicalHeight,
                modes: m.modes || [],
                selectedSpec: curSpec
            });
        }
        root.draftMonitors = copy;
        if (copy.length > 0) {
            var idx = Math.max(0, Math.min(copy.length - 1, root.selectedMonitorIdx));
            root.selectMonitor(idx);
        }
        root.layoutDirty = false;
    }

    function selectMonitor(idx) {
        if (!root.draftMonitors || idx < 0 || idx >= root.draftMonitors.length) return;
        root.selectedMonitorIdx = idx;
        var m = root.draftMonitors[idx];
        root.draftEnabled = m.enabled;
        root.draftScale = m.scale || 1.0;
        root.draftTransform = m.transform || "normal";
        root.draftModeSpec = m.selectedSpec || "";
    }

    function applyPresetPosition(preset) {
        if (!root.draftMonitors || root.draftMonitors.length < 2) return;
        var copy = root.draftMonitors.map(function(m) { return Object.assign({}, m, { enabled: true }); });
        var m0 = copy[0];
        var m1 = copy[1];
        if (preset === "right") {
            m0.x = 0; m0.y = 0;
            m1.x = m0.logicalWidth; m1.y = 0;
        } else if (preset === "left") {
            m1.x = 0; m1.y = 0;
            m0.x = m1.logicalWidth; m0.y = 0;
        } else if (preset === "below") {
            m0.x = 0; m0.y = 0;
            m1.x = 0; m1.y = m0.logicalHeight;
        } else if (preset === "above") {
            m1.x = 0; m1.y = 0;
            m0.x = 0; m0.y = m1.logicalHeight;
        } else if (preset === "mirror") {
            m0.x = 0; m0.y = 0;
            m1.x = 0; m1.y = 0;
        } else if (preset === "swap") {
            if (m0.x <= m1.x) {
                m1.x = 0; m1.y = 0;
                m0.x = m1.logicalWidth; m0.y = 0;
            } else {
                m0.x = 0; m0.y = 0;
                m1.x = m0.logicalWidth; m1.y = 0;
            }
        }
        root.draftMonitors = copy;
        root.layoutDirty = false;
        Services.applyMonitorLayout(copy);
    }

    Component.onCompleted: {
        syncDraftMonitors();
        if (root.activeTab === "monitors") Services.refreshMonitors();
    }

    Connections {
        target: Services
        function onMonitorsChanged() { root.syncDraftMonitors(); }
    }

    function openImagePicker(target, startDir, title) {
        root.pinnedRequested();
        root.pickerTarget = target;
        fileBrowser.open(startDir || Services.wallpaperDir, title);
    }

    function applyPickedImage(filePath) {
        if (!filePath || filePath.length === 0) return;
        if (root.pickerTarget === "wallpaper") {
            Services.applyWallpaper(filePath, Services.wallpaperMode);
        } else if (root.pickerTarget === "lockscreen") {
            Services.setLockWallpaper(filePath);
        } else if (root.pickerTarget === "sddm") {
            Services.applySddmWallpaper(filePath);
        }
    }

    function basename(path) {
        if (!path) return "";
        const idx = path.lastIndexOf("/");
        return idx >= 0 ? path.slice(idx + 1) : path;
    }

    function formatSpeed(kbps) {
        const v = Number(kbps) || 0;
        if (v >= 1024) return (v / 1024).toFixed(1) + " MB/s";
        return Math.round(v) + " KB/s";
    }

    function wallFillMode(mode) {
        switch (mode) {
        case "fit": return Image.PreserveAspectFit;
        case "stretch": return Image.Stretch;
        case "tile": return Image.Tile;
        default: return Image.PreserveAspectCrop;
        }
    }

    ParallelAnimation {
        id: tabSwitchAnim
        NumberAnimation {
            target: contentPane
            property: "opacity"
            from: 0.25
            to: 1.0
            duration: 185
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.22, 1.0, 0.36, 1.0, 1.0, 1.0]
        }
        NumberAnimation {
            target: root
            property: "tabSlideX"
            to: 0
            duration: 230
            easing.type: Easing.BezierSpline
            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
        }
    }

    Column {
        anchors.fill: parent
        spacing: 10

        // ── Horizontal Top Navigation Bar ──
        Rectangle {
            id: topNav
            width: parent.width
            height: 36
            radius: 12
            color: Theme.surface
            border.width: 1
            border.color: Theme.border

            Row {
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 22
                    height: 22
                    radius: 7
                    color: Theme.surfaceHover

                    Text {
                        anchors.centerIn: parent
                        text: "󰒓"
                        color: Theme.accent
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                    }
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Settings"
                    color: Theme.textBright
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    font.weight: Font.Bold
                }
            }

            Item {
                id: navWrap
                anchors.centerIn: parent
                width: navRow.implicitWidth
                height: 26

                readonly property Item activeNavItem: navRepeater.count > root.activeTabIdx
                                                    ? navRepeater.itemAt(root.activeTabIdx)
                                                    : null

                // Smooth sliding active pill indicator
                Rectangle {
                    id: navIndicator
                    visible: navWrap.activeNavItem !== null
                    x: navWrap.activeNavItem ? navWrap.activeNavItem.x : 0
                    y: 0
                    width: navWrap.activeNavItem ? navWrap.activeNavItem.width : 60
                    height: 26
                    radius: 8
                    color: Theme.accent

                    Behavior on x {
                        enabled: root.navPillReady
                        NumberAnimation {
                            duration: 240
                            easing.type: Easing.BezierSpline
                            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                        }
                    }
                    Behavior on width {
                        enabled: root.navPillReady
                        NumberAnimation {
                            duration: 240
                            easing.type: Easing.BezierSpline
                            easing.bezierCurve: [0.16, 1.0, 0.3, 1.0, 1.0, 1.0]
                        }
                    }
                }

                Row {
                    id: navRow
                    anchors.fill: parent
                    spacing: 6

                    Repeater {
                        id: navRepeater
                        model: [
                            { id: "time",        icon: "󰥔", label: "Time" },
                            { id: "personalize", icon: "󰸉", label: "Personalize" },
                            { id: "monitors",    icon: "󰍹", label: "Display" },
                            { id: "system",      icon: "󰍛", label: "System" },
                            { id: "controls",    icon: "󰕾", label: "Controls" }
                        ]

                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool isSel: root.activeTab === modelData.id

                            width: navItemRow.implicitWidth + 18
                            height: 26
                            radius: 8
                            color: (!isSel && navMa.containsMouse) ? Theme.surfaceHover : "transparent"
                            Behavior on color { ColorAnimation { duration: 120 } }

                            Row {
                                id: navItemRow
                                anchors.centerIn: parent
                                spacing: 6

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.icon
                                    color: isSel ? Theme.accentText : Theme.accent
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 12
                                    Behavior on color { ColorAnimation { duration: 150 } }
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: modelData.label
                                    color: isSel ? Theme.accentText : (navMa.containsMouse ? Theme.textBright : Theme.text)
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.weight: isSel ? Font.DemiBold : Font.Medium
                                    Behavior on color { ColorAnimation { duration: 150 } }
                                }
                            }

                            MouseArea {
                                id: navMa
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.pinnedRequested();
                                    root.switchTab(modelData.id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Full-Width Compact Content Area ──
        Item {
            id: contentPane
            width: parent.width
            height: parent.height - topNav.height - 10
            clip: true
            transform: Translate { x: root.tabSlideX }

            readonly property var curFlick: {
                if (root.activeTab === "time") return timeFlick;
                if (root.activeTab === "personalize") return bgFlick;
                if (root.activeTab === "monitors") return monFlick;
                if (root.activeTab === "system") return sysFlick;
                if (root.activeTab === "controls") return ctrlFlick;
                return null;
            }

            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                onWheel: event => {
                    var fl = contentPane.curFlick;
                    if (!fl) return;
                    var maxY = Math.max(0, fl.contentHeight - fl.height);
                    if (maxY <= 0) return;
                    var step = (event.pixelDelta && event.pixelDelta.y !== 0)
                               ? -event.pixelDelta.y
                               : -(event.angleDelta.y / 120.0) * 44.0;
                    fl.contentY = Math.max(0, Math.min(maxY, fl.contentY + step));
                    event.accepted = true;
                }
            }

            // Minimal subtle scroll indicator on right edge
            Rectangle {
                z: 25
                readonly property var fl: contentPane.curFlick
                readonly property real maxY: fl ? Math.max(0, fl.contentHeight - fl.height) : 0
                visible: fl !== null && maxY > 1
                anchors.right: parent.right
                anchors.rightMargin: 1
                width: 3
                radius: 1.5
                color: Theme.accent
                opacity: 0.45
                height: fl && fl.contentHeight > 0
                        ? Math.max(24, Math.round(parent.height * (fl.height / fl.contentHeight)))
                        : 24
                y: maxY > 0
                   ? Math.round((parent.height - height) * (fl.contentY / maxY))
                   : 0
            }

            // ══════════════════════════════════════════════════
            // TAB 1: TIME & CALENDAR (Full-width horizontal split)
            // ══════════════════════════════════════════════════
            Flickable {
                id: timeFlick
                anchors.fill: parent
                anchors.rightMargin: 6
                visible: root.activeTab === "time"
                contentHeight: timeRow.implicitHeight
                interactive: false
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                onContentHeightChanged: {
                    var m = Math.max(0, contentHeight - height);
                    if (contentY > m) contentY = m;
                }

                readonly property date displayMonthDate: new Date(
                    liveClock.date.getFullYear(),
                    liveClock.date.getMonth() + root.calOffsetMonths,
                    1
                )
                readonly property int calYear: displayMonthDate.getFullYear()
                readonly property int calMonth: displayMonthDate.getMonth()
                readonly property int firstDayOffset: (new Date(calYear, calMonth, 1).getDay() + 6) % 7
                readonly property int todayYear: liveClock.date.getFullYear()
                readonly property int todayMonth: liveClock.date.getMonth()
                readonly property int todayDay: liveClock.date.getDate()
                property int selectedKey: -1

                Row {
                    id: timeRow
                    width: parent.width
                    height: timeFlick.height
                    spacing: 10

                    // Left: Digital Clock Panel
                    Rectangle {
                        width: 246
                        height: parent.height
                        radius: 14
                        color: Theme.surface
                        border.width: 1
                        border.color: Theme.border

                        Column {
                            anchors.centerIn: parent
                            spacing: 10

                            Item {
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: mainTimeTxt.implicitWidth + 6 + secTxt.implicitWidth
                                height: mainTimeTxt.implicitHeight

                                Text {
                                    id: mainTimeTxt
                                    x: 0
                                    y: 0
                                    text: Qt.formatDateTime(liveClock.date, Services.lock24Hour ? "HH:mm" : "hh:mm")
                                    color: Theme.textBright
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 42
                                    font.weight: Font.Bold
                                }

                                Text {
                                    id: secTxt
                                    anchors.left: mainTimeTxt.right
                                    anchors.leftMargin: 6
                                    anchors.baseline: mainTimeTxt.baseline
                                    text: Qt.formatDateTime(liveClock.date, Services.lock24Hour ? ":ss" : ":ss AP")
                                    color: Theme.accent
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 16
                                    font.weight: Font.DemiBold
                                }
                            }

                            Row {
                                anchors.horizontalCenter: parent.horizontalCenter
                                spacing: 8

                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: dayBadgeTxt.implicitWidth + 16
                                    height: 22
                                    radius: 8
                                    color: Theme.surfaceHover
                                    border.width: 1
                                    border.color: Theme.border

                                    Text {
                                        id: dayBadgeTxt
                                        anchors.centerIn: parent
                                        text: Qt.formatDateTime(liveClock.date, "dddd")
                                        color: Theme.accent
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 11
                                        font.weight: Font.DemiBold
                                    }
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: Qt.formatDateTime(liveClock.date, "dd MMM yyyy")
                                    color: Theme.text
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.weight: Font.Medium
                                }
                            }

                            ActionPill {
                                anchors.horizontalCenter: parent.horizontalCenter
                                icon: "󰥔"
                                label: Services.lock24Hour ? "24-Hour" : "12-Hour"
                                active: Services.lock24Hour
                                onClicked: {
                                    root.pinnedRequested();
                                    Services.lock24Hour = !Services.lock24Hour;
                                    Services.saveSettings();
                                }
                            }
                        }
                    }

                    // Right: Compact Wide Calendar
                    Rectangle {
                        width: parent.width - 256
                        height: parent.height
                        radius: 14
                        color: Theme.surface
                        border.width: 1
                        border.color: Theme.border

                        Column {
                            id: calCardInner
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 6

                            Item {
                                width: parent.width
                                height: 26

                                Row {
                                    anchors.left: parent.left
                                    anchors.leftMargin: 4
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 6

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: Qt.formatDateTime(timeFlick.displayMonthDate, "MMMM")
                                        color: Theme.textBright
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 14
                                        font.weight: Font.Bold
                                    }

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: Qt.formatDateTime(timeFlick.displayMonthDate, "yyyy")
                                        color: Theme.muted
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 13
                                        font.weight: Font.Medium
                                    }
                                }

                                Row {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 5

                                    ActionPill {
                                        label: "‹"
                                        onClicked: {
                                            root.pinnedRequested();
                                            root.calOffsetMonths -= 1;
                                        }
                                    }

                                    ActionPill {
                                        label: "Today"
                                        active: root.calOffsetMonths === 0
                                        onClicked: {
                                            root.pinnedRequested();
                                            root.calOffsetMonths = 0;
                                            timeFlick.selectedKey = -1;
                                        }
                                    }

                                    ActionPill {
                                        label: "›"
                                        onClicked: {
                                            root.pinnedRequested();
                                            root.calOffsetMonths += 1;
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                width: parent.width
                                height: 22
                                radius: 7
                                color: Theme.bgDeep

                                Row {
                                    anchors.fill: parent
                                    spacing: 5

                                    Repeater {
                                        model: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                                        Text {
                                            required property string modelData
                                            required property int index
                                            width: calGrid.cellW
                                            height: parent.height
                                            horizontalAlignment: Text.AlignHCenter
                                            verticalAlignment: Text.AlignVCenter
                                            text: modelData
                                            color: index >= 5 ? Theme.accent : Theme.muted
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            font.weight: Font.DemiBold
                                        }
                                    }
                                }
                            }

                            Grid {
                                id: calGrid
                                width: parent.width
                                columns: 7
                                spacing: 4

                                readonly property real cellW: Math.floor((width - 6 * 4) / 7)
                                readonly property real cellH: Math.max(26, Math.floor((calCardInner.height - 26 - 22 - 12 - 5 * 4) / 6))

                                Repeater {
                                    model: 42

                                    Rectangle {
                                        required property int index
                                        readonly property date cellDate: new Date(
                                            timeFlick.calYear,
                                            timeFlick.calMonth,
                                            1 - timeFlick.firstDayOffset + index
                                        )
                                        readonly property int cellYear: cellDate.getFullYear()
                                        readonly property int cellMonth: cellDate.getMonth()
                                        readonly property int cellDay: cellDate.getDate()
                                        readonly property int cellKey: cellYear * 10000 + cellMonth * 100 + cellDay
                                        readonly property bool inCurrentMonth: cellMonth === timeFlick.calMonth
                                        readonly property bool isToday: cellYear === timeFlick.todayYear
                                            && cellMonth === timeFlick.todayMonth
                                            && cellDay === timeFlick.todayDay
                                        readonly property bool isSelected: timeFlick.selectedKey === cellKey
                                        readonly property bool isWeekend: (index % 7) >= 5

                                        width: calGrid.cellW
                                        height: calGrid.cellH
                                        radius: 8
                                        color: isToday
                                            ? Theme.accent
                                            : isSelected
                                              ? Theme.surfaceActive
                                              : (dayMa.containsMouse ? Theme.surfaceHover : "transparent")
                                        border.width: (isSelected && !isToday) ? 1 : 0
                                        border.color: Theme.accent
                                        Behavior on color { ColorAnimation { duration: 120 } }

                                        Text {
                                            anchors.centerIn: parent
                                            text: cellDay
                                            opacity: inCurrentMonth ? 1.0 : 0.28
                                            color: isToday
                                                ? Theme.accentText
                                                : (isWeekend && inCurrentMonth ? Theme.accent : Theme.textBright)
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 11
                                            font.weight: isToday ? Font.Bold : (inCurrentMonth ? Font.Medium : Font.Normal)
                                        }

                                        MouseArea {
                                            id: dayMa
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                root.pinnedRequested();
                                                timeFlick.selectedKey = (timeFlick.selectedKey === cellKey) ? -1 : cellKey;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ══════════════════════════════════════════════════
            // TAB 2: PERSONALIZE (Full-width ultra-compact 2-col)
            // ══════════════════════════════════════════════════
            Flickable {
                id: bgFlick
                anchors.fill: parent
                anchors.rightMargin: 6
                visible: root.activeTab === "personalize"
                contentHeight: bgCol.implicitHeight + 4
                interactive: false
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                onContentHeightChanged: {
                    var m = Math.max(0, contentHeight - height);
                    if (contentY > m) contentY = m;
                }

                readonly property string currentPreviewPath: {
                    if (root.bgTarget === "lockscreen") {
                        return Services.lockUseSameWallpaper ? Services.wallpaperPath : Services.lockWallpaperPath;
                    }
                    if (root.bgTarget === "sddm") {
                        return Services.sddmSelectedPath || Services.sddmCurrentBg;
                    }
                    return Services.wallpaperPath;
                }

                Column {
                    id: bgCol
                    width: parent.width
                    spacing: 8

                    // Sub-tabs left + actions right
                    Item {
                        width: parent.width
                        height: 28

                        Row {
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 6

                            Repeater {
                                model: [
                                    { id: "theme",      icon: "󰏘", label: "Theme & Bar" },
                                    { id: "mango",      icon: "󰖲", label: "MangoWM" },
                                    { id: "desktop",    icon: "󰸉", label: "Desktop" },
                                    { id: "lockscreen", icon: "󰌾", label: "Lock" },
                                    { id: "sddm",       icon: "󰍂", label: "Login" }
                                ]
                                delegate: ActionPill {
                                    required property var modelData
                                    icon: modelData.icon
                                    label: modelData.label
                                    active: root.bgTarget === modelData.id
                                    onClicked: {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        root.bgTarget = modelData.id;
                                        if (modelData.id === "mango") Services.loadMangoConfig();
                                    }
                                }
                            }
                        }

                        Row {
                            visible: root.bgTarget !== "theme" && root.bgTarget !== "mango"
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 6

                            ActionPill {
                                visible: root.bgTarget === "desktop"
                                icon: "󰒟"
                                label: "Random"
                                onClicked: {
                                    root.pinnedRequested();
                                    Services.applyRandomWallpaper();
                                }
                            }

                            ActionPill {
                                visible: root.bgTarget === "lockscreen"
                                icon: "󰌾"
                                label: "Lock Now"
                                onClicked: {
                                    root.closeRequested();
                                    Services.lockScreen();
                                }
                            }

                            ActionPill {
                                visible: root.bgTarget === "sddm"
                                icon: "󰸉"
                                label: "Sync Desktop"
                                busy: Services.sddmApplying
                                onClicked: {
                                    root.pinnedRequested();
                                    Services.applySddmWallpaper(Services.wallpaperPath);
                                }
                            }

                            ActionPill {
                                icon: "󰉋"
                                label: "Browse"
                                busy: root.bgTarget === "sddm" && Services.sddmApplying
                                onClicked: {
                                    if (root.bgTarget === "lockscreen") {
                                        root.openImagePicker("lockscreen", Services.wallpaperDir, "Select Lock Screen Wallpaper");
                                    } else if (root.bgTarget === "sddm") {
                                        root.openImagePicker("sddm", Services.wallpaperDir, "Select SDDM Login Wallpaper");
                                    } else {
                                        root.openImagePicker("wallpaper", Services.wallpaperDir, "Select Desktop Wallpaper");
                                    }
                                }
                            }
                        }
                    }

                    // ── THEME & BAR SUB-TAB (2 wide columns, 0 scroll) ──
                    Row {
                        visible: root.bgTarget === "theme"
                        width: parent.width
                        spacing: 10

                        // Left: Theme, Accent, Bar Position & 2x2 Modules Grid
                        Rectangle {
                            width: Math.floor((parent.width - 10) * 0.5)
                            height: Math.max(themeLeftCol.implicitHeight + 20, themeRightCol.implicitHeight + 20)
                            radius: 14
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                id: themeLeftCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 8

                                OptionDropdown {
                                    label: "Theme"
                                    selectedValue: Services.themeName
                                    currentText: Services.currentTheme.label || "Nord"
                                    expanded: root.openMonitorDropdown === "themePreset"
                                    options: [
                                        { value: "nord",       label: "Nord" },
                                        { value: "catppuccin", label: "Catppuccin" },
                                        { value: "gruvbox",    label: "Gruvbox" },
                                        { value: "tokyonight", label: "Tokyo Night" },
                                        { value: "dracula",    label: "Dracula" },
                                        { value: "rosepine",   label: "Rosé Pine" },
                                        { value: "everforest", label: "Everforest" }
                                    ]
                                    onHeaderClicked: {
                                        root.pinnedRequested();
                                        root.toggleMonitorDropdown("themePreset");
                                    }
                                    onOptionPicked: (val, lbl) => {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        Services.themeName = val;
                                        Services.accentPreset = "theme";
                                        Services.saveSettings();
                                    }
                                }

                                Item {
                                    width: parent.width
                                    height: 26

                                    Text {
                                        anchors.left: parent.left
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: "Accent"
                                        color: Theme.text
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }

                                    Row {
                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        spacing: 6

                                        Repeater {
                                            model: [
                                                { id: "theme",    hex: Services.currentTheme.accent },
                                                { id: "lavender", hex: "#c6d0f5" },
                                                { id: "frost",    hex: "#88c0d0" },
                                                { id: "blue",     hex: "#81a1c1" },
                                                { id: "sage",     hex: "#a3be8c" },
                                                { id: "amber",    hex: "#ebcb8b" },
                                                { id: "rose",     hex: "#bf616a" },
                                                { id: "snow",     hex: "#eceff4" }
                                            ]

                                            delegate: Rectangle {
                                                required property var modelData
                                                readonly property bool isSel: Services.accentPreset === modelData.id

                                                width: 20
                                                height: 20
                                                radius: 10
                                                color: modelData.hex
                                                border.width: isSel ? 2 : 1
                                                border.color: isSel ? "#ffffff" : Theme.border
                                                scale: isSel ? 1.12 : (swMa.containsMouse ? 1.06 : 1.0)
                                                Behavior on scale { NumberAnimation { duration: 120 } }

                                                Text {
                                                    visible: isSel
                                                    anchors.centerIn: parent
                                                    text: "󰄬"
                                                    color: Theme.accentText
                                                    font.family: Theme.fontFamily
                                                    font.pixelSize: 9
                                                    font.weight: Font.Bold
                                                }

                                                MouseArea {
                                                    id: swMa
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        root.pinnedRequested();
                                                        Services.accentPreset = modelData.id;
                                                        Services.saveSettings();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                OptionDropdown {
                                    label: "Bar Position"
                                    selectedValue: Services.barPosition
                                    currentText: {
                                        var map = { top: "Top", bottom: "Bottom", left: "Left", right: "Right" };
                                        return map[Services.barPosition] || "Top";
                                    }
                                    expanded: root.openMonitorDropdown === "barPos"
                                    options: [
                                        { value: "top",    label: "Top" },
                                        { value: "bottom", label: "Bottom" },
                                        { value: "left",   label: "Left" },
                                        { value: "right",  label: "Right" }
                                    ]
                                    onHeaderClicked: {
                                        root.pinnedRequested();
                                        root.toggleMonitorDropdown("barPos");
                                    }
                                    onOptionPicked: (val, lbl) => {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        root.barPositionRequested(val);
                                    }
                                }

                                // 2x2 switches grid
                                Grid {
                                    width: parent.width
                                    columns: 2
                                    columnSpacing: 14
                                    rowSpacing: 6

                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Floating Bar"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.barFloating; onToggled: { root.pinnedRequested(); Services.barFloating = !Services.barFloating; Services.saveSettings(); } }
                                    }
                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Show RAM"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.showRamOnBar; onToggled: { root.pinnedRequested(); Services.showRamOnBar = !Services.showRamOnBar; Services.saveSettings(); } }
                                    }
                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "System Tray"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.showTrayOnBar; onToggled: { root.pinnedRequested(); Services.showTrayOnBar = !Services.showTrayOnBar; Services.saveSettings(); } }
                                    }
                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Media Pill"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.showMediaOnBar; onToggled: { root.pinnedRequested(); Services.showMediaOnBar = !Services.showMediaOnBar; Services.saveSettings(); } }
                                    }
                                }
                            }
                        }

                        // Right: 4 Bar/Popup Sliders
                        Rectangle {
                            width: parent.width - Math.floor((parent.width - 10) * 0.5) - 10
                            height: Math.max(themeLeftCol.implicitHeight + 20, themeRightCol.implicitHeight + 20)
                            radius: 14
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                id: themeRightCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 7

                                Column {
                                    width: parent.width; spacing: 3
                                    Item {
                                        width: parent.width; height: 15
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Bar Size"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Services.barSize + " px"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 28; maxValue: 48; value: Services.barSize; onMoved: v => { Services.barSize = Math.round(v); Services.saveSettings(); } }
                                }

                                Column {
                                    width: parent.width; spacing: 3
                                    Item {
                                        width: parent.width; height: 15
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Bar Opacity"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Math.round(Services.barOpacity * 100) + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 35; maxValue: 100; value: Math.round(Services.barOpacity * 100); onMoved: v => { Services.barOpacity = Math.round(v) / 100.0; Services.saveSettings(); } }
                                }

                                Column {
                                    width: parent.width; spacing: 3
                                    Item {
                                        width: parent.width; height: 15
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Popup Opacity"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Math.round(Services.popupOpacity * 100) + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 55; maxValue: 100; value: Math.round(Services.popupOpacity * 100); onMoved: v => { Services.popupOpacity = Math.round(v) / 100.0; Services.saveSettings(); } }
                                }

                                Column {
                                    width: parent.width; spacing: 3
                                    Item {
                                        width: parent.width; height: 15
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Corner Radius"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Services.cornerRadius + " px"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 0; maxValue: 20; value: Services.cornerRadius; onMoved: v => { Services.cornerRadius = Math.round(v); Services.saveSettings(); } }
                                }
                            }
                        }
                    }

                    // ── MANGOWM SUB-TAB (2 wide columns, 0 scroll) ──
                    Row {
                        visible: root.bgTarget === "mango"
                        width: parent.width
                        spacing: 10

                        // Left: 5 Geometry/Opacity Sliders
                        Rectangle {
                            width: Math.floor((parent.width - 10) * 0.5)
                            height: Math.max(mangoLeftCol.implicitHeight + 20, mangoRightCol.implicitHeight + 20)
                            radius: 14
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                id: mangoLeftCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 5

                                Column {
                                    width: parent.width; spacing: 2
                                    Item {
                                        width: parent.width; height: 14
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Inner Gap"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Services.mangoGapInner + " px"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 10; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 0; maxValue: 24; value: Services.mangoGapInner; onMoved: v => { Services.mangoGapInner = Math.round(v); } }
                                }

                                Column {
                                    width: parent.width; spacing: 2
                                    Item {
                                        width: parent.width; height: 14
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Outer Gap"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Services.mangoGapOuter + " px"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 10; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 0; maxValue: 32; value: Services.mangoGapOuter; onMoved: v => { Services.mangoGapOuter = Math.round(v); } }
                                }

                                Column {
                                    width: parent.width; spacing: 2
                                    Item {
                                        width: parent.width; height: 14
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Border Width"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Services.mangoBorderPx + " px"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 10; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 0; maxValue: 6; value: Services.mangoBorderPx; onMoved: v => { Services.mangoBorderPx = Math.round(v); } }
                                }

                                Column {
                                    width: parent.width; spacing: 2
                                    Item {
                                        width: parent.width; height: 14
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Window Radius"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Services.mangoBorderRadius + " px"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 10; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 0; maxValue: 20; value: Services.mangoBorderRadius; onMoved: v => { Services.mangoBorderRadius = Math.round(v); } }
                                }

                                Column {
                                    width: parent.width; spacing: 2
                                    Item {
                                        width: parent.width; height: 14
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Inactive Opacity"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Math.round(Services.mangoUnfocusedOpacity * 100) + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 10; font.weight: Font.DemiBold }
                                    }
                                    SmoothSlider { width: parent.width; minValue: 50; maxValue: 100; value: Math.round(Services.mangoUnfocusedOpacity * 100); onMoved: v => { Services.mangoUnfocusedOpacity = Math.round(v) / 100.0; } }
                                }
                            }
                        }

                        // Right: Toggles grid, Animation Dropdowns & Apply Actions
                        Rectangle {
                            width: parent.width - Math.floor((parent.width - 10) * 0.5) - 10
                            height: Math.max(mangoLeftCol.implicitHeight + 20, mangoRightCol.implicitHeight + 20)
                            radius: 14
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                id: mangoRightCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 7

                                Grid {
                                    width: parent.width
                                    columns: 2
                                    columnSpacing: 14
                                    rowSpacing: 6

                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Blur"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.mangoBlur; onToggled: { root.pinnedRequested(); Services.mangoBlur = !Services.mangoBlur; } }
                                    }
                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Shadows"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.mangoShadows; onToggled: { root.pinnedRequested(); Services.mangoShadows = !Services.mangoShadows; } }
                                    }
                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Hide Single"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.mangoNoBorderSingle; onToggled: { root.pinnedRequested(); Services.mangoNoBorderSingle = !Services.mangoNoBorderSingle; } }
                                    }
                                    Item {
                                        width: (parent.width - 14) / 2; height: 24
                                        Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Animations"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                        SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.mangoAnimations; onToggled: { root.pinnedRequested(); Services.mangoAnimations = !Services.mangoAnimations; } }
                                    }
                                }

                                Item {
                                    width: parent.width; height: 24
                                    Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Follow Mouse Focus"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Medium }
                                    SoftSwitch { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; checked: Services.mangoSloppyFocus; onToggled: { root.pinnedRequested(); Services.mangoSloppyFocus = !Services.mangoSloppyFocus; } }
                                }

                                Row {
                                    width: parent.width
                                    spacing: 8

                                    OptionDropdown {
                                        width: (parent.width - 8) / 2
                                        label: "Open"
                                        selectedValue: Services.mangoAnimOpen
                                        currentText: Services.mangoAnimOpen
                                        expanded: root.openMonitorDropdown === "mangoOpen"
                                        options: [
                                            { value: "slide", label: "slide" },
                                            { value: "zoom",  label: "zoom" },
                                            { value: "fade",  label: "fade" },
                                            { value: "none",  label: "none" }
                                        ]
                                        onHeaderClicked: { root.pinnedRequested(); root.toggleMonitorDropdown("mangoOpen"); }
                                        onOptionPicked: (val, lbl) => { root.pinnedRequested(); root.openMonitorDropdown = ""; Services.mangoAnimOpen = val; }
                                    }

                                    OptionDropdown {
                                        width: (parent.width - 8) / 2
                                        label: "Close"
                                        selectedValue: Services.mangoAnimClose
                                        currentText: Services.mangoAnimClose
                                        expanded: root.openMonitorDropdown === "mangoClose"
                                        options: [
                                            { value: "slide", label: "slide" },
                                            { value: "zoom",  label: "zoom" },
                                            { value: "fade",  label: "fade" },
                                            { value: "none",  label: "none" }
                                        ]
                                        onHeaderClicked: { root.pinnedRequested(); root.toggleMonitorDropdown("mangoClose"); }
                                        onOptionPicked: (val, lbl) => { root.pinnedRequested(); root.openMonitorDropdown = ""; Services.mangoAnimClose = val; }
                                    }
                                }

                                Item {
                                    width: parent.width
                                    height: 28

                                    ActionPill {
                                        anchors.left: parent.left
                                        anchors.verticalCenter: parent.verticalCenter
                                        icon: "󰏘"
                                        label: "Sync WM Colors"
                                        onClicked: {
                                            root.pinnedRequested();
                                            Services.syncMangoThemeColors();
                                        }
                                    }

                                    Rectangle {
                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 120
                                        height: 26
                                        radius: 8
                                        color: applyMangoMa.containsMouse ? Theme.textBright : Theme.accent

                                        Text {
                                            anchors.centerIn: parent
                                            text: "Apply & Reload"
                                            color: Theme.accentText
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            font.weight: Font.DemiBold
                                        }

                                        MouseArea {
                                            id: applyMangoMa
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                root.pinnedRequested();
                                                Services.applyMangoFormSettings();
                                            }
                                        }
                                    }
                                }

                                Text {
                                    visible: Services.mangoApplyStatus.length > 0
                                    text: Services.mangoApplyStatus
                                    color: Theme.accent
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 10
                                }
                            }
                        }
                    }

                    // Compact Shared Hero Card (Desktop / Lock / Login)
                    Rectangle {
                        visible: root.bgTarget !== "theme" && root.bgTarget !== "mango"
                        width: parent.width
                        height: 98
                        radius: 14
                        color: Theme.surface
                        border.width: 1
                        border.color: Theme.border

                        Row {
                            anchors.fill: parent
                            anchors.margins: 9
                            spacing: 12

                            ClippingRectangle {
                                width: 152
                                height: parent.height
                                radius: 9
                                color: Theme.bgDeep

                                Image {
                                    anchors.fill: parent
                                    source: {
                                        const p = bgFlick.currentPreviewPath;
                                        if (!p) return "";
                                        return root.bgTarget === "sddm"
                                            ? ("file://" + p + "?v=" + Services.sddmBgRevision)
                                            : ("file://" + p);
                                    }
                                    fillMode: root.bgTarget === "desktop" ? root.wallFillMode(Services.wallpaperMode) : Image.PreserveAspectCrop
                                    asynchronous: true
                                    smooth: true
                                    sourceSize.width: 400
                                    sourceSize.height: 240
                                }

                                Rectangle {
                                    visible: root.bgTarget === "lockscreen"
                                    anchors.fill: parent
                                    color: Qt.rgba(0, 0, 0, Services.lockDimOpacity)
                                }

                                Text {
                                    anchors.centerIn: parent
                                    visible: root.bgTarget === "lockscreen" && Services.lockShowClock
                                    text: Services.lock24Hour ? "14:28" : "02:28"
                                    color: "#eceff4"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 18
                                    font.weight: Font.Bold
                                }
                            }

                            Column {
                                width: parent.width - 164
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 6

                                Text {
                                    width: parent.width
                                    text: root.basename(bgFlick.currentPreviewPath) || "None"
                                    color: Theme.textBright
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.weight: Font.Medium
                                    elide: Text.ElideMiddle
                                }

                                Flow {
                                    visible: root.bgTarget === "desktop"
                                    width: parent.width
                                    spacing: 5

                                    Repeater {
                                        model: [
                                            { id: "fill",    label: "Fill" },
                                            { id: "fit",     label: "Fit" },
                                            { id: "stretch", label: "Stretch" },
                                            { id: "center",  label: "Center" },
                                            { id: "tile",    label: "Tile" }
                                        ]

                                        delegate: ActionPill {
                                            required property var modelData
                                            label: modelData.label
                                            active: Services.wallpaperMode === modelData.id
                                            onClicked: {
                                                root.pinnedRequested();
                                                Services.applyWallpaper(Services.wallpaperPath, modelData.id);
                                            }
                                        }
                                    }

                                    ActionPill {
                                        icon: "󰌾"
                                        label: Services.lockUseSameWallpaper ? "Lock Synced" : "Sync Lock"
                                        active: Services.lockUseSameWallpaper && Services.lockWallpaperPath === Services.wallpaperPath
                                        onClicked: {
                                            root.pinnedRequested();
                                            Services.lockUseSameWallpaper = true;
                                            Services.setLockWallpaper(Services.wallpaperPath);
                                        }
                                    }

                                    ActionPill {
                                        icon: "󰍂"
                                        label: "To SDDM"
                                        busy: Services.sddmApplying
                                        onClicked: {
                                            root.pinnedRequested();
                                            Services.applySddmWallpaper(Services.wallpaperPath);
                                        }
                                    }
                                }

                                Flow {
                                    visible: root.bgTarget === "lockscreen"
                                    width: parent.width
                                    spacing: 6

                                    ActionPill {
                                        icon: "󰸉"
                                        label: "Sync Desktop"
                                        active: Services.lockUseSameWallpaper
                                        onClicked: {
                                            root.pinnedRequested();
                                            Services.lockUseSameWallpaper = !Services.lockUseSameWallpaper;
                                            Services.saveSettings();
                                        }
                                    }

                                    ActionPill {
                                        icon: "󰥔"
                                        label: "Clock"
                                        active: Services.lockShowClock
                                        onClicked: {
                                            root.pinnedRequested();
                                            Services.lockShowClock = !Services.lockShowClock;
                                            Services.saveSettings();
                                        }
                                    }

                                    ActionPill {
                                        label: "24h"
                                        active: Services.lock24Hour
                                        onClicked: {
                                            root.pinnedRequested();
                                            Services.lock24Hour = !Services.lock24Hour;
                                            Services.saveSettings();
                                        }
                                    }
                                }

                                SmoothSlider {
                                    visible: root.bgTarget === "lockscreen"
                                    width: parent.width
                                    icon: "󰃟"
                                    minValue: 0
                                    maxValue: 85
                                    value: Math.round(Services.lockDimOpacity * 100)
                                    labelText: Math.round(Services.lockDimOpacity * 100) + "%"
                                    onMoved: v => {
                                        root.pinnedRequested();
                                        Services.lockDimOpacity = v / 100.0;
                                        Services.saveSettings();
                                    }
                                }

                                Flow {
                                    visible: root.bgTarget === "sddm"
                                    width: parent.width
                                    spacing: 6

                                    ActionPill {
                                        icon: "󰌾"
                                        label: "Sync Lock"
                                        busy: Services.sddmApplying
                                        onClicked: {
                                            root.pinnedRequested();
                                            const p = Services.lockUseSameWallpaper ? Services.wallpaperPath : Services.lockWallpaperPath;
                                            if (p) Services.applySddmWallpaper(p);
                                        }
                                    }

                                    ActionPill {
                                        icon: "󰒓"
                                        label: Services.sddmTheme || "default"
                                        onClicked: root.pinnedRequested()
                                    }
                                }

                                Text {
                                    visible: root.bgTarget === "sddm" && Services.sddmStatus.length > 0
                                    width: parent.width
                                    text: Services.sddmStatus
                                    color: Services.sddmStatusError ? Theme.red : Theme.accent
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 10
                                    wrapMode: Text.WordWrap
                                }
                            }
                        }
                    }

                    // Single shared image browser for active target
                    ImageBrowserGrid {
                        visible: root.bgTarget !== "theme" && root.bgTarget !== "mango"
                        width: parent.width
                        selectedPath: bgFlick.currentPreviewPath
                        onImageSelected: filePath => {
                            root.pinnedRequested();
                            if (root.bgTarget === "lockscreen") {
                                Services.setLockWallpaper(filePath);
                            } else if (root.bgTarget === "sddm") {
                                Services.applySddmWallpaper(filePath);
                            } else {
                                Services.applyWallpaper(filePath, Services.wallpaperMode);
                            }
                        }
                    }
                }
            }

            // ══════════════════════════════════════════════════
            // TAB 3: SYSTEM MONITOR (Ultra-compact full-width, 0 scroll)
            // ══════════════════════════════════════════════════
            Flickable {
                id: sysFlick
                anchors.fill: parent
                anchors.rightMargin: 6
                visible: root.activeTab === "system"
                contentHeight: sysCol.implicitHeight
                interactive: false
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                onContentHeightChanged: {
                    var m = Math.max(0, contentHeight - height);
                    if (contentY > m) contentY = m;
                }

                Column {
                    id: sysCol
                    width: parent.width
                    spacing: 8

                    // Compact header + Uptime/Tasks/Load summary
                    Item {
                        width: parent.width
                        height: 26

                        Text {
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            text: "System Telemetry"
                            color: Theme.textBright
                            font.family: Theme.fontFamily
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }

                        Rectangle {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            width: sysStatRow.implicitWidth + 18
                            height: 24
                            radius: 8
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Row {
                                id: sysStatRow
                                anchors.centerIn: parent
                                spacing: 14

                                Row {
                                    spacing: 5
                                    Text { text: "󰅐"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11 }
                                    Text { text: Services.sysUptime; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                }
                                Row {
                                    spacing: 5
                                    Text { text: "󰒓"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11 }
                                    Text { text: Services.sysProcessCount + " tasks"; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                }
                                Row {
                                    spacing: 5
                                    Text { text: "󰓅"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11 }
                                    Text { text: Services.sysLoadAvg; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                }
                            }
                        }
                    }

                    // Row 1: CPU + Memory side-by-side
                    Row {
                        width: parent.width
                        spacing: 10

                        Rectangle {
                            width: (parent.width - 10) / 2
                            height: 142
                            radius: 13
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 6

                                Row {
                                    width: parent.width
                                    Text { width: parent.width - 50; text: "󰻠 CPU (" + Services.sysCpuCores + "T)"; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 12; font.weight: Font.DemiBold }
                                    Text { width: 50; horizontalAlignment: Text.AlignRight; text: Services.sysCpuPercent + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 12; font.weight: Font.Bold }
                                }

                                StatBar { width: parent.width; ratio: Services.sysCpuPercent / 100.0; fillColor: Theme.accent }

                                Row {
                                    width: parent.width
                                    spacing: 10
                                    Text { width: parent.width - 120; text: Services.sysCpuModel; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10; elide: Text.ElideRight }
                                    Text { text: "󰔏 " + Services.sysCpuTemp; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                    Text { text: "󰓅 " + Services.sysCpuFreqGhz; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                }

                                HistoryBars { width: parent.width; samples: Services.sysCpuHistory; barColor: Theme.accent }
                            }
                        }

                        Rectangle {
                            width: (parent.width - 10) / 2
                            height: 142
                            radius: 13
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 6

                                Row {
                                    width: parent.width
                                    Text { width: parent.width - 50; text: "󰍛 Memory"; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 12; font.weight: Font.DemiBold }
                                    Text { width: 50; horizontalAlignment: Text.AlignRight; text: Services.ramPercent + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 12; font.weight: Font.Bold }
                                }

                                StatBar { width: parent.width; ratio: Services.ramPercent / 100.0; fillColor: Theme.accent }

                                Row {
                                    width: parent.width
                                    spacing: 12
                                    Text { text: Services.ramUsedGb + " / " + Services.ramTotalGb + " GB"; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                    Text {
                                        text: Number(Services.sysSwapTotalGb) > 0
                                            ? ("Swap " + Services.sysSwapUsedGb + "/" + Services.sysSwapTotalGb + "G")
                                            : "Swap Off"
                                        color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 10
                                    }
                                }

                                HistoryBars { width: parent.width; samples: Services.sysRamHistory; barColor: Theme.accent }
                            }
                        }
                    }

                    // Row 2: 3-column Disk + Network + Battery
                    Row {
                        width: parent.width
                        spacing: 10

                        Rectangle {
                            width: (parent.width - 20) / 3
                            height: 98
                            radius: 13
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 6

                                Row {
                                    width: parent.width
                                    Text { width: parent.width - 42; text: "󰋊 Disk (/)"; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    Text { width: 42; horizontalAlignment: Text.AlignRight; text: Services.sysDiskPercent + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Bold }
                                }

                                StatBar { width: parent.width; ratio: Services.sysDiskPercent / 100.0; fillColor: Theme.accent }

                                Text { width: parent.width; text: Services.sysDiskUsedGb + " / " + Services.sysDiskTotalGb + " GB"; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10; elide: Text.ElideRight }
                            }
                        }

                        Rectangle {
                            width: (parent.width - 20) / 3
                            height: 98
                            radius: 13
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 6

                                Row {
                                    width: parent.width
                                    Text { width: parent.width - 90; text: "󰖩 Network"; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    Text { width: 90; horizontalAlignment: Text.AlignRight; text: Services.wifiEnabled ? (Services.wifiCurrentSsid || "Connected") : "Offline"; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10; elide: Text.ElideRight }
                                }

                                Row {
                                    width: parent.width
                                    spacing: 16
                                    Column {
                                        spacing: 1
                                        Text { text: "󰇚 Down"; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                        Text { text: root.formatSpeed(Services.sysNetDownKbps); color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 12; font.weight: Font.DemiBold }
                                    }
                                    Column {
                                        spacing: 1
                                        Text { text: "󰕒 Up"; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10 }
                                        Text { text: root.formatSpeed(Services.sysNetUpKbps); color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 12; font.weight: Font.DemiBold }
                                    }
                                }
                            }
                        }

                        Rectangle {
                            width: (parent.width - 20) / 3
                            height: 98
                            radius: 13
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 6

                                Row {
                                    width: parent.width
                                    Text { width: parent.width - 42; text: (Services.batteryCharging ? "󰂄 " : "󰁹 ") + "Battery"; color: Theme.textBright; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    Text { width: 42; horizontalAlignment: Text.AlignRight; text: Services.batteryPercent + "%"; color: Theme.accent; font.family: Theme.fontFamily; font.pixelSize: 11; font.weight: Font.Bold }
                                }

                                StatBar { width: parent.width; ratio: Services.batteryPercent / 100.0; fillColor: Theme.accent }

                                Text { width: parent.width; text: Services.batteryDetail || Services.batteryStatus; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 10; elide: Text.ElideRight }
                            }
                        }
                    }
                }
            }

            // ══════════════════════════════════════════════════
            // TAB 4: DISPLAY & MONITORS (Wide 2-col, compact height)
            // ══════════════════════════════════════════════════
            Flickable {
                id: monFlick
                anchors.fill: parent
                anchors.rightMargin: 6
                visible: root.activeTab === "monitors"
                contentHeight: monCol.implicitHeight + 4
                interactive: false
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                onContentHeightChanged: {
                    var m = Math.max(0, contentHeight - height);
                    if (contentY > m) contentY = m;
                }

                Column {
                    id: monCol
                    width: parent.width
                    spacing: 8

                    Item {
                        width: parent.width
                        height: 26

                        Text {
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Display & Monitors"
                            color: Theme.textBright
                            font.family: Theme.fontFamily
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }

                        Rectangle {
                            visible: Services.monitorStatus.length > 0
                            anchors.right: monRefreshPill.left
                            anchors.rightMargin: 8
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(340, monStatTxt.implicitWidth + 16)
                            height: 24
                            radius: 8
                            color: Services.monitorStatusError ? "#3a292f" : Theme.surface
                            border.width: 1
                            border.color: Services.monitorStatusError ? Theme.red : Theme.border

                            Text {
                                id: monStatTxt
                                anchors.centerIn: parent
                                width: parent.width - 12
                                horizontalAlignment: Text.AlignHCenter
                                text: Services.monitorStatus
                                color: Services.monitorStatusError ? "#e0959c" : Theme.accent
                                font.family: Theme.fontFamily
                                font.pixelSize: 10
                                elide: Text.ElideRight
                            }
                        }

                        ActionPill {
                            id: monRefreshPill
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            icon: "󰑓"
                            label: Services.monitorsLoading ? "..." : "Refresh"
                            onClicked: {
                                root.pinnedRequested();
                                Services.refreshMonitors();
                            }
                        }
                    }

                    Row {
                        width: parent.width
                        spacing: 10

                        // Left: Display Mode + Arrangement Canvas
                        Rectangle {
                            id: arrangementBox
                            width: Math.floor((parent.width - 10) * 0.5)
                            height: Math.max(arrCol.implicitHeight + 20, monDetailCol.implicitHeight + 20)
                            radius: 14
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                id: arrCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 7

                                OptionDropdown {
                                    label: "Mode"
                                    selectedValue: Services.displayMode
                                    currentText: {
                                        var map = { primary: "Primary Only", extend: "Extend", mirror: "Mirror", secondary: "External Only" };
                                        return map[Services.displayMode] || "Primary Only";
                                    }
                                    expanded: root.openMonitorDropdown === "mode"
                                    options: [
                                        { value: "primary",   label: "Primary Only" },
                                        { value: "extend",    label: "Extend" },
                                        { value: "mirror",    label: "Mirror" },
                                        { value: "secondary", label: "External Only" }
                                    ]
                                    onHeaderClicked: {
                                        root.pinnedRequested();
                                        root.toggleMonitorDropdown("mode");
                                    }
                                    onOptionPicked: (val, lbl) => {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        Services.applyDisplayMode(val);
                                    }
                                }

                                Rectangle {
                                    id: layoutCanvas
                                    width: parent.width
                                    height: 132
                                    radius: 11
                                    color: Theme.bgDeep
                                    border.width: 1
                                    border.color: Theme.border
                                    clip: true

                                    property bool dragging: false
                                    property real frozenScale: 0.06
                                    property real frozenOffsetX: 40
                                    property real frozenOffsetY: 30

                                    readonly property var metrics: {
                                        var list = root.draftMonitors || [];
                                        if (list.length === 0) {
                                            return { minX: 0, minY: 0, scale: 0.06, offX: 40, offY: 30 };
                                        }
                                        var minX = list[0].x, minY = list[0].y;
                                        var maxX = list[0].x + list[0].logicalWidth;
                                        var maxY = list[0].y + list[0].logicalHeight;
                                        for (var i = 1; i < list.length; i++) {
                                            var m = list[i];
                                            if (m.x < minX) minX = m.x;
                                            if (m.y < minY) minY = m.y;
                                            if (m.x + m.logicalWidth > maxX) maxX = m.x + m.logicalWidth;
                                            if (m.y + m.logicalHeight > maxY) maxY = m.y + m.logicalHeight;
                                        }
                                        var spanW = Math.max(640, maxX - minX);
                                        var spanH = Math.max(480, maxY - minY);
                                        var pad = 16;
                                        var availW = Math.max(100, layoutCanvas.width - pad * 2);
                                        var availH = Math.max(50, layoutCanvas.height - pad * 2);
                                        var sc = Math.min(availW / spanW, availH / spanH) * (list.length > 1 ? 0.78 : 0.72);
                                        var offX = (layoutCanvas.width - spanW * sc) / 2 - minX * sc;
                                        var offY = (layoutCanvas.height - spanH * sc) / 2 - minY * sc;
                                        return { minX: minX, minY: minY, scale: sc, offX: offX, offY: offY };
                                    }

                                    readonly property real effScale: dragging ? frozenScale : metrics.scale
                                    readonly property real effOffX: dragging ? frozenOffsetX : metrics.offX
                                    readonly property real effOffY: dragging ? frozenOffsetY : metrics.offY

                                    Repeater {
                                        model: root.draftMonitors

                                        delegate: Rectangle {
                                            required property var modelData
                                            required property int index
                                            readonly property bool isSel: root.selectedMonitorIdx === index
                                            readonly property bool isMirroredOverlap: root.draftMonitors.length >= 2
                                                && index > 0
                                                && modelData.enabled
                                                && root.draftMonitors[0].enabled
                                                && modelData.x === root.draftMonitors[0].x
                                                && modelData.y === root.draftMonitors[0].y

                                            x: Math.round(layoutCanvas.effOffX + modelData.x * layoutCanvas.effScale) + (isMirroredOverlap ? 8 : 0)
                                            y: Math.round(layoutCanvas.effOffY + modelData.y * layoutCanvas.effScale) + (isMirroredOverlap ? 8 : 0)
                                            width: Math.max(54, Math.round(modelData.logicalWidth * layoutCanvas.effScale))
                                            height: Math.max(34, Math.round(modelData.logicalHeight * layoutCanvas.effScale))
                                            z: isSel ? 5 : (index + 1)
                                            radius: 8
                                            color: !modelData.enabled ? Theme.bgDeep : (isSel ? Theme.surfaceHover : Theme.surface)
                                            border.width: isSel ? 2 : 1
                                            border.color: isSel ? Theme.accent : (modelData.enabled ? Theme.border : Theme.dim)
                                            opacity: modelData.enabled ? 1.0 : 0.55

                                            Column {
                                                anchors.centerIn: parent
                                                spacing: 2
                                                width: parent.width - 8

                                                Text {
                                                    width: parent.width
                                                    horizontalAlignment: Text.AlignHCenter
                                                    text: modelData.name
                                                    color: Theme.textBright
                                                    font.family: Theme.fontFamily
                                                    font.pixelSize: 10
                                                    font.weight: Font.DemiBold
                                                    elide: Text.ElideRight
                                                }

                                                Text {
                                                    width: parent.width
                                                    horizontalAlignment: Text.AlignHCenter
                                                    text: modelData.enabled ? (modelData.x + "," + modelData.y) : "Off"
                                                    color: isSel ? Theme.accent : Theme.muted
                                                    font.family: Theme.fontFamily
                                                    font.pixelSize: 9
                                                    elide: Text.ElideRight
                                                }
                                            }

                                            MouseArea {
                                                anchors.fill: parent
                                                hoverEnabled: true
                                                cursorShape: root.draftMonitors.length >= 2 ? Qt.OpenHandCursor : Qt.PointingHandCursor
                                                property real pressGlobalX: 0
                                                property real pressGlobalY: 0
                                                property real origX: 0
                                                property real origY: 0

                                                onPressed: mouse => {
                                                    root.pinnedRequested();
                                                    root.selectMonitor(index);
                                                    if (root.draftMonitors.length < 2) return;
                                                    var g = mapToItem(layoutCanvas, mouse.x, mouse.y);
                                                    pressGlobalX = g.x;
                                                    pressGlobalY = g.y;
                                                    origX = modelData.x;
                                                    origY = modelData.y;
                                                    layoutCanvas.frozenScale = layoutCanvas.metrics.scale;
                                                    layoutCanvas.frozenOffsetX = layoutCanvas.metrics.offX;
                                                    layoutCanvas.frozenOffsetY = layoutCanvas.metrics.offY;
                                                    layoutCanvas.dragging = true;
                                                }

                                                onPositionChanged: mouse => {
                                                    if (!pressed || root.draftMonitors.length < 2) return;
                                                    var g = mapToItem(layoutCanvas, mouse.x, mouse.y);
                                                    var sc = Math.max(0.01, layoutCanvas.frozenScale);
                                                    var nx = Math.round(origX + (g.x - pressGlobalX) / sc);
                                                    var ny = Math.round(origY + (g.y - pressGlobalY) / sc);

                                                    var list = root.draftMonitors;
                                                    var cur = list[index];
                                                    var snapDist = 140;
                                                    for (var j = 0; j < list.length; j++) {
                                                        if (j === index) continue;
                                                        var o = list[j];
                                                        if (!o.enabled) continue;
                                                        var xCands = [
                                                            o.x - cur.logicalWidth,
                                                            o.x + o.logicalWidth,
                                                            o.x,
                                                            o.x + Math.round((o.logicalWidth - cur.logicalWidth) / 2)
                                                        ];
                                                        var yCands = [
                                                            o.y - cur.logicalHeight,
                                                            o.y + o.logicalHeight,
                                                            o.y,
                                                            o.y + Math.round((o.logicalHeight - cur.logicalHeight) / 2)
                                                        ];
                                                        for (var xi = 0; xi < xCands.length; xi++) {
                                                            if (Math.abs(nx - xCands[xi]) < snapDist) {
                                                                nx = xCands[xi];
                                                                break;
                                                            }
                                                        }
                                                        for (var yi = 0; yi < yCands.length; yi++) {
                                                            if (Math.abs(ny - yCands[yi]) < snapDist) {
                                                                ny = yCands[yi];
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    var updated = list.slice();
                                                    updated[index] = Object.assign({}, cur, { x: nx, y: ny });
                                                    root.draftMonitors = updated;
                                                    root.layoutDirty = true;
                                                }

                                                onReleased: {
                                                    if (!layoutCanvas.dragging) return;
                                                    layoutCanvas.dragging = false;
                                                    var list = root.draftMonitors || [];
                                                    if (list.length === 0) return;
                                                    var minX = list[0].x, minY = list[0].y;
                                                    for (var i = 1; i < list.length; i++) {
                                                        if (list[i].x < minX) minX = list[i].x;
                                                        if (list[i].y < minY) minY = list[i].y;
                                                    }
                                                    if (minX !== 0 || minY !== 0) {
                                                        var norm = [];
                                                        for (var k = 0; k < list.length; k++) {
                                                            norm.push(Object.assign({}, list[k], {
                                                                x: list[k].x - minX,
                                                                y: list[k].y - minY
                                                            }));
                                                        }
                                                        root.draftMonitors = norm;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                OptionDropdown {
                                    visible: root.draftMonitors.length >= 2
                                    label: "Position"
                                    currentText: {
                                        if (root.draftMonitors.length < 2) return "Single";
                                        var a = root.draftMonitors[0], b = root.draftMonitors[1];
                                        if (b.x === a.x && b.y === a.y) return "Mirror";
                                        if (b.x >= a.x + a.logicalWidth - 20) return "Right of";
                                        if (b.x + b.logicalWidth <= a.x + 20) return "Left of";
                                        if (b.y + b.logicalHeight <= a.y + 20) return "Above";
                                        if (b.y >= a.y + a.logicalHeight - 20) return "Below";
                                        return "Custom";
                                    }
                                    selectedValue: ""
                                    expanded: root.openMonitorDropdown === "pos"
                                    options: [
                                        { value: "right", label: "Right of primary" },
                                        { value: "left",  label: "Left of primary" },
                                        { value: "above", label: "Above primary" },
                                        { value: "below", label: "Below primary" },
                                        { value: "swap",  label: "Swap sides" }
                                    ]
                                    onHeaderClicked: {
                                        root.pinnedRequested();
                                        root.toggleMonitorDropdown("pos");
                                    }
                                    onOptionPicked: (val, lbl) => {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        root.applyPresetPosition(val);
                                    }
                                }

                                Item {
                                    visible: root.draftMonitors.length >= 2 && root.layoutDirty
                                    width: parent.width
                                    height: 26

                                    Rectangle {
                                        anchors.right: parent.right
                                        width: 108
                                        height: 26
                                        radius: 8
                                        color: applyPosMa.containsMouse ? Theme.textBright : Theme.accent

                                        Text {
                                            anchors.centerIn: parent
                                            text: "Apply Position"
                                            color: Theme.accentText
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            font.weight: Font.DemiBold
                                        }

                                        MouseArea {
                                            id: applyPosMa
                                            anchors.fill: parent
                                            enabled: !Services.monitorsApplying
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                root.pinnedRequested();
                                                root.layoutDirty = false;
                                                Services.applyMonitorLayout(root.draftMonitors);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Right: Monitor Settings
                        Rectangle {
                            id: monDetailBox
                            visible: root.draftMonitors.length > 0
                            width: parent.width - Math.floor((parent.width - 10) * 0.5) - 10
                            height: Math.max(arrCol.implicitHeight + 20, monDetailCol.implicitHeight + 20)
                            radius: 14
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            readonly property var curMon: (root.selectedMonitorIdx >= 0 && root.selectedMonitorIdx < root.draftMonitors.length)
                                                          ? root.draftMonitors[root.selectedMonitorIdx]
                                                          : null
                            readonly property int enabledCount: {
                                var c = 0;
                                for (var i = 0; i < root.draftMonitors.length; i++) {
                                    if (root.draftMonitors[i].enabled) c++;
                                }
                                return c;
                            }

                            Column {
                                id: monDetailCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 7

                                OptionDropdown {
                                    visible: root.draftMonitors.length >= 2
                                    label: "Monitor"
                                    selectedValue: root.selectedMonitorIdx
                                    currentText: monDetailBox.curMon ? monDetailBox.curMon.name : ""
                                    expanded: root.openMonitorDropdown === "monitor"
                                    options: {
                                        var arr = [];
                                        for (var i = 0; i < root.draftMonitors.length; i++) {
                                            arr.push({ value: i, label: root.draftMonitors[i].name });
                                        }
                                        return arr;
                                    }
                                    onHeaderClicked: {
                                        root.pinnedRequested();
                                        root.toggleMonitorDropdown("monitor");
                                    }
                                    onOptionPicked: (val, lbl) => {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        root.selectMonitor(val);
                                    }
                                }

                                Item {
                                    visible: root.draftMonitors.length >= 2
                                    width: parent.width
                                    height: 24

                                    Text {
                                        anchors.left: parent.left
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: "Enabled"
                                        color: Theme.text
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 11
                                        font.weight: Font.Medium
                                    }

                                    SoftSwitch {
                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        checked: root.draftEnabled
                                        onToggled: {
                                            var cur = monDetailBox.curMon;
                                            if (root.draftEnabled && monDetailBox.enabledCount <= 1 && cur && cur.enabled) {
                                                return;
                                            }
                                            root.draftEnabled = !root.draftEnabled;
                                        }
                                    }
                                }

                                OptionDropdown {
                                    label: "Resolution"
                                    selectedValue: root.draftModeSpec
                                    currentText: {
                                        var cur = monDetailBox.curMon;
                                        if (!cur || !cur.modes) return root.draftModeSpec;
                                        for (var i = 0; i < cur.modes.length; i++) {
                                            if (cur.modes[i].spec === root.draftModeSpec) return cur.modes[i].label;
                                        }
                                        return root.draftModeSpec;
                                    }
                                    expanded: root.openMonitorDropdown === "res"
                                    options: {
                                        var cur = monDetailBox.curMon;
                                        if (!cur || !cur.modes) return [];
                                        return cur.modes.map(function(m) {
                                            return { value: m.spec, label: m.label };
                                        });
                                    }
                                    onHeaderClicked: {
                                        root.pinnedRequested();
                                        root.toggleMonitorDropdown("res");
                                    }
                                    onOptionPicked: (val, lbl) => {
                                        root.pinnedRequested();
                                        root.openMonitorDropdown = "";
                                        root.draftModeSpec = val;
                                    }
                                }

                                Row {
                                    width: parent.width
                                    spacing: 8

                                    OptionDropdown {
                                        width: (parent.width - 8) / 2
                                        label: "Scale"
                                        selectedValue: root.draftScale
                                        currentText: root.draftScale + "x"
                                        expanded: root.openMonitorDropdown === "scale"
                                        options: [
                                            { value: 1.0,  label: "1x" },
                                            { value: 1.25, label: "1.25x" },
                                            { value: 1.5,  label: "1.5x" },
                                            { value: 1.75, label: "1.75x" },
                                            { value: 2.0,  label: "2x" }
                                        ]
                                        onHeaderClicked: {
                                            root.pinnedRequested();
                                            root.toggleMonitorDropdown("scale");
                                        }
                                        onOptionPicked: (val, lbl) => {
                                            root.pinnedRequested();
                                            root.openMonitorDropdown = "";
                                            root.draftScale = val;
                                        }
                                    }

                                    OptionDropdown {
                                        width: (parent.width - 8) / 2
                                        label: "Rotate"
                                        selectedValue: root.draftTransform
                                        currentText: {
                                            var map = { "normal": "0°", "90": "90°", "180": "180°", "270": "270°" };
                                            return map[root.draftTransform] || "0°";
                                        }
                                        expanded: root.openMonitorDropdown === "rot"
                                        options: [
                                            { value: "normal", label: "0°" },
                                            { value: "90",     label: "90°" },
                                            { value: "180",    label: "180°" },
                                            { value: "270",    label: "270°" }
                                        ]
                                        onHeaderClicked: {
                                            root.pinnedRequested();
                                            root.toggleMonitorDropdown("rot");
                                        }
                                        onOptionPicked: (val, lbl) => {
                                            root.pinnedRequested();
                                            root.openMonitorDropdown = "";
                                            root.draftTransform = val;
                                        }
                                    }
                                }

                                Item {
                                    width: parent.width
                                    height: 28

                                    Text {
                                        visible: root.draftMonitors.length === 1 && monDetailBox.curMon !== null
                                        anchors.left: parent.left
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: monDetailBox.curMon ? monDetailBox.curMon.name : ""
                                        color: Theme.muted
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 11
                                    }

                                    Rectangle {
                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 100
                                        height: 26
                                        radius: 8
                                        color: Services.monitorsApplying ? Theme.surfaceHover : (applyMonMa.containsMouse ? Theme.textBright : Theme.accent)

                                        Text {
                                            anchors.centerIn: parent
                                            text: Services.monitorsApplying ? "Applying..." : "Apply"
                                            color: Theme.accentText
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            font.weight: Font.DemiBold
                                        }

                                        MouseArea {
                                            id: applyMonMa
                                            anchors.fill: parent
                                            enabled: !Services.monitorsApplying
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                var cur = monDetailBox.curMon;
                                                if (!cur) return;
                                                root.pinnedRequested();
                                                root.openMonitorDropdown = "";
                                                Services.applyMonitorSettings(
                                                    cur.name,
                                                    root.draftEnabled,
                                                    root.draftModeSpec,
                                                    root.draftScale,
                                                    root.draftTransform,
                                                    cur.x,
                                                    cur.y
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ══════════════════════════════════════════════════
            // TAB 5: QUICK CONTROLS & SESSION (Ultra-compact 1-screen)
            // ══════════════════════════════════════════════════
            Flickable {
                id: ctrlFlick
                anchors.fill: parent
                anchors.rightMargin: 6
                visible: root.activeTab === "controls"
                contentHeight: ctrlCol.implicitHeight
                interactive: false
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                onContentHeightChanged: {
                    var m = Math.max(0, contentHeight - height);
                    if (contentY > m) contentY = m;
                }

                Column {
                    id: ctrlCol
                    width: parent.width
                    spacing: 10

                    Text {
                        text: "Controls & Session"
                        color: Theme.textBright
                        font.family: Theme.fontFamily
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }

                    // Row 1: Volume & Brightness side-by-side
                    Row {
                        width: parent.width
                        spacing: 10

                        SmoothSlider {
                            width: (parent.width - 10) / 2
                            icon: Services.volumeMuted ? "󰝟" : "󰕾"
                            minValue: 0
                            maxValue: 150
                            value: Services.volumePercent
                            labelText: Services.volumeMuted ? "Muted" : ("Volume " + Services.volumePercent + "%")
                            onMoved: v => {
                                root.pinnedRequested();
                                Services.setVolume(v);
                            }
                            onIconClicked: {
                                root.pinnedRequested();
                                Services.toggleMute();
                            }
                        }

                        SmoothSlider {
                            width: (parent.width - 10) / 2
                            icon: "󰃠"
                            minValue: 5
                            maxValue: 100
                            value: Services.brightnessPercent
                            labelText: "Brightness " + Services.brightnessPercent + "%"
                            onMoved: v => {
                                root.pinnedRequested();
                                Services.setBrightness(v);
                            }
                        }
                    }

                    // Row 2: 4 Quick Toggles
                    Row {
                        width: parent.width
                        spacing: 10

                        ToggleCard {
                            width: (parent.width - 30) / 4
                            icon: Services.wifiEnabled ? "󰖩" : "󰖪"
                            title: "Wi-Fi"
                            subtitle: Services.wifiEnabled ? (Services.wifiCurrentSsid || "On") : "Off"
                            active: Services.wifiEnabled
                            onClicked: {
                                root.pinnedRequested();
                                Services.toggleWifi();
                            }
                        }

                        ToggleCard {
                            width: (parent.width - 30) / 4
                            icon: Services.btPowered ? "󰂯" : "󰂲"
                            title: "Bluetooth"
                            subtitle: Services.btPowered
                                ? (Services.btConnectedCount > 0 ? Services.btConnectedCount + " conn" : "On")
                                : "Off"
                            active: Services.btPowered
                            onClicked: {
                                root.pinnedRequested();
                                Services.toggleBluetooth();
                            }
                        }

                        ToggleCard {
                            width: (parent.width - 30) / 4
                            icon: "󰒓"
                            title: "Lotus"
                            subtitle: "IME"
                            active: false
                            onClicked: {
                                Services.openLotusSettings();
                                root.closeRequested();
                            }
                        }

                        ToggleCard {
                            width: (parent.width - 30) / 4
                            icon: "󰑓"
                            title: "MangoWM"
                            subtitle: "Reload"
                            active: false
                            onClicked: {
                                root.pinnedRequested();
                                Services.reloadMango();
                            }
                        }
                    }

                    Text {
                        text: "Power"
                        color: Theme.textBright
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                    }

                    // Row 3: All 5 Power options in 1 horizontal row
                    Row {
                        width: parent.width
                        spacing: 8

                        Repeater {
                            model: [
                                { id: "lock",     icon: "󰌾", label: "Lock" },
                                { id: "suspend",  icon: "󰤄", label: "Suspend" },
                                { id: "logout",   icon: "󰍃", label: "Log Out" },
                                { id: "reboot",   icon: "󰜉", label: "Reboot" },
                                { id: "shutdown", icon: "󰐥", label: "Shut Down" }
                            ]
                            delegate: ActionPill {
                                required property var modelData
                                width: (ctrlCol.width - 32) / 5
                                height: 34
                                icon: modelData.icon
                                label: modelData.label
                                onClicked: {
                                    root.closeRequested();
                                    Services.powerAction(modelData.id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    FileBrowserDialog {
        id: fileBrowser
        z: 50
        onAccepted: filePath => {
            root.applyPickedImage(filePath);
            root.forceActiveFocus();
        }
        onCancelled: root.forceActiveFocus()
    }
}
