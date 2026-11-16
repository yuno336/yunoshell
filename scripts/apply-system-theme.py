#!/usr/bin/env python3
"""Apply the selected QuickShell theme & accent preset across desktop apps."""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

HOME = Path.home()
CFG = HOME / ".config"


def hex_norm(val: str, fallback: str = "#2e3440") -> str:
    s = str(val or fallback).strip()
    if not s.startswith("#"):
        s = "#" + s
    if len(s) == 7:
        return s.lower()
    return fallback.lower()


def hex_0x_rgba(val: str) -> str:
    return "0x" + hex_norm(val)[1:] + "ff"


def hex_rasi_rgba(val: str, alpha: str = "ff") -> str:
    return hex_norm(val) + alpha


def write_if_changed(path: Path, content: str) -> bool:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        try:
            if path.read_text(encoding="utf-8") == content:
                return False
        except OSError:
            pass
    path.write_text(content, encoding="utf-8")
    return True


def apply_kitty(t: dict) -> None:
    kitty_dir = CFG / "kitty"
    if not kitty_dir.exists():
        return
    theme_conf = kitty_dir / "theme-current.conf"
    kitty_conf = kitty_dir / "kitty.conf"

    if kitty_conf.exists():
        txt = kitty_conf.read_text(encoding="utf-8")
        if "theme-current.conf" not in txt:
            if not txt.endswith("\n"):
                txt += "\n"
            txt += "\n# System theme managed by QuickShell\ninclude theme-current.conf\n"
            kitty_conf.write_text(txt, encoding="utf-8")

    blue = hex_norm(t.get("blue"), hex_norm(t["cyan"]))
    body = f"""# {t['label']} Theme for Kitty (managed by QuickShell)
foreground              {t['textBright']}
background              {t['bg']}
selection_foreground    {t['accentText']}
selection_background    {t['accent']}
url_color               {t['accent']}
cursor                  {t['textBright']}
cursor_text_color       {t['bg']}

active_border_color     {t['accent']}
inactive_border_color   {t['surface']}
bell_border_color       {t['yellow']}

active_tab_foreground   {t['accentText']}
active_tab_background   {t['accent']}
inactive_tab_foreground {t['text']}
inactive_tab_background {t['surface']}
tab_bar_background      {t['bg']}

color0  {t['surface']}
color8  {t['dim']}

color1  {t['red']}
color9  {t['red']}

color2  {t['green']}
color10 {t['green']}

color3  {t['yellow']}
color11 {t['yellow']}

color4  {blue}
color12 {t['accent']}

color5  {t['purple']}
color13 {t['purple']}

color6  {t['cyan']}
color14 {t['cyan']}

color7  {t['text']}
color15 {t['textBright']}
"""
    write_if_changed(theme_conf, body)
    subprocess.run(["pkill", "-USR1", "-x", "kitty"], check=False)


def apply_gtk(t: dict) -> None:
    css = f"""/* {t['label']} Theme (managed by QuickShell) */
@define-color theme_bg_color {t['bg']};
@define-color theme_fg_color {t['text']};
@define-color theme_base_color {t['surface']};
@define-color theme_text_color {t['textBright']};
@define-color theme_selected_bg_color {t['accent']};
@define-color theme_selected_fg_color {t['accentText']};
@define-color window_bg_color {t['bg']};
@define-color window_fg_color {t['text']};
@define-color view_bg_color {t['surface']};
@define-color view_fg_color {t['textBright']};
@define-color headerbar_bg_color {t['bg']};
@define-color headerbar_fg_color {t['textBright']};
@define-color card_bg_color {t['surface']};
@define-color card_fg_color {t['textBright']};
@define-color popover_bg_color {t['surface']};
@define-color popover_fg_color {t['textBright']};
@define-color dialog_bg_color {t['bg']};
@define-color dialog_fg_color {t['textBright']};
@define-color sidebar_bg_color {t['bgDeep']};
@define-color sidebar_fg_color {t['text']};
@define-color secondary_sidebar_bg_color {t['bgDeep']};
@define-color secondary_sidebar_fg_color {t['text']};
@define-color accent_bg_color {t['accent']};
@define-color accent_fg_color {t['accentText']};
@define-color accent_color {t['accent']};
@define-color borders {t['border']};
"""
    for sub in ("gtk-3.0", "gtk-4.0"):
        write_if_changed(CFG / sub / "gtk.css", css)
    subprocess.run(
        ["gsettings", "set", "org.gnome.desktop.interface", "color-scheme", "prefer-dark"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def apply_qt_and_kvantum(t: dict) -> None:
    # 1. Qt5ct / Qt6ct custom QPalette color scheme
    # Roles (21): WindowText, Button, Light, Midlight, Dark, Mid, Text, BrightText,
    # ButtonText, Base, Window, Shadow, Highlight, HighlightedText, Link, LinkVisited,
    # AlternateBase, NoRole, ToolTipBase, ToolTipText, PlaceholderText
    active_roles = [
        t["text"], t["surface"], t["surfaceHover"], t["surfaceActive"], t["bgDeep"],
        t["border"], t["textBright"], t["textBright"], t["text"], t["surface"],
        t["bg"], "#000000", t["accent"], t["accentText"], t["cyan"], t["purple"],
        t["bgDeep"], t["bg"], t["surface"], t["textBright"], t["muted"],
    ]
    disabled_roles = list(active_roles)
    disabled_roles[0] = t["muted"]
    disabled_roles[6] = t["muted"]
    disabled_roles[8] = t["muted"]

    scheme_body = (
        "[ColorScheme]\n"
        f"active_colors={', '.join(active_roles)}\n"
        f"disabled_colors={', '.join(disabled_roles)}\n"
        f"inactive_colors={', '.join(active_roles)}\n"
    )

    for qt_dir_name in ("qt5ct", "qt6ct"):
        qt_dir = CFG / qt_dir_name
        if not qt_dir.exists():
            continue
        colors_file = qt_dir / "colors" / "QuickShell.conf"
        write_if_changed(colors_file, scheme_body)
        conf_file = qt_dir / f"{qt_dir_name}.conf"
        if conf_file.exists():
            lines = conf_file.read_text(encoding="utf-8").splitlines()
            out = []
            seen_pal = False
            seen_path = False
            in_appearance = False
            for ln in lines:
                s = ln.strip()
                if s.startswith("["):
                    if in_appearance:
                        if not seen_pal:
                            out.append("custom_palette=true")
                            seen_pal = True
                        if not seen_path:
                            out.append(f"color_scheme_path={colors_file}")
                            seen_path = True
                    in_appearance = (s == "[Appearance]")
                elif in_appearance and s.startswith("custom_palette="):
                    out.append("custom_palette=true")
                    seen_pal = True
                    continue
                elif in_appearance and s.startswith("color_scheme_path="):
                    out.append(f"color_scheme_path={colors_file}")
                    seen_path = True
                    continue
                out.append(ln)
            if in_appearance:
                if not seen_pal:
                    out.append("custom_palette=true")
                if not seen_path:
                    out.append(f"color_scheme_path={colors_file}")
            write_if_changed(conf_file, "\n".join(out) + "\n")

    # 2. Dynamic Kvantum theme based on Nordic template if installed
    tpl_kv = Path("/usr/share/Kvantum/Nordic/Nordic.kvconfig")
    tpl_svg = Path("/usr/share/Kvantum/Nordic/Nordic.svg")
    kv_dir = CFG / "Kvantum" / "QuickShell"
    if tpl_kv.exists() and tpl_svg.exists():
        kv_txt = tpl_kv.read_text(encoding="utf-8")
        gen_block = (
            "[GeneralColors]\n"
            f"window.color={t['bg']}\n"
            f"base.color={t['surface']}\n"
            f"alt.base.color={t['bgDeep']}cc\n"
            f"button.color={t['surfaceHover']}\n"
            f"light.color={t['surfaceHover']}\n"
            f"mid.light.color={t['surface']}\n"
            f"dark.color={t['bgDeep']}\n"
            f"mid.color={t['border']}\n"
            f"highlight.color={t['accent']}\n"
            f"inactive.highlight.color={t['surfaceActive']}\n"
            f"text.color={t['textBright']}\n"
            f"window.text.color={t['text']}\n"
            f"button.text.color={t['text']}\n"
            f"disabled.text.color={t['muted']}99\n"
            f"tooltip.text.color={t['textBright']}\n"
            f"highlight.text.color={t['accentText']}\n"
            f"link.color={t['cyan']}\n"
            f"link.visited.color={t['purple']}\n"
            f"progress.indicator.text.color={t['accentText']}\n\n"
        )
        kv_txt = re.sub(
            r"\[GeneralColors\][\s\S]*?(?=\n\[|\Z)",
            gen_block.rstrip() + "\n",
            kv_txt,
            count=1,
        )
        write_if_changed(kv_dir / "QuickShell.kvconfig", kv_txt)

        svg_txt = tpl_svg.read_text(encoding="utf-8")
        nord_map = {
            "#2e3440": t["bg"],
            "#2f343f": t["bg"],
            "#232831": t["bgDeep"],
            "#22252e": t["bgDeep"],
            "#373b43": t["surface"],
            "#3b4252": t["surface"],
            "#3c4454": t["surfaceHover"],
            "#434c5e": t["border"],
            "#4c566a": t["surfaceActive"],
            "#555761": t["border"],
            "#5c616c": t["muted"],
            "#88c0d0": t["accent"],
            "#8fbcbb": t["accent"],
            "#81a1c1": t["accent"],
            "#5e81ac": t["accent"],
            "#3daee9": t["accent"],
            "#d8dee9": t["text"],
            "#e5e9f0": t["text"],
            "#eceff4": t["textBright"],
        }
        pattern = re.compile("|".join(re.escape(k) for k in nord_map), re.IGNORECASE)
        svg_txt = pattern.sub(lambda m: nord_map[m.group(0).lower()], svg_txt)
        write_if_changed(kv_dir / "QuickShell.svg", svg_txt)
        write_if_changed(CFG / "Kvantum" / "kvantum.kvconfig", "[General]\ntheme=QuickShell\n")


def apply_rofi(t: dict) -> None:
    rasi = CFG / "rofi" / "config.rasi"
    if not rasi.exists():
        return
    txt = rasi.read_text(encoding="utf-8")
    repls = {
        "bg": hex_rasi_rgba(t["bg"], "f7"),
        "bg-alt": hex_rasi_rgba(t["surface"], "ff"),
        "bg-selected": hex_rasi_rgba(t["accent"], "ff"),
        "fg-selected": hex_rasi_rgba(t["accentText"], "ff"),
        "border-col": hex_rasi_rgba(t["border"], "ff"),
        "fg": hex_rasi_rgba(t["text"], "ff"),
        "fg-muted": hex_rasi_rgba(t["muted"], "ff"),
    }
    for k, v in repls.items():
        txt = re.sub(
            rf"^(\s*{re.escape(k)}\s*:\s*)#[0-9a-fA-F]{{6,8}}(\s*;)",
            rf"\g<1>{v}\g<2>",
            txt,
            flags=re.MULTILINE,
        )
    write_if_changed(rasi, txt)


def apply_fastfetch(t: dict) -> None:
    ff = CFG / "fastfetch" / "config.jsonc"
    if not ff.exists():
        return
    txt = ff.read_text(encoding="utf-8")
    txt = re.sub(r'("keys"\s*:\s*")#[0-9a-fA-F]{6}(")', rf'\g<1>{t["accent"]}\g<2>', txt)
    txt = re.sub(r'("title"\s*:\s*")#[0-9a-fA-F]{6}(")', rf'\g<1>{t["accent"]}\g<2>', txt)
    txt = re.sub(r'("output"\s*:\s*")#[0-9a-fA-F]{6}(")', rf'\g<1>{t["textBright"]}\g<2>', txt)
    write_if_changed(ff, txt)


def apply_mango(t: dict) -> None:
    conf = CFG / "mango" / "config.conf"
    if not conf.exists():
        return
    upd = {
        "root_color": hex_0x_rgba(t["bg"]),
        "border_color": hex_0x_rgba(t["border"]),
        "focus_color": hex_0x_rgba(t["accent"]),
        "urgent_color": hex_0x_rgba(t["red"]),
    }
    lines = conf.read_text(encoding="utf-8").splitlines()
    seen = set()
    out = []
    for ln in lines:
        s = ln.strip()
        if s and not s.startswith("#") and "=" in s:
            k = s.split("=", 1)[0].strip()
            if k in upd:
                out.append(f"{k}={upd[k]}")
                seen.add(k)
                continue
        out.append(ln)
    for k, v in upd.items():
        if k not in seen:
            out.append(f"{k}={v}")
    changed = write_if_changed(conf, "\n".join(out) + "\n")
    if changed:
        subprocess.run(["mmsg", "dispatch", "reload_config"], check=False)


def main() -> int:
    if len(sys.argv) < 2:
        return 1
    raw = json.loads(sys.argv[1])
    t = {
        "themeName": raw.get("themeName", "nord"),
        "label": raw.get("label", "Nord"),
        "bg": hex_norm(raw.get("bg"), "#2e3440"),
        "bgDeep": hex_norm(raw.get("bgDeep"), "#242933"),
        "surface": hex_norm(raw.get("surface"), "#323846"),
        "surfaceHover": hex_norm(raw.get("surfaceHover"), "#3e4658"),
        "surfaceActive": hex_norm(raw.get("surfaceActive"), "#4c566a"),
        "border": hex_norm(raw.get("border"), "#434c5e"),
        "text": hex_norm(raw.get("text"), "#c6d0f5"),
        "textBright": hex_norm(raw.get("textBright"), "#eceff4"),
        "muted": hex_norm(raw.get("muted"), "#9099ab"),
        "dim": hex_norm(raw.get("dim"), "#4c566a"),
        "accent": hex_norm(raw.get("accent"), "#88c0d0"),
        "accentText": hex_norm(raw.get("accentText"), "#2e3440"),
        "red": hex_norm(raw.get("red"), "#bf616a"),
        "orange": hex_norm(raw.get("orange"), "#d08770"),
        "yellow": hex_norm(raw.get("yellow"), "#ebcb8b"),
        "green": hex_norm(raw.get("green"), "#a3be8c"),
        "purple": hex_norm(raw.get("purple"), "#b48ead"),
        "cyan": hex_norm(raw.get("cyan"), "#88c0d0"),
        "blue": hex_norm(raw.get("blue"), raw.get("cyan", "#81a1c1")),
    }

    apply_kitty(t)
    apply_gtk(t)
    apply_qt_and_kvantum(t)
    apply_rofi(t)
    apply_fastfetch(t)
    apply_mango(t)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
