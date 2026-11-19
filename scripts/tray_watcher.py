#!/usr/bin/env python3
"""Lightweight org.kde.StatusNotifierWatcher + SNI + DBusMenu bridge for yunoshell."""

import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import threading
import zlib

from gi.repository import Gio, GLib

CACHE_DIR = "/tmp/yunoshell-tray"
os.makedirs(CACHE_DIR, exist_ok=True)

WATCHER_XML = """
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem">
      <arg type="s" name="service" direction="in"/>
    </method>
    <method name="RegisterStatusNotifierHost">
      <arg type="s" name="service" direction="in"/>
    </method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered">
      <arg type="s" name="service"/>
    </signal>
    <signal name="StatusNotifierItemUnregistered">
      <arg type="s" name="service"/>
    </signal>
    <signal name="StatusNotifierHostRegistered"/>
    <signal name="StatusNotifierHostUnregistered"/>
  </interface>
</node>
"""


def write_rgba_png(path: str, w: int, h: int, argb_bytes: bytes) -> bool:
    if w <= 0 or h <= 0 or len(argb_bytes) < w * h * 4:
        return False
    # SNI IconPixmap is big-endian ARGB32 (A, R, G, B per pixel) -> convert to RGBA for PNG
    rows = []
    stride = w * 4
    for y in range(h):
        off = y * stride
        row = bytearray(1 + stride)
        row[0] = 0  # filter none
        chunk = argb_bytes[off : off + stride]
        for x in range(0, stride, 4):
            a, r, g, b = chunk[x], chunk[x + 1], chunk[x + 2], chunk[x + 3]
            row[1 + x] = r
            row[1 + x + 1] = g
            row[1 + x + 2] = b
            row[1 + x + 3] = a
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk_tag(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk_tag(b"IHDR", ihdr)
        + chunk_tag(b"IDAT", zlib.compress(raw, 6))
        + chunk_tag(b"IEND", b"")
    )
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(png)
    os.replace(tmp, path)
    return True


def _strip_mnemonic(text: str) -> str:
    # Convert DBusMenu underline mnemonics "_File" -> "File", keep "__" as "_"
    out = []
    i = 0
    while i < len(text):
        if text[i] == "_":
            if i + 1 < len(text) and text[i + 1] == "_":
                out.append("_")
                i += 2
                continue
            i += 1
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def resolve_icon_to_png(icon_name: str, theme_path: str, pixmaps) -> str:
    if icon_name and os.path.isabs(icon_name) and os.path.isfile(icon_name):
        return _ensure_png(icon_name)

    if icon_name:
        found = _find_named_icon(icon_name, theme_path)
        if found:
            return _ensure_png(found)

    if pixmaps:
        best = None
        best_score = 10**9
        for w, h, data in pixmaps:
            if w <= 0 or h <= 0 or not data:
                continue
            score = abs(w - 32)
            if w < 16:
                score += 100
            if score < best_score:
                best_score = score
                best = (w, h, bytes(data))
        if best:
            w, h, raw = best
            digest = hashlib.sha256(struct.pack(">II", w, h) + raw).hexdigest()[:16]
            out = os.path.join(CACHE_DIR, f"pix_{digest}_{w}x{h}.png")
            if not os.path.isfile(out):
                write_rgba_png(out, w, h, raw)
            return out
    return ""


_ICON_CACHE = {}


def _find_named_icon(name: str, theme_path: str) -> str:
    key = (name, theme_path or "")
    if key in _ICON_CACHE:
        p = _ICON_CACHE[key]
        if p and os.path.isfile(p):
            return p

    roots = []
    if theme_path and os.path.isdir(theme_path):
        roots.append(theme_path)
    home = os.path.expanduser("~")
    roots.extend([
        os.path.join(home, ".local/share/icons"),
        "/usr/share/icons/Papirus-Dark",
        "/usr/share/icons/Papirus",
        "/usr/share/icons/breeze-dark",
        "/usr/share/icons/breeze",
        "/usr/share/icons/Adwaita",
        "/usr/share/icons/hicolor",
        "/usr/share/pixmaps",
    ])
    exts = (".png", ".svg")
    cands = [name]
    if name.endswith("-symbolic"):
        cands.append(name[:-9])
    for root in roots:
        if not os.path.isdir(root):
            continue
        # Quick direct match first
        for cand in cands:
            for ext in exts:
                p = os.path.join(root, cand + ext)
                if os.path.isfile(p):
                    _ICON_CACHE[key] = p
                    return p
        # Subdir match preferring 24x24 / 32x32 / scalable
        pref_dirs = (
            "24x24", "32x32", "22x22", "48x48", "16x16",
            "scalable", "apps", "status", "panel", "symbolic",
        )
        matched = []
        for cur, _, files in os.walk(root):
            base_set = set(files)
            for cand in cands:
                for ext in exts:
                    fn = cand + ext
                    if fn in base_set:
                        full = os.path.join(cur, fn)
                        pri = 99
                        for idx, pd in enumerate(pref_dirs):
                            if pd in cur:
                                pri = idx
                                break
                        if ext == ".png":
                            pri -= 0.5
                        matched.append((pri, full))
            if matched and len(matched) >= 4:
                break
        if matched:
            matched.sort(key=lambda x: x[0])
            _ICON_CACHE[key] = matched[0][1]
            return matched[0][1]
    _ICON_CACHE[key] = ""
    return ""


def _ensure_png(path: str) -> str:
    if path.lower().endswith(".png"):
        return path
    st = os.stat(path)
    key = hashlib.sha256(f"{path}:{st.st_mtime_ns}".encode()).hexdigest()[:16]
    out = os.path.join(CACHE_DIR, f"ico_{key}.png")
    if os.path.isfile(out):
        return out
    rsvg = shutil.which("rsvg-convert")
    if rsvg and path.lower().endswith(".svg"):
        try:
            subprocess.run([rsvg, "-w", "32", "-h", "32", path, "-o", out], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if os.path.isfile(out):
                return out
        except Exception:
            pass
    magick = shutil.which("magick")
    if magick:
        try:
            subprocess.run([magick, "-background", "none", path, "-resize", "32x32", out], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if os.path.isfile(out):
                return out
        except Exception:
            pass
    return ""


class TrayWatcher:
    def __init__(self, conn: Gio.DBusConnection):
        self.conn = conn
        self.items = {}  # key ("bus_name|obj_path") -> dict
        self.node_info = Gio.DBusNodeInfo.new_for_xml(WATCHER_XML)
        self.iface_info = self.node_info.interfaces[0]
        self.reg_id = conn.register_object(
            "/StatusNotifierWatcher",
            self.iface_info,
            self._on_method_call,
            self._on_get_property,
            None,
        )
        Gio.bus_own_name_on_connection(
            conn,
            "org.kde.StatusNotifierWatcher",
            Gio.BusNameOwnerFlags.ALLOW_REPLACEMENT | Gio.BusNameOwnerFlags.REPLACE,
            None,
            None,
        )
        conn.signal_subscribe(
            "org.freedesktop.DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
            "/org/freedesktop/DBus",
            None,
            Gio.DBusSignalFlags.NONE,
            self._on_name_owner_changed,
        )
        GLib.idle_add(self._discover_existing)

    def _emit(self, payload: dict):
        sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
        sys.stdout.flush()

    def _emit_items(self):
        out = []
        for k, it in sorted(self.items.items(), key=lambda kv: (kv[1].get("id", ""), kv[0])):
            if it.get("status") == "Passive" and not it.get("icon_png"):
                continue
            out.append({
                "key": k,
                "service": it["service"],
                "path": it["path"],
                "id": it.get("id", ""),
                "title": it.get("title", "") or it.get("id", ""),
                "tooltip": it.get("tooltip", ""),
                "icon_name": it.get("icon_name", ""),
                "icon_png": it.get("icon_png", ""),
                "menu_path": it.get("menu_path", ""),
                "item_is_menu": bool(it.get("item_is_menu", False)),
                "status": it.get("status", "Active"),
            })
        self._emit({"type": "items", "items": out})

    def _discover_existing(self):
        try:
            res = self.conn.call_sync(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "ListNames",
                None,
                GLib.VariantType("(as)"),
                Gio.DBusCallFlags.NONE,
                1500,
                None,
            )
            names = res.unpack()[0]
            for n in names:
                if n.startswith("org.kde.StatusNotifierItem-"):
                    self._register_item(n, "/StatusNotifierItem")
        except Exception:
            pass
        self._emit_items()
        return False

    def _on_method_call(self, _conn, sender, _obj_path, _iface, method, params, inv):
        arg = params.unpack()[0]
        if method == "RegisterStatusNotifierItem":
            if arg.startswith("/"):
                service, obj_path = sender, arg
            else:
                service, obj_path = arg, "/StatusNotifierItem"
            self._register_item(service, obj_path)
            inv.return_value(None)
        elif method == "RegisterStatusNotifierHost":
            try:
                self.conn.emit_signal(
                    None, "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher",
                    "StatusNotifierHostRegistered", None,
                )
            except Exception:
                pass
            inv.return_value(None)

    def _on_get_property(self, _conn, _sender, _obj_path, _iface, prop_name):
        if prop_name == "RegisteredStatusNotifierItems":
            vals = [f"{v['service']}{v['path']}" for v in self.items.values()]
            return GLib.Variant("as", vals)
        if prop_name == "IsStatusNotifierHostRegistered":
            return GLib.Variant("b", True)
        if prop_name == "ProtocolVersion":
            return GLib.Variant("i", 0)
        return None

    def _resolve_owner(self, name: str) -> str:
        if name.startswith(":"):
            return name
        try:
            res = self.conn.call_sync(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "GetNameOwner",
                GLib.Variant("(s)", (name,)),
                GLib.VariantType("(s)"),
                Gio.DBusCallFlags.NONE,
                1000,
                None,
            )
            return res.unpack()[0]
        except Exception:
            return ""

    def _register_item(self, service: str, obj_path: str):
        owner = self._resolve_owner(service)
        if not owner:
            return
        key = f"{service}|{obj_path}"
        is_new = key not in self.items
        if is_new:
            sub_ids = []
            for sig in ("NewIcon", "NewAttentionIcon", "NewOverlayIcon", "NewStatus", "NewTitle", "NewToolTip"):
                sid = self.conn.signal_subscribe(
                    owner,
                    "org.kde.StatusNotifierItem",
                    sig,
                    obj_path,
                    None,
                    Gio.DBusSignalFlags.NONE,
                    lambda *_args, k=key: self._refresh_item(k),
                )
                sub_ids.append(sid)
            self.items[key] = {
                "service": service,
                "owner": owner,
                "path": obj_path,
                "subs": sub_ids,
            }
            try:
                self.conn.emit_signal(
                    None,
                    "/StatusNotifierWatcher",
                    "org.kde.StatusNotifierWatcher",
                    "StatusNotifierItemRegistered",
                    GLib.Variant("(s)", (f"{service}{obj_path}",)),
                )
            except Exception:
                pass
        self._refresh_item(key)

    def _on_name_owner_changed(self, _conn, _sender, _path, _iface, _sig, params):
        name, _old_owner, new_owner = params.unpack()
        if new_owner:
            return
        gone = [k for k, v in self.items.items() if v["service"] == name or v["owner"] == name]
        if not gone:
            return
        for k in gone:
            it = self.items.pop(k, None)
            if it:
                for sid in it.get("subs", []):
                    self.conn.signal_unsubscribe(sid)
                try:
                    self.conn.emit_signal(
                        None,
                        "/StatusNotifierWatcher",
                        "org.kde.StatusNotifierWatcher",
                        "StatusNotifierItemUnregistered",
                        GLib.Variant("(s)", (f"{it['service']}{it['path']}",)),
                    )
                except Exception:
                    pass
        self._emit_items()

    def _refresh_item(self, key: str):
        it = self.items.get(key)
        if not it:
            return
        try:
            res = self.conn.call_sync(
                it["owner"],
                it["path"],
                "org.freedesktop.DBus.Properties",
                "GetAll",
                GLib.Variant("(s)", ("org.kde.StatusNotifierItem",)),
                GLib.VariantType("(a{sv})"),
                Gio.DBusCallFlags.NONE,
                1500,
                None,
            )
            props = res.unpack()[0]
        except Exception:
            return
        it["id"] = str(props.get("Id", "") or "")
        it["title"] = str(props.get("Title", "") or "")
        it["status"] = str(props.get("Status", "Active") or "Active")
        it["item_is_menu"] = bool(props.get("ItemIsMenu", False))
        it["menu_path"] = str(props.get("Menu", "") or "")
        theme_path = str(props.get("IconThemePath", "") or "")
        icon_name = str(props.get("IconName", "") or "")
        pixmaps = props.get("IconPixmap")
        if it["status"] == "NeedsAttention":
            att_name = str(props.get("AttentionIconName", "") or "")
            if att_name:
                icon_name = att_name
            att_pix = props.get("AttentionIconPixmap")
            if att_pix:
                pixmaps = att_pix
        tt = props.get("ToolTip")
        if isinstance(tt, tuple) and len(tt) >= 3:
            it["tooltip"] = str(tt[2] or "")
        it["icon_name"] = icon_name
        it["icon_png"] = resolve_icon_to_png(icon_name, theme_path, pixmaps)
        self._emit_items()

    def handle_cmd(self, cmd: dict):
        act = cmd.get("cmd")
        key = cmd.get("key", "")
        it = self.items.get(key)
        if not it:
            return
        if act == "activate":
            method = "ContextMenu" if it.get("item_is_menu") else "Activate"
            self._call_sni(it, method, cmd.get("x", 0), cmd.get("y", 0))
            if it.get("item_is_menu") and it.get("menu_path"):
                self._fetch_and_emit_menu(key, it)
        elif act == "secondary":
            self._call_sni(it, "SecondaryActivate", cmd.get("x", 0), cmd.get("y", 0))
        elif act == "menu_open":
            if it.get("menu_path"):
                self._fetch_and_emit_menu(key, it)
            else:
                self._call_sni(it, "ContextMenu", cmd.get("x", 0), cmd.get("y", 0))
        elif act == "menu_click":
            mid = int(cmd.get("id", 0))
            menu_path = it.get("menu_path", "")
            if menu_path:
                try:
                    self.conn.call_sync(
                        it["owner"],
                        menu_path,
                        "com.canonical.dbusmenu",
                        "Event",
                        GLib.Variant("(isvu)", (mid, "clicked", GLib.Variant("s", ""), 0)),
                        None,
                        Gio.DBusCallFlags.NONE,
                        1500,
                        None,
                    )
                except Exception:
                    pass
                GLib.timeout_add(120, lambda k=key: self._refresh_item(k) or False)

    def _call_sni(self, it: dict, method: str, x: int, y: int):
        try:
            self.conn.call(
                it["owner"],
                it["path"],
                "org.kde.StatusNotifierItem",
                method,
                GLib.Variant("(ii)", (int(x), int(y))),
                None,
                Gio.DBusCallFlags.NONE,
                1500,
                None,
                None,
            )
        except Exception:
            pass

    def _fetch_and_emit_menu(self, key: str, it: dict):
        menu_path = it.get("menu_path", "")
        if not menu_path:
            return
        try:
            try:
                self.conn.call_sync(
                    it["owner"],
                    menu_path,
                    "com.canonical.dbusmenu",
                    "AboutToShow",
                    GLib.Variant("(i)", (0,)),
                    GLib.VariantType("(b)"),
                    Gio.DBusCallFlags.NONE,
                    1000,
                    None,
                )
            except Exception:
                pass
            res = self.conn.call_sync(
                it["owner"],
                menu_path,
                "com.canonical.dbusmenu",
                "GetLayout",
                GLib.Variant("(iias)", (0, 2, ["label", "enabled", "visible", "type", "toggle-type", "toggle-state"])),
                GLib.VariantType("(u(ia{sv}av))"),
                Gio.DBusCallFlags.NONE,
                1500,
                None,
            )
            _rev, root = res.unpack()
            entries = []
            for child in root[2]:
                cid, cprops, cchildren = child
                if not cprops.get("visible", True):
                    continue
                ctype = cprops.get("type", "")
                if ctype == "separator":
                    entries.append({"id": int(cid), "label": "", "enabled": False, "sep": True, "checked": False})
                    continue
                lbl = _strip_mnemonic(str(cprops.get("label", "") or ""))
                if not lbl:
                    continue
                t_type = cprops.get("toggle-type", "")
                checked = bool(t_type) and int(cprops.get("toggle-state", 0)) == 1
                entries.append({
                    "id": int(cid),
                    "label": lbl,
                    "enabled": bool(cprops.get("enabled", True)),
                    "sep": False,
                    "checked": checked,
                })
                # Flatten 1 level of submenu inline with indent prefix if small
                if cchildren and len(cchildren) <= 8:
                    for sub in cchildren:
                        sid, sprops, _ = sub
                        if not sprops.get("visible", True) or sprops.get("type", "") == "separator":
                            continue
                        slbl = _strip_mnemonic(str(sprops.get("label", "") or ""))
                        if slbl:
                            st_type = sprops.get("toggle-type", "")
                            schecked = bool(st_type) and int(sprops.get("toggle-state", 0)) == 1
                            entries.append({
                                "id": int(sid),
                                "label": "  · " + slbl,
                                "enabled": bool(sprops.get("enabled", True)),
                                "sep": False,
                                "checked": schecked,
                            })
            self._emit({"type": "menu", "key": key, "title": it.get("title", "") or it.get("id", "Tray"), "items": entries[:18]})
        except Exception:
            pass


def main():
    conn = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    watcher = TrayWatcher(conn)
    loop = GLib.MainLoop()

    def stdin_reader():
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                cmd = json.loads(line)
                GLib.idle_add(lambda c=cmd: watcher.handle_cmd(c) or False)
            except Exception:
                pass
        GLib.idle_add(loop.quit)

    t = threading.Thread(target=stdin_reader, daemon=True)
    t.start()
    loop.run()


if __name__ == "__main__":
    main()
