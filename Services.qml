pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Mpris
import Quickshell.Services.Notifications
import Quickshell.Services.Pipewire

Item {
    id: root

    // ==================== WORKSPACES (MANGO) ====================
    property var tags: []

    function parseTagsJson(line) {
        if (!line || line.trim().length === 0) return;
        try {
            var data = JSON.parse(line);
            if (data.all_tags && data.all_tags.length > 0) {
                root.tags = data.all_tags[0].tags || [];
            } else if (data.tags) {
                root.tags = data.tags;
            }
        } catch (e) {}
    }

    function switchTag(idx) {
        Quickshell.execDetached(["mmsg", "dispatch", "view," + idx + ",0"]);
    }

    Process {
        command: ["mmsg", "get", "all-tags"]
        running: true
        stdout: SplitParser {
            onRead: data => root.parseTagsJson(data)
        }
    }

    Process {
        id: watchTagsProc
        command: ["mmsg", "watch", "all-tags"]
        running: true
        stdout: SplitParser {
            onRead: data => root.parseTagsJson(data)
        }
        onExited: tagsRestartTimer.restart()
    }

    Timer {
        id: tagsRestartTimer
        interval: 1500
        onTriggered: watchTagsProc.running = true
    }

    // ==================== RAM USAGE ====================
    property int ramPercent: 0
    property real ramUsedGb: 0
    property real ramTotalGb: 0

    Process {
        id: ramProc
        command: ["sh", "-c", "awk '/MemTotal/{t=$2}/MemAvailable/{a=$2}END{u=t-a; printf \"%d %.1f %.1f\\n\", (u*100/t), u/1048576, t/1048576}' /proc/meminfo"]
        running: true
        stdout: SplitParser {
            onRead: data => {
                var parts = data.trim().split(/\s+/);
                if (parts.length >= 3) {
                    root.ramPercent = parseInt(parts[0]) || 0;
                    root.ramUsedGb = parseFloat(parts[1]) || 0;
                    root.ramTotalGb = parseFloat(parts[2]) || 0;
                }
            }
        }
    }

    Timer {
        interval: 3000
        running: true
        repeat: true
        onTriggered: ramProc.running = true
    }

    // ==================== VOLUME & AUDIO OUTPUTS ====================
    PwObjectTracker {
        objects: [ Pipewire.defaultAudioSink ]
    }

    readonly property var audioSink: Pipewire.defaultAudioSink
    readonly property bool volumeMuted: audioSink?.audio?.muted ?? false
    readonly property real volumeLevel: audioSink?.audio?.volume ?? 0.0
    readonly property int volumePercent: Math.round(volumeLevel * 100)
    property var audioOutputs: []

    function setVolume(pct) {
        if (!audioSink || !audioSink.audio) return;
        var clamped = Math.max(0, Math.min(100, pct));
        audioSink.audio.muted = false;
        audioSink.audio.volume = clamped / 100.0;
    }

    function toggleMute() {
        if (!audioSink || !audioSink.audio) return;
        audioSink.audio.muted = !audioSink.audio.muted;
    }

    function refreshAudioOutputs() {
        audioOutProc.running = true;
    }

    function selectAudioOutput(sinkName, portName) {
        if (portName && portName.length > 0) {
            audioSwitchProc.command = ["sh", "-c", "pactl set-default-sink '" + sinkName + "' && pactl set-sink-port '" + sinkName + "' '" + portName + "'"];
        } else {
            audioSwitchProc.command = ["pactl", "set-default-sink", sinkName];
        }
        audioSwitchProc.running = true;
    }

    Process {
        id: audioOutProc
        command: [
            "python3", "-c",
            "import json, subprocess\n" +
            "try:\n" +
            "  def_sink = subprocess.check_output(['pactl', 'get-default-sink'], text=True).strip()\n" +
            "  sinks = json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'sinks'], text=True))\n" +
            "  for s in sinks:\n" +
            "    sn = s.get('name', '')\n" +
            "    sd = s.get('description', sn)\n" +
            "    is_d = (sn == def_sink)\n" +
            "    ports = s.get('ports') or []\n" +
            "    ap = s.get('active_port')\n" +
            "    if ports:\n" +
            "      for p in ports:\n" +
            "        pn = p.get('name', '')\n" +
            "        pd = p.get('description', pn)\n" +
            "        act = '1' if (is_d and pn == ap) else '0'\n" +
            "        print(f'OUT|{act}|{sn}|{pn}|{pd}')\n" +
            "    else:\n" +
            "      act = '1' if is_d else '0'\n" +
            "      print(f'OUT|{act}|{sn}||{sd}')\n" +
            "except Exception:\n" +
            "  pass\n"
        ]
        running: true
        property var list: []
        onStarted: list = []
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (!line.startsWith("OUT|")) return;
                var parts = line.split("|");
                if (parts.length >= 5) {
                    audioOutProc.list.push({
                        active: parts[1] === "1",
                        sink: parts[2],
                        port: parts[3],
                        label: parts.slice(4).join("|")
                    });
                }
            }
        }
        onExited: root.audioOutputs = audioOutProc.list
    }

    Process {
        id: audioSwitchProc
        onExited: root.refreshAudioOutputs()
    }

    // ==================== BRIGHTNESS ====================
    property int brightnessPercent: 70

    Process {
        id: brightnessGetProc
        command: ["brightnessctl", "-m"]
        running: true
        stdout: SplitParser {
            onRead: data => {
                var parts = data.trim().split(",");
                if (parts.length >= 4) {
                    root.brightnessPercent = parseInt(parts[3].replace("%", "")) || 0;
                }
            }
        }
    }

    Timer {
        interval: 5000
        running: true
        repeat: true
        onTriggered: brightnessGetProc.running = true
    }

    function setBrightness(pct) {
        var clamped = Math.max(1, Math.min(100, Math.round(pct)));
        root.brightnessPercent = clamped;
        Quickshell.execDetached(["brightnessctl", "set", clamped + "%"]);
    }

    // ==================== BATTERY & AUTO-CPUFREQ STATUS ====================
    property int batteryPercent: 100
    property string batteryStatus: "Discharging"
    property string batteryDetail: ""
    property string powerProfile: "balance_power"
    property string cpuTemp: "40°C"
    property string cpuFreqGhz: "0.8 GHz"

    readonly property bool batteryCharging: batteryStatus === "Charging" || batteryStatus === "Full"

    function refreshBattery() {
        batteryProc.running = true;
    }

    function setPowerProfile(prof) {
        // Handled automatically by auto-cpufreq daemon
        root.refreshBattery();
    }

    Process {
        id: batteryProc
        command: [
            "sh", "-c",
            "cap=$(cat /sys/class/power_supply/BAT*/capacity 2>/dev/null | head -n1); " +
            "st=$(cat /sys/class/power_supply/BAT*/status 2>/dev/null | head -n1); " +
            "pw=$(cat /sys/class/power_supply/BAT*/power_now 2>/dev/null | head -n1); " +
            "en=$(cat /sys/class/power_supply/BAT*/energy_now 2>/dev/null | head -n1); " +
            "ef=$(cat /sys/class/power_supply/BAT*/energy_full 2>/dev/null | head -n1); " +
            "epp=$(cat /sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference 2>/dev/null | head -n1); " +
            "freq=$(awk '{s+=$1; n++} END {if(n>0) printf \"%.2f GHz\", s/n/1000000}' /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq 2>/dev/null); " +
            "temp=$(awk '{printf \"%.0f°C\", $1/1000}' /sys/class/thermal/thermal_zone0/temp 2>/dev/null); " +
            "echo \"BAT|${cap:-100}|${st:-Unknown}|${pw:-0}|${en:-0}|${ef:-0}|${epp:-balance_power}|${freq:-0.8 GHz}|${temp:-40°C}\""
        ]
        running: true
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (!line.startsWith("BAT|")) return;
                var p = line.split("|");
                if (p.length >= 9) {
                    root.batteryPercent = parseInt(p[1]) || 0;
                    root.batteryStatus = p[2] || "Unknown";
                    var pw = parseInt(p[3]) || 0;
                    var en = parseInt(p[4]) || 0;
                    var ef = parseInt(p[5]) || 0;
                    root.powerProfile = p[6] || "balance_power";
                    root.cpuFreqGhz = p[7] || "0.8 GHz";
                    root.cpuTemp = p[8] || "40°C";

                    var watts = (pw / 1000000.0).toFixed(1);
                    if (pw > 100000 && root.batteryStatus === "Discharging") {
                        var hours = en / pw;
                        var h = Math.floor(hours);
                        var m = Math.round((hours - h) * 60);
                        root.batteryDetail = watts + " W  •  " + h + "h " + m + "m remaining";
                    } else if (pw > 100000 && root.batteryStatus === "Charging" && ef > en) {
                        var chHours = (ef - en) / pw;
                        var chH = Math.floor(chHours);
                        var chM = Math.round((chHours - chH) * 60);
                        root.batteryDetail = watts + " W  •  " + chH + "h " + chM + "m until full";
                    } else {
                        root.batteryDetail = root.batteryStatus;
                    }
                }
            }
        }
    }

    Process {
        id: powerProfileSetProc
        onExited: root.refreshBattery()
    }

    Timer {
        interval: 5000
        running: true
        repeat: true
        onTriggered: root.refreshBattery()
    }

    // ==================== WI-FI (NMCLI) ====================
    property bool wifiEnabled: true
    property string wifiCurrentSsid: ""
    property int wifiCurrentSignal: 0
    property var wifiNetworks: []
    property bool wifiScanning: false
    property bool wifiConnecting: false
    property string wifiStatusMsg: ""

    function refreshWifi() {
        wifiListProc.running = true;
    }

    function rescanWifi() {
        if (root.wifiScanning) return;
        root.wifiScanning = true;
        root.wifiStatusMsg = "Scanning networks...";
        wifiRescanProc.running = true;
    }

    function toggleWifi() {
        var nextState = root.wifiEnabled ? "off" : "on";
        root.wifiEnabled = !root.wifiEnabled;
        root.wifiStatusMsg = "";
        wifiToggleProc.command = ["nmcli", "radio", "wifi", nextState];
        wifiToggleProc.running = true;
    }

    function connectWifi(ssid, password, isSaved) {
        root.wifiConnecting = true;
        root.wifiStatusMsg = "Connecting to " + ssid + "...";
        if (password && password.length > 0) {
            // Delete stale profile if password was re-entered, then connect fresh
            wifiConnectProc.command = ["nmcli", "device", "wifi", "connect", ssid, "password", password];
        } else if (isSaved) {
            wifiConnectProc.command = ["nmcli", "connection", "up", "id", ssid];
        } else {
            wifiConnectProc.command = ["nmcli", "device", "wifi", "connect", ssid];
        }
        wifiConnectProc.running = true;
    }

    function disconnectWifi(ssid) {
        root.wifiStatusMsg = "Disconnecting...";
        wifiDisconnectProc.command = ["nmcli", "connection", "down", "id", ssid];
        wifiDisconnectProc.running = true;
    }

    Process {
        id: wifiListProc
        command: [
            "python3", "-c",
            "import subprocess\n" +
            "try:\n" +
            "  r = subprocess.check_output(['nmcli', 'radio', 'wifi'], text=True).strip()\n" +
            "  print('RADIO|' + r)\n" +
            "  if r == 'enabled':\n" +
            "    saved = set()\n" +
            "    for line in subprocess.check_output(['nmcli', '-t', '-f', 'NAME,TYPE', 'connection', 'show'], text=True).splitlines():\n" +
            "      if ':802-11-wireless' in line:\n" +
            "        saved.add(line.rsplit(':', 1)[0])\n" +
            "    out = subprocess.check_output(['nmcli', '-t', '-f', 'ACTIVE,SSID,SIGNAL,SECURITY', 'device', 'wifi', 'list'], text=True)\n" +
            "    for line in out.splitlines():\n" +
            "      p = line.split(':')\n" +
            "      if len(p) < 4: continue\n" +
            "      act = '1' if p[0] == 'yes' else '0'\n" +
            "      sec = p[-1].strip()\n" +
            "      sig = p[-2].strip()\n" +
            "      ssid = ':'.join(p[1:-2]).replace('\\\\:', ':').strip()\n" +
            "      if not ssid: continue\n" +
            "      sv = '1' if ssid in saved else '0'\n" +
            "      print(f'NET|{act}|{sv}|{sig}|{sec}|{ssid}')\n" +
            "except Exception:\n" +
            "  pass\n"
        ]
        running: true
        property var buffer: []
        onStarted: buffer = []
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (line.startsWith("RADIO|")) {
                    root.wifiEnabled = (line.slice(6).trim() === "enabled");
                } else if (line.startsWith("NET|")) {
                    var parts = line.split("|");
                    if (parts.length >= 6) {
                        wifiListProc.buffer.push({
                            active: parts[1] === "1",
                            saved: parts[2] === "1",
                            signal: parseInt(parts[3]) || 0,
                            security: parts[4],
                            ssid: parts.slice(5).join("|")
                        });
                    }
                }
            }
        }
        onExited: {
            var map = {};
            var activeSsid = "";
            var activeSig = 0;
            for (var i = 0; i < wifiListProc.buffer.length; i++) {
                var item = wifiListProc.buffer[i];
                if (item.active) {
                    activeSsid = item.ssid;
                    activeSig = item.signal;
                }
                if (!map[item.ssid] || item.active || item.signal > map[item.ssid].signal) {
                    map[item.ssid] = item;
                }
            }
            var list = [];
            for (var k in map) list.push(map[k]);
            list.sort((a, b) => {
                if (a.active !== b.active) return a.active ? -1 : 1;
                if (a.saved !== b.saved) return a.saved ? -1 : 1;
                return b.signal - a.signal;
            });
            root.wifiCurrentSsid = activeSsid;
            root.wifiCurrentSignal = activeSig;
            var newKey = list.map(x => x.ssid + "|" + x.active + "|" + x.saved + "|" + x.security + "|" + Math.round(x.signal / 10)).join("\n");
            var oldKey = root.wifiNetworks.map(x => x.ssid + "|" + x.active + "|" + x.saved + "|" + x.security + "|" + Math.round(x.signal / 10)).join("\n");
            if (newKey !== oldKey || root.wifiNetworks.length === 0) {
                root.wifiNetworks = list;
            }
        }
    }

    Process {
        id: wifiRescanProc
        command: ["nmcli", "device", "wifi", "rescan"]
        onExited: {
            root.wifiScanning = false;
            root.wifiStatusMsg = "";
            root.refreshWifi();
        }
    }

    Process {
        id: wifiToggleProc
        onExited: wifiToggleDelay.restart()
    }

    Timer {
        id: wifiToggleDelay
        interval: 800
        onTriggered: root.refreshWifi()
    }

    Process {
        id: wifiConnectProc
        onExited: (code) => {
            root.wifiConnecting = false;
            if (code === 0) {
                root.wifiStatusMsg = "Connected";
                statusClearTimer.restart();
            } else {
                root.wifiStatusMsg = "Authentication failed — check password";
            }
            root.refreshWifi();
        }
    }

    Process {
        id: wifiDisconnectProc
        onExited: {
            root.wifiStatusMsg = "";
            root.refreshWifi();
        }
    }

    Timer {
        id: statusClearTimer
        interval: 2500
        onTriggered: root.wifiStatusMsg = ""
    }

    Timer {
        interval: 8000
        running: true
        repeat: true
        onTriggered: {
            if (!root.wifiConnecting) root.refreshWifi();
        }
    }

    // ==================== BLUETOOTH ====================
    property bool btPowered: false
    property bool btScanning: false
    property string btStatusMsg: ""
    property var btPairedDevices: []
    property var btAvailableDevices: []
    property int btConnectedCount: 0

    function refreshBluetooth() {
        btStateProc.running = true;
    }

    function toggleBluetooth() {
        var next = root.btPowered ? "off" : "on";
        root.btPowered = !root.btPowered;
        root.btStatusMsg = "";
        btPowerProc.command = ["bluetoothctl", "power", next];
        btPowerProc.running = true;
    }

    function scanBluetooth() {
        if (root.btScanning) {
            Quickshell.execDetached(["pkill", "-f", "bluetoothctl.*scan"]);
            root.btScanning = false;
            root.btStatusMsg = "";
            return;
        }
        root.btScanning = true;
        root.btStatusMsg = "Searching for nearby devices...";
        btScanProc.running = true;
    }

    function connectBt(mac, name) {
        root.btStatusMsg = "Connecting to " + (name || mac) + "...";
        btActionProc.command = ["bluetoothctl", "connect", mac];
        btActionProc.running = true;
    }

    function pairAndConnectBt(mac, name) {
        root.btStatusMsg = "Pairing with " + (name || mac) + "...";
        btActionProc.command = [
            "sh", "-c",
            "bluetoothctl pairable on >/dev/null 2>&1; " +
            "bluetoothctl pair '" + mac + "' >/dev/null 2>&1 || true; " +
            "bluetoothctl trust '" + mac + "' >/dev/null 2>&1 || true; " +
            "bluetoothctl connect '" + mac + "'"
        ];
        btActionProc.running = true;
    }

    function disconnectBt(mac) {
        root.btStatusMsg = "Disconnecting...";
        btActionProc.command = ["bluetoothctl", "disconnect", mac];
        btActionProc.running = true;
    }

    function removeBt(mac) {
        root.btStatusMsg = "Removing device...";
        btActionProc.command = ["bluetoothctl", "remove", mac];
        btActionProc.running = true;
    }

    Process {
        id: btStateProc
        command: [
            "python3", "-c",
            "import subprocess\n" +
            "try:\n" +
            "  show = subprocess.check_output(['bluetoothctl', 'show'], text=True)\n" +
            "  pw = 'yes' if 'Powered: yes' in show else 'no'\n" +
            "  print('POWER:' + pw)\n" +
            "  if pw == 'yes':\n" +
            "    paired_set = set()\n" +
            "    for line in subprocess.check_output(['bluetoothctl', 'devices', 'Paired'], text=True).splitlines():\n" +
            "      p = line.split(' ', 2)\n" +
            "      if len(p) >= 2: paired_set.add(p[1])\n" +
            "    for line in subprocess.check_output(['bluetoothctl', 'devices'], text=True).splitlines():\n" +
            "      p = line.split(' ', 2)\n" +
            "      if len(p) < 2: continue\n" +
            "      mac = p[1].strip()\n" +
            "      name = p[2].strip() if len(p) >= 3 else mac\n" +
            "      info = subprocess.check_output(['bluetoothctl', 'info', mac], text=True)\n" +
            "      conn = '1' if 'Connected: yes' in info else '0'\n" +
            "      prd = '1' if (mac in paired_set or 'Paired: yes' in info) else '0'\n" +
            "      print(f'DEV|{mac}|{conn}|{prd}|{name}')\n" +
            "except Exception:\n" +
            "  pass\n"
        ]
        running: true
        property var pairedList: []
        property var availList: []
        property int connCount: 0
        onStarted: {
            pairedList = [];
            availList = [];
            connCount = 0;
        }
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (line.startsWith("POWER:")) {
                    root.btPowered = (line.slice(6).trim() === "yes");
                } else if (line.startsWith("DEV|")) {
                    var parts = line.split("|");
                    if (parts.length >= 5) {
                        var mac = parts[1];
                        var connected = (parts[2] === "1");
                        var paired = (parts[3] === "1");
                        var name = parts.slice(4).join("|") || mac;
                        if (connected) btStateProc.connCount++;
                        var item = {
                            mac: mac,
                            connected: connected,
                            paired: paired,
                            name: name
                        };
                        if (paired) {
                            btStateProc.pairedList.push(item);
                        } else {
                            btStateProc.availList.push(item);
                        }
                    }
                }
            }
        }
        onExited: {
            btStateProc.pairedList.sort((a, b) => (a.connected === b.connected ? 0 : (a.connected ? -1 : 1)));
            // Put named devices before raw MAC-only devices in available list
            btStateProc.availList.sort((a, b) => {
                var aRaw = /^[0-9A-F]{2}(-[0-9A-F]{2}){5}$/i.test(a.name);
                var bRaw = /^[0-9A-F]{2}(-[0-9A-F]{2}){5}$/i.test(b.name);
                if (aRaw !== bRaw) return aRaw ? 1 : -1;
                return a.name.localeCompare(b.name);
            });
            if (JSON.stringify(root.btPairedDevices) !== JSON.stringify(btStateProc.pairedList)) {
                root.btPairedDevices = btStateProc.pairedList;
            }
            if (JSON.stringify(root.btAvailableDevices) !== JSON.stringify(btStateProc.availList)) {
                root.btAvailableDevices = btStateProc.availList;
            }
            root.btConnectedCount = btStateProc.connCount;
        }
    }

    Process {
        id: btPowerProc
        onExited: root.refreshBluetooth()
    }

    Process {
        id: btScanProc
        command: ["sh", "-c", "bluetoothctl power on >/dev/null 2>&1; bluetoothctl pairable on >/dev/null 2>&1; bluetoothctl --timeout 15 scan on >/dev/null 2>&1 || true"]
        onExited: {
            root.btScanning = false;
            root.btStatusMsg = "";
            root.refreshBluetooth();
        }
    }

    Timer {
        interval: 2000
        running: root.btScanning
        repeat: true
        onTriggered: root.refreshBluetooth()
    }

    Process {
        id: btActionProc
        onExited: (code) => {
            root.btStatusMsg = code === 0 ? "" : "Operation failed — make sure device is in pairing mode";
            btMsgClearTimer.restart();
            root.refreshBluetooth();
        }
    }

    Timer {
        id: btMsgClearTimer
        interval: 3000
        onTriggered: {
            if (!root.btScanning) root.btStatusMsg = "";
        }
    }

    Timer {
        interval: 10000
        running: true
        repeat: true
        onTriggered: root.refreshBluetooth()
    }

    // ==================== SETTINGS / WALLPAPER / LOCK ====================
    property string wallpaperPath: "/home/huyle/.config/mango/wallpaper.jpg"
    property string wallpaperMode: "fill"
    property string wallpaperDir: "/home/huyle/Pictures"
    property string barPosition: "top"
    property int barSize: 34
    property real barOpacity: 0.95
    property real popupOpacity: 0.98
    property bool barFloating: false
    property int cornerRadius: 0
    property string themeName: "nord"
    property string accentPreset: "theme"
    readonly property var themePresets: ({
        "nord": {
            label: "Nord",
            bg: "#2e3440", surface: "#323846", surfaceHover: "#3e4658", surfaceActive: "#4c566a",
            border: "#434c5e", text: "#c6d0f5", textBright: "#eceff4", muted: "#9099ab",
            dim: "#4c566a", accent: "#c6d0f5", accentText: "#2e3440",
            red: "#bf616a", orange: "#d08770", yellow: "#ebcb8b", green: "#a3be8c", purple: "#b48ead", cyan: "#88c0d0", blue: "#81a1c1"
        },
        "catppuccin": {
            label: "Catppuccin",
            bg: "#1e1e2e", surface: "#262638", surfaceHover: "#36384f", surfaceActive: "#45475a",
            border: "#45475a", text: "#cdd6f4", textBright: "#f5e0dc", muted: "#a6adc8",
            dim: "#585b70", accent: "#cba6f7", accentText: "#1e1e2e",
            red: "#f38ba8", orange: "#fab387", yellow: "#f9e2af", green: "#a6e3a1", purple: "#cba6f7", cyan: "#89dceb", blue: "#89b4fa"
        },
        "gruvbox": {
            label: "Gruvbox",
            bg: "#282828", surface: "#32302f", surfaceHover: "#423e3c", surfaceActive: "#504945",
            border: "#504945", text: "#ebdbb2", textBright: "#fbf1c7", muted: "#a89984",
            dim: "#665c54", accent: "#fabd2f", accentText: "#282828",
            red: "#fb4934", orange: "#fe8019", yellow: "#fabd2f", green: "#b8bb26", purple: "#d3869b", cyan: "#8ec07c", blue: "#83a598"
        },
        "tokyonight": {
            label: "Tokyo Night",
            bg: "#1a1b26", surface: "#222536", surfaceHover: "#2f344d", surfaceActive: "#414868",
            border: "#333952", text: "#c0caf5", textBright: "#e0e7ff", muted: "#8897d6",
            dim: "#414868", accent: "#7aa2f7", accentText: "#1a1b26",
            red: "#f7768e", orange: "#ff9e64", yellow: "#e0af68", green: "#9ece6a", purple: "#bb9af7", cyan: "#7dcfff", blue: "#7aa2f7"
        },
        "dracula": {
            label: "Dracula",
            bg: "#282a36", surface: "#313442", surfaceHover: "#414457", surfaceActive: "#535770",
            border: "#44475a", text: "#f8f8f2", textBright: "#ffffff", muted: "#9ea7cc",
            dim: "#6272a4", accent: "#bd93f9", accentText: "#282a36",
            red: "#ff5555", orange: "#ffb86c", yellow: "#f1fa8c", green: "#50fa7b", purple: "#bd93f9", cyan: "#8be9fd", blue: "#6272a4"
        },
        "rosepine": {
            label: "Rosé Pine",
            bg: "#191724", surface: "#211f30", surfaceHover: "#2d2a42", surfaceActive: "#403d52",
            border: "#403d52", text: "#e0def4", textBright: "#faf4ed", muted: "#908caa",
            dim: "#6e6a86", accent: "#ebbcba", accentText: "#191724",
            red: "#eb6f92", orange: "#f6c177", yellow: "#f6c177", green: "#9ccfd8", purple: "#c4a7e7", cyan: "#9ccfd8", blue: "#31748f"
        },
        "everforest": {
            label: "Everforest",
            bg: "#2d353b", surface: "#343f44", surfaceHover: "#414b50", surfaceActive: "#4f585e",
            border: "#475258", text: "#d3c6aa", textBright: "#ede4ce", muted: "#859289",
            dim: "#56635f", accent: "#a7c080", accentText: "#2d353b",
            red: "#e67e80", orange: "#e69875", yellow: "#dbbc7f", green: "#a7c080", purple: "#d699b6", cyan: "#83c092", blue: "#7fbbb3"
        }
    })
    readonly property var currentTheme: themePresets[themeName] || themePresets["nord"]
    readonly property var accentPalette: ({
        "theme":    currentTheme.accent,
        "lavender": "#c6d0f5",
        "frost":    "#88c0d0",
        "blue":     "#81a1c1",
        "sage":     "#a3be8c",
        "amber":    "#ebcb8b",
        "rose":     "#bf616a",
        "snow":     "#eceff4"
    })
    readonly property color accentColor: accentPalette[accentPreset] || currentTheme.accent
    onThemeNameChanged: if (settingsReady) systemThemeSyncTimer.restart()
    onAccentPresetChanged: if (settingsReady) systemThemeSyncTimer.restart()

    Timer {
        id: systemThemeSyncTimer
        interval: 60
        repeat: false
        onTriggered: root.applySystemThemeNow()
    }

    function applySystemThemeNow() {
        var t = root.currentTheme;
        var bgCol = Qt.color(t.bg);
        var deepCol = Qt.darker(bgCol, 1.18);
        function toHex(c) {
            return String(c).slice(0, 7);
        }
        var payload = JSON.stringify({
            themeName: root.themeName,
            label: t.label,
            bg: t.bg,
            bgDeep: toHex(deepCol),
            surface: t.surface,
            surfaceHover: t.surfaceHover,
            surfaceActive: t.surfaceActive,
            border: t.border,
            text: t.text,
            textBright: t.textBright,
            muted: t.muted,
            dim: t.dim,
            accent: toHex(root.accentColor),
            accentText: t.accentText,
            red: t.red,
            orange: t.orange,
            yellow: t.yellow,
            green: t.green,
            purple: t.purple,
            cyan: t.cyan,
            blue: t.blue || t.cyan
        });
        Quickshell.execDetached([
            "python3",
            "/home/huyle/.config/quickshell/scripts/apply-system-theme.py",
            payload
        ]);
    }
    property bool showRamOnBar: true
    property bool showTrayOnBar: true
    property bool showMediaOnBar: true
    property bool lockUseSameWallpaper: true
    property string lockWallpaperPath: "/home/huyle/.config/mango/wallpaper.jpg"
    property bool lockShowClock: true
    property bool lock24Hour: true
    property real lockDimOpacity: 0.45
    property bool screenLocked: false
    property string pickTarget: "desktop"
    property string sddmTheme: "minimal"
    property string sddmCurrentBg: "/usr/share/sddm/themes/minimal/background.jpg"
    property string sddmSelectedPath: ""
    property int sddmBgRevision: 0
    property bool sddmApplying: false
    property string sddmStatus: ""
    property bool sddmStatusError: false
    property bool settingsReady: false
    property var fileBookmarks: []

    FileView {
        id: settingsFile
        blockLoading: true
        printErrors: false
    }

    function applySettingsJson(text) {
        if (!text || text.trim().length === 0) return;
        try {
            var d = JSON.parse(text);
            if (d.wallpaperPath) root.wallpaperPath = d.wallpaperPath;
            if (d.wallpaperMode) root.wallpaperMode = d.wallpaperMode;
            if (d.wallpaperDir) root.wallpaperDir = d.wallpaperDir;
            if (d.barPosition === "bottom" || d.barPosition === "top" || d.barPosition === "left" || d.barPosition === "right") root.barPosition = d.barPosition;
            if (d.barSize !== undefined) root.barSize = Math.max(28, Math.min(52, Math.round(Number(d.barSize) || 34)));
            if (d.barOpacity !== undefined) root.barOpacity = Math.max(0.35, Math.min(1.0, Number(d.barOpacity)));
            if (d.popupOpacity !== undefined) root.popupOpacity = Math.max(0.55, Math.min(1.0, Number(d.popupOpacity)));
            if (d.barFloating !== undefined) root.barFloating = !!d.barFloating;
            if (d.cornerRadius !== undefined) root.cornerRadius = Math.max(0, Math.min(24, Math.round(Number(d.cornerRadius) || 0)));
            if (d.themeName && root.themePresets[d.themeName]) root.themeName = d.themeName;
            if (d.accentPreset && root.accentPalette[d.accentPreset]) root.accentPreset = d.accentPreset;
            if (d.showRamOnBar !== undefined) root.showRamOnBar = !!d.showRamOnBar;
            if (d.showTrayOnBar !== undefined) root.showTrayOnBar = !!d.showTrayOnBar;
            if (d.showMediaOnBar !== undefined) root.showMediaOnBar = !!d.showMediaOnBar;
            if (d.lockUseSameWallpaper !== undefined) root.lockUseSameWallpaper = d.lockUseSameWallpaper;
            if (d.lockWallpaperPath) root.lockWallpaperPath = d.lockWallpaperPath;
            if (d.lockShowClock !== undefined) root.lockShowClock = d.lockShowClock;
            if (d.lock24Hour !== undefined) root.lock24Hour = d.lock24Hour;
            if (d.lockDimOpacity !== undefined) root.lockDimOpacity = d.lockDimOpacity;
            if (Array.isArray(d.fileBookmarks)) root.fileBookmarks = d.fileBookmarks;
        } catch (e) {}
    }

    Component.onCompleted: {
        settingsFile.path = "/home/huyle/.config/quickshell/settings.json";
        applySettingsJson(settingsFile.text());
        root.settingsReady = true;
        refreshMonitors();
        loadMangoConfig();
        applySystemThemeNow();
    }

    Process {
        id: sddmDetectProc
        command: [
            "/bin/bash", "-c",
            "t=minimal; for f in /etc/sddm.conf /etc/sddm.conf.d/*.conf; do [ -r \"$f\" ] && v=$(awk -F= '/^[[:space:]]*Current[[:space:]]*=/{gsub(/^[[:space:]]+|[[:space:]]+$/,\"\",$2);print $2}' \"$f\" | tail -n1); [ -n \"$v\" ] && t=\"$v\"; done; printf '%s' \"$t\""
        ]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                var t = text.trim();
                if (t.length > 0) {
                    root.sddmTheme = t;
                    root.sddmCurrentBg = "/usr/share/sddm/themes/" + t + "/background.jpg";
                }
            }
        }
    }

    function saveSettings() {
        var payload = JSON.stringify({
            wallpaperPath: root.wallpaperPath,
            wallpaperMode: root.wallpaperMode,
            wallpaperDir: root.wallpaperDir,
            barPosition: root.barPosition,
            barSize: root.barSize,
            barOpacity: root.barOpacity,
            popupOpacity: root.popupOpacity,
            barFloating: root.barFloating,
            cornerRadius: root.cornerRadius,
            themeName: root.themeName,
            accentPreset: root.accentPreset,
            showRamOnBar: root.showRamOnBar,
            showTrayOnBar: root.showTrayOnBar,
            showMediaOnBar: root.showMediaOnBar,
            lockUseSameWallpaper: root.lockUseSameWallpaper,
            lockWallpaperPath: root.lockWallpaperPath,
            lockShowClock: root.lockShowClock,
            lock24Hour: root.lock24Hour,
            lockDimOpacity: root.lockDimOpacity,
            fileBookmarks: root.fileBookmarks
        }, null, 2);
        Quickshell.execDetached([
            "python3", "-c",
            "import sys; open('/home/huyle/.config/quickshell/settings.json', 'w').write(sys.argv[1])",
            payload
        ]);
    }

    function normalizeBookmarkDir(dirPath) {
        if (!dirPath || dirPath.length === 0) return "";
        return (dirPath.length > 1 && dirPath.endsWith("/")) ? dirPath.slice(0, -1) : dirPath;
    }

    function hasFileBookmark(dirPath) {
        var clean = normalizeBookmarkDir(dirPath);
        for (var i = 0; i < root.fileBookmarks.length; i++) {
            if (root.fileBookmarks[i].dir === clean) return true;
        }
        return false;
    }

    function addFileBookmark(dirPath, label) {
        var clean = normalizeBookmarkDir(dirPath);
        if (!clean || hasFileBookmark(clean)) return false;
        var name = (label && label.length > 0) ? label : (clean === "/" ? "Root /" : clean.split("/").pop());
        var list = root.fileBookmarks.slice();
        list.push({ label: name, dir: clean, icon: "󰉋" });
        root.fileBookmarks = list;
        root.saveSettings();
        return true;
    }

    function removeFileBookmark(dirPath) {
        var clean = normalizeBookmarkDir(dirPath);
        var list = [];
        var removed = false;
        for (var i = 0; i < root.fileBookmarks.length; i++) {
            if (root.fileBookmarks[i].dir === clean) {
                removed = true;
            } else {
                list.push(root.fileBookmarks[i]);
            }
        }
        if (removed) {
            root.fileBookmarks = list;
            root.saveSettings();
        }
        return removed;
    }

    signal wallpaperTransitionRequested(string prevPath, string prevMode, string nextPath, string nextMode)

    function commitSwaybg(path, mode) {
        Quickshell.execDetached(["/bin/bash", "-c", "pkill -x swaybg 2>/dev/null || true; setsid -f swaybg -i \"$1\" -m \"$2\" >/dev/null 2>&1", "_", path || root.wallpaperPath, mode || root.wallpaperMode]);
    }

    function applyWallpaper(path, mode) {
        var prevPath = root.wallpaperPath;
        var prevMode = root.wallpaperMode;
        if (path && path.length > 0) root.wallpaperPath = path;
        if (mode && mode.length > 0) root.wallpaperMode = mode;
        root.saveSettings();
        root.wallpaperTransitionRequested(prevPath, prevMode, root.wallpaperPath, root.wallpaperMode);
    }

    function applyRandomWallpaper() {
        randomWallProc.command = [
            "python3", "-c",
            "import os, random, sys\n" +
            "d, cur = sys.argv[1], sys.argv[2]\n" +
            "exts = {'.png', '.jpg', '.jpeg', '.webp', '.bmp'}\n" +
            "if not os.path.isdir(d): sys.exit(0)\n" +
            "files = [os.path.join(d, f) for f in sorted(os.listdir(d)) if os.path.splitext(f)[1].lower() in exts and os.path.isfile(os.path.join(d, f))]\n" +
            "pool = [f for f in files if f != cur] or files\n" +
            "if pool: print(random.choice(pool))\n",
            root.wallpaperDir,
            root.wallpaperPath
        ];
        randomWallProc.running = false;
        randomWallProc.running = true;
    }

    Process {
        id: randomWallProc
        stdout: StdioCollector {
            onStreamFinished: {
                var p = text.trim();
                if (p.length > 0) root.applyWallpaper(p, root.wallpaperMode);
            }
        }
    }

    // ==================== REALTIME SYSTEM TELEMETRY ====================
    property bool sysMonitorActive: false
    property string sysHostname: "arch"
    property string sysOsPretty: "Arch Linux"
    property string sysKernel: ""
    property string sysCpuModel: ""
    property int sysCpuCores: 0
    property int sysCpuPercent: 0
    property string sysCpuTemp: "--"
    property string sysCpuFreqGhz: "--"
    property string sysLoadAvg: "0.00  0.00  0.00"
    property string sysUptime: "--"
    property int sysProcessCount: 0
    property real sysSwapUsedGb: 0
    property real sysSwapTotalGb: 0
    property real sysDiskUsedGb: 0
    property real sysDiskTotalGb: 0
    property int sysDiskPercent: 0
    property real sysNetDownKbps: 0
    property real sysNetUpKbps: 0
    property var sysCpuHistory: []
    property var sysRamHistory: []

    property real _prevCpuTotal: 0
    property real _prevCpuIdle: 0
    property real _prevNetRx: 0
    property real _prevNetTx: 0
    property real _prevNetTs: 0

    function refreshSystemStats() {
        if (!sysStatsProc.running) sysStatsProc.running = true;
    }

    Process {
        id: sysStaticProc
        command: [
            "python3", "-c",
            "import os, platform\n" +
            "host = platform.node() or 'arch'\n" +
            "kern = platform.release()\n" +
            "osn = 'Linux'\n" +
            "try:\n" +
            "  for line in open('/etc/os-release'):\n" +
            "    if line.startswith('PRETTY_NAME='):\n" +
            "      osn = line.split('=', 1)[1].strip().strip('\"')\n" +
            "      break\n" +
            "except Exception: pass\n" +
            "model, cores = '', os.cpu_count() or 0\n" +
            "try:\n" +
            "  for line in open('/proc/cpuinfo'):\n" +
            "    if line.startswith('model name'):\n" +
            "      model = line.split(':', 1)[1].strip()\n" +
            "      break\n" +
            "except Exception: pass\n" +
            "print(f'{host}|{osn}|{kern}|{model}|{cores}')\n"
        ]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                var p = text.trim().split("|");
                if (p.length >= 5) {
                    root.sysHostname = p[0] || root.sysHostname;
                    root.sysOsPretty = p[1] || root.sysOsPretty;
                    root.sysKernel = p[2] || "";
                    root.sysCpuModel = (p[3] || "").replace(/\(R\)|\(TM\)|CPU|@/g, "").replace(/\s+/g, " ").trim();
                    root.sysCpuCores = parseInt(p[4]) || 0;
                }
            }
        }
    }

    Process {
        id: sysStatsProc
        command: [
            "python3", "-c",
            "import glob, os, time\n" +
            "with open('/proc/stat') as f:\n" +
            "  cs = [int(x) for x in f.readline().split()[1:]]\n" +
            "idle = cs[3] + (cs[4] if len(cs) > 4 else 0)\n" +
            "tot = sum(cs)\n" +
            "mi = {}\n" +
            "for line in open('/proc/meminfo'):\n" +
            "  k, v = line.split(':', 1)\n" +
            "  mi[k] = int(v.strip().split()[0])\n" +
            "mt = mi.get('MemTotal', 1); ma = mi.get('MemAvailable', 0); mu = max(0, mt - ma)\n" +
            "st = mi.get('SwapTotal', 0); sf = mi.get('SwapFree', 0); su = max(0, st - sf)\n" +
            "freqs = []\n" +
            "for fp in glob.glob('/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq'):\n" +
            "  try: freqs.append(int(open(fp).read().strip()))\n" +
            "  except Exception: pass\n" +
            "fg = f'{(sum(freqs)/len(freqs)/1e6):.2f} GHz' if freqs else '--'\n" +
            "temps = []\n" +
            "for tz in glob.glob('/sys/class/thermal/thermal_zone*/temp'):\n" +
            "  try:\n" +
            "    v = int(open(tz).read().strip())\n" +
            "    if 10000 < v < 120000: temps.append(v / 1000.0)\n" +
            "  except Exception: pass\n" +
            "tp = f'{max(temps):.0f}°C' if temps else root_temp_fallback if False else '--'\n" +
            "la = ' '.join(open('/proc/loadavg').read().split()[:3])\n" +
            "up_s = int(float(open('/proc/uptime').read().split()[0]))\n" +
            "d, r = divmod(up_s, 86400); h, r = divmod(r, 3600); m = r // 60\n" +
            "ups = (f'{d}d ' if d else '') + f'{h}h {m}m'\n" +
            "sv = os.statvfs('/')\n" +
            "dtot = sv.f_blocks * sv.f_frsize; dfree = sv.f_bavail * sv.f_frsize; duse = max(0, dtot - dfree)\n" +
            "rx = tx = 0\n" +
            "for line in open('/proc/net/dev').readlines()[2:]:\n" +
            "  if ':' not in line: continue\n" +
            "  ifn, rest = line.split(':', 1)\n" +
            "  if ifn.strip() == 'lo': continue\n" +
            "  nums = rest.split()\n" +
            "  if len(nums) >= 10:\n" +
            "    rx += int(nums[0]); tx += int(nums[8])\n" +
            "procs = sum(1 for n in os.listdir('/proc') if n.isdigit())\n" +
            "print(f'SYS|{tot}|{idle}|{mu/1048576:.2f}|{mt/1048576:.2f}|{int(mu*100/mt)}|{su/1048576:.2f}|{st/1048576:.2f}|{fg}|{tp}|{la}|{ups}|{duse/1073741824:.1f}|{dtot/1073741824:.1f}|{int(duse*100/max(1,dtot))}|{rx}|{tx}|{time.time():.3f}|{procs}')\n"
        ]
        running: true
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (!line.startsWith("SYS|")) return;
                var p = line.split("|");
                if (p.length < 19) return;
                var tot = parseFloat(p[1]) || 0;
                var idle = parseFloat(p[2]) || 0;
                if (root._prevCpuTotal > 0 && tot > root._prevCpuTotal) {
                    var dt = tot - root._prevCpuTotal;
                    var di = idle - root._prevCpuIdle;
                    var pct = Math.round(Math.max(0, Math.min(100, (1.0 - di / dt) * 100.0)));
                    root.sysCpuPercent = pct;
                    var ch = root.sysCpuHistory.slice();
                    ch.push(pct);
                    if (ch.length > 24) ch = ch.slice(ch.length - 24);
                    root.sysCpuHistory = ch;
                }
                root._prevCpuTotal = tot;
                root._prevCpuIdle = idle;

                root.ramUsedGb = parseFloat(p[3]) || root.ramUsedGb;
                root.ramTotalGb = parseFloat(p[4]) || root.ramTotalGb;
                root.ramPercent = parseInt(p[5]) || root.ramPercent;
                var rh = root.sysRamHistory.slice();
                rh.push(root.ramPercent);
                if (rh.length > 24) rh = rh.slice(rh.length - 24);
                root.sysRamHistory = rh;

                root.sysSwapUsedGb = parseFloat(p[6]) || 0;
                root.sysSwapTotalGb = parseFloat(p[7]) || 0;
                root.sysCpuFreqGhz = p[8] || "--";
                root.sysCpuTemp = p[9] || "--";
                root.sysLoadAvg = p[10] || "";
                root.sysUptime = p[11] || "--";
                root.sysDiskUsedGb = parseFloat(p[12]) || 0;
                root.sysDiskTotalGb = parseFloat(p[13]) || 0;
                root.sysDiskPercent = parseInt(p[14]) || 0;

                var rx = parseFloat(p[15]) || 0;
                var tx = parseFloat(p[16]) || 0;
                var ts = parseFloat(p[17]) || 0;
                if (root._prevNetTs > 0 && ts > root._prevNetTs) {
                    var elapsed = ts - root._prevNetTs;
                    root.sysNetDownKbps = Math.max(0, (rx - root._prevNetRx) / 1024.0 / elapsed);
                    root.sysNetUpKbps = Math.max(0, (tx - root._prevNetTx) / 1024.0 / elapsed);
                }
                root._prevNetRx = rx;
                root._prevNetTx = tx;
                root._prevNetTs = ts;
                root.sysProcessCount = parseInt(p[18]) || 0;
            }
        }
    }

    Timer {
        interval: 1200
        running: root.sysMonitorActive
        repeat: true
        onTriggered: root.refreshSystemStats()
    }

    function setLockWallpaper(path) {
        if (path && path.length > 0) {
            root.lockWallpaperPath = path;
            root.lockUseSameWallpaper = false;
            root.saveSettings();
        }
    }

    Process {
        id: pickImageProc
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                var p = text.trim();
                if (p.length === 0) return;
                var idx = p.lastIndexOf("/");
                if (idx > 0) root.wallpaperDir = p.slice(0, idx);
                if (root.pickTarget === "lock") {
                    root.setLockWallpaper(p);
                } else if (root.pickTarget === "sddm") {
                    root.sddmSelectedPath = p;
                    root.sddmStatus = "";
                } else {
                    root.applyWallpaper(p, root.wallpaperMode);
                }
            }
        }
    }

    function pickImageFile(target) {
        root.pickTarget = target || "desktop";
        pickImageProc.command = ["/home/huyle/.local/bin/pick-wallpaper", root.wallpaperDir];
        pickImageProc.running = true;
    }

    Process {
        id: applySddmProc
        running: false
        stderr: StdioCollector {
            onStreamFinished: {
                root.sddmApplying = false;
            }
        }
        onExited: (exitCode, exitStatus) => {
            root.sddmApplying = false;
            if (exitCode === 0) {
                root.sddmSelectedPath = "";
                root.sddmBgRevision += 1;
                root.sddmStatus = "Applied to SDDM (" + root.sddmTheme + ")";
                root.sddmStatusError = false;
            } else if (exitCode === 126 || exitCode === 127) {
                root.sddmStatus = "Authentication cancelled";
                root.sddmStatusError = true;
            } else {
                root.sddmStatus = "Failed to update SDDM (exit " + exitCode + ")";
                root.sddmStatusError = true;
            }
        }
    }

    function applySddmWallpaper(path) {
        var target = (path && path.length > 0)
            ? path
            : (root.sddmSelectedPath.length > 0 ? root.sddmSelectedPath : root.wallpaperPath);
        if (!target || target.length === 0 || root.sddmApplying) return;
        root.sddmSelectedPath = target;
        root.sddmApplying = true;
        root.sddmStatus = "Waiting for authentication...";
        root.sddmStatusError = false;
        applySddmProc.command = ["/home/huyle/.local/bin/set-sddm-wallpaper", target];
        applySddmProc.running = true;
    }

    function lockScreen() {
        root.screenLocked = true;
    }

    function unlockScreen() {
        root.screenLocked = false;
    }

    function openLotusSettings() {
        Quickshell.execDetached(["fcitx5-lotus-settings"]);
    }

    function openTerminal() {
        Quickshell.execDetached(["kitty"]);
    }

    function openFiles() {
        Quickshell.execDetached(["/home/huyle/.local/bin/filesR"]);
    }

    function reloadMango() {
        Quickshell.execDetached(["mmsg", "dispatch", "reload_config"]);
    }

    // ==================== POWER ====================
    function powerAction(action) {
        if (action === "lock") {
            root.lockScreen();
        } else if (action === "suspend") {
            Quickshell.execDetached(["systemctl", "suspend"]);
        } else if (action === "logout") {
            Quickshell.execDetached(["mmsg", "dispatch", "quit"]);
        } else if (action === "reboot") {
            Quickshell.execDetached(["systemctl", "reboot"]);
        } else if (action === "shutdown") {
            Quickshell.execDetached(["systemctl", "poweroff"]);
        }
    }

    // ==================== NOTIFICATIONS ====================
    property bool dndEnabled: false
    property int unreadNotifCount: 0
    property int notifSeq: 0
    property var notifHistory: []
    property var popupNotifs: []

    function stripNotifMarkup(str) {
        if (!str) return "";
        return str
            .replace(/<br\s*\/?>/gi, "\n")
            .replace(/<[^>]+>/g, "")
            .replace(/&amp;/g, "&")
            .replace(/&lt;/g, "<")
            .replace(/&gt;/g, ">")
            .replace(/&quot;/g, "\"")
            .trim();
    }

    function validateIconCandidate(val) {
        if (!val || val.length === 0) return "";
        if (val.startsWith("image://icon/")) {
            var iconName = val.slice("image://icon/".length).split("?")[0];
            return Quickshell.iconPath(iconName, true);
        }
        if (val.startsWith("/") || val.startsWith("file://") || val.startsWith("image://")) {
            return val;
        }
        return Quickshell.iconPath(val, true);
    }

    function resolveNotifIcon(notif) {
        if (!notif) return "";
        var imgValid = validateIconCandidate(notif.image);
        if (imgValid.length > 0) return imgValid;

        var appValid = validateIconCandidate(notif.appIcon);
        if (appValid.length > 0) return appValid;

        if (notif.desktopEntry && notif.desktopEntry.length > 0) {
            var de = DesktopEntries.byId(notif.desktopEntry);
            if (de && de.icon) {
                var byDe = validateIconCandidate(de.icon);
                if (byDe.length > 0) return byDe;
            }
        }
        if (notif.appIcon && notif.appIcon.length > 0) {
            var byIconLookup = DesktopEntries.heuristicLookup(notif.appIcon);
            if (byIconLookup && byIconLookup.icon) {
                var validIconLookup = validateIconCandidate(byIconLookup.icon);
                if (validIconLookup.length > 0) return validIconLookup;
            }
        }
        if (notif.appName && notif.appName.length > 0) {
            var guessed = DesktopEntries.heuristicLookup(notif.appName);
            if (guessed && guessed.icon) {
                var byGuess = validateIconCandidate(guessed.icon);
                if (byGuess.length > 0) return byGuess;
            }
            var targetName = notif.appName.toLowerCase();
            var targetIcon = (notif.appIcon || "").toLowerCase();
            var apps = DesktopEntries.applications.values || [];
            for (var i = 0; i < apps.length; i++) {
                var e = apps[i];
                if (!e || !e.icon) continue;
                var n = (e.name || "").toLowerCase();
                var d = (e.id || "").toLowerCase();
                if (n === targetName || (targetIcon.length > 0 && d.indexOf(targetIcon) >= 0)) {
                    var matched = validateIconCandidate(e.icon);
                    if (matched.length > 0) return matched;
                }
            }
        }
        return "";
    }

    function dismissPopupToast(uid) {
        root.popupNotifs = root.popupNotifs.filter(function(item) {
            return item.uid !== uid;
        });
    }

    function clearPopupToasts() {
        root.popupNotifs = [];
    }

    function removeNotif(uid) {
        var targetObj = null;
        for (var i = 0; i < root.notifHistory.length; i++) {
            if (root.notifHistory[i].uid === uid) {
                targetObj = root.notifHistory[i].notifObj;
                break;
            }
        }
        root.popupNotifs = root.popupNotifs.filter(function(item) { return item.uid !== uid; });
        root.notifHistory = root.notifHistory.filter(function(item) { return item.uid !== uid; });
        if (targetObj) {
            try { targetObj.dismiss(); } catch (e) {}
        }
    }

    function clearAllNotifs() {
        var hist = root.notifHistory.slice();
        root.popupNotifs = [];
        root.notifHistory = [];
        root.unreadNotifCount = 0;
        for (var i = 0; i < hist.length; i++) {
            if (hist[i].notifObj) {
                try { hist[i].notifObj.dismiss(); } catch (e) {}
            }
        }
    }

    function markNotifsRead() {
        root.unreadNotifCount = 0;
    }

    function toggleDnd() {
        root.dndEnabled = !root.dndEnabled;
        if (root.dndEnabled) {
            root.popupNotifs = [];
        }
    }

    NotificationServer {
        id: notifServer
        keepOnReload: true
        persistenceSupported: true
        bodySupported: true
        bodyMarkupSupported: true
        actionsSupported: true
        actionIconsSupported: true
        imageSupported: true

        onNotification: notif => {
            notif.tracked = true;
            root.notifSeq += 1;
            var uid = root.notifSeq;
            var actionList = [];
            if (notif.actions) {
                for (var i = 0; i < notif.actions.length; i++) {
                    var act = notif.actions[i];
                    if (act && act.text && act.text.length > 0) {
                        actionList.push(act);
                    }
                }
            }
            var isCritical = (notif.urgency === NotificationUrgency.Critical);
            var timeoutMs = (notif.expireTimeout > 0)
                ? Math.min(Math.max(Math.round(notif.expireTimeout), 2500), 12000)
                : (isCritical ? 10000 : 5000);

            var entry = {
                uid: uid,
                notifId: notif.id,
                notifObj: notif,
                appName: (notif.appName && notif.appName.length > 0) ? notif.appName : "Notification",
                summary: notif.summary || "",
                body: root.stripNotifMarkup(notif.body || ""),
                iconSource: root.resolveNotifIcon(notif),
                isCritical: isCritical,
                timeStr: Qt.formatTime(new Date(), "HH:mm"),
                expireMs: timeoutMs,
                actions: actionList
            };

            notif.closed.connect(function() {
                for (var k = 0; k < root.notifHistory.length; k++) {
                    if (root.notifHistory[k].uid === uid) {
                        root.notifHistory[k].notifObj = null;
                        break;
                    }
                }
            });

            if (!notif.transient) {
                var filtered = root.notifHistory.filter(function(it) {
                    return !(it.notifId === notif.id && it.appName === entry.appName);
                });
                var nextHist = [entry].concat(filtered);
                if (nextHist.length > 50) nextHist = nextHist.slice(0, 50);
                root.notifHistory = nextHist;
                root.unreadNotifCount += 1;
            }

            if (!root.dndEnabled && !root.screenLocked) {
                var filteredToasts = root.popupNotifs.filter(function(it) {
                    return !(it.notifId === notif.id && it.appName === entry.appName);
                });
                var nextToasts = [entry].concat(filteredToasts);
                if (nextToasts.length > 4) nextToasts = nextToasts.slice(0, 4);
                root.popupNotifs = nextToasts;
            }
        }
    }

    // ==================== MEDIA (MPRIS) ====================
    property string preferredPlayerBus: ""

    readonly property var mediaPlayers: {
        var all = Mpris.players.values || [];
        var list = [];
        for (var i = 0; i < all.length; i++) {
            var p = all[i];
            if (!p) continue;
            var hasTitle = p.trackTitle && p.trackTitle.trim().length > 0;
            if (p.isPlaying || p.playbackState === MprisPlaybackState.Paused || hasTitle) {
                list.push(p);
            }
        }
        return list;
    }

    readonly property var activeMediaPlayer: {
        var list = root.mediaPlayers;
        if (list.length === 0) return null;
        if (root.preferredPlayerBus.length > 0) {
            for (var i = 0; i < list.length; i++) {
                if (list[i].dbusName === root.preferredPlayerBus) return list[i];
            }
        }
        for (var j = 0; j < list.length; j++) {
            if (list[j].isPlaying) return list[j];
        }
        return list[0];
    }

    function selectMediaPlayer(dbusName) {
        root.preferredPlayerBus = dbusName || "";
    }

    function mediaTogglePlay() {
        var p = root.activeMediaPlayer;
        if (!p) return;
        if (p.canTogglePlaying) {
            p.togglePlaying();
        } else if (p.isPlaying && p.canPause) {
            p.pause();
        } else if (!p.isPlaying && p.canPlay) {
            p.play();
        }
    }

    function mediaNext() {
        var p = root.activeMediaPlayer;
        if (p && p.canGoNext) p.next();
    }

    function mediaPrev() {
        var p = root.activeMediaPlayer;
        if (p && p.canGoPrevious) p.previous();
    }

    function mediaSourceInfo(player) {
        if (!player) return { label: "Media", icon: "󰎈" };
        var url = "";
        try {
            if (player.metadata && player.metadata["xesam:url"]) {
                url = String(player.metadata["xesam:url"]).toLowerCase();
            }
        } catch (e) {}
        var id = ((player.identity || "") + " " + (player.desktopEntry || "")).toLowerCase();
        if (url.indexOf("music.youtube") >= 0) return { label: "YT Music", icon: "󰗃" };
        if (url.indexOf("youtube.com") >= 0 || url.indexOf("youtu.be") >= 0) return { label: "YouTube", icon: "󰗃" };
        if (url.indexOf("soundcloud.com") >= 0) return { label: "SoundCloud", icon: "󰓀" };
        if (url.indexOf("spotify") >= 0 || id.indexOf("spotify") >= 0) return { label: "Spotify", icon: "󰓇" };
        if (id.indexOf("mpv") >= 0) return { label: "mpv", icon: "󰐹" };
        if (id.indexOf("vlc") >= 0) return { label: "VLC", icon: "󰕼" };
        if (player.identity && player.identity.length > 0) return { label: player.identity, icon: "󰎈" };
        return { label: "Media", icon: "󰎈" };
    }

    // ==================== ANTIGRAVITY MINI ====================
    readonly property var agyModels: [
        { id: "gemini-3.8-flash",  label: "Flash",    efforts: [
            { id: "low", label: "Low" }, { id: "medium", label: "Med" }, { id: "high", label: "High" }
        ] },
        { id: "gemini-4-argon",    label: "Argon",    efforts: [
            { id: "low", label: "Low" }, { id: "medium", label: "Med" }, { id: "high", label: "High" }
        ] },
        { id: "claude-sonnet-5-5", label: "Sonnet",   efforts: [
            { id: "low", label: "Low" }, { id: "medium", label: "Med" }, { id: "high", label: "High" }
        ] },
        { id: "gemini-4-hydrogen", label: "Hydrogen", efforts: [
            { id: "low", label: "Low" }, { id: "medium", label: "Med" }, { id: "high", label: "High" }, { id: "xhigh", label: "XHigh" }
        ] },
        { id: "claude-opus-5-5",   label: "Opus",     efforts: [
            { id: "low", label: "Low" }, { id: "medium", label: "Med" }, { id: "high", label: "High" }
        ] },
        { id: "gpt-oss-120b",      label: "GPT-OSS",  efforts: [
            { id: "medium", label: "Med" }
        ] }
    ]
    property int agyModelIdx: 0
    readonly property string agyModelId: agyModels[agyModelIdx].id
    readonly property string agyModelLabel: agyModels[agyModelIdx].label

    readonly property var agyEfforts: agyModels[agyModelIdx].efforts
    property int agyEffortIdx: 1
    readonly property int agySafeEffortIdx: Math.max(0, Math.min(agyEffortIdx, agyEfforts.length - 1))
    readonly property string agyEffortId: agyEfforts[agySafeEffortIdx].id
    readonly property string agyEffortLabel: agyEfforts[agySafeEffortIdx].label

    property string agyConversationId: ""
    property bool agyBusy: false
    property string agyStatus: ""
    property string agyLiveText: ""
    property string agyLastMeta: ""
    property int agySeq: 0
    property var agyMessages: []
    property int agyActiveMsgIdx: -1
    property string agyErrBuf: ""

    property var agyHistory: []
    property bool agyHistoryLoading: false

    function agyCycleModel() {
        var curEffort = root.agyEffortId;
        var nextIdx = (root.agyModelIdx + 1) % root.agyModels.length;
        var nextEfforts = root.agyModels[nextIdx].efforts;
        var matchIdx = -1;
        for (var i = 0; i < nextEfforts.length; i++) {
            if (nextEfforts[i].id === curEffort) {
                matchIdx = i;
                break;
            }
        }
        if (matchIdx < 0) {
            for (var j = 0; j < nextEfforts.length; j++) {
                if (nextEfforts[j].id === "high" || nextEfforts[j].id === "medium")
                    matchIdx = j;
            }
            if (matchIdx < 0) matchIdx = 0;
        }
        root.agyModelIdx = nextIdx;
        root.agyEffortIdx = matchIdx;
    }

    function agySetEffort(idx) {
        if (idx >= 0 && idx < root.agyEfforts.length) {
            root.agyEffortIdx = idx;
        }
    }

    function agyCycleEffort() {
        root.agyEffortIdx = (root.agyEffortIdx + 1) % root.agyEfforts.length;
    }

    function agyCopyText(txt) {
        if (!txt || txt.length === 0) return;
        Quickshell.execDetached(["python3", "-c",
            "import subprocess, sys; subprocess.run(['wl-copy'], input=sys.argv[1].encode())",
            txt]);
    }

    function agyOpenTerminal() {
        var cmd = ["kitty", "--working-directory", "/home/huyle", "-e", "/home/huyle/.local/bin/agy"];
        if (root.agyConversationId.length > 0) {
            cmd.push("--conversation");
            cmd.push(root.agyConversationId);
        }
        Quickshell.execDetached(cmd);
    }

    function refreshAgyHistory() {
        root.agyHistoryLoading = true;
        agyHistoryProc.running = false;
        agyHistoryProc.running = true;
    }

    function agyLoadConversation(convId) {
        if (!convId || convId.length === 0 || root.agyBusy) return;
        root.agyConversationId = convId;
        root.agyStatus = "";
        root.agyLiveText = "";
        root.agyActiveMsgIdx = -1;
        agyLoadProc.command = [
            "python3", "-c",
            "import json, os, re, sys\n" +
            "cid = sys.argv[1]\n" +
            "p = f'/home/huyle/.gemini/antigravity-cli/brain/{cid}/.system_generated/logs/transcript.jsonl'\n" +
            "if not os.path.exists(p): sys.exit(0)\n" +
            "msgs = []\n" +
            "def clean_user(s):\n" +
            "    m = re.search(r'<USER_REQUEST>\\n?(.*?)\\n?</USER_REQUEST>', s, re.S)\n" +
            "    t = m.group(1) if m else s\n" +
            "    t = re.sub(r'^\\[Antigravity Mini:[^\\]]*\\]\\s*', '', t)\n" +
            "    return t.strip()\n" +
            "with open(p, 'r', errors='replace') as f:\n" +
            "    for line in f:\n" +
            "        try: obj = json.loads(line)\n" +
            "        except Exception: continue\n" +
            "        tp = obj.get('type', '')\n" +
            "        ct = (obj.get('content') or '').strip()\n" +
            "        if not ct: continue\n" +
            "        if tp == 'USER_INPUT':\n" +
            "            u = clean_user(ct)\n" +
            "            if u: msgs.append({'role': 'user', 'text': u, 'meta': '', 'streaming': False})\n" +
            "        elif tp == 'PLANNER_RESPONSE':\n" +
            "            mdl = obj.get('model') or ''\n" +
            "            msgs.append({'role': 'assistant', 'text': ct, 'meta': mdl, 'streaming': False})\n" +
            "for i, m in enumerate(msgs[-30:]):\n" +
            "    m['uid'] = i + 1\n" +
            "    print(json.dumps(m))\n",
            convId
        ];
        agyLoadProc.running = false;
        agyLoadProc.running = true;
    }

    function agyDeleteConversation(convId) {
        if (!convId || convId.length === 0) return;
        if (root.agyBusy && convId === root.agyConversationId) return;
        if (convId === root.agyConversationId) {
            root.agyNewChat();
        }
        root.agyHistory = root.agyHistory.filter(function(item) {
            return item.id !== convId;
        });
        agyDeleteProc.command = [
            "python3", "-c",
            "import os, re, shutil, sqlite3, sys\n" +
            "cid = sys.argv[1]\n" +
            "if not re.fullmatch(r'[0-9a-fA-F-]+', cid): raise SystemExit(0)\n" +
            "base = '/home/huyle/.gemini/antigravity-cli'\n" +
            "db = os.path.join(base, 'conversation_summaries.db')\n" +
            "ids = [cid]\n" +
            "if os.path.exists(db):\n" +
            "    conn = sqlite3.connect(db, timeout=5.0)\n" +
            "    try:\n" +
            "        children = [r[0] for r in conn.execute('SELECT conversation_id FROM conversation_summaries WHERE parent_conversation_id = ?', (cid,)).fetchall() if r[0] and re.fullmatch(r'[0-9a-fA-F-]+', r[0])]\n" +
            "        for c in children:\n" +
            "            if c not in ids: ids.append(c)\n" +
            "        for tid in ids:\n" +
            "            for q in [\n" +
            "                ('DELETE FROM conversation_summaries WHERE conversation_id = ?', (tid,)),\n" +
            "                ('DELETE FROM conversations_fts WHERE conversation_id = ?', (tid,)),\n" +
            "                ('DELETE FROM conversation_parents WHERE conversation_id = ? OR parent_id = ?', (tid, tid)),\n" +
            "            ]:\n" +
            "                try: conn.execute(q[0], q[1])\n" +
            "                except Exception: pass\n" +
            "        conn.commit()\n" +
            "    finally:\n" +
            "        conn.close()\n" +
            "for tid in ids:\n" +
            "    shutil.rmtree(os.path.join(base, 'brain', tid), ignore_errors=True)\n" +
            "    for p in [\n" +
            "        os.path.join(base, 'conversations', tid + '.db'),\n" +
            "        os.path.join(base, 'conversations', tid + '.db-shm'),\n" +
            "        os.path.join(base, 'conversations', tid + '.db-wal'),\n" +
            "        os.path.join(base, 'annotations', tid + '.pbtxt'),\n" +
            "    ]:\n" +
            "        try:\n" +
            "            if os.path.exists(p): os.remove(p)\n" +
            "        except Exception: pass\n",
            convId
        ];
        agyDeleteProc.running = false;
        agyDeleteProc.running = true;
    }

    function agyStop() {
        if (agyProc.running) {
            agyProc.running = false;
        }
        root.agyBusy = false;
        root.agyStatus = "";
        if (root.agyActiveMsgIdx >= 0 && root.agyActiveMsgIdx < root.agyMessages.length) {
            var arr = root.agyMessages.slice();
            var m = Object.assign({}, arr[root.agyActiveMsgIdx]);
            m.streaming = false;
            var txt = root.agyLiveText.replace(/\s+$/, "");
            m.text = txt.length > 0 ? txt : "(Stopped)";
            arr[root.agyActiveMsgIdx] = m;
            root.agyMessages = arr;
        }
        root.agyLiveText = "";
        root.agyActiveMsgIdx = -1;
    }

    function agyNewChat() {
        if (agyProc.running) agyProc.running = false;
        root.agyBusy = false;
        root.agyStatus = "";
        root.agyLiveText = "";
        root.agyLastMeta = "";
        root.agyConversationId = "";
        root.agyActiveMsgIdx = -1;
        root.agyMessages = [];
    }

    function agySend(promptText) {
        var trimmed = (promptText || "").trim();
        if (trimmed.length === 0 || root.agyBusy) return;

        root.agySeq += 2;
        root.agyLiveText = "";
        root.agyLastMeta = root.agyModelLabel + " · " + root.agyEffortLabel;
        var arr = root.agyMessages.slice();
        arr.push({ uid: root.agySeq - 1, role: "user", text: trimmed, meta: "", streaming: false });
        arr.push({ uid: root.agySeq, role: "assistant", text: "", meta: root.agyLastMeta, streaming: true });
        root.agyActiveMsgIdx = arr.length - 1;
        root.agyMessages = arr;
        root.agyBusy = true;
        root.agyStatus = "Thinking...";
        root.agyErrBuf = "";

        var cliPrompt = "[Antigravity Mini: Reply concisely, directly, and straight to the point without greetings or filler. Keep formatting compact and match the user's language.]\n\n" + trimmed;
        var cmd = [
            "/home/huyle/.local/bin/agy",
            "-p", cliPrompt,
            "--output-format", "stream-json",
            "--model", root.agyModelId,
            "--effort", root.agyEffortId,
            "--print-timeout", "120s"
        ];
        if (root.agyConversationId.length > 0) {
            cmd.push("--conversation");
            cmd.push(root.agyConversationId);
        }
        agyProc.command = cmd;
        agyProc.running = true;
    }

    function agyPatchActive(patchFn) {
        var idx = root.agyActiveMsgIdx;
        if (idx < 0 || idx >= root.agyMessages.length) return;
        var arr = root.agyMessages.slice();
        var item = Object.assign({}, arr[idx]);
        patchFn(item);
        arr[idx] = item;
        root.agyMessages = arr;
    }

    Process {
        id: agyHistoryProc
        command: [
            "python3", "-c",
            "import sqlite3, json, os, re\n" +
            "from datetime import datetime, timezone\n" +
            "db = '/home/huyle/.gemini/antigravity-cli/conversation_summaries.db'\n" +
            "if not os.path.exists(db): raise SystemExit(0)\n" +
            "conn = sqlite3.connect(f'file:{db}?mode=ro', uri=True)\n" +
            "rows = conn.execute(\"SELECT conversation_id, title, preview, step_count, last_modified_time FROM conversation_summaries WHERE COALESCE(parent_conversation_id,'')='' AND step_count > 0 ORDER BY last_modified_time DESC LIMIT 20\").fetchall()\n" +
            "for cid, title, prev, steps, mod in rows:\n" +
            "    label = (title or '').strip() or (prev or '').strip()\n" +
            "    label = re.sub(r'^\\[Antigravity Mini:[^\\]]*\\]\\s*', '', label).strip()\n" +
            "    if not label: label = cid[:8]\n" +
            "    tstr = ''\n" +
            "    try:\n" +
            "        dt = datetime.fromisoformat(mod.replace('Z', '+00:00').split('.')[0] + '+00:00').astimezone()\n" +
            "        tstr = dt.strftime('%m/%d %H:%M')\n" +
            "    except Exception:\n" +
            "        tstr = (mod or '')[:16]\n" +
            "    print(json.dumps({'id': cid, 'title': label, 'steps': steps, 'time': tstr}))\n"
        ]
        property var buffer: []
        onStarted: buffer = []
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (!line) return;
                try { agyHistoryProc.buffer.push(JSON.parse(line)); } catch (e) {}
            }
        }
        onExited: {
            root.agyHistory = agyHistoryProc.buffer;
            root.agyHistoryLoading = false;
        }
    }

    Process {
        id: agyDeleteProc
        onExited: root.refreshAgyHistory()
    }

    Process {
        id: agyLoadProc
        property var buffer: []
        onStarted: buffer = []
        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (!line) return;
                try { agyLoadProc.buffer.push(JSON.parse(line)); } catch (e) {}
            }
        }
        onExited: {
            root.agySeq += agyLoadProc.buffer.length + 2;
            root.agyMessages = agyLoadProc.buffer;
        }
    }

    Process {
        id: agyProc
        workingDirectory: "/home/huyle"

        stdout: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (line.length === 0) return;
                try {
                    var ev = JSON.parse(line);
                    if (ev.conversation_id && ev.conversation_id.length > 0) {
                        root.agyConversationId = ev.conversation_id;
                    }
                    if (ev.event === "step_update" && ev.step_update) {
                        var su = ev.step_update;
                        if (su.conversation_id && su.conversation_id.length > 0) {
                            root.agyConversationId = su.conversation_id;
                        }
                        if (su.step_type === "thinking") {
                            root.agyStatus = "Thinking...";
                        } else if (su.step_type === "tool_call" || su.step_type === "command_execution") {
                            root.agyStatus = "Running tool...";
                        } else if (su.step_type === "agent_response") {
                            root.agyStatus = "Streaming...";
                            if (su.text_delta && su.text_delta.length > 0) {
                                root.agyLiveText += su.text_delta;
                            }
                        }
                    } else if (ev.event === "result" && ev.result) {
                        var res = ev.result;
                        if (res.conversation_id && res.conversation_id.length > 0) {
                            root.agyConversationId = res.conversation_id;
                        }
                        if (root.agyLiveText.trim().length === 0 && res.response) {
                            root.agyLiveText = res.response;
                        }
                        var dur = res.duration_seconds ? (res.duration_seconds.toFixed(1) + "s") : "";
                        root.agyLastMeta = root.agyModelLabel + " · " + root.agyEffortLabel + (dur.length > 0 ? (" · " + dur) : "");
                    }
                } catch (e) {}
            }
        }

        stderr: SplitParser {
            onRead: data => {
                var line = data.trim();
                if (line.length > 0) root.agyErrBuf += line + "\n";
            }
        }

        onExited: (code) => {
            root.agyBusy = false;
            root.agyStatus = "";
            var finalTxt = (root.agyLiveText || "").replace(/\s+$/, "");
            root.agyPatchActive(m => {
                m.streaming = false;
                m.meta = root.agyLastMeta;
                if (finalTxt.length > 0) {
                    m.text = finalTxt;
                } else if (code !== 0 && root.agyErrBuf.trim().length > 0) {
                    m.role = "error";
                    m.text = root.agyErrBuf.trim();
                } else if (code !== 0) {
                    m.role = "error";
                    m.text = "Antigravity exited with code " + code;
                } else {
                    m.text = "(Empty response)";
                }
            });
            root.agyLiveText = "";
            root.agyActiveMsgIdx = -1;
        }
    }

    // ==================== MONITORS / DISPLAY (wlr-randr) ====================
    property var monitors: []
    property string displayMode: "primary"
    property bool monitorsLoading: false
    property bool monitorsApplying: false
    property string monitorStatus: ""
    property bool monitorStatusError: false

    function isInternalMonitorName(name) {
        return /^(eDP|LVDS|DSI)/i.test(name || "");
    }

    function monitorSortKey(name) {
        return isInternalMonitorName(name) ? ("0_" + name) : ("1_" + name);
    }

    function refreshMonitors() {
        if (monitorsProc.running) return;
        monitorsLoading = true;
        monitorsProc.running = true;
    }

    function runWlrRandr(args, okMsg) {
        if (monitorApplyProc.running || !args || args.length === 0) return;
        monitorsApplying = true;
        monitorStatus = "";
        monitorStatusError = false;
        monitorApplyProc.pendingOkMsg = okMsg || "Display updated";
        monitorApplyProc.errBuf = "";
        monitorApplyProc.command = ["wlr-randr"].concat(args);
        monitorApplyProc.running = true;
    }

    function applyDisplayMode(mode) {
        var list = root.monitors || [];
        if (list.length === 0) {
            root.monitorStatus = "No monitors detected";
            root.monitorStatusError = true;
            return;
        }
        if (list.length === 1 && mode !== "primary") {
            root.monitorStatus = "Connect a 2nd monitor for Extend / Mirror / External";
            root.monitorStatusError = false;
            return;
        }
        var m0 = list[0];
        var args = [];
        if (mode === "primary") {
            args.push("--output", m0.name, "--on", "--pos", "0,0");
            for (var i = 1; i < list.length; i++) {
                args.push("--output", list[i].name, "--off");
            }
            runWlrRandr(args, "Switched to " + m0.name + " only");
        } else if (mode === "secondary" && list.length >= 2) {
            var m1 = list[1];
            args.push("--output", m1.name, "--on", "--pos", "0,0");
            args.push("--output", m0.name, "--off");
            for (var j = 2; j < list.length; j++) {
                args.push("--output", list[j].name, "--off");
            }
            runWlrRandr(args, "Switched to " + m1.name + " only");
        } else if (mode === "mirror" && list.length >= 2) {
            args.push("--output", m0.name, "--on", "--pos", "0,0");
            args.push("--output", list[1].name, "--on", "--pos", "0,0");
            runWlrRandr(args, "Mirror mode active (overlapping at 0,0)");
        } else if (mode === "extend" && list.length >= 2) {
            var extX = Math.max(640, m0.logicalWidth || 1536);
            args.push("--output", m0.name, "--on", "--pos", "0,0");
            args.push("--output", list[1].name, "--on", "--pos", extX + ",0");
            runWlrRandr(args, "Extend mode active");
        }
    }

    function applyMonitorLayout(layoutList) {
        if (!layoutList || layoutList.length === 0) return;
        var args = [];
        for (var i = 0; i < layoutList.length; i++) {
            var item = layoutList[i];
            args.push("--output", item.name);
            if (item.enabled === false) {
                args.push("--off");
            } else {
                args.push("--on", "--pos", Math.round(item.x) + "," + Math.round(item.y));
            }
        }
        runWlrRandr(args, "Monitor positions applied");
    }

    function applyMonitorSettings(name, enabled, modeStr, scaleVal, transformStr, posX, posY) {
        if (!name) return;
        var args = ["--output", name];
        if (!enabled) {
            args.push("--off");
        } else {
            args.push("--on");
            if (modeStr && modeStr.length > 0) {
                args.push("--mode", modeStr);
            }
            if (scaleVal && scaleVal > 0) {
                args.push("--scale", String(scaleVal));
            }
            if (transformStr && transformStr.length > 0) {
                args.push("--transform", transformStr);
            }
            args.push("--pos", Math.round(posX || 0) + "," + Math.round(posY || 0));
        }
        runWlrRandr(args, "Updated " + name);
    }

    Process {
        id: monitorsProc
        command: ["wlr-randr", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.monitorsLoading = false;
                if (!text || text.trim().length === 0) return;
                try {
                    var raw = JSON.parse(text);
                    if (!Array.isArray(raw)) return;
                    var parsed = [];
                    for (var i = 0; i < raw.length; i++) {
                        var o = raw[i];
                        var curW = 1920, curH = 1080, curHz = 60.0;
                        var modesOut = [];
                        var seen = {};
                        var rawModes = Array.isArray(o.modes) ? o.modes : [];
                        for (var k = 0; k < rawModes.length; k++) {
                            var md = rawModes[k];
                            var hz = md.refresh ? Number(md.refresh) : 60.0;
                            var hzStr = hz.toFixed(3).replace(/\.?0+$/, "");
                            var spec = md.width + "x" + md.height + "@" + hzStr + "Hz";
                            var label = md.width + "×" + md.height + " @ " + Math.round(hz) + "Hz";
                            if (md.current) {
                                curW = md.width;
                                curH = md.height;
                                curHz = hz;
                            }
                            if (!seen[spec]) {
                                seen[spec] = true;
                                modesOut.push({
                                    width: md.width,
                                    height: md.height,
                                    refresh: hz,
                                    spec: spec,
                                    label: label,
                                    preferred: !!md.preferred,
                                    current: !!md.current
                                });
                            }
                        }
                        if (modesOut.length > 0 && curW === 1920 && !rawModes.some(function(x){ return x.current; })) {
                            curW = modesOut[0].width;
                            curH = modesOut[0].height;
                            curHz = modesOut[0].refresh;
                        }
                        var sc = (o.scale && o.scale > 0) ? Number(o.scale) : 1.0;
                        var tr = o.transform || "normal";
                        var rotated = (tr === "90" || tr === "270" || tr === "flipped-90" || tr === "flipped-270");
                        var lw = Math.max(320, Math.round((rotated ? curH : curW) / sc));
                        var lh = Math.max(240, Math.round((rotated ? curW : curH) / sc));
                        var px = (o.position && o.position.x !== undefined) ? Number(o.position.x) : 0;
                        var py = (o.position && o.position.y !== undefined) ? Number(o.position.y) : 0;
                        parsed.push({
                            name: o.name || ("OUT-" + i),
                            description: o.description || o.model || o.name || "",
                            enabled: o.enabled !== false,
                            scale: sc,
                            transform: tr,
                            x: px,
                            y: py,
                            width: curW,
                            height: curH,
                            refresh: curHz,
                            logicalWidth: lw,
                            logicalHeight: lh,
                            modes: modesOut
                        });
                    }
                    parsed.sort(function(a, b) {
                        var ka = root.monitorSortKey(a.name);
                        var kb = root.monitorSortKey(b.name);
                        return ka < kb ? -1 : (ka > kb ? 1 : 0);
                    });
                    root.monitors = parsed;

                    var enabledList = parsed.filter(function(m) { return m.enabled; });
                    if (parsed.length <= 1) {
                        root.displayMode = "primary";
                    } else if (enabledList.length === 1) {
                        root.displayMode = (enabledList[0].name === parsed[0].name) ? "primary" : "secondary";
                    } else if (enabledList.length >= 2) {
                        if (enabledList[0].x === enabledList[1].x && enabledList[0].y === enabledList[1].y) {
                            root.displayMode = "mirror";
                        } else {
                            root.displayMode = "extend";
                        }
                    }
                } catch (e) {}
            }
        }
        onExited: root.monitorsLoading = false
    }

    Process {
        id: monitorApplyProc
        property string pendingOkMsg: ""
        property string errBuf: ""
        stderr: StdioCollector {
            onStreamFinished: monitorApplyProc.errBuf = (text || "").trim()
        }
        onExited: code => {
            root.monitorsApplying = false;
            if (code === 0) {
                root.monitorStatus = monitorApplyProc.pendingOkMsg;
                root.monitorStatusError = false;
            } else {
                root.monitorStatus = monitorApplyProc.errBuf.length > 0 ? monitorApplyProc.errBuf : "Failed to apply display settings";
                root.monitorStatusError = true;
            }
            root.refreshMonitors();
        }
    }

    // ==================== MANGOWM PERSONALIZATION ====================
    property int mangoGapInner: 5
    property int mangoGapOuter: 10
    property int mangoBorderPx: 2
    property int mangoBorderRadius: 0
    property bool mangoBlur: false
    property bool mangoShadows: false
    property bool mangoAnimations: true
    property string mangoAnimOpen: "slide"
    property string mangoAnimClose: "slide"
    property real mangoUnfocusedOpacity: 1.0
    property bool mangoSloppyFocus: true
    property bool mangoNoBorderSingle: false
    property string mangoApplyStatus: ""

    FileView {
        id: mangoCfgFile
        blockLoading: true
        printErrors: false
    }

    function loadMangoConfig() {
        mangoCfgFile.path = "/home/huyle/.config/mango/config.conf";
        var txt = mangoCfgFile.text();
        if (!txt || txt.length === 0) return;
        var lines = txt.split("\n");
        var vals = {};
        for (var i = 0; i < lines.length; i++) {
            var line = lines[i].trim();
            if (!line || line.startsWith("#")) continue;
            var eq = line.indexOf("=");
            if (eq <= 0) continue;
            var k = line.slice(0, eq).trim();
            var v = line.slice(eq + 1).trim();
            vals[k] = v;
        }
        if (vals.gap_inner_horizontal !== undefined) root.mangoGapInner = Math.max(0, Math.min(32, parseInt(vals.gap_inner_horizontal) || 0));
        if (vals.gap_outer_horizontal !== undefined) root.mangoGapOuter = Math.max(0, Math.min(40, parseInt(vals.gap_outer_horizontal) || 0));
        if (vals.border_px !== undefined) root.mangoBorderPx = Math.max(0, Math.min(8, parseInt(vals.border_px) || 0));
        if (vals.border_radius !== undefined) root.mangoBorderRadius = Math.max(0, Math.min(24, parseInt(vals.border_radius) || 0));
        if (vals.blur !== undefined) root.mangoBlur = (vals.blur === "1");
        if (vals.shadows !== undefined) root.mangoShadows = (vals.shadows === "1");
        if (vals.animations !== undefined) root.mangoAnimations = (vals.animations === "1");
        if (vals.animation_type_open) root.mangoAnimOpen = vals.animation_type_open;
        if (vals.animation_type_close) root.mangoAnimClose = vals.animation_type_close;
        if (vals.unfocused_opacity !== undefined) root.mangoUnfocusedOpacity = Math.max(0.4, Math.min(1.0, parseFloat(vals.unfocused_opacity) || 1.0));
        if (vals.sloppy_focus !== undefined) root.mangoSloppyFocus = (vals.sloppy_focus === "1");
        if (vals.no_border_when_single !== undefined) root.mangoNoBorderSingle = (vals.no_border_when_single === "1");
    }

    function saveAndReloadMangoConfig(updates, okMsg) {
        if (!updates) return;
        var jsonPayload = JSON.stringify(updates);
        mangoSaveProc.pendingMsg = okMsg || "MangoWM updated";
        mangoSaveProc.command = [
            "python3", "-c",
            "import json, re, sys\n" +
            "path = '/home/huyle/.config/mango/config.conf'\n" +
            "upd = json.loads(sys.argv[1])\n" +
            "lines = open(path, 'r').read().splitlines()\n" +
            "seen = set()\n" +
            "out = []\n" +
            "for ln in lines:\n" +
            "    s = ln.strip()\n" +
            "    if s and not s.startswith('#') and '=' in s:\n" +
            "        k = s.split('=', 1)[0].strip()\n" +
            "        if k in upd:\n" +
            "            out.append(f'{k}={upd[k]}')\n" +
            "            seen.add(k)\n" +
            "            continue\n" +
            "    out.append(ln)\n" +
            "for k, v in upd.items():\n" +
            "    if k not in seen:\n" +
            "        out.append(f'{k}={v}')\n" +
            "open(path, 'w').write('\\n'.join(out) + '\\n')\n",
            jsonPayload
        ];
        mangoSaveProc.running = true;
    }

    function applyMangoFormSettings() {
        saveAndReloadMangoConfig({
            gap_inner_horizontal: root.mangoGapInner,
            gap_inner_vertical: root.mangoGapInner,
            gap_outer_horizontal: root.mangoGapOuter,
            gap_outer_vertical: root.mangoGapOuter,
            border_px: root.mangoBorderPx,
            border_radius: root.mangoBorderRadius,
            blur: root.mangoBlur ? 1 : 0,
            shadows: root.mangoShadows ? 1 : 0,
            animations: root.mangoAnimations ? 1 : 0,
            animation_type_open: root.mangoAnimOpen,
            animation_type_close: root.mangoAnimClose,
            unfocused_opacity: root.mangoUnfocusedOpacity.toFixed(2),
            sloppy_focus: root.mangoSloppyFocus ? 1 : 0,
            no_border_when_single: root.mangoNoBorderSingle ? 1 : 0
        }, "Applied & reloaded MangoWM");
    }

    function syncMangoThemeColors() {
        var t = root.currentTheme;
        function toHex0x(h) {
            var s = String(h || "#2e3440").replace("#", "").toLowerCase();
            if (s.length === 6) s += "ff";
            return "0x" + s;
        }
        saveAndReloadMangoConfig({
            root_color: toHex0x(t.bg),
            border_color: toHex0x(t.border),
            focus_color: toHex0x(root.accentColor),
            urgent_color: toHex0x(t.red)
        }, "Synced MangoWM borders with " + t.label);
    }

    Process {
        id: mangoSaveProc
        property string pendingMsg: ""
        onExited: code => {
            if (code === 0) {
                root.mangoApplyStatus = mangoSaveProc.pendingMsg;
                root.reloadMango();
            } else {
                root.mangoApplyStatus = "Failed to save MangoWM config";
            }
        }
    }
}
