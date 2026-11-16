import QtQuick
import ".."

Item {
    id: root
    width: parent ? parent.width : 324
    implicitHeight: mainCol.implicitHeight

    signal closeRequested()

    property bool showHistory: false
    property string menuConvId: ""
    property real menuX: 0
    property real menuY: 0

    onShowHistoryChanged: menuConvId = ""

    function focusInput() {
        promptInput.forceActiveFocus();
    }

    function mdToHtml(raw) {
        if (!raw)
            return "";
        var normalized = raw.replace(/\r\n/g, "\n").replace(/\n{3,}/g, "\n\n").replace(/\s+$/, "");
        function escHtml(s) {
            return s.replace(/&/g, "&amp;")
                    .replace(/</g, "&lt;")
                    .replace(/>/g, "&gt;");
        }
        function formatSegment(seg) {
            if (!seg)
                return "";
            var codeTokens = [];
            var withTokens = seg.replace(/`([^`\n]+)`/g, function(_, code) {
                var idx = codeTokens.length;
                codeTokens.push("<span style=\"background-color:#3b4252; color:#eceff4;\">&nbsp;" + escHtml(code) + "&nbsp;</span>");
                return "\x00C" + idx + "\x00";
            });
            var lines = escHtml(withTokens).split("\n");
            for (var i = 0; i < lines.length; i++) {
                var ln = lines[i];
                var hm = ln.match(/^\s{0,3}#{1,6}\s+(.*)$/);
                if (hm) {
                    ln = "<b><font color=\"#eceff4\">" + hm[1] + "</font></b>";
                } else {
                    ln = ln.replace(/^(\s*)[-*]\s+/, "$1• ");
                }
                ln = ln.replace(/\*\*([^\n]+?)\*\*/g, "<b><font color=\"#eceff4\">$1</font></b>");
                ln = ln.replace(/__([^\n]+?)__/g, "<b><font color=\"#eceff4\">$1</font></b>");
                ln = ln.replace(/(^|[^\w*])\*([^\n*]+?)\*(?!\w)/g, "$1<i>$2</i>");
                ln = ln.replace(/\x00C(\d+)\x00/g, function(_, n) {
                    return codeTokens[parseInt(n, 10)] || "";
                });
                lines[i] = ln;
            }
            return lines.join("<br/>");
        }

        // Split fenced code blocks ```...``` without <div> block margin quirks
        var parts = normalized.split("```");
        var chunks = [];
        for (var p = 0; p < parts.length; p++) {
            if (p % 2 === 0) {
                var txt = parts[p].replace(/^\n+/, "").replace(/\n+$/, "");
                if (txt.length > 0)
                    chunks.push(formatSegment(txt));
            } else {
                var block = parts[p].replace(/^[a-zA-Z0-9_+-]*\n/, "").replace(/^\n+/, "").replace(/\n+$/, "");
                if (block.length > 0) {
                    var bLines = escHtml(block).split("\n");
                    for (var b = 0; b < bLines.length; b++) {
                        bLines[b] = "<span style=\"background-color:#2b303b; color:#d8dee9;\">&nbsp;" + bLines[b].replace(/ /g, "&nbsp;") + "&nbsp;</span>";
                    }
                    chunks.push(bLines.join("<br/>"));
                }
            }
        }
        return chunks.join("<br/>");
    }

    function submitPrompt() {
        var txt = promptInput.text.trim();
        if (txt.length === 0 || Services.agyBusy)
            return;
        root.showHistory = false;
        promptInput.text = "";
        Services.agySend(txt);
        Qt.callLater(function() {
            chatList.scrollToBottom(false);
        });
    }

    Connections {
        target: Services
        function onAgyMessagesChanged() {
            if (!root.showHistory) {
                Qt.callLater(function() {
                    chatList.scrollToBottom(false);
                });
            }
        }
    }

    Column {
        id: mainCol
        width: parent.width
        spacing: 6

        // 1. Compact Header
        Rectangle {
            width: parent.width
            height: 38
            radius: 11
            color: Theme.surface
            border.width: 1
            border.color: Theme.border

            Item {
                anchors.fill: parent
                anchors.leftMargin: 9
                anchors.rightMargin: 7

                Row {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 7

                    Image {
                        anchors.verticalCenter: parent.verticalCenter
                        source: Qt.resolvedUrl("../assets/yuno-gasai-icon.png")
                        width: 18
                        height: 18
                        fillMode: Image.PreserveAspectFit
                        smooth: true
                        mipmap: true
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: {
                            if (root.showHistory)
                                return "History · " + Services.agyHistory.length;
                            if (Services.agyBusy)
                                return "● " + (Services.agyStatus.length > 0 ? Services.agyStatus : "Thinking");
                            if (Services.agyConversationId.length > 0)
                                return "● Active";
                            return "○ Ready";
                        }
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        color: Services.agyBusy ? Theme.textBright : Theme.muted
                    }
                }

                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4

                    // Model cycle chip
                    Rectangle {
                        width: modelLabelText.implicitWidth + 14
                        height: 24
                        radius: 7
                        color: modelMouse.containsMouse ? Theme.surfaceActive : Theme.surfaceHover
                        border.width: 1
                        border.color: Theme.border

                        Behavior on color { ColorAnimation { duration: 100 } }

                        Text {
                            id: modelLabelText
                            anchors.centerIn: parent
                            text: Services.agyModelLabel
                            font.family: Theme.fontFamily
                            font.pixelSize: 10
                            font.weight: Font.Medium
                            color: Theme.text
                        }

                        MouseArea {
                            id: modelMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Services.agyCycleModel()
                        }
                    }

                    // History toggle button
                    Rectangle {
                        width: 24
                        height: 24
                        radius: 7
                        color: root.showHistory
                            ? Theme.accent
                            : (histBtnMouse.containsMouse ? Theme.surfaceActive : Theme.surfaceHover)
                        border.width: 1
                        border.color: root.showHistory ? Theme.accent : Theme.border

                        Behavior on color { ColorAnimation { duration: 100 } }

                        Text {
                            anchors.centerIn: parent
                            text: "󰋚"
                            font.family: Theme.fontFamily
                            font.pixelSize: 13
                            color: root.showHistory ? Theme.accentText : Theme.text
                        }

                        MouseArea {
                            id: histBtnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.showHistory = !root.showHistory;
                                if (root.showHistory)
                                    Services.refreshAgyHistory();
                            }
                        }
                    }

                    // Open full terminal session
                    Rectangle {
                        width: 24
                        height: 24
                        radius: 7
                        color: termMouse.containsMouse ? Theme.surfaceActive : Theme.surfaceHover
                        border.width: 1
                        border.color: Theme.border

                        Behavior on color { ColorAnimation { duration: 100 } }

                        Text {
                            anchors.centerIn: parent
                            text: "󰆍"
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            color: Theme.text
                        }

                        MouseArea {
                            id: termMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                Services.agyOpenTerminal();
                                root.closeRequested();
                            }
                        }
                    }

                    // New chat
                    Rectangle {
                        width: 24
                        height: 24
                        radius: 7
                        color: newChatMouse.containsMouse ? Theme.surfaceActive : Theme.surfaceHover
                        border.width: 1
                        border.color: Theme.border

                        Behavior on color { ColorAnimation { duration: 100 } }

                        Text {
                            anchors.centerIn: parent
                            text: "󰐕"
                            font.family: Theme.fontFamily
                            font.pixelSize: 13
                            color: Theme.text
                        }

                        MouseArea {
                            id: newChatMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.showHistory = false;
                                Services.agyNewChat();
                                root.focusInput();
                            }
                        }
                    }
                }
            }
        }

        // 2. Draggable Effort Slider Bar
        Rectangle {
            width: parent.width
            height: 28
            radius: 9
            color: Theme.surface
            border.width: 1
            border.color: Theme.border

            Item {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 7

                Text {
                    id: effortPrefix
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Effort"
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                }

                Rectangle {
                    id: effortBadge
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: 44
                    height: 19
                    radius: 6
                    color: Theme.surfaceHover
                    border.width: 1
                    border.color: Theme.border

                    Text {
                        anchors.centerIn: parent
                        text: Services.agyEffortLabel
                        font.family: Theme.fontFamily
                        font.pixelSize: 9
                        font.bold: true
                        color: Theme.textBright
                    }
                }

                Item {
                    id: sliderArea
                    anchors.left: effortPrefix.right
                    anchors.right: effortBadge.left
                    anchors.leftMargin: 10
                    anchors.rightMargin: 9
                    height: parent.height

                    readonly property int count: Math.max(1, Services.agyEfforts.length)
                    readonly property real pad: 7
                    readonly property real usableW: Math.max(1, width - pad * 2)
                    property bool dragging: false
                    property real dragFrac: 0
                    readonly property real targetFrac: count <= 1 ? 1.0 : (Services.agySafeEffortIdx / (count - 1))
                    readonly property real curFrac: dragging ? dragFrac : targetFrac
                    readonly property real knobX: pad + usableW * curFrac

                    // Background rail
                    Rectangle {
                        x: sliderArea.pad
                        width: sliderArea.usableW
                        height: 5
                        radius: 3
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.surfaceActive
                    }

                    // Active fill rail
                    Rectangle {
                        x: sliderArea.pad
                        width: Math.max(5, sliderArea.usableW * sliderArea.curFrac)
                        height: 5
                        radius: 3
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.accent

                        Behavior on width {
                            enabled: !sliderArea.dragging
                            NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
                        }
                    }

                    // Step dots along track
                    Repeater {
                        model: sliderArea.count

                        Rectangle {
                            readonly property real stopFrac: sliderArea.count <= 1 ? 1.0 : (index / (sliderArea.count - 1))
                            width: 5
                            height: 5
                            radius: 3
                            x: sliderArea.pad + sliderArea.usableW * stopFrac - width / 2
                            anchors.verticalCenter: parent.verticalCenter
                            color: stopFrac <= sliderArea.curFrac + 0.01 ? Theme.accentText : Theme.muted
                            opacity: 0.75
                        }
                    }

                    // Draggable thumb
                    Rectangle {
                        id: effortKnob
                        width: (sliderArea.dragging || sliderMouse.containsMouse) ? 14 : 12
                        height: width
                        radius: width / 2
                        x: sliderArea.knobX - width / 2
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.textBright
                        border.width: 2
                        border.color: Theme.accentText

                        Behavior on width { NumberAnimation { duration: 90 } }
                        Behavior on x {
                            enabled: !sliderArea.dragging
                            NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
                        }
                    }

                    MouseArea {
                        id: sliderMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor

                        function applyMouse(mx) {
                            if (sliderArea.count <= 1) {
                                sliderArea.dragFrac = 1.0;
                                Services.agySetEffort(0);
                                return;
                            }
                            var frac = Math.max(0, Math.min(1, (mx - sliderArea.pad) / sliderArea.usableW));
                            sliderArea.dragFrac = frac;
                            var nearest = Math.round(frac * (sliderArea.count - 1));
                            Services.agySetEffort(nearest);
                        }

                        onPressed: mouse => {
                            sliderArea.dragging = true;
                            applyMouse(mouse.x);
                        }
                        onPositionChanged: mouse => {
                            if (pressed)
                                applyMouse(mouse.x);
                        }
                        onReleased: sliderArea.dragging = false
                        onCanceled: sliderArea.dragging = false
                        onWheel: wheel => {
                            if (sliderArea.count <= 1) return;
                            var step = wheel.angleDelta.y > 0 ? 1 : -1;
                            var next = Math.max(0, Math.min(sliderArea.count - 1, Services.agySafeEffortIdx + step));
                            Services.agySetEffort(next);
                        }
                    }
                }
            }
        }

        // 3. Body Card (collapsible: hidden until a chat message exists or History is open)
        Rectangle {
            id: bodyCard
            readonly property bool showBody: root.showHistory || Services.agyMessages.length > 0
            width: parent.width
            height: showBody ? 196 : 0
            visible: height > 0.5
            opacity: showBody ? 1.0 : 0.0
            radius: 12
            color: Theme.surface
            border.width: 1
            border.color: Theme.border
            clip: true

            Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
            Behavior on opacity { NumberAnimation { duration: 120 } }

            // A. Chat View
            Item {
                id: chatBox
                anchors.fill: parent
                anchors.margins: 6
                visible: !root.showHistory && Services.agyMessages.length > 0
                clip: true

                ListView {
                    id: chatList
                    anchors.fill: parent
                    anchors.rightMargin: 6
                    interactive: false
                    boundsBehavior: Flickable.StopAtBounds
                    spacing: 6
                    topMargin: 2
                    bottomMargin: 2
                    clip: true
                    model: Services.agyMessages

                    property real savedY: 0
                    property bool autoFollow: true
                    property bool instantJump: false

                    function minScroll() {
                        return originY;
                    }

                    function maxScroll() {
                        return Math.max(originY, originY + contentHeight - height);
                    }

                    function applyScroll(yVal, instant) {
                        var clamped = Math.max(minScroll(), Math.min(yVal, maxScroll()));
                        savedY = clamped;
                        if (instant) {
                            instantJump = true;
                            contentY = clamped;
                            instantJump = false;
                        } else {
                            contentY = clamped;
                        }
                    }

                    function scrollToBottom(instant) {
                        autoFollow = true;
                        applyScroll(maxScroll(), !!instant);
                    }

                    Behavior on contentY {
                        enabled: !chatList.instantJump
                        NumberAnimation { duration: 55; easing.type: Easing.OutCubic }
                    }

                    onContentHeightChanged: {
                        var m = maxScroll();
                        if (autoFollow) {
                            applyScroll(m, true);
                        } else if (savedY > m) {
                            applyScroll(m, true);
                        } else if (savedY < minScroll()) {
                            applyScroll(minScroll(), true);
                        }
                    }

                    onCountChanged: {
                        if (autoFollow) {
                            Qt.callLater(function() {
                                chatList.scrollToBottom(false);
                            });
                        }
                    }

                    onVisibleChanged: {
                        if (visible) {
                            Qt.callLater(function() {
                                chatList.scrollToBottom(true);
                            });
                        }
                    }

                    delegate: Item {
                        id: delItem
                        width: Math.max(120, chatList.width)
                        height: bubble.height

                        readonly property bool isUser: modelData.role === "user"
                        readonly property bool isError: modelData.role === "error"
                        readonly property string displayBody: modelData.streaming
                            ? (Services.agyLiveText.length > 0 ? Services.agyLiveText : "...")
                            : modelData.text

                        Text {
                            id: userSizer
                            visible: false
                            text: isUser ? displayBody : ""
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                        }

                        Rectangle {
                            id: bubble
                            anchors.right: isUser ? parent.right : undefined
                            anchors.left: isUser ? undefined : parent.left
                            width: isUser
                                ? Math.min(parent.width * 0.86, Math.max(64, userSizer.implicitWidth + 20))
                                : parent.width
                            height: Math.round(bubbleCol.implicitHeight + 12)
                            radius: 10
                            color: isUser
                                ? Theme.surfaceActive
                                : (isError ? "#453338" : "#343b49")
                            border.width: 1
                            border.color: isError ? "#6e474d" : Theme.border

                            Column {
                                id: bubbleCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 6
                                anchors.leftMargin: 9
                                anchors.rightMargin: 9
                                spacing: 2

                                Item {
                                    visible: !isUser
                                    width: parent.width
                                    height: visible ? 14 : 0

                                    Text {
                                        anchors.left: parent.left
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: modelData.streaming
                                            ? ("● " + (Services.agyStatus.length > 0 ? Services.agyStatus : "Streaming"))
                                            : (modelData.meta && modelData.meta.length > 0
                                                ? modelData.meta
                                                : (isError ? "Error" : "Antigravity"))
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 9
                                        color: Theme.muted
                                    }

                                    Rectangle {
                                        id: copyBtn
                                        property bool copied: false
                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 18
                                        height: 14
                                        radius: 4
                                        color: copyMouse.containsMouse ? Theme.surfaceHover : "transparent"

                                        Timer {
                                            id: copyTimer
                                            interval: 1200
                                            onTriggered: copyBtn.copied = false
                                        }

                                        Text {
                                            anchors.centerIn: parent
                                            text: copyBtn.copied ? "󰄬" : "󰆏"
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 10
                                            color: copyBtn.copied ? Theme.textBright : Theme.muted
                                        }

                                        MouseArea {
                                            id: copyMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                Services.agyCopyText(displayBody);
                                                copyBtn.copied = true;
                                                copyTimer.restart();
                                            }
                                        }
                                    }
                                }

                                TextEdit {
                                    id: msgEdit
                                    width: isUser ? Math.max(44, parent.width) : Math.max(120, parent.width)
                                    textFormat: isUser ? TextEdit.PlainText : TextEdit.RichText
                                    text: isUser ? displayBody : root.mdToHtml(displayBody)
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    selectionColor: Theme.accentSoft
                                    selectedTextColor: Theme.textBright
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    color: isUser ? Theme.textBright : Theme.text
                                }
                            }
                        }
                    }
                }

                // Visual scrollbar indicator
                Rectangle {
                    id: chatScrollTrack
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.topMargin: 2
                    anchors.bottomMargin: 2
                    width: 3
                    radius: 2
                    color: "#22c6d0f5"
                    visible: chatList.contentHeight > chatList.height + 2

                    Rectangle {
                        readonly property real maxSpan: Math.max(1, chatList.contentHeight - chatList.height)
                        readonly property real ratio: Math.min(1, chatList.height / Math.max(1, chatList.contentHeight))
                        width: parent.width
                        height: Math.max(18, parent.height * ratio)
                        y: (parent.height - height) * Math.max(0, Math.min(1, (chatList.contentY - chatList.originY) / maxSpan))
                        radius: 2
                        color: Theme.muted
                    }
                }

                WheelHandler {
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onWheel: function(event) {
                        if (Services.agyMessages.length === 0)
                            return;
                        var delta = (event.pixelDelta && event.pixelDelta.y !== 0)
                            ? (event.pixelDelta.y * 1.9)
                            : (event.angleDelta.y / 120.0) * 92;
                        var minY = chatList.minScroll();
                        var maxY = chatList.maxScroll();
                        var nextY = Math.max(minY, Math.min(chatList.savedY - delta, maxY));
                        chatList.autoFollow = (maxY - nextY) < 16;
                        chatList.applyScroll(nextY, false);
                        event.accepted = true;
                    }
                }
            }

            // B. History View
            Item {
                id: historyBox
                anchors.fill: parent
                anchors.margins: 6
                visible: root.showHistory
                clip: true

                Text {
                    anchors.centerIn: parent
                    visible: Services.agyHistoryLoading && Services.agyHistory.length === 0
                    text: "Đang tải lịch sử..."
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                }

                Text {
                    anchors.centerIn: parent
                    visible: !Services.agyHistoryLoading && Services.agyHistory.length === 0
                    text: "Chưa có lịch sử chat nào"
                    font.family: Theme.fontFamily
                    font.pixelSize: 10
                    color: Theme.muted
                }

                ListView {
                    id: historyList
                    anchors.fill: parent
                    anchors.rightMargin: histScrollTrack.visible ? 6 : 0
                    interactive: false
                    boundsBehavior: Flickable.StopAtBounds
                    spacing: 4
                    clip: true
                    model: Services.agyHistory

                    property real savedY: 0
                    property bool instantJump: false

                    function maxScroll() {
                        return Math.max(0, contentHeight - height);
                    }

                    function applyScroll(yVal, instant) {
                        var clamped = Math.max(0, Math.min(yVal, maxScroll()));
                        savedY = clamped;
                        if (instant) {
                            instantJump = true;
                            contentY = clamped;
                            instantJump = false;
                        } else {
                            contentY = clamped;
                        }
                    }

                    Behavior on contentY {
                        enabled: !historyList.instantJump
                        NumberAnimation { duration: 55; easing.type: Easing.OutCubic }
                    }

                    onVisibleChanged: {
                        if (visible)
                            applyScroll(0, true);
                    }

                    delegate: Rectangle {
                        width: historyList.width
                        height: 40
                        radius: 9

                        readonly property bool isCurrent: modelData.id === Services.agyConversationId

                        color: isCurrent
                            ? Theme.surfaceActive
                            : (histRowMouse.containsMouse ? Theme.surfaceHover : "#343b49")
                        border.width: 1
                        border.color: isCurrent ? Theme.accent : Theme.border

                        Behavior on color { ColorAnimation { duration: 90 } }

                        Column {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: 9
                            anchors.rightMargin: 9
                            spacing: 1

                            Text {
                                width: parent.width
                                text: modelData.title
                                elide: Text.ElideRight
                                font.family: Theme.fontFamily
                                font.pixelSize: 10
                                font.bold: isCurrent
                                color: isCurrent ? Theme.textBright : Theme.text
                            }

                            Item {
                                width: parent.width
                                height: 12

                                Text {
                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: (modelData.time || "") + " · " + modelData.steps + " steps"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 9
                                    color: Theme.muted
                                }

                                Text {
                                    visible: isCurrent
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "Active"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 9
                                    font.bold: true
                                    color: Theme.textBright
                                }
                            }
                        }

                        MouseArea {
                            id: histRowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            cursorShape: Qt.PointingHandCursor
                            onClicked: mouse => {
                                if (mouse.button === Qt.RightButton) {
                                    var pt = histRowMouse.mapToItem(root, mouse.x, mouse.y);
                                    root.menuX = Math.max(4, Math.min(pt.x, root.width - ctxMenu.width - 4));
                                    root.menuY = Math.max(4, Math.min(pt.y, root.height - ctxMenu.height - 4));
                                    root.menuConvId = modelData.id;
                                    return;
                                }
                                root.menuConvId = "";
                                root.showHistory = false;
                                Services.agyLoadConversation(modelData.id);
                                root.focusInput();
                            }
                        }
                    }
                }

                Rectangle {
                    id: histScrollTrack
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.topMargin: 2
                    anchors.bottomMargin: 2
                    width: 3
                    radius: 2
                    color: "#22c6d0f5"
                    visible: historyList.contentHeight > historyList.height + 2

                    Rectangle {
                        readonly property real maxY: Math.max(1, historyList.contentHeight - historyList.height)
                        readonly property real ratio: Math.min(1, historyList.height / Math.max(1, historyList.contentHeight))
                        width: parent.width
                        height: Math.max(18, parent.height * ratio)
                        y: (parent.height - height) * Math.max(0, Math.min(1, historyList.contentY / maxY))
                        radius: 2
                        color: Theme.muted
                    }
                }

                WheelHandler {
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onWheel: function(event) {
                        if (Services.agyHistory.length === 0)
                            return;
                        var delta = (event.pixelDelta && event.pixelDelta.y !== 0)
                            ? (event.pixelDelta.y * 1.9)
                            : (event.angleDelta.y / 120.0) * 92;
                        var maxY = historyList.maxScroll();
                        var nextY = Math.max(0, Math.min(historyList.savedY - delta, maxY));
                        historyList.applyScroll(nextY, false);
                        event.accepted = true;
                    }
                }
            }
        }

        // 4. Prompt Input Pill
        Rectangle {
            width: parent.width
            height: 36
            radius: 11
            color: Theme.surface
            border.width: 1
            border.color: promptInput.activeFocus ? Theme.accent : Theme.border

            Behavior on border.color { ColorAnimation { duration: 120 } }

            Item {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 5

                Text {
                    anchors.left: parent.left
                    anchors.right: actionBtn.left
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    visible: promptInput.text.length === 0
                    text: Services.agyBusy ? "Generating..." : "Ask anything..."
                    elide: Text.ElideRight
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.muted
                }

                TextInput {
                    id: promptInput
                    anchors.left: parent.left
                    anchors.right: actionBtn.left
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    clip: true
                    font.family: Theme.fontFamily
                    font.pixelSize: 11
                    color: Theme.textBright
                    selectionColor: Theme.accentSoft
                    selectedTextColor: Theme.textBright
                    onAccepted: root.submitPrompt()
                    Keys.onEscapePressed: root.closeRequested()
                }

                Rectangle {
                    id: actionBtn
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: 26
                    height: 26
                    radius: 8

                    readonly property bool canSend: promptInput.text.trim().length > 0

                    color: Services.agyBusy
                        ? (actionMouse.containsMouse ? "#5c3b41" : "#4a3236")
                        : (canSend ? Theme.accent : Theme.surfaceHover)
                    opacity: (Services.agyBusy || canSend) ? 1.0 : 0.45

                    Behavior on color { ColorAnimation { duration: 100 } }

                    Text {
                        anchors.centerIn: parent
                        text: Services.agyBusy ? "󰅖" : "󰜷"
                        font.family: Theme.fontFamily
                        font.pixelSize: 13
                        color: Services.agyBusy
                            ? Theme.textBright
                            : (actionBtn.canSend ? Theme.accentText : Theme.muted)
                    }

                    MouseArea {
                        id: actionMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (Services.agyBusy)
                                Services.agyStop();
                            else
                                root.submitPrompt();
                        }
                    }
                }
            }
        }
    }

    // Standard desktop right-click context menu for history items
    Item {
        id: ctxMenuOverlay
        anchors.fill: parent
        z: 50
        visible: root.menuConvId.length > 0

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: root.menuConvId = ""
        }

        Rectangle {
            id: ctxMenu
            x: root.menuX
            y: root.menuY
            width: 112
            height: ctxCol.implicitHeight + 8
            radius: 9
            color: "#2b303b"
            border.width: 1
            border.color: Theme.border

            Column {
                id: ctxCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 4
                spacing: 2

                Rectangle {
                    width: parent.width
                    height: 26
                    radius: 6
                    color: ctxOpenMouse.containsMouse ? Theme.surfaceHover : "transparent"

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 7

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "󰏌"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.text
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Mở chat"
                            font.family: Theme.fontFamily
                            font.pixelSize: 10
                            color: Theme.textBright
                        }
                    }

                    MouseArea {
                        id: ctxOpenMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            var cid = root.menuConvId;
                            root.menuConvId = "";
                            root.showHistory = false;
                            Services.agyLoadConversation(cid);
                            root.focusInput();
                        }
                    }
                }

                Rectangle {
                    width: parent.width - 8
                    height: 1
                    anchors.horizontalCenter: parent.horizontalCenter
                    color: Theme.border
                    opacity: 0.7
                }

                Rectangle {
                    width: parent.width
                    height: 26
                    radius: 6
                    color: ctxDelMouse.containsMouse ? "#453338" : "transparent"

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 7

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "󰆴"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: "#e06c75"
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Xóa"
                            font.family: Theme.fontFamily
                            font.pixelSize: 10
                            color: ctxDelMouse.containsMouse ? "#eceff4" : "#e06c75"
                        }
                    }

                    MouseArea {
                        id: ctxDelMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            var cid = root.menuConvId;
                            root.menuConvId = "";
                            Services.agyDeleteConversation(cid);
                        }
                    }
                }
            }
        }
    }
}
