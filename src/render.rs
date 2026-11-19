use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::PathBuf;

use cairo::{Context, FontSlant, FontWeight, ImageSurface, Operator};

use crate::services::{
    local_clock_strings, AgyMsg, AppItem, DriveUsage, FileItem, FileViewMode, FilesPrompt,
    MangoSummary, SortBy, SysStats, AGY_MODELS,
};
use crate::theme::{Rgb, ShellConfig, Theme};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PopupKind {
    None,
    Files,
    Launcher,
    Controls,
    Power,
    Settings,
    Media,
    Notifications,
    Volume,
    Brightness,
    Battery,
    Wifi,
    Bluetooth,
    Antigravity,
    TrayMenu,
}

impl PopupKind {
    pub fn from_name(s: &str) -> Self {
        match s {
            "files" => Self::Files,
            "launcher" => Self::Launcher,
            "controls" => Self::Controls,
            "power" => Self::Power,
            "settings" => Self::Settings,
            "media" => Self::Media,
            "notifications" => Self::Notifications,
            "volume" => Self::Volume,
            "brightness" => Self::Brightness,
            "battery" => Self::Battery,
            "wifi" => Self::Wifi,
            "bluetooth" => Self::Bluetooth,
            "antigravity" => Self::Antigravity,
            "tray_menu" => Self::TrayMenu,
            _ => Self::None,
        }
    }

    /// True for popups that render inside the centered top Dynamic Island.
    pub fn is_top_island(self) -> bool {
        matches!(
            self,
            Self::Launcher
                | Self::Settings
                | Self::Files
                | Self::Notifications
                | Self::Media
        )
    }

    pub fn card_width(self) -> f64 {
        match self {
            Self::Settings => 700.0,
            Self::Files => 760.0,
            Self::Launcher => 680.0,
            Self::Antigravity => 360.0,
            Self::Media => 320.0,
            Self::Notifications => 316.0,
            Self::Wifi | Self::Bluetooth => 292.0,
            Self::Volume => 280.0,
            Self::Battery => 292.0,
            Self::Brightness => 252.0,
            Self::Power => 228.0,
            Self::Controls => 316.0,
            Self::TrayMenu => 232.0,
            Self::None => 164.0,
        }
    }

    pub fn target_size(
        self,
        launcher_rows: usize,
        stats: &SysStats,
        wifi_has_prompt: bool,
        cc_expanded: &str,
    ) -> (f64, f64) {
        let w = self.card_width();
        let h = match self {
            Self::None => 2.0,
            Self::Files => 454.0,
            Self::Launcher => 78.0 + launcher_rows.clamp(1, 3) as f64 * 72.0,
            Self::Settings => 324.0,
            Self::Notifications => {
                if stats.notifications.is_empty() {
                    132.0
                } else {
                    76.0 + (stats.notifications.len() as f64 * 64.0).min(320.0)
                }
            }
            Self::Media => {
                if stats.media_players.len() > 1 {
                    166.0
                } else if stats.media_players.is_empty() {
                    72.0
                } else {
                    140.0
                }
            }
            Self::Volume => {
                let n = stats.audio_outputs.len();
                118.0 + n as f64 * 38.0
            }
            Self::Brightness => 74.0,
            Self::Battery => 114.0,
            Self::Wifi => {
                if !stats.wifi_enabled {
                    118.0
                } else {
                    let mut wh = 286.0;
                    if !stats.wifi_status_msg.is_empty() {
                        wh += 22.0;
                    }
                    if wifi_has_prompt {
                        wh += 68.0;
                    }
                    wh
                }
            }
            Self::Bluetooth => {
                if !stats.bt_powered {
                    118.0
                } else if stats.bt_paired.is_empty() && stats.bt_available.is_empty() && !stats.bt_scanning {
                    168.0 + if stats.bt_status_msg.is_empty() { 0.0 } else { 22.0 }
                } else {
                    300.0 + if stats.bt_status_msg.is_empty() { 0.0 } else { 22.0 }
                }
            }
            Self::Power => 208.0,
            Self::Antigravity => 386.0,
            Self::TrayMenu => {
                let n = stats.tray_menu_items.len().max(1);
                let seps = stats.tray_menu_items.iter().filter(|m| m.sep).count();
                let normal = n.saturating_sub(seps);
                (44.0 + normal as f64 * 28.0 + seps as f64 * 9.0).clamp(86.0, 420.0)
            }
            Self::Controls => {
                // Base Control Center: header (26) + 8 + connectivity (46) + 8 + sliders (92) + padding (34) = 214
                let extra = match cc_expanded {
                    "wifi" => {
                        if !stats.wifi_enabled {
                            54.0
                        } else {
                            let mut eh = 194.0;
                            if !stats.wifi_status_msg.is_empty() {
                                eh += 20.0;
                            }
                            if wifi_has_prompt {
                                eh += 66.0;
                            }
                            eh
                        }
                    }
                    "bt" => {
                        if !stats.bt_powered {
                            54.0
                        } else if stats.bt_paired.is_empty() && stats.bt_available.is_empty() && !stats.bt_scanning {
                            76.0 + if stats.bt_status_msg.is_empty() { 0.0 } else { 20.0 }
                        } else {
                            194.0 + if stats.bt_status_msg.is_empty() { 0.0 } else { 20.0 }
                        }
                    }
                    "audio" => {
                        let n = stats.audio_outputs.len().max(1);
                        22.0 + n as f64 * 36.0
                    }
                    "battery" => 62.0,
                    "power" => 74.0,
                    _ => 0.0,
                };
                214.0 + extra
            }
        };
        // Never let a popup taller than the overlay surface.
        (w, h.min(900.0))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WheelAction {
    None,
    Volume,
    Brightness,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum HitTarget {
    BarPopup(PopupKind),
    BarWorkspace(u32),
    IslandClose,
    FilesNavBack,
    FilesNavFwd,
    FilesNavUp,
    FilesNavHome,
    FilesRefresh,
    FilesViewModeToggle,
    FilesBreadcrumb(PathBuf),
    FilesFocusFilter,
    FilesToggleHidden,
    FilesTogglePin,
    FilesPopOutFilesR,
    FilesPortalAccept,
    FilesPortalCancel,
    FilesPlace(PathBuf),
    FilesDrive(PathBuf),
    FilesUnpin(String),
    FilesQuickAccessDrop,
    BarTray(usize),
    TrayMenuClick(i32),
    FilesRow(usize),
    FilesAction(&'static str),
    FilesSortCycle,
    FilesSortColumn(SortBy),
    FilesEditPath,
    FilesNewFolder,
    FilesNewFile,
    FilesPromptInput,
    FilesPromptConfirm,
    FilesPromptCancel,
    FilesEmptyTrash,
    FilesContextMenuAction(&'static str),
    FilesListArea,
    LauncherTile(usize),
    ControlsMuteToggle,
    ControlsVolSlider(f64, f64),
    ControlsBrightSlider(f64, f64),
    VolumeMute,
    VolumeSlider(f64, f64),
    VolumeOutput(usize),
    BrightnessSlider(f64, f64),
    PowerAction(&'static str),
    MediaPlay,
    MediaPrev,
    MediaNext,
    MediaSeek(f64, f64),
    MediaRew,
    MediaFwd,
    MediaRaise,
    MediaPlayerChip(usize),
    WifiToggle,
    WifiRescan,
    WifiRow(usize),
    WifiPassInput,
    WifiPassConnect,
    WifiPassClose,
    BtToggle,
    BtScan,
    BtPairRow(usize),
    BtPairedRow(usize),
    BtForget(usize),
    NotifClear,
    NotifDnd,
    NotifDismiss(usize),
    SettingsTab(usize),
    SettingsCalPrev,
    SettingsCalNext,
    SettingsCalToday,
    SettingsCalDay(i32),
    SettingsToggle24h,
    SettingsBarPos(&'static str),
    SettingsToggleFloating,
    SettingsToggleRam,
    SettingsToggleMedia,
    SettingsToggleTray,
    SettingsBarSize(f64, f64),
    SettingsCorner(f64, f64),
    SettingsTheme(usize),
    SettingsRefresh,
    SettingsPower(&'static str),
    SettingsOpenPower,
    SettingsBgTarget(&'static str),
    SettingsThemeDropdown,
    SettingsThemePick(&'static str),
    SettingsAccent(&'static str),
    SettingsBarPosDropdown,
    SettingsBarPosPick(&'static str),
    SettingsBarOpacity(f64, f64),
    SettingsPopupOpacity(f64, f64),
    SettingsRandomWallpaper,
    SettingsBrowseWallpaper,
    SettingsWallpaperMode(&'static str),
    SettingsWallpaperDir(&'static str),
    SettingsWallpaperPick(usize),
    SettingsBrowseLockWallpaper,
    SettingsToggleLockSame,
    SettingsToggleLockClock,
    SettingsLockDim(f64, f64),
    SettingsLockNow,
    SettingsSyncSddm,
    SettingsBrowseSddmWallpaper,
    SettingsMangoToggle(&'static str),
    SettingsMangoReload,
    SettingsOpenLotus,
    SettingsDisplayDropdown(&'static str),
    SettingsDisplayModePick(&'static str),
    SettingsDisplayScalePick(&'static str),
    SettingsDisplayRotPick(&'static str),
    SettingsDisplayApply,
    AgyEffort(usize),
    AgyCycleModel,
    AgyNewChat,
    AgySend,
    CcExpand(&'static str),
}

#[derive(Clone, Debug)]
pub struct HitBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub target: HitTarget,
    pub wheel: WheelAction,
}

impl HitBox {
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

pub fn rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 1.5 * PI);
    cr.close_path();
}

pub fn dynamic_island_path(cr: &Context, bx: f64, by: f64, bw: f64, bh: f64, ear: f64, bot_r: f64) {
    let r = bot_r.min(bw / 2.0).min((bh - 1.0).max(1.0)).max(4.0);
    let e = ear.min(bh / 2.0).max(0.0);
    let rx = bx + bw;
    let bot = by + bh;

    cr.new_path();
    cr.move_to(bx - e, by);
    cr.arc(bx - e, by + e, e, -PI / 2.0, 0.0);
    cr.line_to(bx, bot - r);
    cr.arc_negative(bx + r, bot - r, r, PI, PI / 2.0);
    cr.line_to(rx - r, bot);
    cr.arc_negative(rx - r, bot - r, r, PI / 2.0, 0.0);
    cr.line_to(rx, by + e);
    cr.arc(rx + e, by + e, e, PI, 1.5 * PI);
    cr.close_path();
}

pub(crate) fn draw_text(cr: &Context, text: &str, x: f64, y: f64, size: f64, bold: bool, col: Rgb, alpha: f64) {
    cr.select_font_face(
        "JetBrainsMono Nerd Font",
        FontSlant::Normal,
        if bold { FontWeight::Bold } else { FontWeight::Normal },
    );
    cr.set_font_size(size);
    cr.set_source_rgba(col.0, col.1, col.2, alpha);
    cr.move_to(x, y);
    let _ = cr.show_text(text);
}

pub(crate) fn draw_glyph_centered(cr: &Context, glyph: &str, bx: f64, by: f64, bw: f64, bh: f64, size: f64, col: Rgb, alpha: f64) {
    cr.select_font_face("JetBrainsMono Nerd Font", FontSlant::Normal, FontWeight::Normal);
    cr.set_font_size(size);
    cr.set_source_rgba(col.0, col.1, col.2, alpha);
    if let Ok(ext) = cr.text_extents(glyph) {
        let dx = bx + (bw - ext.width()) / 2.0 - ext.x_bearing();
        let dy = by + (bh - ext.height()) / 2.0 - ext.y_bearing();
        cr.move_to(dx, dy);
    } else {
        cr.move_to(bx + bw * 0.25, by + bh * 0.72);
    }
    let _ = cr.show_text(glyph);
}

pub(crate) fn draw_surface_fitted(cr: &Context, surf: &ImageSurface, bx: f64, by: f64, bw: f64, bh: f64, alpha: f64) {
    let sw = surf.width() as f64;
    let sh = surf.height() as f64;
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }
    let scale = (bw / sw).min(bh / sh);
    let dw = sw * scale;
    let dh = sh * scale;
    let dx = bx + (bw - dw) / 2.0;
    let dy = by + (bh - dh) / 2.0;
    cr.save().ok();
    cr.translate(dx, dy);
    cr.scale(scale, scale);
    let _ = cr.set_source_surface(surf, 0.0, 0.0);
    let _ = cr.paint_with_alpha(alpha);
    cr.restore().ok();
}

pub(crate) fn text_width(cr: &Context, text: &str, size: f64, bold: bool) -> f64 {
    cr.select_font_face(
        "JetBrainsMono Nerd Font",
        FontSlant::Normal,
        if bold { FontWeight::Bold } else { FontWeight::Normal },
    );
    cr.set_font_size(size);
    cr.text_extents(text).map(|e| e.x_advance()).unwrap_or(0.0)
}

#[allow(dead_code)]
pub(crate) fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BarPillVisual {
    pub slot: f64,
    pub pill_w: f64,
    pub pill_h: f64,
    pub alpha: f64,
}

fn font_height(cr: &Context, size: f64) -> f64 {
    cr.set_font_size(size);
    cr.text_extents("Ag")
        .map(|e| e.height())
        .unwrap_or(size)
        .max(size * 0.9)
}

pub(crate) fn fit_text(cr: &Context, text: &str, size: f64, bold: bool, max_w: f64) -> String {
    cr.select_font_face(
        "JetBrainsMono Nerd Font",
        FontSlant::Normal,
        if bold { FontWeight::Bold } else { FontWeight::Normal },
    );
    cr.set_font_size(size);
    if text_width(cr, text, size, bold) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 0 {
        let s: String = chars[..n].iter().collect::<String>() + "…";
        if text_width(cr, &s, size, bold) <= max_w {
            return s;
        }
        n -= 1;
    }
    "…".to_string()
}

#[allow(clippy::too_many_arguments)]
pub fn draw_bar(
    cr: &Context,
    w: f64,
    h: f64,
    cfg: &ShellConfig,
    t: &Theme,
    stats: &SysStats,
    active_popup: PopupKind,
    cc_expanded: &str,
    hover: &Option<HitTarget>,
    yuno_icon: Option<&ImageSurface>,
    tray_icons: &HashMap<String, ImageSurface>,
    pills: &[BarPillVisual],
    media_phase: f64,
) -> Vec<HitBox> {
    let mut hits = Vec::new();
    let vertical = cfg.bar_position == "left" || cfg.bar_position == "right";

    cr.set_operator(Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    let _ = cr.paint();
    cr.set_operator(Operator::Over);
    if let Ok(mut fo) = cairo::FontOptions::new() {
        fo.set_antialias(cairo::Antialias::Gray);
        fo.set_hint_style(cairo::HintStyle::Slight);
        fo.set_hint_metrics(cairo::HintMetrics::Off);
        cr.set_font_options(&fo);
    }

    // ---- Bar background (identical to shell.qml barContent) ----
    let pad = if cfg.bar_floating { 5.0 } else { 0.0 };
    let bx = pad;
    let by = pad;
    let bw = (w - 2.0 * pad).max(10.0);
    let bh = (h - 2.0 * pad).max(10.0);
    let radius = if cfg.bar_floating { cfg.corner_radius.max(11.0) } else { cfg.corner_radius };

    rounded_rect(cr, bx, by, bw, bh, radius);
    cr.set_source_rgba(t.bg.0, t.bg.1, t.bg.2, cfg.bar_opacity);
    let _ = cr.fill();

    if cfg.bar_floating || radius > 0.0 {
        rounded_rect(cr, bx + 0.5, by + 0.5, bw - 1.0, bh - 1.0, radius);
        cr.set_source_rgba(t.border.0, t.border.1, t.border.2, 0.9);
        cr.set_line_width(1.0);
        let _ = cr.stroke();
    } else {
        // Edge subtle 1px divider (docked square mode only)
        cr.set_source_rgba(t.border.0, t.border.1, t.border.2, 0.9);
        cr.set_line_width(1.0);
        if cfg.bar_position == "left" {
            cr.move_to(w - 0.5, 0.0);
            cr.line_to(w - 0.5, h);
        } else if cfg.bar_position == "right" {
            cr.move_to(0.5, 0.0);
            cr.line_to(0.5, h);
        } else if cfg.bar_position == "bottom" {
            cr.move_to(0.0, 0.5);
            cr.line_to(w, 0.5);
        } else {
            cr.move_to(0.0, h - 0.5);
            cr.line_to(w, h - 0.5);
        }
        let _ = cr.stroke();
    }

    // ---- BarIcon: visible hover background + 0.88 -> 1.0 opacity ----
    let icon_opacity = |target: &HitTarget, active: bool| {
        if active || hover.as_ref() == Some(target) {
            1.0
        } else {
            0.88
        }
    };
    let draw_icon = |hits: &mut Vec<HitBox>,
                     x: f64,
                     y: f64,
                     iw: f64,
                     ih: f64,
                     glyph: &str,
                     img: Option<&ImageSurface>,
                     active: bool,
                     target: HitTarget,
                     wheel: WheelAction| {
        let is_hov = hover.as_ref() == Some(&target);
        if active {
            fill_rr(cr, x + 2.0, y + 3.0, iw - 4.0, (ih - 6.0).max(16.0), (cfg.corner_radius as f64).min(8.0), t.surface_active, 0.92);
        } else if is_hov {
            fill_rr(cr, x + 2.0, y + 3.0, iw - 4.0, (ih - 6.0).max(16.0), (cfg.corner_radius as f64).min(8.0), t.surface_hover, 0.92);
        }
        let alpha = icon_opacity(&target, active);
        if let Some(s) = img {
            let max_sz: f64 = if iw <= 24.5 && ih <= 24.5 { 16.0 } else { 23.0 };
            let sz = max_sz.min(ih.max(8.0)).min(iw.max(8.0));
            draw_surface_fitted(cr, s, x + (iw - sz) / 2.0, y + (ih - sz) / 2.0, sz, sz, alpha);
        } else {
            draw_glyph_centered(cr, glyph, x, y, iw, ih, 14.0, if active { t.accent } else { t.text }, alpha);
        }
        hits.push(HitBox { x, y, w: iw, h: ih, target, wheel });
    };

    let bar_span = cfg.bar_size as f64;
    let icon_h = (font_height(cr, 14.0) + 10.0).max(28.0);

    // Glyph widths drive horizontal BarIcon widths (max(28, row.width + 12)).
    let gw = |g: &str| (text_width(cr, g, 14.0, false) + 12.0).max(28.0);

    let battery_icon = if stats.battery_charging {
        "󰂄"
    } else if stats.battery_percent >= 90 {
        "󰁹"
    } else if stats.battery_percent >= 75 {
        "󰂁"
    } else if stats.battery_percent >= 55 {
        "󰁿"
    } else if stats.battery_percent >= 35 {
        "󰁽"
    } else if stats.battery_percent >= 15 {
        "󰁻"
    } else {
        "󰁺"
    };
    let wifi_icon = if !stats.wifi_enabled {
        "󰤭"
    } else if stats.wifi_ssid.is_empty() {
        "󰤯"
    } else if stats.wifi_signal >= 75 {
        "󰤨"
    } else if stats.wifi_signal >= 50 {
        "󰤥"
    } else if stats.wifi_signal >= 25 {
        "󰤢"
    } else {
        "󰤟"
    };
    let bt_icon = if !stats.bt_powered {
        "󰂲"
    } else if stats.bt_connected > 0 {
        "󰂱"
    } else {
        "󰂯"
    };
    let vol_icon = if stats.volume_muted || stats.volume_percent == 0 {
        "󰝟"
    } else if stats.volume_percent >= 60 {
        "󰕾"
    } else {
        "󰖀"
    };

    // Widget order matches shell.qml exactly.
    let has_media = cfg.show_media_on_bar && stats.media.active;

    if vertical {
        let cx = |iw: f64| bx + ((bw - iw) / 2.0).round();

        // LEFT / TOP cluster: Launcher + Files + Workspaces + RAM
        let mut y = by + 8.0;
        draw_icon(
            &mut hits,
            cx(bar_span),
            y,
            bar_span,
            33.0f64.max(icon_h),
            "󰀻",
            yuno_icon,
            active_popup == PopupKind::Antigravity,
            HitTarget::BarPopup(PopupKind::Antigravity),
            WheelAction::None,
        );
        y += 33.0f64.max(icon_h) + 6.0;
        draw_icon(
            &mut hits,
            cx(bar_span),
            y,
            bar_span,
            icon_h,
            "󰉋",
            None,
            active_popup == PopupKind::Files,
            HitTarget::BarPopup(PopupKind::Files),
            WheelAction::None,
        );
        y += icon_h + 6.0;

        // WorkspaceBar: container = sum(slot) + 6*(n-1) + 10
        if !pills.is_empty() {
            let n = pills.len() as f64;
            let ws_h: f64 = pills.iter().map(|p| p.slot).sum::<f64>() + 6.0 * (n - 1.0) + 10.0;
            let mut py = y + 5.0;
            for (i, pv) in pills.iter().enumerate() {
                let target = HitTarget::BarWorkspace(i as u32 + 1);
                let slot = pv.slot.max(2.0);
                rounded_rect(
                    cr,
                    cx(bar_span) + (bar_span - pv.pill_w) / 2.0,
                    py + (slot - pv.pill_h) / 2.0,
                    pv.pill_w,
                    pv.pill_h,
                    pv.pill_w.min(pv.pill_h) / 2.0,
                );
                cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, pv.alpha);
                let _ = cr.fill();
                hits.push(HitBox {
                    x: bx + 2.0,
                    y: py,
                    w: bw - 4.0,
                    h: slot,
                    target,
                    wheel: WheelAction::None,
                });
                py += slot + 6.0;
            }
            y += ws_h + 6.0;
        }

        if cfg.show_ram_on_bar {
            let ram_s = format!("{}%", stats.ram_percent);
            let rh = font_height(cr, 10.0) + 8.0;
            draw_text(
                cr,
                &ram_s,
                bx + (bw - text_width(cr, &ram_s, 10.0, false)) / 2.0,
                y + rh / 2.0 + 3.5,
                10.0,
                false,
                t.text,
                1.0,
            );
        }

        // CENTER cluster: Clock + Media pill
        let (hh, mm, _, _) = local_clock_strings(cfg.lock_24_hour);
        let hh_h = font_height(cr, 12.0);
        let mm_h = font_height(cr, 11.0);
        let clock_h = hh_h + mm_h + 10.0;
        let media_h = if has_media { 26.0 } else { 0.0 };
        let center_h = clock_h + if has_media { 8.0 + media_h } else { 0.0 };
        let mut cy = by + ((bh - center_h) / 2.0).round();
        let clock_target = HitTarget::BarPopup(PopupKind::Settings);
        let clock_active = active_popup == PopupKind::Settings;
        let tc = if clock_active { t.accent } else { t.text_bright };
        let hh_w = text_width(cr, &hh, 12.0, true);
        let mm_w = text_width(cr, &mm, 11.0, false);
        draw_text(cr, &hh, bx + ((bw - hh_w) / 2.0).round(), cy + hh_h, 12.0, true, tc, 1.0);
        draw_text(
            cr,
            &mm,
            bx + ((bw - mm_w) / 2.0).round(),
            cy + hh_h + mm_h,
            11.0,
            false,
            t.muted,
            1.0,
        );
        hits.push(HitBox {
            x: bx + 2.0,
            y: cy,
            w: bw - 4.0,
            h: clock_h,
            target: clock_target,
            wheel: WheelAction::None,
        });
        cy += clock_h + 8.0;

        if has_media {
            let mx = cx(26.0);
            let active = active_popup == PopupKind::Media;
            rounded_rect(cr, mx, cy, 26.0, 26.0, 12.0);
            cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 1.0);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(t.border.0, t.border.1, t.border.2, if active { 1.0 } else { 0.7 });
            cr.set_line_width(1.0);
            let _ = cr.stroke();
            draw_glyph_centered(cr, "󰎆", mx, cy, 26.0, 26.0, 10.0, t.text, 1.0);
            hits.push(HitBox {
                x: mx,
                y: cy,
                w: 26.0,
                h: 26.0,
                target: HitTarget::BarPopup(PopupKind::Media),
                wheel: WheelAction::None,
            });
        }

        let is_cc_item_active = |pk: PopupKind, sec: &str| -> bool {
            if active_popup == pk {
                return true;
            }
            if active_popup == PopupKind::Controls {
                return cc_expanded == sec;
            }
            false
        };

        // RIGHT / BOTTOM cluster: Tray + Bluetooth + Brightness + Volume + Battery + Wi-Fi + Power
        let tray_n = if cfg.show_tray_on_bar { stats.tray_items.len().min(6) } else { 0 };
        let mut items: Vec<(&str, HitTarget, WheelAction, bool, bool)> = Vec::new();
        items.push((bt_icon, HitTarget::BarPopup(PopupKind::Bluetooth), WheelAction::None, is_cc_item_active(PopupKind::Bluetooth, "bt"), true));
        items.push(("󰃠", HitTarget::BarPopup(PopupKind::Brightness), WheelAction::Brightness, is_cc_item_active(PopupKind::Brightness, "bright"), true));
        items.push((vol_icon, HitTarget::BarPopup(PopupKind::Volume), WheelAction::Volume, is_cc_item_active(PopupKind::Volume, "audio"), true));
        if stats.battery_present {
            items.push((battery_icon, HitTarget::BarPopup(PopupKind::Battery), WheelAction::None, is_cc_item_active(PopupKind::Battery, "battery"), true));
        }
        items.push((wifi_icon, HitTarget::BarPopup(PopupKind::Wifi), WheelAction::None, is_cc_item_active(PopupKind::Wifi, "wifi"), true));
        items.push(("󰐥", HitTarget::BarPopup(PopupKind::Power), WheelAction::None, is_cc_item_active(PopupKind::Power, "power"), true));

        let tray_slot_h = 22.0;
        let tray_block_h = if tray_n > 0 { tray_n as f64 * (tray_slot_h + 2.0) + 4.0 } else { 0.0 };
        let total_h = items.len() as f64 * icon_h + (items.len() as f64 - 1.0) * 2.0 + tray_block_h;
        let mut ry = by + bh - 6.0 - total_h;
        if tray_n > 0 {
            for (ti, item) in stats.tray_items.iter().take(6).enumerate() {
                let tgt = HitTarget::BarTray(ti);
                let act = active_popup == PopupKind::TrayMenu && stats.tray_menu_key == item.key;
                let tx = cx(bar_span);
                let img = tray_icons.get(&item.icon_png);
                draw_icon(&mut hits, tx, ry, bar_span, tray_slot_h, "󰀻", img, act, tgt, WheelAction::None);
                ry += tray_slot_h + 2.0;
            }
            ry += 4.0;
        }
        for (glyph, target, wheel, active, _) in items {
            draw_icon(&mut hits, cx(bar_span), ry, bar_span, icon_h, glyph, None, active, target, wheel);
            ry += icon_h + 2.0;
        }
    } else {
        // LEFT cluster
        let mut x = bx + 8.0;
        draw_icon(
            &mut hits,
            x,
            by + ((bh - bar_span) / 2.0).round(),
            35.0,
            bar_span,
            "󰀻",
            yuno_icon,
            active_popup == PopupKind::Antigravity,
            HitTarget::BarPopup(PopupKind::Antigravity),
            WheelAction::None,
        );
        x += 35.0 + 6.0;
        let files_w = gw("󰉋");
        draw_icon(
            &mut hits,
            x,
            by + ((bh - bar_span) / 2.0).round(),
            files_w,
            bar_span,
            "󰉋",
            None,
            active_popup == PopupKind::Files,
            HitTarget::BarPopup(PopupKind::Files),
            WheelAction::None,
        );
        x += files_w + 6.0;

        if !pills.is_empty() {
            let n = pills.len() as f64;
            let ws_w: f64 = pills.iter().map(|p| p.slot).sum::<f64>() + 6.0 * (n - 1.0) + 10.0;
            let mut px = x + 5.0;
            for (i, pv) in pills.iter().enumerate() {
                let target = HitTarget::BarWorkspace(i as u32 + 1);
                let slot = pv.slot.max(2.0);
                rounded_rect(
                    cr,
                    px + (slot - pv.pill_w) / 2.0,
                    by + (bh - pv.pill_h) / 2.0,
                    pv.pill_w,
                    pv.pill_h,
                    pv.pill_w.min(pv.pill_h) / 2.0,
                );
                cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, pv.alpha);
                let _ = cr.fill();
                hits.push(HitBox {
                    x: px,
                    y: by + 2.0,
                    w: slot,
                    h: bh - 4.0,
                    target,
                    wheel: WheelAction::None,
                });
                px += slot + 6.0;
            }
            x += ws_w + 6.0;
        }

        if cfg.show_ram_on_bar {
            let ram_s = format!("{:.1}/{}", stats.ram_used_gb, stats.ram_total_gb.round() as u32);
            draw_text(cr, &ram_s, x, by + bh / 2.0 + 4.0, 11.0, false, t.text, 1.0);
        }

        // CENTER cluster
        let (time_full, date_str) = {
            let (_, _, tf, ds) = local_clock_strings(cfg.lock_24_hour);
            (tf, ds)
        };
        let tw1 = text_width(cr, &time_full, 12.0, true);
        let tw2 = text_width(cr, &date_str, 11.0, false);
        let clock_w = tw1 + 8.0 + tw2 + 16.0;
        let media_w = if has_media { 0.0 } else { 0.0 }; // computed below
        let media_w = if has_media {
            let title = if stats.media.title.is_empty() { &stats.media.player } else { &stats.media.title };
            let fitted = fit_text(cr, title, 11.0, false, 138.0);
            let ttl_w = text_width(cr, &fitted, 11.0, false);
            (16.0 + 6.0 + ttl_w + 6.0 + 10.0 + 16.0).min(206.0).max(0.0)
        } else {
            media_w
        };
        let center_w = clock_w + if has_media { 8.0 + media_w } else { 0.0 };
        let mut cxx = bx + ((bw - center_w) / 2.0).round();
        let clock_tgt = HitTarget::BarPopup(PopupKind::Settings);
        let clock_active = active_popup == PopupKind::Settings;
        let clock_hov = hover.as_ref() == Some(&clock_tgt);
        if clock_hov && !clock_active {
            fill_rr(cr, cxx - 8.0, by + 3.0, clock_w, (bh - 6.0).max(16.0), (cfg.corner_radius as f64).min(8.0), t.surface_hover, 0.85);
        }
        let tc = if clock_active { t.accent } else { t.text_bright };
        draw_text(cr, &time_full, cxx, by + bh / 2.0 + 4.0, 12.0, true, tc, 1.0);
        draw_text(cr, &date_str, cxx + tw1 + 8.0, by + bh / 2.0 + 4.0, 11.0, false, t.muted, 1.0);
        hits.push(HitBox {
            x: cxx - 8.0,
            y: by + 2.0,
            w: clock_w,
            h: bh - 4.0,
            target: clock_tgt,
            wheel: WheelAction::None,
        });
        cxx += clock_w + 8.0;

        if has_media {
            let my = by + ((bh - 24.0) / 2.0).round();
            let active = active_popup == PopupKind::Media;
            let hov = hover.as_ref() == Some(&HitTarget::BarPopup(PopupKind::Media));
            rounded_rect(cr, cxx, my, media_w, 24.0, 12.0);
            let surface = if active {
                t.surface_hover
            } else if hov {
                t.surface_hover
            } else {
                t.surface
            };
            cr.set_source_rgba(surface.0, surface.1, surface.2, 1.0);
            let _ = cr.fill_preserve();
            let bc = if active { t.accent } else { t.border };
            cr.set_source_rgba(bc.0, bc.1, bc.2, if active { 1.0 } else { 0.7 });
            cr.set_line_width(1.0);
            let _ = cr.stroke();

            rounded_rect(cr, cxx + 6.0, my + 4.0, 16.0, 16.0, 4.0);
            cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 1.0);
            let _ = cr.fill();
            draw_glyph_centered(cr, "󰎆", cxx + 6.0, my + 4.0, 16.0, 16.0, 10.0, t.text, 1.0);

            let title = if stats.media.title.is_empty() { &stats.media.player } else { &stats.media.title };
            let fitted = fit_text(cr, title, 11.0, false, 138.0);
            draw_text(cr, &fitted, cxx + 28.0, my + 16.0, 11.0, false, t.text_bright, 1.0);

            // Animated 3-bar mini equalizer
            let bars = [(4.0, 11.0, 0.32), (5.0, 12.0, 0.26), (3.0, 9.0, 0.38)];
            for (i, (lo, hi, dur)) in bars.iter().enumerate() {
                let bh_ = if stats.media.playing {
                    let phase = (media_phase / dur + i as f64 * 0.35) * std::f64::consts::TAU;
                    lo + (hi - lo) * (0.5 + 0.5 * phase.sin())
                } else {
                    4.0
                };
                let ex = cxx + media_w - 16.0 + i as f64 * 4.0;
                rounded_rect(cr, ex, my + 24.0 - 2.0 - bh_, 2.0, bh_, 1.0);
                let col = if stats.media.playing { t.accent } else { t.muted };
                cr.set_source_rgba(col.0, col.1, col.2, 1.0);
                let _ = cr.fill();
            }
            hits.push(HitBox {
                x: cxx,
                y: my,
                w: media_w,
                h: 24.0,
                target: HitTarget::BarPopup(PopupKind::Media),
                wheel: WheelAction::None,
            });
        }

        let is_cc_item_active = |pk: PopupKind, sec: &str| -> bool {
            if active_popup == pk {
                return true;
            }
            if active_popup == PopupKind::Controls {
                return cc_expanded == sec;
            }
            false
        };

        // RIGHT cluster
        let tray_n = if cfg.show_tray_on_bar { stats.tray_items.len().min(6) } else { 0 };
        let mut items: Vec<(&str, HitTarget, WheelAction, bool)> = Vec::new();
        items.push((bt_icon, HitTarget::BarPopup(PopupKind::Bluetooth), WheelAction::None, is_cc_item_active(PopupKind::Bluetooth, "bt")));
        items.push(("󰃠", HitTarget::BarPopup(PopupKind::Brightness), WheelAction::Brightness, is_cc_item_active(PopupKind::Brightness, "bright")));
        items.push((vol_icon, HitTarget::BarPopup(PopupKind::Volume), WheelAction::Volume, is_cc_item_active(PopupKind::Volume, "audio")));
        if stats.battery_present {
            items.push((battery_icon, HitTarget::BarPopup(PopupKind::Battery), WheelAction::None, is_cc_item_active(PopupKind::Battery, "battery")));
        }
        items.push((wifi_icon, HitTarget::BarPopup(PopupKind::Wifi), WheelAction::None, is_cc_item_active(PopupKind::Wifi, "wifi")));
        items.push(("󰐥", HitTarget::BarPopup(PopupKind::Power), WheelAction::None, is_cc_item_active(PopupKind::Power, "power")));

        let tray_slot_w = 24.0;
        let tray_block_w = if tray_n > 0 { tray_n as f64 * (tray_slot_w + 2.0) + 6.0 } else { 0.0 };
        let total_w: f64 = items.iter().map(|(g, ..)| gw(g)).sum::<f64>()
            + (items.len() as f64 - 1.0) * 2.0
            + tray_block_w;
        let mut rx = bx + bw - 6.0 - total_w;
        let iy = by + ((bh - bar_span) / 2.0).round();
        if tray_n > 0 {
            for (ti, item) in stats.tray_items.iter().take(6).enumerate() {
                let tgt = HitTarget::BarTray(ti);
                let act = active_popup == PopupKind::TrayMenu && stats.tray_menu_key == item.key;
                let img = tray_icons.get(&item.icon_png);
                draw_icon(&mut hits, rx, iy, tray_slot_w, bar_span, "󰀻", img, act, tgt, WheelAction::None);
                rx += tray_slot_w + 2.0;
            }
            rx += 6.0;
        }
        for (glyph, target, wheel, active) in items {
            let iw = gw(glyph);
            draw_icon(&mut hits, rx, iy, iw, bar_span, glyph, None, active, target, wheel);
            rx += iw + 2.0;
        }
    }

    hits
}

#[allow(dead_code)]
pub struct IslandRenderState<'a> {
    pub kind: PopupKind,
    pub cfg: &'a ShellConfig,
    pub theme: &'a Theme,
    pub stats: &'a SysStats,
    pub hover: &'a Option<HitTarget>,
    pub current_dir: &'a PathBuf,
    pub can_back: bool,
    pub can_fwd: bool,
    pub show_hidden: bool,
    pub filter_query: &'a str,
    pub filter_focused: bool,
    pub entries: &'a [FileItem],
    pub selected_idx: Option<usize>,
    pub scroll_y: f64,
    pub status_toast: &'a str,
    pub launcher_query: &'a str,
    pub filtered_apps: &'a [AppItem],
    pub launcher_idx: usize,
    pub launcher_scroll: f64,
    pub settings_tab: usize,
    pub settings_prev_tab: usize,
    pub settings_tab_pill_progress: f64,
    pub settings_tab_slide_x: f64,
    pub settings_bg_target: &'a str,
    pub settings_open_dropdown: &'a str,
    pub wallpapers: &'a [(String, PathBuf, Option<ImageSurface>)],
    pub current_preview_surf: Option<&'a ImageSurface>,
    pub wallpaper_prev_surf: Option<&'a ImageSurface>,
    pub wallpaper_preview_progress: f64,
    pub mango_info: &'a MangoSummary,
    pub cal_offset_months: i32,
    pub cal_selected_key: i32,
    pub agy_messages: &'a [AgyMsg],
    pub agy_input: &'a str,
    pub agy_busy: bool,
    pub agy_status: &'a str,
    pub agy_meta: &'a str,
    pub agy_model_idx: usize,
    pub agy_effort: usize,
    pub agy_scroll: f64,
    pub agy_cursor_on: bool,
    pub wifi_selected_ssid: &'a str,
    pub wifi_password: &'a str,
    pub wifi_scroll: f64,
    pub bt_scroll: f64,
    pub notif_scroll: f64,
    pub cc_expanded: &'a str,
    pub media_art_surf: Option<&'a ImageSurface>,
    pub settings_body_cache: Option<&'a ImageSurface>,
    pub selected_paths: &'a [PathBuf],
    pub files_sort_by: SortBy,
    pub files_sort_asc: bool,
    pub files_prompt: &'a FilesPrompt,
    pub files_prompt_text: &'a str,
    pub files_clip_count: usize,
    pub files_clip_cut: bool,
    pub files_preview_surf: Option<&'a ImageSurface>,
    pub files_preview_lines: &'a [String],
    pub files_preview_dir_count: Option<usize>,
    pub files_context_menu: Option<(f64, f64, Option<usize>)>,
    pub portal_info: Option<(&'a str, &'a str)>,
    pub files_view_mode: FileViewMode,
    pub files_drive_usages: &'a [DriveUsage],
    pub files_dnd_target: Option<&'a std::path::Path>,
    pub files_is_dragging: bool,
    pub files_drag_count: usize,
    pub files_drag_label: &'a str,
    pub files_drag_pos: (f64, f64),
    pub files_marquee_rect: Option<(f64, f64, f64, f64)>,
    pub files_thumbs: &'a HashMap<PathBuf, ImageSurface>,
    pub files_search_expand: f64,
    pub files_crumb_focus: f64,
    pub files_sidebar_pill_y: f64,
    pub files_sidebar_pill_r: f64,
    pub files_ctx_open: f64,
    pub files_action_slide: f64,
    pub files_qa_dnd_hover: bool,
    pub files_drag_in_cache: bool,
    pub pressed_target: &'a Option<HitTarget>,
}

/// Renders ONLY the inner popup UI into an offscreen surface + builds its hitboxes (offset to canvas coordinates).
pub fn render_popup_content(
    cr: &Context,
    origin_x: f64,
    origin_y: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
) -> Vec<HitBox> {
    let mut hits = Vec::new();
    cr.set_operator(Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    let _ = cr.paint();
    cr.set_operator(Operator::Over);
    if let Ok(mut fo) = cairo::FontOptions::new() {
        fo.set_antialias(cairo::Antialias::Gray);
        fo.set_hint_style(cairo::HintStyle::Slight);
        fo.set_hint_metrics(cairo::HintMetrics::Off);
        cr.set_font_options(&fo);
    }

    match st.kind {
        PopupKind::Files => crate::files_ui::draw_files_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Launcher => draw_launcher_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Brightness => draw_brightness_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Power => draw_power_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Battery => draw_battery_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Volume => draw_volume_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Media => draw_media_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Wifi => draw_wifi_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Bluetooth => draw_bluetooth_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Notifications => draw_notifications_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Settings => draw_settings_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Antigravity => draw_antigravity_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::Controls => draw_controls_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::TrayMenu => draw_tray_menu_popup(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        PopupKind::None => {}
    }

    for hb in &mut hits {
        hb.x += origin_x;
        hb.y += origin_y;
    }
    hits
}

/// Ultra-fast 60fps compositor pass: draws ONLY the Dynamic Island shell and blits the cached content surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct IslandFrameLayout {
    pub body_x: f64,
    pub body_y: f64,
    pub body_w: f64,
    pub body_h: f64,
    pub vertical: bool,
    pub invert_x: bool,
    pub invert_y: bool,
    pub content_x: f64,
    pub content_y: f64,
    pub slide_x: f64,
    pub slide_y: f64,
}

fn horiz_island_path(cr: &Context, bx: f64, by: f64, bw: f64, bh: f64, ear: f64, bot_r: f64, invert_y: bool) {
    cr.save().ok();
    if invert_y {
        cr.translate(0.0, 2.0 * by + bh);
        cr.scale(1.0, -1.0);
    }
    dynamic_island_path(cr, bx, by, bw, bh, ear, bot_r);
    cr.restore().ok();
}

fn vertical_island_path(cr: &Context, bx: f64, by: f64, bw: f64, bh: f64, invert_x: bool) {
    let ear = 16.0f64.min(bh * 0.5).max(0.0);
    let r = (bw * 0.48).min(bh * 0.62).min(24.0).max(0.5);
    cr.save().ok();
    if invert_x {
        cr.translate(2.0 * bx + bw, 0.0);
        cr.scale(-1.0, 1.0);
    }
    cr.new_path();
    cr.move_to(bx, by - ear);
    cr.curve_to(bx, by - ear + 10.56, bx + 5.44, by, bx + 16.0, by);
    cr.line_to(bx + bw - r, by);
    cr.arc(bx + bw - r, by + r, r, -PI / 2.0, 0.0);
    cr.line_to(bx + bw, by + bh - r);
    cr.arc(bx + bw - r, by + bh - r, r, 0.0, PI / 2.0);
    cr.line_to(bx + 16.0, by + bh);
    cr.curve_to(bx + 5.44, by + bh, bx, by + bh + 10.56, bx, by + bh + ear);
    cr.close_path();
    cr.restore().ok();
}

/// Compositor pass: draws the Dynamic Island shell (horizontal or bar-attached
/// vertical) and blits the cached popup content inside it.
pub fn draw_island_frame(
    cr: &Context,
    st: &IslandRenderState<'_>,
    l: &IslandFrameLayout,
    content_cache: Option<&ImageSurface>,
    content_alpha: f64,
) -> (f64, f64, f64, f64) {
    cr.save().ok();
    cr.set_operator(Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    cr.rectangle(0.0, 0.0, l.body_x + l.body_w + 36.0, l.body_y + l.body_h + 36.0);
    let _ = cr.fill();
    cr.restore().ok();
    cr.set_operator(Operator::Over);

    if st.kind == PopupKind::None || l.body_h < 0.5 || l.body_w < 0.5 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    let bx = l.body_x;
    let by = l.body_y;
    let bw = l.body_w;
    let bh = l.body_h;

    cr.save().ok();
    if l.vertical {
        vertical_island_path(cr, bx, by, bw, bh, l.invert_x);
    } else {
        let ear = 16.0f64.min(bh * 0.34);
        horiz_island_path(cr, bx, by, bw, bh, ear, st.cfg.corner_radius.max(20.0), l.invert_y);
    }
    cr.set_source_rgba(st.theme.bg.0, st.theme.bg.1, st.theme.bg.2, st.cfg.popup_opacity);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(st.theme.border.0, st.theme.border.1, st.theme.border.2, 0.85);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    if content_alpha > 0.02 {
        if let Some(cache) = content_cache {
            if l.vertical {
                vertical_island_path(cr, bx, by, bw, bh, l.invert_x);
            } else {
                let ear = 16.0f64.min(bh * 0.34);
                horiz_island_path(cr, bx, by, bw, bh, ear, st.cfg.corner_radius.max(20.0), l.invert_y);
            }
            cr.clip();
            let sx = ((l.content_x + l.slide_x) * 2.0).round() / 2.0;
            let sy = ((l.content_y + l.slide_y) * 2.0).round() / 2.0;
            let _ = cr.set_source_surface(cache, sx, sy);
            let _ = cr.paint_with_alpha(content_alpha);
        }
    }
    cr.restore().ok();

    if l.vertical {
        (bx, by - 16.0, bw, bh + 32.0)
    } else {
        (bx - 16.0, by, bw + 32.0, bh)
    }
}

fn draw_launcher_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    rounded_rect(cr, cx, cy, cw, 36.0, 11.0);
    cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 1.0);
    let _ = cr.fill_preserve();
    if st.launcher_query.is_empty() {
        cr.set_source_rgba(t.border.0, t.border.1, t.border.2, 0.55);
    } else {
        cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.45);
    }
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    let icon_col = if st.launcher_query.is_empty() { t.muted } else { t.accent };
    draw_glyph_centered(cr, "󰍉", cx + 8.0, cy, 22.0, 36.0, 15.0, icon_col, 1.0);
    let q_txt = if st.launcher_query.is_empty() { "Search apps..." } else { st.launcher_query };
    text_left_v(
        cr,
        q_txt,
        cx + 36.0,
        cy,
        36.0,
        12.0,
        false,
        if st.launcher_query.is_empty() { t.muted } else { t.text_bright },
        1.0,
    );
    if let Some(sel) = st.filtered_apps.get(st.launcher_idx) {
        let lbl = truncate_chars(&sel.name, 22);
        let tw = (text_width(cr, &lbl, 11.0, true) + 16.0).min(188.0);
        let bx = cx + cw - tw - 8.0;
        let by = cy + 7.0;
        rounded_rect(cr, bx, by, tw, 22.0, 7.0);
        cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.20);
        let _ = cr.fill();
        text_center_v(cr, &lbl, bx + tw / 2.0, by, 22.0, 11.0, true, t.accent, 1.0);
    }

    let gy = cy + 44.0;
    let gh = (ch - 44.0).max(16.0);
    let cols = 8usize;
    let grid_w = (cw - 6.0).max(1.0);
    let cell_w = (grid_w / cols as f64).floor();
    let cell_h = 72.0;

    cr.save().ok();
    rounded_rect(cr, cx, gy, cw, gh, 8.0);
    cr.clip();

    if st.filtered_apps.is_empty() {
        text_center_v(cr, "󰀻  No matching application", cx + cw / 2.0, gy, gh, 12.0, false, t.muted, 1.0);
    } else {
        for (idx, app) in st.filtered_apps.iter().enumerate() {
            let col_i = idx % cols;
            let row_i = idx / cols;
            let tx = cx + (col_i as f64) * cell_w;
            let ty = gy + (row_i as f64) * cell_h - st.launcher_scroll;
            if ty + cell_h < gy || ty > gy + gh {
                continue;
            }
            let tgt = HitTarget::LauncherTile(idx);
            let sel = idx == st.launcher_idx;
            let hov = st.hover.as_ref() == Some(&tgt);
            let active = sel || hov;
            let icon_sz = if active { 50.0 } else { 44.0 };
            let icon_x = tx + (cell_w - icon_sz) / 2.0;
            let icon_y = ty + (cell_h - icon_sz) / 2.0 - 4.0 - if active { 2.0 } else { 0.0 };

            if let Some(ref surf) = app.icon_surface {
                draw_surface_fitted(cr, surf, icon_x, icon_y, icon_sz, icon_sz, 1.0);
            } else {
                let badge_cols = [t.blue, t.purple, t.cyan, t.green, t.orange, t.yellow, t.red, t.accent];
                let bcol = badge_cols[idx % badge_cols.len()];
                rounded_rect(cr, icon_x, icon_y, icon_sz, icon_sz, 12.0);
                cr.set_source_rgba(bcol.0, bcol.1, bcol.2, if active { 0.28 } else { 0.16 });
                let _ = cr.fill();
                let ch_icon: String = app.name.chars().take(1).collect::<String>().to_uppercase();
                draw_glyph_centered(cr, &ch_icon, icon_x, icon_y, icon_sz, icon_sz, 16.0, bcol, 1.0);
            }

            if sel {
                rounded_rect(cr, tx + (cell_w - 16.0) / 2.0, ty + cell_h - 9.0, 16.0, 3.0, 1.5);
                cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 1.0);
                let _ = cr.fill();
            }
            hits.push(HitBox { x: tx, y: ty.max(gy), w: cell_w, h: cell_h, target: tgt, wheel: WheelAction::None });
        }
    }
    cr.restore().ok();
}

const SETTINGS_RAIL_W: f64 = 156.0;

fn hairline_h(cr: &Context, x: f64, y: f64, w: f64, t: &Theme, alpha: f64) {
    cr.set_source_rgba(t.border.0, t.border.1, t.border.2, alpha);
    cr.rectangle(x, y, w, 1.0);
    let _ = cr.fill();
}

fn hairline_v(cr: &Context, x: f64, y: f64, h: f64, t: &Theme, alpha: f64) {
    cr.set_source_rgba(t.border.0, t.border.1, t.border.2, alpha);
    cr.rectangle(x, y, 1.0, h);
    let _ = cr.fill();
}

fn compact_switch(cr: &Context, x: f64, y: f64, on: bool, t: &Theme) {
    let (w, h) = (32.0, 18.0);
    let r = 9.0;
    let track_bg = if on { t.accent } else { t.surface_hover };
    fill_rr(cr, x, y, w, h, r, track_bg, 1.0);
    let thumb_d = 12.0;
    let thumb_x = if on { x + w - thumb_d - 3.0 } else { x + 3.0 };
    let thumb_y = y + (h - thumb_d) / 2.0;
    let thumb_col = if on { t.accent_text } else { t.muted };
    fill_rr(cr, thumb_x, thumb_y, thumb_d, thumb_d, thumb_d / 2.0, thumb_col, 1.0);
}

#[allow(clippy::too_many_arguments)]
fn tonal_pill(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    icon: &str,
    label: &str,
    active: bool,
    hovered: bool,
    danger: bool,
    busy: bool,
    t: &Theme,
    target: HitTarget,
    hits: &mut Vec<HitBox>,
) {
    let r = 7.0;
    let opacity = if busy { 0.6 } else { 1.0 };
    let bg = if active {
        t.accent
    } else if danger && hovered {
        Rgb(0.30, 0.18, 0.21)
    } else if hovered {
        t.surface_hover
    } else {
        t.surface
    };
    fill_rr(cr, x, y, w, h, r, bg, if active || hovered { opacity } else { 0.72 * opacity });

    let fg_icon = if active {
        t.accent_text
    } else if danger && hovered {
        t.red
    } else if danger {
        t.muted
    } else {
        t.accent
    };
    let fg_lbl = if active {
        t.accent_text
    } else if danger && hovered {
        t.red
    } else if hovered {
        t.text_bright
    } else {
        t.text
    };
    let iw = if icon.is_empty() { 0.0 } else { text_width(cr, icon, 11.5, false) };
    let lw = if label.is_empty() { 0.0 } else { text_width(cr, label, 11.0, active) };
    let gap = if iw > 0.0 && lw > 0.0 { 5.0 } else { 0.0 };
    let mut tx = x + (w - iw - lw - gap) / 2.0;
    if iw > 0.0 {
        draw_text(cr, icon, tx, y + h / 2.0 + 3.8, 11.5, false, fg_icon, opacity);
        tx += iw + gap;
    }
    if lw > 0.0 {
        draw_text(cr, label, tx, y + h / 2.0 + 3.8, 11.0, active, fg_lbl, opacity);
    }
    hits.push(HitBox { x, y, w, h, target, wheel: WheelAction::None });
}

#[allow(clippy::too_many_arguments)]
fn slim_slider(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    label: &str,
    val_text: &str,
    value: f64,
    min: f64,
    max: f64,
    icon: &str,
    t: &Theme,
) -> (f64, f64) {
    // Row header: label left, tabular value right
    text_left_v(cr, label, x, y, 14.0, 11.0, false, t.text, 1.0);
    text_right_v(cr, val_text, x + w, y, 14.0, 11.0, true, t.text_bright, 1.0);

    let track_y = y + 18.0;
    let icon_w = if icon.is_empty() { 0.0 } else { 18.0 };
    if !icon.is_empty() {
        draw_glyph_centered(cr, icon, x, track_y, 16.0, 18.0, 12.5, t.accent, 1.0);
    }
    let sx = x + icon_w;
    let sw = (w - icon_w).max(24.0);
    let frac = ((value - min) / (max - min).max(1.0)).clamp(0.0, 1.0);
    let thumb = 11.0;
    let cx = sx + thumb / 2.0 + (sw - thumb).max(1.0) * frac;
    let bar_y = track_y + 7.0;
    fill_rr(cr, sx, bar_y, sw, 4.0, 2.0, t.bg_deep, 0.9);
    fill_rr(cr, sx, bar_y, (cx - sx).max(4.0).min(sw), 4.0, 2.0, t.accent, 1.0);
    fill_rr(cr, (cx - thumb / 2.0).clamp(sx, sx + sw - thumb), track_y + 3.5, thumb, thumb, thumb / 2.0, t.text_bright, 1.0);
    (sx, sw)
}

#[allow(clippy::too_many_arguments)]
fn compact_dropdown(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    label: &str,
    current_text: &str,
    expanded: bool,
    hovered: bool,
    t: &Theme,
    target: HitTarget,
    hits: &mut Vec<HitBox>,
) {
    let h = 26.0;
    text_left_v(cr, label, x, y, h, 11.5, false, t.text, 1.0);
    let tw = (w * 0.56).min(138.0).max(96.0);
    let tx = x + w - tw;
    let bg = if expanded { t.surface_active } else if hovered { t.surface_hover } else { t.surface };
    fill_rr(cr, tx, y, tw, h, 7.0, bg, if expanded || hovered { 1.0 } else { 0.8 });
    if expanded {
        stroke_rr(cr, tx + 0.5, y + 0.5, tw - 1.0, h - 1.0, 7.0, t.accent, 0.85);
    }
    let arrow = if expanded { "󰅃" } else { "󰅀" };
    let shown = fit_text(cr, current_text, 11.0, true, tw - 28.0);
    text_left_v(cr, &shown, tx + 10.0, y, h, 11.0, true, t.text_bright, 1.0);
    text_right_v(cr, arrow, tx + tw - 8.0, y, h, 11.5, false, if expanded { t.accent } else { t.muted }, 1.0);
    hits.push(HitBox { x: tx, y, w: tw, h, target, wheel: WheelAction::None });
}

fn settings_stat_bar(cr: &Context, x: f64, y: f64, w: f64, ratio: f64, col: Rgb, t: &Theme) {
    fill_rr(cr, x, y, w, 3.0, 1.5, t.bg_deep, 0.95);
    fill_rr(cr, x, y, (w * ratio.clamp(0.0, 1.0)).max(3.0), 3.0, 1.5, col, 1.0);
}

fn draw_settings_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let rail_w = 140.0;

    // ── Left Navigation Rail (frameless, clean hierarchy) ───────────────
    draw_text(cr, "Settings", cx + 8.0, cy + 16.0, 13.0, true, t.text_bright, 1.0);
    let sub = fit_text(cr, &st.cfg.theme_name, 10.0, false, rail_w - 16.0);
    draw_text(cr, &sub, cx + 8.0, cy + 31.0, 10.0, false, t.muted, 0.9);

    let tabs: [(&str, &str); 5] = [
        ("󰥔", "Time"),
        ("󰏘", "Appearance"),
        ("󰍹", "Display"),
        ("󰍛", "System"),
        ("󰕾", "Controls"),
    ];

    let list_y = cy + 46.0;
    let row_h = 32.0;
    let row_gap = 4.0;
    let active_idx = st.settings_tab.min(4);
    let prev_idx = st.settings_prev_tab.min(4);
    let p = st.settings_tab_pill_progress.clamp(0.0, 1.0);
    let y0 = list_y + prev_idx as f64 * (row_h + row_gap);
    let y1 = list_y + active_idx as f64 * (row_h + row_gap);
    let anim_y = y0 + (y1 - y0) * p;

    // Subtle tonal active surface + thin accent bar (no garish full-pill flood)
    fill_rr(cr, cx, anim_y, rail_w, row_h, 8.0, t.surface_hover, 0.95);
    fill_rr(cr, cx + 3.0, anim_y + 8.0, 3.0, row_h - 16.0, 1.5, t.accent, 1.0);

    for (i, (ic, lb)) in tabs.iter().enumerate() {
        let ry = list_y + i as f64 * (row_h + row_gap);
        let sel = i == active_idx;
        let target = HitTarget::SettingsTab(i);
        let hov = st.hover.as_ref() == Some(&target);
        if hov && !sel {
            fill_rr(cr, cx, ry, rail_w, row_h, 8.0, t.surface, 0.7);
        }
        let fg_ic = if sel { t.accent } else if hov { t.text_bright } else { t.muted };
        let fg_lb = if sel || hov { t.text_bright } else { t.text };
        draw_glyph_centered(cr, ic, cx + 11.0, ry, 18.0, row_h, 13.0, fg_ic, 1.0);
        text_left_v(cr, lb, cx + 35.0, ry, row_h, 11.5, sel, fg_lb, 1.0);
        hits.push(HitBox { x: cx, y: ry, w: rail_w, h: row_h, target, wheel: WheelAction::None });
    }

    // Bottom rail quick session button (isolated destructive entrypoint)
    let pwr_y = cy + ch - 28.0;
    let pwr_tgt = HitTarget::SettingsOpenPower;
    let pwr_hov = st.hover.as_ref() == Some(&pwr_tgt);
    tonal_pill(cr, cx, pwr_y, rail_w, 28.0, "󰐥", "Session…", false, pwr_hov, true, false, t, pwr_tgt, hits);

    // Subtle vertical divider between rail & content pane
    hairline_v(cr, cx + rail_w + 8.0, cy + 4.0, ch - 8.0, t, 0.35);

    // ── Right Content Pane ──────────────────────────────────────────────
    let content_x = cx + SETTINGS_RAIL_W;
    let content_w = (cw - SETTINGS_RAIL_W).max(120.0);
    let slide_x = ((st.settings_tab_slide_x) * 2.0).round() / 2.0;

    cr.save().ok();
    rounded_rect(cr, content_x, cy, content_w, ch, 10.0);
    cr.clip();

    if let Some(body_surf) = st.settings_body_cache {
        let _ = cr.set_source_surface(body_surf, content_x + slide_x, cy);
        let alpha = (0.35 + 0.65 * p).clamp(0.0, 1.0);
        let _ = cr.paint_with_alpha(alpha);
    } else {
        let slide_cx = content_x + slide_x;
        match st.settings_tab {
            0 => draw_settings_time(cr, slide_cx, cy, content_w, ch, st, hits),
            1 => draw_settings_personalize(cr, slide_cx, cy, content_w, ch, st, hits),
            2 => draw_settings_display(cr, slide_cx, cy, content_w, ch, st, hits),
            3 => draw_settings_system(cr, slide_cx, cy, content_w, ch, st, hits),
            _ => draw_settings_controls(cr, slide_cx, cy, content_w, ch, st, hits),
        }
    }
    cr.restore().ok();
}

pub fn render_settings_tab_body(
    cr: &Context,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
) -> Vec<HitBox> {
    let mut hits = Vec::new();
    cr.set_operator(Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    let _ = cr.paint();
    cr.set_operator(Operator::Over);
    if let Ok(mut fo) = cairo::FontOptions::new() {
        fo.set_antialias(cairo::Antialias::Gray);
        fo.set_hint_style(cairo::HintStyle::Slight);
        fo.set_hint_metrics(cairo::HintMetrics::Off);
        cr.set_font_options(&fo);
    }
    match st.settings_tab {
        0 => draw_settings_time(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        1 => draw_settings_personalize(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        2 => draw_settings_display(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        3 => draw_settings_system(cr, 0.0, 0.0, cw, ch, st, &mut hits),
        _ => draw_settings_controls(cr, 0.0, 0.0, cw, ch, st, &mut hits),
    }
    for hb in &mut hits {
        hb.x += SETTINGS_RAIL_W;
        match &mut hb.target {
            HitTarget::SettingsBarSize(sx, _)
            | HitTarget::SettingsBarOpacity(sx, _)
            | HitTarget::SettingsPopupOpacity(sx, _)
            | HitTarget::SettingsCorner(sx, _)
            | HitTarget::SettingsLockDim(sx, _)
            | HitTarget::ControlsVolSlider(sx, _)
            | HitTarget::ControlsBrightSlider(sx, _)
            | HitTarget::VolumeSlider(sx, _)
            | HitTarget::BrightnessSlider(sx, _)
            | HitTarget::MediaSeek(sx, _) => *sx += SETTINGS_RAIL_W,
            _ => {}
        }
    }
    hits
}

fn draw_settings_time(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    use chrono::{Datelike, Local, NaiveDate};

    let t = st.theme;
    let now = Local::now();
    let left_w = 194.0;
    let gap = 14.0;

    // ── Left: Clean Clock & Format Toggle (no container box) ─────────────
    let main_txt = if st.cfg.lock_24_hour {
        now.format("%H:%M").to_string()
    } else {
        now.format("%I:%M").to_string()
    };
    let sec_txt = if st.cfg.lock_24_hour {
        now.format(":%S").to_string()
    } else {
        now.format(":%S %p").to_string()
    };
    let mw = text_width(cr, &main_txt, 38.0, true);
    draw_text(cr, &main_txt, cx + 2.0, cy + 44.0, 38.0, true, t.text_bright, 1.0);
    draw_text(cr, &sec_txt, cx + 2.0 + mw + 5.0, cy + 44.0, 14.0, true, t.accent, 1.0);

    let date_line = now.format("%A, %d %b %Y").to_string();
    draw_text(cr, &date_line, cx + 4.0, cy + 66.0, 11.5, false, t.muted, 1.0);

    hairline_h(cr, cx + 2.0, cy + 88.0, left_w - 8.0, t, 0.3);

    // 24-Hour setting row
    let sw_y = cy + 104.0;
    let tgt = HitTarget::SettingsToggle24h;
    let hov = st.hover.as_ref() == Some(&tgt);
    if hov {
        fill_rr(cr, cx, sw_y, left_w - 4.0, 30.0, 8.0, t.surface, 0.8);
    }
    text_left_v(cr, "24-hour clock", cx + 4.0, sw_y, 30.0, 11.5, false, t.text, 1.0);
    compact_switch(cr, cx + left_w - 38.0, sw_y + 6.0, st.cfg.lock_24_hour, t);
    hits.push(HitBox { x: cx, y: sw_y, w: left_w - 4.0, h: 30.0, target: tgt, wheel: WheelAction::None });

    // Subtle uptime footer balancing left column
    let up_s = format!("Up {}", st.stats.sys_uptime);
    draw_text(cr, &up_s, cx + 4.0, cy + ch - 8.0, 10.5, false, t.muted, 0.8);

    // ── Right: Tonal Frameless Calendar ──────────────────────────────────
    let rx2 = cx + left_w + gap;
    let rw = (cw - left_w - gap).max(160.0);
    fill_rr(cr, rx2, cy, rw, ch, 12.0, t.surface, 0.58);

    let ix = rx2 + 12.0;
    let iw = rw - 24.0;
    let iy = cy + 10.0;
    let ih = ch - 20.0;

    let total_months = now.year() * 12 + (now.month() as i32 - 1) + st.cal_offset_months;
    let disp_y = total_months.div_euclid(12);
    let disp_m = total_months.rem_euclid(12) as u32 + 1;
    let first = NaiveDate::from_ymd_opt(disp_y, disp_m, 1).unwrap_or(now.date_naive());
    let month_label = first.format("%B").to_string();
    let year_label = format!("{}", disp_y);

    let mw2 = text_width(cr, &month_label, 13.0, true);
    draw_text(cr, &month_label, ix + 2.0, iy + 16.0, 13.0, true, t.text_bright, 1.0);
    draw_text(cr, &year_label, ix + 2.0 + mw2 + 6.0, iy + 16.0, 12.0, false, t.muted, 1.0);

    let nav_targets = [
        ("‹", 22.0, HitTarget::SettingsCalPrev),
        ("Today", 46.0, HitTarget::SettingsCalToday),
        ("›", 22.0, HitTarget::SettingsCalNext),
    ];
    let nav_w: f64 = nav_targets.iter().map(|(_, w, _)| *w).sum::<f64>() + 6.0;
    let mut nx = ix + iw - nav_w;
    for (label, w, target) in nav_targets {
        let hov = st.hover.as_ref() == Some(&target);
        let active = label == "Today" && st.cal_offset_months == 0;
        tonal_pill(cr, nx, iy, w, 22.0, "", label, active, hov, false, false, t, target, hits);
        nx += w + 3.0;
    }

    // Weekday row (clean typographic header, no pill box)
    let wy = iy + 28.0;
    let days = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let cell_w = ((iw - 18.0) / 7.0).floor();
    let grid_w = cell_w * 7.0 + 18.0;
    let gx = ix + (iw - grid_w) / 2.0;
    for (i, d) in days.iter().enumerate() {
        let col = if i >= 5 { t.accent } else { t.muted };
        text_center_v(cr, d, gx + i as f64 * (cell_w + 3.0) + cell_w / 2.0, wy, 18.0, 10.0, true, col, if i >= 5 { 0.85 } else { 0.85 });
    }

    // Calendar grid
    let gy = wy + 22.0;
    let cell_h = ((ih - 50.0 - 15.0) / 6.0).max(24.0);
    let first_offset = first.weekday().num_days_from_monday() as i32;
    let next_month = if disp_m == 12 {
        NaiveDate::from_ymd_opt(disp_y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(disp_y, disp_m + 1, 1)
    }
    .unwrap_or(first);
    let dim = next_month.signed_duration_since(first).num_days() as i32;
    let prev_dim = first.pred_opt().map(|d| d.day() as i32).unwrap_or(31);
    let today_key = now.year() * 10000 + now.month() as i32 * 100 + now.day() as i32;

    for idx in 0..42i32 {
        let dnum = idx - first_offset + 1;
        let (cy2, cm2, cd2, in_month) = if dnum < 1 {
            let pm = if disp_m == 1 { 12 } else { disp_m - 1 };
            let py2 = if disp_m == 1 { disp_y - 1 } else { disp_y };
            (py2, pm, prev_dim + dnum, false)
        } else if dnum > dim {
            let nm = if disp_m == 12 { 1 } else { disp_m + 1 };
            let ny = if disp_m == 12 { disp_y + 1 } else { disp_y };
            (ny, nm, dnum - dim, false)
        } else {
            (disp_y, disp_m, dnum, true)
        };
        let key = cy2 * 10000 + cm2 as i32 * 100 + cd2;
        let is_today = key == today_key;
        let is_sel = key == st.cal_selected_key;
        let target = HitTarget::SettingsCalDay(key);
        let hov = st.hover.as_ref() == Some(&target);
        let col_i = idx % 7;
        let row_i = idx / 7;
        let bx = gx + col_i as f64 * (cell_w + 3.0);
        let by = gy + row_i as f64 * (cell_h + 3.0);
        if is_today {
            fill_rr(cr, bx + 2.0, by + 1.0, cell_w - 4.0, cell_h - 2.0, 7.0, t.accent, 1.0);
        } else if is_sel {
            fill_rr(cr, bx + 2.0, by + 1.0, cell_w - 4.0, cell_h - 2.0, 7.0, t.surface_active, 1.0);
        } else if hov {
            fill_rr(cr, bx + 2.0, by + 1.0, cell_w - 4.0, cell_h - 2.0, 7.0, t.surface_hover, 0.9);
        }
        let weekend = col_i >= 5;
        let fg = if is_today {
            t.accent_text
        } else if weekend && in_month {
            t.accent
        } else {
            t.text_bright
        };
        let alpha = if in_month { 1.0 } else { 0.25 };
        text_center_v(cr, &format!("{}", cd2), bx + cell_w / 2.0, by, cell_h, 11.0, is_today || is_sel, fg, alpha);
        hits.push(HitBox { x: bx, y: by, w: cell_w, h: cell_h, target, wheel: WheelAction::None });
    }
}

fn draw_settings_personalize(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let cfg = st.cfg;
    let bg_target = st.settings_bg_target;

    // ── Top Segmented Bar (compact, frameless) ──────────────────────────
    let sub_tabs = [
        ("theme", "Theme & Bar"),
        ("mango", "MangoWM"),
        ("desktop", "Wallpaper"),
        ("lockscreen", "Lock"),
        ("sddm", "Login"),
    ];
    let sub_h = 26.0;
    let mut sub_x = cx;
    for (id, lb) in sub_tabs {
        let active = bg_target == id;
        let target = HitTarget::SettingsBgTarget(id);
        let hov = st.hover.as_ref() == Some(&target);
        let w = text_width(cr, lb, 11.0, active) + 18.0;
        tonal_pill(cr, sub_x, cy, w, sub_h, "", lb, active, hov, false, false, t, target, hits);
        sub_x += w + 5.0;
    }

    if bg_target != "theme" && bg_target != "mango" {
        let browse_tgt = match bg_target {
            "lockscreen" => HitTarget::SettingsBrowseLockWallpaper,
            "sddm" => HitTarget::SettingsBrowseSddmWallpaper,
            _ => HitTarget::SettingsBrowseWallpaper,
        };
        let bw = text_width(cr, "󰉋", 11.5, false) + text_width(cr, "Browse", 11.0, false) + 20.0;
        let bx = cx + cw - bw;
        tonal_pill(
            cr,
            bx,
            cy,
            bw,
            sub_h,
            "󰉋",
            "Browse",
            false,
            st.hover.as_ref() == Some(&browse_tgt),
            false,
            bg_target == "sddm" && cfg.sddm_applying,
            t,
            browse_tgt,
            hits,
        );

        let (ico, lbl, left_tgt, busy) = match bg_target {
            "lockscreen" => ("󰌾", "Lock", HitTarget::SettingsLockNow, false),
            "sddm" => ("󰸉", "Sync", HitTarget::SettingsSyncSddm, cfg.sddm_applying),
            _ => ("󰒟", "Random", HitTarget::SettingsRandomWallpaper, false),
        };
        let lw2 = text_width(cr, ico, 11.5, false) + text_width(cr, lbl, 11.0, false) + 20.0;
        tonal_pill(
            cr,
            bx - 5.0 - lw2,
            cy,
            lw2,
            sub_h,
            ico,
            lbl,
            false,
            st.hover.as_ref() == Some(&left_tgt),
            false,
            busy,
            t,
            left_tgt,
            hits,
        );
    }

    let content_y = cy + sub_h + 10.0;
    let content_h = (ch - sub_h - 10.0).max(40.0);

    match bg_target {
        "theme" => draw_theme_bar_subtab(cr, cx, content_y, cw, content_h, st, hits),
        "mango" => draw_mango_subtab(cr, cx, content_y, cw, content_h, st, hits),
        "desktop" | "lockscreen" | "sddm" => {
            draw_wallpaper_subtab(cr, cx, content_y, cw, content_h, st, hits);
        }
        _ => {}
    }
}

fn draw_dropdown_menu(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    options: &[(&'static str, &'static str)],
    selected: &str,
    make_target: fn(&'static str) -> HitTarget,
    hover: &Option<HitTarget>,
    t: &Theme,
    hits: &mut Vec<HitBox>,
) {
    let row_h = 24.0;
    let pad = 4.0;
    let total_h = pad * 2.0 + options.len() as f64 * row_h;
    fill_rr(cr, x, y, w, total_h, 9.0, t.bg_deep, 0.99);
    stroke_rr(cr, x + 0.5, y + 0.5, w - 1.0, total_h - 1.0, 9.0, t.accent, 0.5);

    for (i, (val, lbl)) in options.iter().enumerate() {
        let ry = y + pad + i as f64 * row_h;
        let active = selected == *val;
        let tgt = make_target(val);
        let hov = hover.as_ref() == Some(&tgt);
        if active || hov {
            fill_rr(
                cr,
                x + 4.0,
                ry + 1.0,
                w - 8.0,
                row_h - 2.0,
                6.0,
                if active { t.accent } else { t.surface_hover },
                1.0,
            );
        }
        let fg = if active { t.accent_text } else { t.text_bright };
        text_left_v(cr, lbl, x + 10.0, ry, row_h, 11.0, active, fg, 1.0);
        hits.push(HitBox {
            x: x + 4.0,
            y: ry,
            w: w - 8.0,
            h: row_h,
            target: tgt,
            wheel: WheelAction::None,
        });
    }
}

fn draw_theme_bar_subtab(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let cfg = st.cfg;

    let left_w = 246.0;
    let gap = 14.0;
    let right_w = (cw - left_w - gap).max(160.0);

    // ── Left: Frameless Controls Column ─────────────────────────────────
    let lx = cx;
    let lw = left_w;
    let mut ly = cy + 2.0;

    let theme_label = match cfg.theme_name.as_str() {
        "nord" => "Nord",
        "catppuccin" => "Catppuccin",
        "gruvbox" => "Gruvbox",
        "tokyonight" => "Tokyo Night",
        "dracula" => "Dracula",
        "rosepine" => "Rosé Pine",
        "everforest" => "Everforest",
        _ => "Catppuccin",
    };
    let theme_exp = st.settings_open_dropdown == "themePreset";
    let theme_hov = st.hover.as_ref() == Some(&HitTarget::SettingsThemeDropdown);
    let theme_dd_y = ly;
    compact_dropdown(cr, lx, ly, lw, "Theme", theme_label, theme_exp, theme_hov, t, HitTarget::SettingsThemeDropdown, hits);
    ly += 34.0;

    // Accent swatches (compact 14px dots)
    text_left_v(cr, "Accent", lx, ly, 24.0, 11.5, false, t.text, 1.0);
    let accent_presets = [
        ("theme", cfg.current_theme.accent.clone()),
        ("lavender", "#c6d0f5".to_string()),
        ("frost", "#88c0d0".to_string()),
        ("blue", "#81a1c1".to_string()),
        ("sage", "#a3be8c".to_string()),
        ("amber", "#ebcb8b".to_string()),
        ("rose", "#bf616a".to_string()),
        ("snow", "#eceff4".to_string()),
    ];
    let dot = 14.0;
    let dot_gap = 4.0;
    let total_dot_w = accent_presets.len() as f64 * dot + (accent_presets.len() - 1) as f64 * dot_gap;
    let mut ax = lx + lw - total_dot_w;
    let ay = ly + 5.0;
    for (id, hex) in accent_presets {
        let color = parse_hex_color(&hex);
        let active = cfg.accent_preset == id;
        let target = HitTarget::SettingsAccent(id);
        let hov = st.hover.as_ref() == Some(&target);
        fill_rr(cr, ax, ay, dot, dot, dot / 2.0, color, 1.0);
        if active {
            stroke_rr(cr, ax - 2.0, ay - 2.0, dot + 4.0, dot + 4.0, (dot + 4.0) / 2.0, t.text_bright, 0.9);
        } else if hov {
            stroke_rr(cr, ax - 1.5, ay - 1.5, dot + 3.0, dot + 3.0, (dot + 3.0) / 2.0, t.muted, 0.7);
        }
        hits.push(HitBox { x: ax - 2.0, y: ay - 3.0, w: dot + 4.0, h: dot + 6.0, target, wheel: WheelAction::None });
        ax += dot + dot_gap;
    }
    ly += 34.0;

    let bar_pos_label = match cfg.bar_position.as_str() {
        "top" => "Top",
        "bottom" => "Bottom",
        "left" => "Left",
        "right" => "Right",
        _ => "Top",
    };
    let bp_exp = st.settings_open_dropdown == "barPos";
    let bp_hov = st.hover.as_ref() == Some(&HitTarget::SettingsBarPosDropdown);
    let bp_dd_y = ly;
    compact_dropdown(cr, lx, ly, lw, "Bar Position", bar_pos_label, bp_exp, bp_hov, t, HitTarget::SettingsBarPosDropdown, hits);
    ly += 36.0;

    hairline_h(cr, lx, ly, lw, t, 0.25);
    ly += 12.0;

    // Compact 2x2 switches
    let switch_rows = [
        ("Floating", cfg.bar_floating, HitTarget::SettingsToggleFloating),
        ("Show RAM", cfg.show_ram_on_bar, HitTarget::SettingsToggleRam),
        ("Sys Tray", cfg.show_tray_on_bar, HitTarget::SettingsToggleTray),
        ("Media", cfg.show_media_on_bar, HitTarget::SettingsToggleMedia),
    ];
    let sw_w = (lw - 10.0) / 2.0;
    for (i, (lbl, on, target)) in switch_rows.iter().enumerate() {
        let col = i % 2;
        let row = i / 2;
        let sx = lx + col as f64 * (sw_w + 10.0);
        let sy = ly + row as f64 * 32.0;
        let hov = st.hover.as_ref() == Some(target);
        if hov {
            fill_rr(cr, sx, sy, sw_w, 26.0, 7.0, t.surface, 0.75);
        }
        text_left_v(cr, lbl, sx + 4.0, sy, 26.0, 11.0, false, t.text, 1.0);
        compact_switch(cr, sx + sw_w - 34.0, sy + 4.0, *on, t);
        hits.push(HitBox { x: sx, y: sy, w: sw_w, h: 26.0, target: target.clone(), wheel: WheelAction::None });
    }

    // ── Right: Tonal Geometry & Opacity Sliders Card (no border) ────────
    let rx = cx + left_w + gap;
    fill_rr(cr, rx, cy, right_w, ch, 12.0, t.surface, 0.58);
    let rix = rx + 12.0;
    let riw = right_w - 24.0;
    let mut rly = cy + 14.0;

    let (sx, sw) = slim_slider(
        cr,
        rix,
        rly,
        riw,
        "Bar Size",
        &format!("{} px", cfg.bar_size),
        cfg.bar_size as f64,
        28.0,
        48.0,
        "",
        t,
    );
    hits.push(HitBox { x: sx, y: rly + 12.0, w: sw, h: 26.0, target: HitTarget::SettingsBarSize(sx, sw), wheel: WheelAction::None });
    rly += 56.0;

    let (sx4, sw4) = slim_slider(
        cr,
        rix,
        rly,
        riw,
        "Bar Opacity",
        &format!("{}%", (cfg.bar_opacity * 100.0).round() as i32),
        cfg.bar_opacity * 100.0,
        35.0,
        100.0,
        "",
        t,
    );
    hits.push(HitBox { x: sx4, y: rly + 12.0, w: sw4, h: 26.0, target: HitTarget::SettingsBarOpacity(sx4, sw4), wheel: WheelAction::None });
    rly += 56.0;

    let (sx3, sw3) = slim_slider(
        cr,
        rix,
        rly,
        riw,
        "Popup Opacity",
        &format!("{}%", (cfg.popup_opacity * 100.0).round() as i32),
        cfg.popup_opacity * 100.0,
        55.0,
        100.0,
        "",
        t,
    );
    hits.push(HitBox { x: sx3, y: rly + 12.0, w: sw3, h: 26.0, target: HitTarget::SettingsPopupOpacity(sx3, sw3), wheel: WheelAction::None });
    rly += 56.0;

    let (sx2, sw2) = slim_slider(
        cr,
        rix,
        rly,
        riw,
        "Corner Radius",
        &format!("{} px", cfg.corner_radius.round() as i32),
        cfg.corner_radius,
        0.0,
        20.0,
        "",
        t,
    );
    hits.push(HitBox { x: sx2, y: rly + 12.0, w: sw2, h: 26.0, target: HitTarget::SettingsCorner(sx2, sw2), wheel: WheelAction::None });

    // Expanded dropdown menu overlay
    if theme_exp {
        let opts = [
            ("nord", "Nord"),
            ("catppuccin", "Catppuccin"),
            ("gruvbox", "Gruvbox"),
            ("tokyonight", "Tokyo Night"),
            ("dracula", "Dracula"),
            ("rosepine", "Rosé Pine"),
            ("everforest", "Everforest"),
        ];
        let mw = 138.0;
        draw_dropdown_menu(
            cr,
            lx + lw - mw,
            theme_dd_y + 28.0,
            mw,
            &opts,
            &cfg.theme_name,
            HitTarget::SettingsThemePick,
            st.hover,
            t,
            hits,
        );
    } else if bp_exp {
        let opts = [
            ("top", "Top"),
            ("bottom", "Bottom"),
            ("left", "Left"),
            ("right", "Right"),
        ];
        let mw = 128.0;
        draw_dropdown_menu(
            cr,
            lx + lw - mw,
            bp_dd_y + 28.0,
            mw,
            &opts,
            &cfg.bar_position,
            HitTarget::SettingsBarPosPick,
            st.hover,
            t,
            hits,
        );
    }
}

fn draw_mango_subtab(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let m = st.mango_info;
    let gap = 14.0;
    let left_w = 252.0;
    let right_w = (cw - left_w - gap).max(150.0);

    // Left: clean key-value spec list (no inner boxes)
    fill_rr(cr, cx, cy, left_w, ch, 12.0, t.surface, 0.58);
    let lx = cx + 14.0;
    let lw = left_w - 28.0;
    let rows = [
        ("Inner Gap", format!("{} px", m.gappih)),
        ("Outer Gap", format!("{} px", m.gappoh)),
        ("Border Width", format!("{} px", m.borderpx)),
        ("Window Radius", format!("{} px", m.border_radius)),
        ("Open / Close", format!("{} / {}", m.anim_open, m.anim_close)),
    ];
    let row_h = ((ch - 20.0) / rows.len() as f64).min(42.0);
    let mut ly = cy + 10.0;
    for (i, (lbl, val)) in rows.iter().enumerate() {
        text_left_v(cr, lbl, lx, ly, row_h, 11.5, false, t.text, 1.0);
        text_right_v(cr, val, lx + lw, ly, row_h, 11.5, true, t.accent, 1.0);
        if i + 1 < rows.len() {
            hairline_h(cr, lx, ly + row_h - 1.0, lw, t, 0.22);
        }
        ly += row_h;
    }

    // Right: toggles + reload action
    let rx = cx + left_w + gap;
    let rix = rx;
    let riw = right_w;
    let mut ry = cy + 4.0;
    for (lbl, on, key) in [
        ("Blur", m.blur, "blur"),
        ("Shadows", m.shadows, "shadows"),
        ("Animations", m.animations, "animations"),
    ] {
        let tgt = HitTarget::SettingsMangoToggle(key);
        let hov = st.hover.as_ref() == Some(&tgt);
        fill_rr(cr, rix, ry, riw, 34.0, 9.0, if hov { t.surface_hover } else { t.surface }, if hov { 0.95 } else { 0.58 });
        text_left_v(cr, lbl, rix + 12.0, ry, 34.0, 11.5, false, t.text_bright, 1.0);
        compact_switch(cr, rix + riw - 44.0, ry + 8.0, on, t);
        hits.push(HitBox { x: rix, y: ry, w: riw, h: 34.0, target: tgt, wheel: WheelAction::None });
        ry += 42.0;
    }

    let btn_tgt = HitTarget::SettingsMangoReload;
    let btn_hov = st.hover.as_ref() == Some(&btn_tgt);
    tonal_pill(
        cr,
        rix,
        cy + ch - 30.0,
        riw,
        30.0,
        "󰑓",
        "Reload MangoWM",
        true,
        btn_hov,
        false,
        false,
        t,
        btn_tgt,
        hits,
    );
}

fn draw_wallpaper_subtab(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let cfg = st.cfg;
    let target_kind = st.settings_bg_target;

    // 1. Compact preview strip (82px height, borderless)
    let hero_h = 82.0;
    fill_rr(cr, cx, cy, cw, hero_h, 11.0, t.surface, 0.58);

    let px = cx + 8.0;
    let py = cy + 8.0;
    let pw = 118.0;
    let ph = hero_h - 16.0;
    fill_rr(cr, px, py, pw, ph, 8.0, t.bg_deep, 1.0);
    let wp_p = st.wallpaper_preview_progress.clamp(0.0, 1.0);
    if st.current_preview_surf.is_some() || st.wallpaper_prev_surf.is_some() {
        cr.save().ok();
        rounded_rect(cr, px, py, pw, ph, 8.0);
        cr.clip();
        if wp_p < 0.999 {
            if let Some(prev_s) = st.wallpaper_prev_surf {
                draw_surface_fitted(cr, prev_s, px, py, pw, ph, 1.0);
            }
        }
        if let Some(surf) = st.current_preview_surf {
            draw_surface_fitted(cr, surf, px, py, pw, ph, wp_p);
        }
        cr.restore().ok();
    } else {
        draw_glyph_centered(cr, "󰸉", px, py, pw, ph, 18.0, t.muted, 1.0);
    }
    if target_kind == "lockscreen" {
        fill_rr(cr, px, py, pw, ph, 8.0, Rgb(0.0, 0.0, 0.0), cfg.lock_dim_opacity);
        if cfg.lock_show_clock {
            let (hh, mm, _, _) = local_clock_strings(cfg.lock_24_hour);
            text_center_v(cr, &format!("{hh}:{mm}"), px + pw / 2.0, py, ph, 15.0, true, Rgb(0.925, 0.937, 0.957), 1.0);
        }
    }

    let rx = px + pw + 12.0;
    let rw = cw - (rx - cx) - 10.0;
    let active_path = match target_kind {
        "lockscreen" => {
            if cfg.lock_use_same_wallpaper { &cfg.wallpaper_path } else { &cfg.lock_wallpaper_path }
        }
        "sddm" => {
            if cfg.sddm_selected_path.is_empty() { &cfg.wallpaper_path } else { &cfg.sddm_selected_path }
        }
        _ => &cfg.wallpaper_path,
    };
    let base = std::path::Path::new(active_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("None");
    text_left_v(cr, &fit_text(cr, base, 11.5, true, rw), rx, cy + 9.0, 16.0, 11.5, true, t.text_bright, 1.0);

    if target_kind == "desktop" {
        let mut bx = rx;
        let by = cy + 38.0;
        for (id, lbl) in [("fill", "Fill"), ("fit", "Fit"), ("stretch", "Stretch"), ("center", "Center"), ("tile", "Tile")] {
            let bw = text_width(cr, lbl, 10.5, cfg.wallpaper_mode == id) + 14.0;
            let tgt = HitTarget::SettingsWallpaperMode(id);
            let hov = st.hover.as_ref() == Some(&tgt);
            tonal_pill(cr, bx, by, bw, 24.0, "", lbl, cfg.wallpaper_mode == id, hov, false, false, t, tgt, hits);
            bx += bw + 4.0;
        }
        let s_lbl = if cfg.lock_use_same_wallpaper { "Lock Synced" } else { "Sync Lock" };
        let s_tgt = HitTarget::SettingsToggleLockSame;
        let sw = text_width(cr, s_lbl, 10.5, cfg.lock_use_same_wallpaper) + 16.0;
        if bx + sw <= rx + rw {
            tonal_pill(cr, bx, by, sw, 24.0, "", s_lbl, cfg.lock_use_same_wallpaper, st.hover.as_ref() == Some(&s_tgt), false, false, t, s_tgt, hits);
        }
    } else if target_kind == "lockscreen" {
        let mut bx = rx;
        let by = cy + 28.0;
        let t1 = HitTarget::SettingsToggleLockSame;
        let w1 = text_width(cr, "Sync", 10.5, cfg.lock_use_same_wallpaper) + 16.0;
        tonal_pill(cr, bx, by, w1, 22.0, "", "Sync", cfg.lock_use_same_wallpaper, st.hover.as_ref() == Some(&t1), false, false, t, t1, hits);
        bx += w1 + 4.0;
        let t2 = HitTarget::SettingsToggleLockClock;
        let w2 = text_width(cr, "Clock", 10.5, cfg.lock_show_clock) + 16.0;
        tonal_pill(cr, bx, by, w2, 22.0, "", "Clock", cfg.lock_show_clock, st.hover.as_ref() == Some(&t2), false, false, t, t2, hits);
        bx += w2 + 4.0;
        let t3 = HitTarget::SettingsToggle24h;
        let w3 = text_width(cr, "24h", 10.5, cfg.lock_24_hour) + 14.0;
        tonal_pill(cr, bx, by, w3, 22.0, "", "24h", cfg.lock_24_hour, st.hover.as_ref() == Some(&t3), false, false, t, t3, hits);

        // Inline slim dim bar on bottom row of lock hero
        let sy = cy + 56.0;
        text_left_v(cr, "Dim", rx, sy, 18.0, 10.5, false, t.muted, 1.0);
        let sx = rx + 28.0;
        let pct = format!("{}%", (cfg.lock_dim_opacity * 100.0).round() as i32);
        let pct_w = text_width(cr, &pct, 10.5, true) + 8.0;
        let sw = (rw - 28.0 - pct_w).max(40.0);
        let frac = (cfg.lock_dim_opacity * 100.0 / 85.0).clamp(0.0, 1.0);
        let thumb = 10.0;
        let cx_t = sx + thumb / 2.0 + (sw - thumb).max(1.0) * frac;
        fill_rr(cr, sx, sy + 7.0, sw, 4.0, 2.0, t.bg_deep, 0.9);
        fill_rr(cr, sx, sy + 7.0, (cx_t - sx).max(4.0).min(sw), 4.0, 2.0, t.accent, 1.0);
        fill_rr(cr, (cx_t - thumb / 2.0).clamp(sx, sx + sw - thumb), sy + 4.0, thumb, thumb, thumb / 2.0, t.text_bright, 1.0);
        text_right_v(cr, &pct, rx + rw, sy, 18.0, 10.5, true, t.text_bright, 1.0);
        hits.push(HitBox { x: sx, y: sy, w: sw, h: 20.0, target: HitTarget::SettingsLockDim(sx, sw), wheel: WheelAction::None });
    } else {
        let status = if cfg.sddm_status.is_empty() {
            format!("Theme: {}  ·  Click thumbnail or Sync", cfg.sddm_theme)
        } else {
            cfg.sddm_status.clone()
        };
        text_left_v(
            cr,
            &fit_text(cr, &status, 11.0, false, rw),
            rx,
            cy + 38.0,
            20.0,
            11.0,
            false,
            if cfg.sddm_status_error { t.red } else { t.accent },
            1.0,
        );
    }

    // 2. Folder filter row
    let row_y = cy + hero_h + 8.0;
    let mut fx = cx;
    for (lbl, dir) in [
        ("Pictures", "/home/huyle/Pictures"),
        ("Downloads", "/home/huyle/Downloads"),
        ("Mango", "/home/huyle/.config/mango"),
    ] {
        let active = cfg.wallpaper_dir == dir;
        let tgt = HitTarget::SettingsWallpaperDir(dir);
        let fw = text_width(cr, lbl, 11.0, active) + 18.0;
        tonal_pill(cr, fx, row_y, fw, 24.0, "", lbl, active, st.hover.as_ref() == Some(&tgt), false, false, t, tgt, hits);
        fx += fw + 5.0;
    }

    // 3. Thumbnail grid (3 borderless rounded previews)
    let gy = row_y + 32.0;
    let gh = (ch - (gy - cy)).max(40.0);
    let cols = 3usize;
    let gap = 10.0;
    let tile_w = ((cw - gap * (cols as f64 - 1.0)) / cols as f64).floor();
    let tile_h = gh.min(132.0);

    if st.wallpapers.is_empty() {
        fill_rr(cr, cx, gy, cw, tile_h, 10.0, t.surface, 0.58);
        text_center_v(cr, "No images in selected folder", cx + cw / 2.0, gy, tile_h, 11.5, false, t.muted, 1.0);
        return;
    }

    for (idx, (name, path, surf_opt)) in st.wallpapers.iter().take(3).enumerate() {
        let tx = cx + idx as f64 * (tile_w + gap);
        let selected = path.to_string_lossy() == *active_path;
        let tgt = HitTarget::SettingsWallpaperPick(idx);
        let hov = st.hover.as_ref() == Some(&tgt);

        fill_rr(cr, tx, gy, tile_w, tile_h, 10.0, t.surface, 0.7);
        if let Some(ref s) = surf_opt {
            cr.save().ok();
            rounded_rect(cr, tx + 2.0, gy + 2.0, tile_w - 4.0, tile_h - 4.0, 9.0);
            cr.clip();
            draw_surface_fitted(cr, s, tx + 2.0, gy + 2.0, tile_w - 4.0, tile_h - 4.0, 1.0);
            cr.restore().ok();
        } else {
            text_center_v(cr, &fit_text(cr, name, 10.5, false, tile_w - 16.0), tx + tile_w / 2.0, gy, tile_h, 10.5, false, t.muted, 1.0);
        }

        if selected {
            stroke_rr(cr, tx + 1.0, gy + 1.0, tile_w - 2.0, tile_h - 2.0, 10.0, t.accent, 1.0);
            fill_rr(cr, tx + tile_w - 24.0, gy + 6.0, 18.0, 18.0, 9.0, t.accent, 1.0);
            draw_glyph_centered(cr, "󰄬", tx + tile_w - 24.0, gy + 6.0, 18.0, 18.0, 9.5, t.accent_text, 1.0);
        } else if hov {
            stroke_rr(cr, tx + 1.0, gy + 1.0, tile_w - 2.0, tile_h - 2.0, 10.0, t.text_bright, 0.45);
        }

        hits.push(HitBox { x: tx, y: gy, w: tile_w, h: tile_h, target: tgt, wheel: WheelAction::None });
    }
}

fn parse_hex_color(hex: &str) -> Rgb {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return Rgb(0.2, 0.2, 0.25);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;
    Rgb(r, g, b)
}

fn draw_history_bars(cr: &Context, x: f64, y: f64, w: f64, h: f64, samples: &[u32], col: Rgb) {
    let n = samples.len().max(1);
    let spacing = 2.0;
    let bar_w = ((w - (n.saturating_sub(1) as f64) * spacing) / n as f64).floor().max(3.0);
    for (i, &val) in samples.iter().enumerate() {
        let bx = x + i as f64 * (bar_w + spacing);
        let norm = (val as f64 / 100.0).clamp(0.05, 1.0);
        let bh = (h * norm).round().max(3.0);
        let by = y + h - bh;
        fill_rr(cr, bx, by, bar_w, bh, 1.5, col, 0.30 + 0.70 * norm);
    }
}

fn fmt_net_speed(kbps: f64) -> String {
    if kbps >= 1024.0 {
        format!("{:.1} MB/s", kbps / 1024.0)
    } else {
        format!("{:.0} KB/s", kbps)
    }
}

fn draw_settings_display(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let mon = st.stats.monitors.first();
    let mon_name = mon.map(|m| m.name.as_str()).unwrap_or("eDP-1");
    let res_label = mon
        .map(|m| format!("{}×{} @ 60Hz", m.width, m.height))
        .unwrap_or_else(|| "1920×1200 @ 60Hz".into());

    text_left_v(cr, "Display", cx + 2.0, cy, 24.0, 13.0, true, t.text_bright, 1.0);
    let sub = format!("{mon_name} · {res_label}");
    draw_text(cr, &sub, cx + 62.0, cy + 16.0, 11.0, false, t.muted, 1.0);
    let target = HitTarget::SettingsRefresh;
    let hov = st.hover.as_ref() == Some(&target);
    tonal_pill(cr, cx + cw - 76.0, cy, 76.0, 24.0, "󰑓", "Refresh", false, hov, false, false, t, target, hits);

    let y = cy + 34.0;
    let pane_h = (ch - 34.0).max(160.0);
    let left_w = 232.0;
    let gap = 14.0;
    let right_w = (cw - left_w - gap).max(160.0);

    // Left: Minimalist monitor arrangement canvas (borderless dark surface)
    fill_rr(cr, cx, y, left_w, pane_h, 12.0, t.bg_deep, 0.9);
    let mon_pos = mon.map(|m| format!("{}, {}", m.x, m.y)).unwrap_or_else(|| "0, 0".into());
    let mw = 132.0;
    let mh = 82.0;
    let mx = cx + (left_w - mw) / 2.0;
    let my = y + (pane_h - mh) / 2.0;
    fill_rr(cr, mx, my, mw, mh, 9.0, t.surface, 1.0);
    stroke_rr(cr, mx + 1.0, my + 1.0, mw - 2.0, mh - 2.0, 9.0, t.accent, 0.9);
    text_center_v(cr, mon_name, mx + mw / 2.0, my + 22.0, 16.0, 11.0, true, t.text_bright, 1.0);
    text_center_v(cr, &mon_pos, mx + mw / 2.0, my + 42.0, 14.0, 10.0, false, t.accent, 1.0);

    // Right: Tonal settings list
    let rx = cx + left_w + gap;
    fill_rr(cr, rx, y, right_w, pane_h, 12.0, t.surface, 0.58);

    let scale_val = mon
        .map(|m| {
            let s = format!("{:.2}", m.scale);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        })
        .unwrap_or_else(|| "1.25".into());
    let scale_label = format!("{scale_val}x");
    let rot_val = mon.map(|m| m.transform.as_str()).unwrap_or("normal");
    let rot_label = match rot_val {
        "90" => "90°",
        "180" => "180°",
        "270" => "270°",
        _ => "0°",
    };

    let ix = rx + 12.0;
    let iw = right_w - 24.0;
    let mut ry = y + 12.0;

    let mode_exp = st.settings_open_dropdown == "dispMode";
    let mode_tgt = HitTarget::SettingsDisplayDropdown("dispMode");
    compact_dropdown(cr, ix, ry, iw, "Mode", "Primary Only", mode_exp, st.hover.as_ref() == Some(&mode_tgt), t, mode_tgt, hits);
    ry += 34.0;

    text_left_v(cr, "Resolution", ix, ry, 26.0, 11.5, false, t.text, 1.0);
    text_right_v(cr, &res_label, ix + iw, ry, 26.0, 11.0, true, t.muted, 1.0);
    ry += 34.0;

    let sc_exp = st.settings_open_dropdown == "dispScale";
    let sc_tgt = HitTarget::SettingsDisplayDropdown("dispScale");
    compact_dropdown(cr, ix, ry, iw, "Scale", &scale_label, sc_exp, st.hover.as_ref() == Some(&sc_tgt), t, sc_tgt, hits);
    ry += 34.0;

    let rot_exp = st.settings_open_dropdown == "dispRot";
    let rot_tgt = HitTarget::SettingsDisplayDropdown("dispRot");
    compact_dropdown(cr, ix, ry, iw, "Rotate", rot_label, rot_exp, st.hover.as_ref() == Some(&rot_tgt), t, rot_tgt, hits);

    let bot_y = y + pane_h - 40.0;
    text_left_v(cr, mon_name, ix, bot_y, 28.0, 11.0, false, t.muted, 1.0);
    let app_t = HitTarget::SettingsDisplayApply;
    let app_h = st.hover.as_ref() == Some(&app_t);
    tonal_pill(cr, ix + iw - 96.0, bot_y, 96.0, 28.0, "", "Apply", true, app_h, false, false, t, app_t, hits);

    if mode_exp {
        let opts = [
            ("primary", "Primary Only"),
            ("extend", "Extend"),
            ("mirror", "Mirror"),
            ("secondary", "External Only"),
        ];
        draw_dropdown_menu(cr, ix + iw - 140.0, y + 40.0, 140.0, &opts, "primary", HitTarget::SettingsDisplayModePick, st.hover, t, hits);
    } else if sc_exp {
        let opts = [
            ("1", "1x"),
            ("1.25", "1.25x"),
            ("1.5", "1.5x"),
            ("1.75", "1.75x"),
            ("2", "2x"),
        ];
        draw_dropdown_menu(cr, ix + iw - 110.0, y + 108.0, 110.0, &opts, &scale_val, HitTarget::SettingsDisplayScalePick, st.hover, t, hits);
    } else if rot_exp {
        let opts = [
            ("normal", "0°"),
            ("90", "90°"),
            ("180", "180°"),
            ("270", "270°"),
        ];
        draw_dropdown_menu(cr, ix + iw - 110.0, y + 142.0, 110.0, &opts, rot_val, HitTarget::SettingsDisplayRotPick, st.hover, t, hits);
    }
}

fn draw_settings_system(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    _hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;

    // Header: Title left, plain inline telemetry summary right (no pill frame)
    text_left_v(cr, "System", cx + 2.0, cy, 22.0, 13.0, true, t.text_bright, 1.0);
    let summary = format!("Up {}  ·  {} tasks  ·  Load {}", s.sys_uptime, s.sys_process_count, s.sys_load_avg);
    text_right_v(cr, &fit_text(cr, &summary, 10.5, false, cw - 80.0), cx + cw - 2.0, cy, 22.0, 10.5, false, t.muted, 1.0);

    // Row 1: CPU + Memory (borderless tonal blocks, sparkline fills lower region)
    let r1_y = cy + 30.0;
    let gap = 10.0;
    let half_w = ((cw - gap) / 2.0).floor();
    let r1_h = 136.0;

    // CPU
    fill_rr(cr, cx, r1_y, half_w, r1_h, 12.0, t.surface, 0.58);
    let cpu_title = format!("CPU · {}T", s.cpu_cores);
    text_left_v(cr, &cpu_title, cx + 12.0, r1_y + 10.0, 20.0, 11.5, true, t.text_bright, 1.0);
    text_right_v(cr, &format!("{}%", s.cpu_percent), cx + half_w - 12.0, r1_y + 10.0, 20.0, 15.0, true, t.accent, 1.0);
    settings_stat_bar(cr, cx + 12.0, r1_y + 36.0, half_w - 24.0, s.cpu_percent as f64 / 100.0, t.accent, t);
    let mdl = fit_text(cr, &s.cpu_model, 10.0, false, (half_w - 135.0).max(50.0));
    text_left_v(cr, &mdl, cx + 12.0, r1_y + 48.0, 16.0, 10.0, false, t.muted, 1.0);
    let tf_str = format!("{} · {}", s.cpu_temp, s.cpu_freq_ghz);
    text_right_v(cr, &tf_str, cx + half_w - 12.0, r1_y + 48.0, 16.0, 10.0, false, t.text, 1.0);
    draw_history_bars(cr, cx + 12.0, r1_y + 74.0, half_w - 24.0, 48.0, &s.cpu_history, t.accent);

    // Memory
    let mx = cx + half_w + gap;
    fill_rr(cr, mx, r1_y, half_w, r1_h, 12.0, t.surface, 0.58);
    text_left_v(cr, "Memory", mx + 12.0, r1_y + 10.0, 20.0, 11.5, true, t.text_bright, 1.0);
    text_right_v(cr, &format!("{}%", s.ram_percent), mx + half_w - 12.0, r1_y + 10.0, 20.0, 15.0, true, t.accent, 1.0);
    settings_stat_bar(cr, mx + 12.0, r1_y + 36.0, half_w - 24.0, s.ram_percent as f64 / 100.0, t.accent, t);
    let ram_sub = format!("{:.1} / {:.1} GB", s.ram_used_gb, s.ram_total_gb);
    text_left_v(cr, &ram_sub, mx + 12.0, r1_y + 48.0, 16.0, 10.0, false, t.muted, 1.0);
    let swap_sub = if s.swap_total_gb > 0.05 {
        format!("Swap {:.1}/{:.1}G", s.swap_used_gb, s.swap_total_gb)
    } else {
        "Swap off".into()
    };
    text_right_v(cr, &swap_sub, mx + half_w - 12.0, r1_y + 48.0, 16.0, 10.0, false, t.text, 1.0);
    draw_history_bars(cr, mx + 12.0, r1_y + 74.0, half_w - 24.0, 48.0, &s.ram_history, t.accent);

    // Row 2: Disk + Network + Battery (remaining height, borderless)
    let r2_y = r1_y + r1_h + 10.0;
    let r2_h = (cy + ch - r2_y).max(88.0);
    let col3_w = ((cw - 20.0) / 3.0).floor();

    // Disk
    fill_rr(cr, cx, r2_y, col3_w, r2_h, 12.0, t.surface, 0.58);
    text_left_v(cr, "Storage (/)", cx + 12.0, r2_y + 12.0, 18.0, 11.0, true, t.text_bright, 1.0);
    text_right_v(cr, &format!("{}%", s.disk_percent), cx + col3_w - 12.0, r2_y + 12.0, 18.0, 12.0, true, t.accent, 1.0);
    settings_stat_bar(cr, cx + 12.0, r2_y + 38.0, col3_w - 24.0, s.disk_percent as f64 / 100.0, t.accent, t);
    text_left_v(
        cr,
        &format!("{:.1} / {:.1} GB", s.disk_used_gb, s.disk_total_gb),
        cx + 12.0,
        r2_y + 52.0,
        16.0,
        10.5,
        false,
        t.muted,
        1.0,
    );
    let free_gb = (s.disk_total_gb - s.disk_used_gb).max(0.0);
    text_left_v(cr, &format!("{:.1} GB free", free_gb), cx + 12.0, r2_y + 74.0, 16.0, 10.5, false, t.text, 0.85);

    // Network (clean 2-column alignment inside 162px width)
    let nx = cx + col3_w + 10.0;
    fill_rr(cr, nx, r2_y, col3_w, r2_h, 12.0, t.surface, 0.58);
    text_left_v(cr, "Network", nx + 12.0, r2_y + 12.0, 18.0, 11.0, true, t.text_bright, 1.0);
    let net_st = if s.wifi_enabled {
        if s.wifi_ssid.is_empty() { "Connected" } else { s.wifi_ssid.as_str() }
    } else {
        "Offline"
    };
    text_right_v(cr, &fit_text(cr, net_st, 10.0, false, col3_w - 76.0), nx + col3_w - 12.0, r2_y + 12.0, 18.0, 10.0, false, t.muted, 1.0);
    hairline_h(cr, nx + 12.0, r2_y + 38.0, col3_w - 24.0, t, 0.25);
    text_left_v(cr, "Down", nx + 12.0, r2_y + 48.0, 18.0, 10.5, false, t.muted, 1.0);
    text_right_v(cr, &fmt_net_speed(s.net_down_kbps), nx + col3_w - 12.0, r2_y + 48.0, 18.0, 11.0, true, t.accent, 1.0);
    text_left_v(cr, "Up", nx + 12.0, r2_y + 72.0, 18.0, 10.5, false, t.muted, 1.0);
    text_right_v(cr, &fmt_net_speed(s.net_up_kbps), nx + col3_w - 12.0, r2_y + 72.0, 18.0, 11.0, true, t.text_bright, 1.0);

    // Battery
    let bx = nx + col3_w + 10.0;
    fill_rr(cr, bx, r2_y, col3_w, r2_h, 12.0, t.surface, 0.58);
    let bat_lbl = if s.battery_charging { "Battery 󰂄" } else { "Battery" };
    text_left_v(cr, bat_lbl, bx + 12.0, r2_y + 12.0, 18.0, 11.0, true, t.text_bright, 1.0);
    text_right_v(cr, &format!("{}%", s.battery_percent), bx + col3_w - 12.0, r2_y + 12.0, 18.0, 12.0, true, t.accent, 1.0);
    settings_stat_bar(cr, bx + 12.0, r2_y + 38.0, col3_w - 24.0, s.battery_percent as f64 / 100.0, t.accent, t);
    text_left_v(cr, &fit_text(cr, &s.battery_status, 10.5, false, col3_w - 24.0), bx + 12.0, r2_y + 52.0, 16.0, 10.5, false, t.text, 0.9);
    if !s.battery_detail.is_empty() {
        text_left_v(cr, &fit_text(cr, &s.battery_detail, 10.0, false, col3_w - 24.0), bx + 12.0, r2_y + 74.0, 16.0, 10.0, false, t.muted, 1.0);
    }
}

fn draw_settings_controls(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;

    text_left_v(cr, "Controls", cx + 2.0, cy, 22.0, 13.0, true, t.text_bright, 1.0);
    text_right_v(cr, "Audio, radios & session", cx + cw - 2.0, cy, 22.0, 10.5, false, t.muted, 1.0);

    // Row 1: Volume & Brightness inside tonal panel
    let r1_y = cy + 32.0;
    let panel_h = 62.0;
    fill_rr(cr, cx, r1_y, cw, panel_h, 12.0, t.surface, 0.58);
    let half_w = ((cw - 36.0) / 2.0).floor();

    let vol_val = if s.volume_muted {
        "Muted".to_string()
    } else {
        format!("{}%", s.volume_percent)
    };
    let (sx1, sw1) = slim_slider(
        cr,
        cx + 12.0,
        r1_y + 12.0,
        half_w,
        "Volume",
        &vol_val,
        if s.volume_muted { 0.0 } else { s.volume_percent as f64 },
        0.0,
        150.0,
        if s.volume_muted { "󰝟" } else { "󰕾" },
        t,
    );
    hits.push(HitBox { x: cx + 10.0, y: r1_y + 24.0, w: 20.0, h: 26.0, target: HitTarget::VolumeMute, wheel: WheelAction::Volume });
    hits.push(HitBox { x: sx1, y: r1_y + 24.0, w: sw1, h: 26.0, target: HitTarget::ControlsVolSlider(sx1, sw1), wheel: WheelAction::Volume });

    let bx = cx + 12.0 + half_w + 16.0;
    let (sx2, sw2) = slim_slider(
        cr,
        bx,
        r1_y + 12.0,
        cw - 24.0 - half_w - 16.0,
        "Brightness",
        &format!("{}%", s.brightness_percent),
        s.brightness_percent as f64,
        5.0,
        100.0,
        "󰃠",
        t,
    );
    hits.push(HitBox { x: sx2, y: r1_y + 24.0, w: sw2, h: 26.0, target: HitTarget::BrightnessSlider(sx2, sw2), wheel: WheelAction::Brightness });

    // Row 2: 4 compact toggle tiles (borderless tonal elevation)
    let r2_y = r1_y + panel_h + 12.0;
    let tw = ((cw - 30.0) / 4.0).floor();
    let wifi_sub = if s.wifi_enabled {
        if s.wifi_ssid.is_empty() { "On".to_string() } else { s.wifi_ssid.clone() }
    } else {
        "Off".to_string()
    };
    let bt_sub = if s.bt_powered {
        if s.bt_connected > 0 { format!("{} conn", s.bt_connected) } else { "On".to_string() }
    } else {
        "Off".to_string()
    };
    let cards: [(&str, &str, String, bool, HitTarget); 4] = [
        (if s.wifi_enabled { "󰖩" } else { "󰖪" }, "Wi-Fi", wifi_sub, s.wifi_enabled, HitTarget::WifiToggle),
        (if s.bt_powered { "󰂯" } else { "󰂲" }, "Bluetooth", bt_sub, s.bt_powered, HitTarget::BtToggle),
        ("󰒓", "Lotus", "Input".into(), false, HitTarget::SettingsOpenLotus),
        ("󰑓", "MangoWM", "Reload".into(), false, HitTarget::SettingsMangoReload),
    ];
    for (i, (icon, title, sub, active, target)) in cards.into_iter().enumerate() {
        let tx = cx + i as f64 * (tw + 10.0);
        let hov = st.hover.as_ref() == Some(&target);
        let bg = if active {
            t.surface_hover
        } else if hov {
            t.surface
        } else {
            t.surface
        };
        let alpha = if active || hov { 0.95 } else { 0.55 };
        fill_rr(cr, tx, r2_y, tw, 50.0, 10.0, bg, alpha);
        if active {
            fill_rr(cr, tx + tw - 9.0, r2_y + 7.0, 4.0, 4.0, 2.0, t.accent, 1.0);
        }
        draw_glyph_centered(cr, icon, tx + 8.0, r2_y + 9.0, 24.0, 32.0, 15.0, if active { t.accent } else { t.muted }, 1.0);
        let text_x = tx + 36.0;
        let max_tw = (tw - 48.0).max(20.0);
        text_left_v(cr, &fit_text(cr, title, 11.5, true, max_tw), text_x, r2_y + 8.0, 16.0, 11.5, true, t.text_bright, 1.0);
        text_left_v(cr, &fit_text(cr, &sub, 10.5, false, max_tw), text_x, r2_y + 25.0, 16.0, 10.5, false, t.muted, 1.0);
        hits.push(HitBox { x: tx, y: r2_y, w: tw, h: 50.0, target, wheel: WheelAction::None });
    }

    // Row 3: Session & Power (separated safely at bottom; destructive actions clearly marked)
    let lbl_y = cy + ch - 56.0;
    hairline_h(cr, cx, lbl_y - 10.0, cw, t, 0.25);
    text_left_v(cr, "Session", cx + 2.0, lbl_y, 16.0, 10.5, true, t.muted, 1.0);
    let py = lbl_y + 22.0;
    let pw = ((cw - 32.0) / 5.0).floor();
    let actions = [
        ("󰌾", "Lock", "lock", false),
        ("󰤄", "Suspend", "suspend", false),
        ("󰍃", "Log Out", "logout", false),
        ("󰜉", "Reboot", "reboot", true),
        ("󰐥", "Shut Down", "shutdown", true),
    ];
    for (i, (icon, label, act, danger)) in actions.iter().enumerate() {
        let px = cx + i as f64 * (pw + 8.0);
        let tgt = HitTarget::SettingsPower(act);
        let hov = st.hover.as_ref() == Some(&tgt);
        tonal_pill(cr, px, py, pw, 32.0, icon, label, false, hov, *danger, false, t, tgt, hits);
    }
}

fn wrap_text(cr: &Context, text: &str, size: f64, bold: bool, max_w: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        if para.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_inclusive(' ') {
            let cand = format!("{line}{word}");
            if !line.is_empty() && text_width(cr, cand.trim_end(), size, bold) > max_w {
                out.push(line.trim_end().to_string());
                line = word.trim_start().to_string();
            } else {
                line = cand;
            }
        }
        if !line.is_empty() {
            out.push(line.trim_end().to_string());
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn draw_antigravity_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;

    // ── Header ──
    fill_rr(cr, cx, cy, cw, 38.0, 10.0, t.surface, 1.0);
    stroke_rr(cr, cx + 0.5, cy + 0.5, cw - 1.0, 37.0, 10.0, t.border, 1.0);
    icon_box(cr, cx + 9.0, cy + 10.0, 18.0, 18.0, 5.0, t.surface_hover, "󰚩", 10.0, t.accent);
    text_left_v(cr, "Antigravity", cx + 33.0, cy, 38.0, 11.5, true, t.text_bright, 1.0);

    // Right-side controls
    let close_t = HitTarget::IslandClose;
    let close_h = st.hover.as_ref() == Some(&close_t);
    fill_rr(cr, cx + cw - 33.0, cy + 7.0, 24.0, 24.0, 7.0, if close_h { t.surface_hover } else { t.surface }, 1.0);
    stroke_rr(cr, cx + cw - 32.5, cy + 7.5, 23.0, 23.0, 7.0, t.border, 1.0);
    draw_glyph_centered(cr, "󰅖", cx + cw - 33.0, cy + 7.0, 24.0, 24.0, 11.0, t.muted, 1.0);
    hits.push(HitBox { x: cx + cw - 33.0, y: cy + 7.0, w: 24.0, h: 24.0, target: close_t, wheel: WheelAction::None });

    let new_t = HitTarget::AgyNewChat;
    let new_h = st.hover.as_ref() == Some(&new_t);
    fill_rr(cr, cx + cw - 61.0, cy + 7.0, 24.0, 24.0, 7.0, if new_h { t.surface_hover } else { t.surface }, 1.0);
    stroke_rr(cr, cx + cw - 60.5, cy + 7.5, 23.0, 23.0, 7.0, t.border, 1.0);
    draw_glyph_centered(cr, "󰐕", cx + cw - 61.0, cy + 7.0, 24.0, 24.0, 11.0, t.text, 1.0);
    hits.push(HitBox { x: cx + cw - 61.0, y: cy + 7.0, w: 24.0, h: 24.0, target: new_t, wheel: WheelAction::None });

    let model = &AGY_MODELS[st.agy_model_idx.min(AGY_MODELS.len() - 1)];
    let effort_idx = st.agy_effort.min(model.efforts.len() - 1);
    let chip = format!("{} · {}", model.label, model.efforts[effort_idx].1);
    let chip_w = text_width(cr, &chip, 9.5, true) + 16.0;
    let chip_t = HitTarget::AgyCycleModel;
    let chip_h = st.hover.as_ref() == Some(&chip_t);
    fill_rr(
        cr,
        cx + cw - 69.0 - chip_w,
        cy + 9.0,
        chip_w,
        20.0,
        6.0,
        if chip_h { t.surface_hover } else { t.bg_deep },
        1.0,
    );
    stroke_rr(cr, cx + cw - 68.5 - chip_w, cy + 9.5, chip_w - 1.0, 19.0, 6.0, t.border, 1.0);
    text_center_v(cr, &chip, cx + cw - 69.0 - chip_w / 2.0, cy + 9.0, 20.0, 9.5, true, t.accent, 1.0);
    hits.push(HitBox {
        x: cx + cw - 69.0 - chip_w,
        y: cy + 9.0,
        w: chip_w,
        h: 20.0,
        target: chip_t,
        wheel: WheelAction::None,
    });

    // ── Effort selector ──
    let ey = cy + 44.0;
    fill_rr(cr, cx, ey, cw, 26.0, 8.0, t.surface, 1.0);
    stroke_rr(cr, cx + 0.5, ey + 0.5, cw - 1.0, 25.0, 8.0, t.border, 0.8);
    text_left_v(cr, "Effort", cx + 10.0, ey, 26.0, 10.0, false, t.muted, 1.0);
    let n = model.efforts.len();
    let pw = if n >= 4 { 42.0 } else { 48.0 };
    let gap = 4.0;
    let mut px = cx + cw - 10.0 - n as f64 * pw - (n.saturating_sub(1)) as f64 * gap;
    for (i, (_, lb)) in model.efforts.iter().enumerate() {
        let target = HitTarget::AgyEffort(i);
        let hov = st.hover.as_ref() == Some(&target);
        let on = effort_idx == i;
        let bg = if on {
            t.accent
        } else if hov {
            t.surface_hover
        } else {
            t.bg_deep
        };
        fill_rr(cr, px, ey + 3.5, pw, 19.0, 6.0, bg, 1.0);
        text_center_v(cr, lb, px + pw / 2.0, ey + 3.5, 19.0, 9.5, on, if on { t.accent_text } else { t.text }, 1.0);
        hits.push(HitBox { x: px, y: ey + 3.5, w: pw, h: 19.0, target, wheel: WheelAction::None });
        px += pw + gap;
    }

    // ── Chat body ──
    let by = cy + 76.0;
    let bh = (ch - 76.0 - 64.0).max(40.0);
    fill_rr(cr, cx, by, cw, bh, 11.0, t.surface, 1.0);
    stroke_rr(cr, cx + 0.5, by + 0.5, cw - 1.0, bh - 1.0, 11.0, t.border, 1.0);

    cr.save().ok();
    rounded_rect(cr, cx + 2.0, by + 2.0, cw - 4.0, bh - 4.0, 9.0);
    cr.clip();

    if st.agy_messages.is_empty() && !st.agy_busy {
        draw_glyph_centered(cr, "󰚩", cx, by + bh / 2.0 - 38.0, cw, 34.0, 30.0, t.accent, 0.85);
        text_center_v(cr, "Antigravity Mini", cx + cw / 2.0, by + bh / 2.0 + 2.0, 18.0, 13.0, true, t.text_bright, 1.0);
        text_center_v(
            cr,
            "Ask anything — press Enter to send",
            cx + cw / 2.0,
            by + bh / 2.0 + 26.0,
            16.0,
            10.0,
            false,
            t.muted,
            1.0,
        );
        cr.restore().ok();
        draw_agy_input(cr, cx, by + bh + 8.0, cw, st, hits);
        return;
    }

    // Lay out bubbles bottom-first so the newest message is always visible.
    let inner_w = cw - 20.0;
    let max_bubble = inner_w * 0.78;
    let size = 10.5;
    let lh = 15.0;
    struct Block {
        lines: Vec<String>,
        user: bool,
        w: f64,
        h: f64,
    }
    let mut blocks: Vec<Block> = Vec::new();
    for m in st.agy_messages {
        let lines = wrap_text(cr, &m.text, size, false, max_bubble - 16.0);
        let w = lines
            .iter()
            .map(|l| text_width(cr, l, size, false))
            .fold(0.0f64, f64::max)
            + 16.0;
        let h = lines.len() as f64 * lh + 12.0;
        blocks.push(Block { lines, user: m.user, w: w.min(max_bubble).max(20.0), h });
    }
    let status_h = if st.agy_busy && !st.agy_status.is_empty() { 16.0 } else { 0.0 };
    let total: f64 = blocks.iter().map(|b| b.h + 6.0).sum::<f64>() + status_h + 8.0;
    let max_off = (total - bh).max(0.0);
    let off = (max_off - st.agy_scroll).clamp(0.0, max_off);
    let mut y = by + 8.0 - off;

    for b in &blocks {
        let bx = if b.user {
            cx + cw - 10.0 - b.w
        } else {
            cx + 10.0
        };
        if y + b.h >= by && y <= by + bh {
            fill_rr(
                cr,
                bx,
                y,
                b.w,
                b.h,
                10.0,
                if b.user { t.accent } else { t.surface_hover },
                1.0,
            );
            let fg = if b.user { t.accent_text } else { t.text_bright };
            for (i, line) in b.lines.iter().enumerate() {
                draw_text(cr, line, bx + 8.0, y + 8.0 + i as f64 * lh + 9.5, size, false, fg, 1.0);
            }
        }
        y += b.h + 6.0;
    }
    if status_h > 0.0 {
        let dots = match st.agy_status {
            "Streaming..." => "Streaming",
            "Thinking..." => "Thinking",
            "Running tool..." => "Running tool",
            other => other,
        };
        let txt = if dots.is_empty() { String::new() } else { format!("󰚩 {}…", dots) };
        text_left_v(cr, &txt, cx + 12.0, y, 16.0, 10.0, false, t.accent, 1.0);
    }
    cr.restore().ok();

    // ── Meta / status line ──
    let meta_y = by + bh + 4.0;
    if st.agy_busy {
        text_left_v(cr, &format!("󰔟 {}", st.agy_status), cx + 4.0, meta_y, 12.0, 9.5, false, t.accent, 1.0);
    } else if !st.agy_meta.is_empty() {
        text_left_v(cr, st.agy_meta, cx + 4.0, meta_y, 12.0, 9.5, false, t.muted, 1.0);
    }

    draw_agy_input(cr, cx, by + bh + 18.0, cw, st, hits);
}

fn draw_agy_input(
    cr: &Context,
    cx: f64,
    y: f64,
    cw: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let h = 36.0;
    fill_rr(cr, cx, y, cw, h, 10.0, t.surface_hover, 1.0);
    stroke_rr(cr, cx + 0.5, y + 0.5, cw - 1.0, h - 1.0, 10.0, t.accent, 0.5);
    let text = if st.agy_input.is_empty() {
        "Message Antigravity…"
    } else {
        st.agy_input
    };
    let col = if st.agy_input.is_empty() { t.muted } else { t.text_bright };
    let avail = cw - 56.0;
    let shown = fit_text(cr, text, 11.0, false, avail);
    text_left_v(cr, &shown, cx + 12.0, y, h, 11.0, false, col, 1.0);
    if !st.agy_input.is_empty() && !st.agy_busy && st.agy_cursor_on {
        let tw = text_width(cr, &shown, 11.0, false);
        fill_rr(cr, cx + 12.0 + tw + 1.0, y + 10.0, 1.5, 16.0, 0.75, t.accent, 0.9);
    }

    let target = HitTarget::AgySend;
    let hov = st.hover.as_ref() == Some(&target);
    let bx = cx + cw - 40.0;
    fill_rr(
        cr,
        bx,
        y + 4.0,
        32.0,
        28.0,
        8.0,
        if hov { t.text_bright } else { t.accent },
        1.0,
    );
    draw_glyph_centered(
        cr,
        if st.agy_busy { "󰔟" } else { "󰒊" },
        bx,
        y + 4.0,
        32.0,
        28.0,
        13.0,
        t.accent_text,
        1.0,
    );
    hits.push(HitBox { x: bx, y: y + 4.0, w: 32.0, h: 28.0, target, wheel: WheelAction::None });
}

fn draw_controls_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;
    let exp = st.cc_expanded;

    // ── 1. Header Bar (Battery summary pill left + DND/Settings/Power right) ──
    let bat_exp = exp == "battery";
    let bat_tgt = HitTarget::CcExpand("battery");
    let bat_hov = st.hover.as_ref() == Some(&bat_tgt);
    let bat_txt = if s.battery_present {
        if s.battery_charging {
            format!("{}% · Charging", s.battery_percent)
        } else {
            format!("{}% · {}", s.battery_percent, s.battery_status)
        }
    } else {
        "AC Power".to_string()
    };
    let bat_ico = if s.battery_charging { "󰂄" } else { "󰁹" };
    let bat_w = (text_width(cr, bat_ico, 13.0, false) + text_width(cr, &bat_txt, 11.0, bat_exp) + 22.0).min(cw - 94.0);
    if bat_exp || bat_hov {
        fill_rr(cr, cx, cy, bat_w, 26.0, 8.0, t.surface_hover, 0.9);
    }
    draw_glyph_centered(cr, bat_ico, cx + 6.0, cy, 16.0, 26.0, 13.0, t.accent, 1.0);
    text_left_v(cr, &fit_text(cr, &bat_txt, 11.0, bat_exp, bat_w - 26.0), cx + 25.0, cy, 26.0, 11.0, bat_exp, t.text_bright, 1.0);
    hits.push(HitBox { x: cx, y: cy, w: bat_w, h: 26.0, target: bat_tgt, wheel: WheelAction::None });

    // Right mini action buttons: DND, Settings, Power
    let btn_w = 26.0;
    let gap = 4.0;
    let mut rx = cx + cw - (btn_w * 3.0 + gap * 2.0);
    let dnd_tgt = HitTarget::NotifDnd;
    let dnd_hov = st.hover.as_ref() == Some(&dnd_tgt);
    if s.dnd_enabled || dnd_hov {
        fill_rr(cr, rx, cy + 1.0, btn_w, 24.0, 7.0, if s.dnd_enabled { t.accent } else { t.surface_hover }, 0.95);
    }
    draw_glyph_centered(cr, if s.dnd_enabled { "󰂛" } else { "󰂚" }, rx, cy + 1.0, btn_w, 24.0, 13.0, if s.dnd_enabled { t.accent_text } else { t.muted }, 1.0);
    hits.push(HitBox { x: rx, y: cy + 1.0, w: btn_w, h: 24.0, target: dnd_tgt, wheel: WheelAction::None });
    rx += btn_w + gap;

    let set_tgt = HitTarget::BarPopup(PopupKind::Settings);
    let set_hov = st.hover.as_ref() == Some(&set_tgt);
    if set_hov {
        fill_rr(cr, rx, cy + 1.0, btn_w, 24.0, 7.0, t.surface_hover, 0.95);
    }
    draw_glyph_centered(cr, "󰒓", rx, cy + 1.0, btn_w, 24.0, 13.0, if set_hov { t.text_bright } else { t.muted }, 1.0);
    hits.push(HitBox { x: rx, y: cy + 1.0, w: btn_w, h: 24.0, target: set_tgt, wheel: WheelAction::None });
    rx += btn_w + gap;

    let pwr_exp = exp == "power";
    let pwr_tgt = HitTarget::CcExpand("power");
    let pwr_hov = st.hover.as_ref() == Some(&pwr_tgt);
    if pwr_exp || pwr_hov {
        fill_rr(cr, rx, cy + 1.0, btn_w, 24.0, 7.0, if pwr_exp { Rgb(0.32, 0.19, 0.22) } else { t.surface_hover }, 0.95);
    }
    draw_glyph_centered(cr, "󰐥", rx, cy + 1.0, btn_w, 24.0, 13.0, if pwr_exp { t.red } else if pwr_hov { t.text_bright } else { t.muted }, 1.0);
    hits.push(HitBox { x: rx, y: cy + 1.0, w: btn_w, h: 24.0, target: pwr_tgt, wheel: WheelAction::None });

    // ── 2. Connectivity Tiles (Wi-Fi + Bluetooth: icon toggles, body expands) ──
    let ty = cy + 34.0;
    let tw = ((cw - 8.0) / 2.0).floor();
    let th = 46.0;

    let wifi_sub = if !s.wifi_enabled {
        "Off".to_string()
    } else if s.wifi_ssid.is_empty() {
        "On".to_string()
    } else {
        s.wifi_ssid.clone()
    };
    let bt_sub = if !s.bt_powered {
        "Off".to_string()
    } else if s.bt_connected > 0 {
        format!("{} conn", s.bt_connected)
    } else {
        "On".to_string()
    };

    let tiles = [
        ("wifi", if s.wifi_enabled { "󰤨" } else { "󰤭" }, "Wi-Fi", wifi_sub, s.wifi_enabled, HitTarget::WifiToggle),
        ("bt", if s.bt_powered { "󰂯" } else { "󰂲" }, "Bluetooth", bt_sub, s.bt_powered, HitTarget::BtToggle),
    ];
    for (i, (sec_id, ico, title, sub, on, toggle_tgt)) in tiles.into_iter().enumerate() {
        let tx = cx + i as f64 * (tw + 8.0);
        let is_exp = exp == sec_id;
        let exp_tgt = HitTarget::CcExpand(sec_id);
        let ico_hov = st.hover.as_ref() == Some(&toggle_tgt);
        let body_hov = st.hover.as_ref() == Some(&exp_tgt);
        let bg = if is_exp || body_hov { t.surface_hover } else { t.surface };
        fill_rr(cr, tx, ty, tw, th, 10.0, bg, if is_exp { 0.95 } else { 0.62 });
        if is_exp {
            fill_rr(cr, tx + 16.0, ty + th - 2.5, tw - 32.0, 2.5, 1.2, t.accent, 1.0);
        }

        // Left circular icon button = instant toggle on/off
        let c_bg = if on {
            if ico_hov { t.text_bright } else { t.accent }
        } else if ico_hov {
            t.surface_active
        } else {
            t.bg_deep
        };
        let c_fg = if on { t.accent_text } else { t.muted };
        fill_rr(cr, tx + 8.0, ty + 8.0, 30.0, 30.0, 15.0, c_bg, 1.0);
        draw_glyph_centered(cr, ico, tx + 8.0, ty + 8.0, 30.0, 30.0, 14.0, c_fg, 1.0);
        hits.push(HitBox { x: tx + 6.0, y: ty + 4.0, w: 34.0, h: th - 8.0, target: toggle_tgt, wheel: WheelAction::None });

        // Right label + chevron = expand/collapse section
        let lx = tx + 45.0;
        let max_lw = (tw - 66.0).max(20.0);
        text_left_v(cr, title, lx, ty + 6.0, 16.0, 11.5, true, t.text_bright, 1.0);
        text_left_v(cr, &fit_text(cr, &sub, 10.0, false, max_lw), lx, ty + 23.0, 15.0, 10.0, false, if on { t.text } else { t.muted }, 0.9);
        draw_glyph_centered(cr, if is_exp { "󰅀" } else { "󰅂" }, tx + tw - 20.0, ty, 16.0, th, 11.5, if is_exp { t.accent } else { t.muted }, 1.0);
        hits.push(HitBox { x: tx + 40.0, y: ty, w: tw - 40.0, h: th, target: exp_tgt, wheel: WheelAction::None });
    }

    // ── 3. Display & Sound Sliders Panel ───────────────────────────────
    let sy = ty + th + 8.0;
    fill_rr(cr, cx, sy, cw, 92.0, 11.0, t.surface, 0.58);
    let sx_pad = cx + 11.0;
    let sw_pad = cw - 22.0;

    let (bsx, bsw) = slim_slider(
        cr,
        sx_pad,
        sy + 8.0,
        sw_pad,
        "Brightness",
        &format!("{}%", s.brightness_percent),
        s.brightness_percent as f64,
        5.0,
        100.0,
        "󰃠",
        t,
    );
    hits.push(HitBox { x: bsx, y: sy + 20.0, w: bsw, h: 24.0, target: HitTarget::ControlsBrightSlider(bsx, bsw), wheel: WheelAction::Brightness });

    hairline_h(cr, sx_pad, sy + 46.0, sw_pad, t, 0.22);

    // Sound slider + clickable output device switcher chip in header
    let vy = sy + 52.0;
    let v_ico = if s.volume_muted || s.volume_percent == 0 { "󰝟" } else { "󰕾" };
    let vol_v = if s.volume_muted { "Muted".to_string() } else { format!("{}%", s.volume_percent) };
    let (vsx, vsw) = slim_slider(
        cr,
        sx_pad,
        vy,
        sw_pad,
        "Sound",
        &vol_v,
        if s.volume_muted { 0.0 } else { s.volume_percent as f64 },
        0.0,
        100.0,
        v_ico,
        t,
    );
    // Output expander trigger on Sound header row between "Sound" and "%"
    let audio_exp = exp == "audio";
    let aud_tgt = HitTarget::CcExpand("audio");
    let aud_hov = st.hover.as_ref() == Some(&aud_tgt);
    let cur_out = s
        .audio_outputs
        .iter()
        .find(|o| o.active)
        .map(|o| o.label.as_str())
        .unwrap_or("Output");
    let pct_w = text_width(cr, &vol_v, 11.0, true);
    let max_out_w = (sw_pad - 58.0 - pct_w).clamp(40.0, 120.0);
    let out_lbl = format!("{} {}", fit_text(cr, cur_out, 10.0, false, max_out_w - 14.0), if audio_exp { "󰅀" } else { "󰅂" });
    let out_w = text_width(cr, &out_lbl, 10.0, audio_exp) + 10.0;
    let out_x = sx_pad + sw_pad - pct_w - 8.0 - out_w;
    if audio_exp || aud_hov {
        fill_rr(cr, out_x, vy - 1.0, out_w, 16.0, 5.0, t.surface_hover, 0.95);
    }
    text_center_v(cr, &out_lbl, out_x + out_w / 2.0, vy - 1.0, 16.0, 10.0, audio_exp, if audio_exp { t.accent } else { t.muted }, 1.0);
    hits.push(HitBox { x: out_x, y: vy - 2.0, w: out_w, h: 18.0, target: aud_tgt, wheel: WheelAction::None });

    hits.push(HitBox { x: sx_pad, y: vy + 14.0, w: 20.0, h: 24.0, target: HitTarget::ControlsMuteToggle, wheel: WheelAction::Volume });
    hits.push(HitBox { x: vsx, y: vy + 14.0, w: vsw, h: 24.0, target: HitTarget::ControlsVolSlider(vsx, vsw), wheel: WheelAction::Volume });

    // ── 4. Expandable Accordion Drawer (appears only for active section) ──
    let dy = sy + 92.0 + 8.0;
    match exp {
        "wifi" => {
            if !s.wifi_enabled {
                fill_rr(cr, cx, dy, cw, 46.0, 10.0, t.surface, 0.58);
                text_left_v(cr, "Wi-Fi is turned off", cx + 12.0, dy, 46.0, 11.0, false, t.muted, 1.0);
                let on_t = HitTarget::WifiToggle;
                tonal_pill(cr, cx + cw - 78.0, dy + 10.0, 68.0, 26.0, "", "Turn On", true, st.hover.as_ref() == Some(&on_t), false, false, t, on_t, hits);
                return;
            }
            text_left_v(cr, "Networks", cx + 2.0, dy, 22.0, 10.5, true, t.muted, 1.0);
            let res_t = HitTarget::WifiRescan;
            let res_h = st.hover.as_ref() == Some(&res_t);
            tonal_pill(cr, cx + cw - 70.0, dy, 70.0, 22.0, if s.wifi_scanning { "󰑐" } else { "󰚰" }, if s.wifi_scanning { "Scan…" } else { "Rescan" }, s.wifi_scanning, res_h, false, false, t, res_t, hits);

            let mut wy = dy + 28.0;
            if !s.wifi_status_msg.is_empty() {
                text_left_v(cr, &fit_text(cr, &s.wifi_status_msg, 10.0, false, cw), cx + 2.0, wy, 14.0, 10.0, false, t.accent, 1.0);
                wy += 20.0;
            }
            if !st.wifi_selected_ssid.is_empty() {
                fill_rr(cr, cx, wy, cw, 58.0, 10.0, t.surface, 0.72);
                text_left_v(cr, &fit_text(cr, &format!("Password · {}", st.wifi_selected_ssid), 10.5, true, cw - 32.0), cx + 10.0, wy + 3.0, 16.0, 10.5, true, t.text_bright, 1.0);
                let ct = HitTarget::WifiPassClose;
                if st.hover.as_ref() == Some(&ct) {
                    fill_rr(cr, cx + cw - 24.0, wy + 3.0, 18.0, 18.0, 5.0, t.surface_hover, 1.0);
                }
                draw_glyph_centered(cr, "󰅖", cx + cw - 24.0, wy + 3.0, 18.0, 18.0, 10.5, t.muted, 1.0);
                hits.push(HitBox { x: cx + cw - 24.0, y: wy + 3.0, w: 18.0, h: 18.0, target: ct, wheel: WheelAction::None });

                let in_w = (cw - 88.0).max(60.0);
                fill_rr(cr, cx + 8.0, wy + 24.0, in_w, 26.0, 7.0, t.bg_deep, 1.0);
                let nc = st.wifi_password.chars().count();
                let masked = if nc == 0 {
                    if st.agy_cursor_on { "|".to_string() } else { "Password...".to_string() }
                } else {
                    let mut d = "•".repeat(nc.min(22));
                    if st.agy_cursor_on { d.push('|'); }
                    d
                };
                text_left_v(cr, &masked, cx + 16.0, wy + 24.0, 26.0, 11.0, false, if nc == 0 && !st.agy_cursor_on { t.muted } else { t.text_bright }, 1.0);
                hits.push(HitBox { x: cx + 8.0, y: wy + 24.0, w: in_w, h: 26.0, target: HitTarget::WifiPassInput, wheel: WheelAction::None });
                let jt = HitTarget::WifiPassConnect;
                tonal_pill(cr, cx + cw - 72.0, wy + 24.0, 64.0, 26.0, "", "Join", true, st.hover.as_ref() == Some(&jt), false, false, t, jt, hits);
                wy += 66.0;
            }

            let list_h = 160.0;
            cr.save().ok();
            rounded_rect(cr, cx, wy, cw, list_h, 8.0);
            cr.clip();
            let mut ry = wy - st.wifi_scroll;
            for (i, net) in s.wifi_networks.iter().enumerate() {
                if ry + 32.0 < wy { ry += 36.0; continue; }
                if ry > wy + list_h { break; }
                let tgt = HitTarget::WifiRow(i);
                let hov = st.hover.as_ref() == Some(&tgt);
                if net.active || hov {
                    fill_rr(cr, cx, ry, cw, 32.0, 8.0, if net.active { t.surface_hover } else { t.surface }, 0.9);
                }
                if net.active {
                    fill_rr(cr, cx + 2.0, ry + 7.0, 3.0, 18.0, 1.5, t.accent, 1.0);
                }
                draw_glyph_centered(cr, signal_icon(net.signal), cx + 9.0, ry, 18.0, 32.0, 13.0, if net.active { t.accent } else { t.muted }, 1.0);
                text_left_v(cr, &fit_text(cr, &net.ssid, 11.0, net.active, cw - 106.0), cx + 32.0, ry, 32.0, 11.0, net.active, if net.active { t.text_bright } else { t.text }, 1.0);
                let main = if net.active { "Connected".to_string() } else { format!("{}%", net.signal) };
                text_right_v(cr, &main, cx + cw - 8.0, ry, 32.0, 10.0, net.active, if net.active { t.accent } else { t.muted }, 1.0);
                if !net.active && net.saved {
                    let mw = text_width(cr, &main, 10.0, false);
                    text_right_v(cr, "Saved", cx + cw - 8.0 - mw - 6.0, ry, 32.0, 9.5, false, t.muted, 0.85);
                } else if !net.active && !net.saved && !net.security.is_empty() && net.security != "--" {
                    let mw = text_width(cr, &main, 10.0, false);
                    draw_glyph_centered(cr, "󰌾", cx + cw - 8.0 - mw - 18.0, ry, 14.0, 32.0, 10.0, t.muted, 0.8);
                }
                let hy = ry.max(wy);
                let hh = (ry + 32.0).min(wy + list_h) - hy;
                if hh > 4.0 {
                    hits.push(HitBox { x: cx, y: hy, w: cw, h: hh, target: tgt, wheel: WheelAction::None });
                }
                ry += 36.0;
            }
            cr.restore().ok();
        }
        "bt" => {
            if !s.bt_powered {
                fill_rr(cr, cx, dy, cw, 46.0, 10.0, t.surface, 0.58);
                text_left_v(cr, "Bluetooth is turned off", cx + 12.0, dy, 46.0, 11.0, false, t.muted, 1.0);
                let on_t = HitTarget::BtToggle;
                tonal_pill(cr, cx + cw - 78.0, dy + 10.0, 68.0, 26.0, "", "Turn On", true, st.hover.as_ref() == Some(&on_t), false, false, t, on_t, hits);
                return;
            }
            text_left_v(cr, "Devices", cx + 2.0, dy, 22.0, 10.5, true, t.muted, 1.0);
            let pt = HitTarget::BtScan;
            tonal_pill(cr, cx + cw - 64.0, dy, 64.0, 22.0, if s.bt_scanning { "󰑐" } else { "󰐕" }, if s.bt_scanning { "Stop" } else { "Scan" }, s.bt_scanning, st.hover.as_ref() == Some(&pt), false, false, t, pt, hits);

            let mut wy = dy + 28.0;
            if !s.bt_status_msg.is_empty() {
                text_left_v(cr, &fit_text(cr, &s.bt_status_msg, 10.0, false, cw), cx + 2.0, wy, 14.0, 10.0, false, t.accent, 1.0);
                wy += 20.0;
            }
            if s.bt_paired.is_empty() && s.bt_available.is_empty() && !s.bt_scanning {
                fill_rr(cr, cx, wy, cw, 42.0, 10.0, t.surface, 0.55);
                text_center_v(cr, "No paired devices · Click Scan", cx + cw / 2.0, wy, 42.0, 10.5, false, t.muted, 1.0);
                return;
            }
            let list_h = 160.0;
            cr.save().ok();
            rounded_rect(cr, cx, wy, cw, list_h, 8.0);
            cr.clip();
            let mut ry = wy - st.bt_scroll;
            if !s.bt_paired.is_empty() {
                if ry + 14.0 >= wy && ry <= wy + list_h {
                    text_left_v(cr, "Saved", cx + 4.0, ry, 14.0, 9.5, true, t.muted, 0.85);
                }
                ry += 18.0;
                for (i, d) in s.bt_paired.iter().enumerate() {
                    if ry + 32.0 < wy { ry += 36.0; continue; }
                    if ry > wy + list_h { break; }
                    let tgt = HitTarget::BtPairedRow(i);
                    let hov = st.hover.as_ref() == Some(&tgt);
                    if d.connected || hov {
                        fill_rr(cr, cx, ry, cw, 32.0, 8.0, if d.connected { t.surface_hover } else { t.surface }, 0.9);
                    }
                    if d.connected {
                        fill_rr(cr, cx + 2.0, ry + 7.0, 3.0, 18.0, 1.5, t.accent, 1.0);
                    }
                    draw_glyph_centered(cr, if d.connected { "󰂱" } else { "󰂯" }, cx + 9.0, ry, 18.0, 32.0, 13.0, if d.connected { t.accent } else { t.muted }, 1.0);
                    text_left_v(cr, &fit_text(cr, &d.name, 11.0, d.connected, cw - 114.0), cx + 32.0, ry, 32.0, 11.0, d.connected, if d.connected { t.text_bright } else { t.text }, 1.0);
                    text_right_v(cr, if d.connected { "Connected" } else { "Connect" }, cx + cw - 28.0, ry, 32.0, 10.0, d.connected, if d.connected { t.accent } else { t.muted }, 1.0);
                    let ft = HitTarget::BtForget(i);
                    let fh = st.hover.as_ref() == Some(&ft);
                    if fh {
                        fill_rr(cr, cx + cw - 24.0, ry + 6.0, 20.0, 20.0, 5.0, t.surface_active, 1.0);
                    }
                    draw_glyph_centered(cr, "󰅖", cx + cw - 24.0, ry + 6.0, 20.0, 20.0, 10.5, if fh { t.red } else { t.muted }, 1.0);
                    let hy = ry.max(wy);
                    let hh = (ry + 32.0).min(wy + list_h) - hy;
                    if hh > 4.0 {
                        hits.push(HitBox { x: cx + 4.0, y: hy, w: cw - 30.0, h: hh, target: tgt, wheel: WheelAction::None });
                        hits.push(HitBox { x: cx + cw - 24.0, y: (ry + 6.0).max(wy), w: 20.0, h: 20.0, target: ft, wheel: WheelAction::None });
                    }
                    ry += 36.0;
                }
            }
            if !s.bt_available.is_empty() || s.bt_scanning {
                if !s.bt_paired.is_empty() { ry += 4.0; }
                if ry + 14.0 >= wy && ry <= wy + list_h {
                    text_left_v(cr, if s.bt_scanning { "Scanning..." } else { "Available" }, cx + 4.0, ry, 14.0, 9.5, true, t.muted, 0.85);
                }
                ry += 18.0;
                for (i, d) in s.bt_available.iter().enumerate() {
                    if ry + 32.0 < wy { ry += 36.0; continue; }
                    if ry > wy + list_h { break; }
                    let tgt = HitTarget::BtPairRow(i);
                    if st.hover.as_ref() == Some(&tgt) {
                        fill_rr(cr, cx, ry, cw, 32.0, 8.0, t.surface, 0.9);
                    }
                    draw_glyph_centered(cr, "󰂯", cx + 9.0, ry, 18.0, 32.0, 13.0, t.muted, 1.0);
                    text_left_v(cr, &fit_text(cr, &d.name, 11.0, false, cw - 76.0), cx + 32.0, ry, 32.0, 11.0, false, t.text, 1.0);
                    text_right_v(cr, "Pair", cx + cw - 8.0, ry, 32.0, 10.0, true, t.accent, 1.0);
                    let hy = ry.max(wy);
                    let hh = (ry + 32.0).min(wy + list_h) - hy;
                    if hh > 4.0 {
                        hits.push(HitBox { x: cx, y: hy, w: cw, h: hh, target: tgt, wheel: WheelAction::None });
                    }
                    ry += 36.0;
                }
            }
            cr.restore().ok();
        }
        "audio" => {
            text_left_v(cr, "Output device", cx + 2.0, dy, 16.0, 10.5, true, t.muted, 1.0);
            let mut oy = dy + 20.0;
            for (i, out) in s.audio_outputs.iter().enumerate() {
                let tgt = HitTarget::VolumeOutput(i);
                let hov = st.hover.as_ref() == Some(&tgt);
                if out.active || hov {
                    fill_rr(cr, cx, oy, cw, 32.0, 8.0, if out.active { t.surface_hover } else { t.surface }, 0.92);
                }
                if out.active {
                    fill_rr(cr, cx + 2.0, oy + 7.0, 3.0, 18.0, 1.5, t.accent, 1.0);
                }
                let lower = out.label.to_lowercase();
                let g = if lower.contains("headphone") { "󰋋" } else { "󰓃" };
                draw_glyph_centered(cr, g, cx + 9.0, oy, 18.0, 32.0, 13.0, if out.active { t.accent } else { t.muted }, 1.0);
                text_left_v(cr, &fit_text(cr, &out.label, 11.0, out.active, cw - 60.0), cx + 32.0, oy, 32.0, 11.0, out.active, if out.active { t.text_bright } else { t.text }, 1.0);
                if out.active {
                    draw_glyph_centered(cr, "󰄬", cx + cw - 24.0, oy, 16.0, 32.0, 12.5, t.accent, 1.0);
                }
                hits.push(HitBox { x: cx, y: oy, w: cw, h: 32.0, target: tgt, wheel: WheelAction::None });
                oy += 36.0;
            }
        }
        "battery" => {
            fill_rr(cr, cx, dy, cw, 54.0, 10.0, t.surface, 0.58);
            settings_stat_bar(cr, cx + 10.0, dy + 11.0, cw - 20.0, s.battery_percent as f64 / 100.0, t.accent, t);
            let det = if s.battery_detail.is_empty() { "On battery" } else { &s.battery_detail };
            text_left_v(cr, &fit_text(cr, det, 10.0, false, cw - 20.0), cx + 10.0, dy + 19.0, 14.0, 10.0, false, t.text, 0.95);
            let prof = format!("{}  ·  {}  ·  {}", s.cpu_freq_ghz, s.cpu_temp, s.power_profile);
            text_left_v(cr, &fit_text(cr, &prof, 10.0, false, cw - 20.0), cx + 10.0, dy + 35.0, 14.0, 10.0, false, t.muted, 0.9);
        }
        "power" => {
            // Safe row (3) + Destructive row (2)
            let row1 = [("󰌾", "Lock", "lock", false), ("󰤄", "Suspend", "suspend", false), ("󰍃", "Log Out", "logout", false)];
            let w3 = ((cw - 12.0) / 3.0).floor();
            for (i, (ico, lbl, act, danger)) in row1.into_iter().enumerate() {
                let px = cx + i as f64 * (w3 + 6.0);
                let tgt = HitTarget::PowerAction(act);
                tonal_pill(cr, px, dy, w3, 28.0, ico, lbl, false, st.hover.as_ref() == Some(&tgt), danger, false, t, tgt, hits);
            }
            let row2 = [("󰜉", "Restart", "reboot", true), ("󰐥", "Power Off", "shutdown", true)];
            let w2 = ((cw - 6.0) / 2.0).floor();
            for (i, (ico, lbl, act, danger)) in row2.into_iter().enumerate() {
                let px = cx + i as f64 * (w2 + 6.0);
                let tgt = HitTarget::PowerAction(act);
                tonal_pill(cr, px, dy + 34.0, w2, 28.0, ico, lbl, false, st.hover.as_ref() == Some(&tgt), danger, false, t, tgt, hits);
            }
        }
        _ => {}
    }
}

fn fill_rr(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, col: Rgb, alpha: f64) {
    rounded_rect(cr, x, y, w, h, r);
    cr.set_source_rgba(col.0, col.1, col.2, alpha);
    let _ = cr.fill();
}

fn stroke_rr(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, col: Rgb, alpha: f64) {
    rounded_rect(cr, x, y, w, h, r);
    cr.set_source_rgba(col.0, col.1, col.2, alpha);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

fn text_left_v(cr: &Context, text: &str, x: f64, y: f64, h: f64, size: f64, bold: bool, col: Rgb, alpha: f64) {
    draw_text(cr, text, x, y + h / 2.0 + size * 0.35, size, bold, col, alpha);
}

fn text_right_v(cr: &Context, text: &str, right: f64, y: f64, h: f64, size: f64, bold: bool, col: Rgb, alpha: f64) {
    let w = text_width(cr, text, size, bold);
    draw_text(cr, text, right - w, y + h / 2.0 + size * 0.35, size, bold, col, alpha);
}

fn text_center_v(cr: &Context, text: &str, cx: f64, y: f64, h: f64, size: f64, bold: bool, col: Rgb, alpha: f64) {
    let w = text_width(cr, text, size, bold);
    draw_text(cr, text, cx - w / 2.0, y + h / 2.0 + size * 0.35, size, bold, col, alpha);
}

#[allow(clippy::too_many_arguments)]
fn icon_box(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: f64,
    bg: Rgb,
    glyph: &str,
    glyph_size: f64,
    fg: Rgb,
) {
    fill_rr(cr, x, y, w, h, r, bg, 1.0);
    draw_glyph_centered(cr, glyph, x, y, w, h, glyph_size, fg, 1.0);
}

fn draw_power_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let safe_items = [
        ("󰌾", "Lock", "lock"),
        ("󰤄", "Suspend", "suspend"),
        ("󰍃", "Log Out", "logout"),
    ];
    let row_h = 30.0;
    let mut y = cy;
    for (icon, label, act) in safe_items {
        let tgt = HitTarget::PowerAction(act);
        let hov = st.hover.as_ref() == Some(&tgt);
        if hov {
            fill_rr(cr, cx, y, cw, row_h, 8.0, t.surface_hover, 0.95);
        }
        draw_glyph_centered(cr, icon, cx + 6.0, y, 22.0, row_h, 13.5, if hov { t.accent } else { t.muted }, 1.0);
        text_left_v(cr, label, cx + 34.0, y, row_h, 11.5, hov, if hov { t.text_bright } else { t.text }, 1.0);
        hits.push(HitBox { x: cx, y, w: cw, h: row_h, target: tgt, wheel: WheelAction::None });
        y += row_h + 2.0;
    }

    hairline_h(cr, cx + 4.0, y + 3.0, cw - 8.0, t, 0.3);
    y += 9.0;

    for (icon, label, act) in [("󰜉", "Restart", "reboot"), ("󰐥", "Power Off", "shutdown")] {
        let tgt = HitTarget::PowerAction(act);
        let hov = st.hover.as_ref() == Some(&tgt);
        if hov {
            fill_rr(cr, cx, y, cw, row_h, 8.0, Rgb(0.30, 0.18, 0.21), 0.95);
        }
        draw_glyph_centered(cr, icon, cx + 6.0, y, 22.0, row_h, 13.5, if hov { t.red } else { t.muted }, 1.0);
        text_left_v(cr, label, cx + 34.0, y, row_h, 11.5, hov, if hov { t.red } else { t.text }, 1.0);
        hits.push(HitBox { x: cx, y, w: cw, h: row_h, target: tgt, wheel: WheelAction::None });
        y += row_h + 2.0;
    }
}

fn draw_brightness_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let (tx, tw) = slim_slider(
        cr,
        cx,
        cy + 2.0,
        cw,
        "Brightness",
        &format!("{}%", st.stats.brightness_percent),
        st.stats.brightness_percent as f64,
        5.0,
        100.0,
        "󰃠",
        t,
    );
    hits.push(HitBox {
        x: tx,
        y: cy + 16.0,
        w: tw,
        h: 24.0,
        target: HitTarget::BrightnessSlider(tx, tw),
        wheel: WheelAction::Brightness,
    });
}

fn draw_battery_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    _hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;

    let icon = if s.battery_charging { "󰂄" } else { "󰁹" };
    draw_glyph_centered(cr, icon, cx, cy, 20.0, 22.0, 15.0, t.accent, 1.0);
    text_left_v(cr, &s.battery_status, cx + 24.0, cy, 22.0, 12.0, true, t.text_bright, 1.0);
    text_right_v(cr, &format!("{}%", s.battery_percent), cx + cw, cy, 22.0, 14.0, true, t.accent, 1.0);

    settings_stat_bar(cr, cx, cy + 28.0, cw, s.battery_percent as f64 / 100.0, t.accent, t);

    let detail = if s.battery_detail.is_empty() { "On battery" } else { &s.battery_detail };
    text_left_v(cr, &fit_text(cr, detail, 10.5, false, cw), cx, cy + 37.0, 15.0, 10.5, false, t.muted, 1.0);

    hairline_h(cr, cx, cy + 57.0, cw, t, 0.28);
    let profile_line = format!("{}  ·  {}  ·  {}", s.cpu_freq_ghz, s.cpu_temp, s.power_profile);
    text_left_v(cr, &fit_text(cr, &profile_line, 10.0, false, cw), cx, cy + 63.0, 15.0, 10.0, false, t.muted, 0.9);
}

fn draw_volume_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;

    text_left_v(cr, "Sound", cx, cy, 22.0, 12.5, true, t.text_bright, 1.0);
    let mute_tgt = HitTarget::VolumeMute;
    let mute_hov = st.hover.as_ref() == Some(&mute_tgt);
    let ico = if s.volume_muted { "󰝟" } else { "󰕾" };
    let label = if s.volume_muted { "Muted" } else { "Mute" };
    tonal_pill(cr, cx + cw - 64.0, cy, 64.0, 22.0, ico, label, s.volume_muted, mute_hov, false, false, t, mute_tgt, hits);

    let sy = cy + 28.0;
    let vol = if s.volume_muted { 0.0 } else { s.volume_percent as f64 };
    let v_ico = if s.volume_muted {
        "󰝟"
    } else if s.volume_percent >= 50 {
        "󰕾"
    } else {
        "󰖀"
    };
    let (tx, tw) = slim_slider(cr, cx, sy, cw, "Output volume", &format!("{}%", s.volume_percent), vol, 0.0, 100.0, v_ico, t);
    hits.push(HitBox {
        x: tx,
        y: sy + 14.0,
        w: tw,
        h: 24.0,
        target: HitTarget::VolumeSlider(tx, tw),
        wheel: WheelAction::Volume,
    });

    hairline_h(cr, cx, sy + 44.0, cw, t, 0.25);

    let mut oy = sy + 52.0;
    for (i, out) in s.audio_outputs.iter().enumerate() {
        let tgt = HitTarget::VolumeOutput(i);
        let hov = st.hover.as_ref() == Some(&tgt);
        if out.active || hov {
            fill_rr(cr, cx, oy, cw, 34.0, 8.0, if out.active { t.surface_hover } else { t.surface }, 0.92);
        }
        if out.active {
            fill_rr(cr, cx + 2.0, oy + 8.0, 3.0, 18.0, 1.5, t.accent, 1.0);
        }
        let lower = out.label.to_lowercase();
        let g = if lower.contains("headphone") { "󰋋" } else { "󰓃" };
        draw_glyph_centered(cr, g, cx + 10.0, oy, 20.0, 34.0, 13.5, if out.active { t.accent } else { t.muted }, 1.0);
        let fg = if out.active { t.text_bright } else { t.text };
        let lbl = fit_text(cr, &out.label, 11.0, out.active, cw - 62.0);
        text_left_v(cr, &lbl, cx + 36.0, oy, 34.0, 11.0, out.active, fg, 1.0);
        if out.active {
            draw_glyph_centered(cr, "󰄬", cx + cw - 24.0, oy, 16.0, 34.0, 13.0, t.accent, 1.0);
        }
        hits.push(HitBox { x: cx, y: oy, w: cw, h: 34.0, target: tgt, wheel: WheelAction::None });
        oy += 38.0;
    }
}

fn fmt_time(sec: f64) -> String {
    if !sec.is_finite() || sec <= 0.0 {
        return "0:00".into();
    }
    let total = sec.floor() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn media_source(player: &str) -> (String, String) {
    let p = player.to_lowercase();
    let (icon, label) = if p.contains("spotify") {
        ("󰓇", "Spotify")
    } else if p.contains("firefox") {
        ("󰈹", "Firefox")
    } else if p.contains("chromium") || p.contains("chrome") {
        ("󰊯", "Chromium")
    } else if p.contains("mpv") {
        ("󰕼", "mpv")
    } else if p.contains("vlc") {
        ("󰕼", "VLC")
    } else if p.contains("music") || p.contains("amberol") {
        ("󰎆", "Music")
    } else {
        ("󰎈", "Media")
    };
    (icon.to_string(), label.to_string())
}

fn draw_media_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;
    let players = &s.media_players;

    if players.is_empty() {
        draw_glyph_centered(cr, "󰝚", cx + 4.0, cy + 4.0, 28.0, 30.0, 17.0, t.muted, 1.0);
        text_left_v(cr, "Nothing playing", cx + 38.0, cy + 4.0, 16.0, 11.5, true, t.text_bright, 1.0);
        text_left_v(cr, "Start playback in an app", cx + 38.0, cy + 20.0, 14.0, 10.0, false, t.muted, 1.0);
        return;
    }

    let player = &s.media;
    let mut y = cy;

    if players.len() > 1 {
        let mut chip_x = cx;
        for (i, p) in players.iter().enumerate() {
            let (icon, label) = media_source(&p.player);
            let is_cur = p.bus == player.bus;
            let tw = text_width(cr, &label, 9.5, is_cur);
            let iw = text_width(cr, &icon, 10.0, false);
            let chip_w = iw + 4.0 + tw + 12.0;
            let tgt = HitTarget::MediaPlayerChip(i);
            let hov = st.hover.as_ref() == Some(&tgt);
            fill_rr(cr, chip_x, y, chip_w, 20.0, 6.0, if is_cur { t.accent } else if hov { t.surface_hover } else { t.surface }, 0.9);
            let fg = if is_cur { t.accent_text } else { t.text };
            draw_text(cr, &icon, chip_x + 6.0, y + 13.5, 10.0, false, fg, 1.0);
            draw_text(cr, &label, chip_x + 6.0 + iw + 4.0, y + 13.2, 9.5, is_cur, fg, 1.0);
            hits.push(HitBox { x: chip_x, y, w: chip_w, h: 20.0, target: tgt, wheel: WheelAction::None });
            chip_x += chip_w + 5.0;
        }
        y += 26.0;
    }

    fill_rr(cr, cx, y, 46.0, 46.0, 8.0, t.surface, 0.8);
    let (src_icon, src_label) = media_source(&player.player);
    if let Some(surf) = st.media_art_surf {
        cr.save().ok();
        rounded_rect(cr, cx, y, 46.0, 46.0, 8.0);
        cr.clip();
        draw_surface_fitted(cr, surf, cx, y, 46.0, 46.0, 1.0);
        cr.restore().ok();
    } else {
        draw_glyph_centered(cr, &src_icon, cx, y, 46.0, 46.0, 20.0, t.muted, 1.0);
    }

    let tx = cx + 56.0;
    let tw = (cw - 56.0).max(10.0);
    let title = if player.title.is_empty() { "Unknown Title" } else { &player.title };
    text_left_v(cr, &fit_text(cr, title, 11.5, true, tw - 24.0), tx, y + 4.0, 18.0, 11.5, true, t.text_bright, 1.0);

    let raise_t = HitTarget::MediaRaise;
    let raise_h = st.hover.as_ref() == Some(&raise_t);
    if raise_h {
        fill_rr(cr, tx + tw - 20.0, y + 3.0, 20.0, 20.0, 6.0, t.surface_hover, 1.0);
    }
    draw_glyph_centered(cr, "󰖯", tx + tw - 20.0, y + 3.0, 20.0, 20.0, 11.0, if raise_h { t.text_bright } else { t.muted }, 1.0);
    hits.push(HitBox { x: tx + tw - 20.0, y: y + 3.0, w: 20.0, h: 20.0, target: raise_t, wheel: WheelAction::None });

    let artist = if !player.artist.is_empty() {
        player.artist.clone()
    } else if !player.album.is_empty() {
        player.album.clone()
    } else {
        src_label.clone()
    };
    let meta = format!("{} · {}", artist, src_label);
    text_left_v(cr, &fit_text(cr, &meta, 10.0, false, tw), tx, y + 24.0, 16.0, 10.0, false, t.muted, 1.0);
    y += 54.0;

    if player.length > 0.0 {
        let elapsed = fmt_time(player.position);
        let total = fmt_time(player.length);
        let ew = text_width(cr, &elapsed, 9.0, false);
        let tw2 = text_width(cr, &total, 9.0, false);
        draw_text(cr, &elapsed, cx, y + 10.0, 9.0, false, t.muted, 1.0);
        let track_x = cx + ew + 8.0;
        let track_w = (cw - ew - tw2 - 16.0).max(40.0);
        let seek_tgt = HitTarget::MediaSeek(track_x, track_w);
        let seek_hov = st.hover.as_ref() == Some(&seek_tgt);
        let bar_h = if seek_hov { 4.5 } else { 3.0 };
        let bar_y = y + (14.0 - bar_h) / 2.0;
        fill_rr(cr, track_x, bar_y, track_w, bar_h, bar_h / 2.0, t.bg_deep, 0.9);
        let ratio = (player.position / player.length.max(1.0)).clamp(0.0, 1.0);
        fill_rr(cr, track_x, bar_y, track_w * ratio, bar_h, bar_h / 2.0, t.accent, 1.0);
        if seek_hov {
            let tx_thumb = track_x + (track_w * ratio - 4.5).clamp(0.0, (track_w - 9.0).max(0.0));
            fill_rr(cr, tx_thumb, y + 2.5, 9.0, 9.0, 4.5, t.text_bright, 1.0);
        }
        draw_text(cr, &total, cx + cw - tw2, y + 10.0, 9.0, false, t.muted, 1.0);
        hits.push(HitBox { x: track_x, y, w: track_w, h: 14.0, target: seek_tgt, wheel: WheelAction::None });
        y += 20.0;
    }

    let total_w = 28.0 + 8.0 + 28.0 + 8.0 + 32.0 + 8.0 + 28.0 + 8.0 + 28.0;
    let mut bx = cx + (cw - total_w) / 2.0;
    let cyc = y + 16.0;

    for (tgt, lbl, is_glyph, size) in [
        (HitTarget::MediaRew, "-10", false, 9.0),
        (HitTarget::MediaPrev, "󰒮", true, 14.0),
    ] {
        let h = st.hover.as_ref() == Some(&tgt);
        if h {
            fill_rr(cr, bx, cyc - 13.0, 28.0, 26.0, 7.0, t.surface_hover, 1.0);
        }
        if is_glyph {
            draw_glyph_centered(cr, lbl, bx, cyc - 13.0, 28.0, 26.0, size, if h { t.text_bright } else { t.text }, 1.0);
        } else {
            text_center_v(cr, lbl, bx + 14.0, cyc - 13.0, 26.0, size, true, if h { t.text_bright } else { t.muted }, 1.0);
        }
        hits.push(HitBox { x: bx, y: cyc - 13.0, w: 28.0, h: 26.0, target: tgt, wheel: WheelAction::None });
        bx += 36.0;
    }

    let play = HitTarget::MediaPlay;
    let play_h = st.hover.as_ref() == Some(&play);
    fill_rr(cr, bx, cyc - 16.0, 32.0, 32.0, 16.0, if play_h { t.text_bright } else { t.accent }, 1.0);
    draw_glyph_centered(cr, if player.playing { "󰏤" } else { "󰐊" }, bx, cyc - 16.0, 32.0, 32.0, 16.0, t.accent_text, 1.0);
    hits.push(HitBox { x: bx, y: cyc - 16.0, w: 32.0, h: 32.0, target: play, wheel: WheelAction::None });
    bx += 40.0;

    for (tgt, lbl, is_glyph, size) in [
        (HitTarget::MediaNext, "󰒭", true, 14.0),
        (HitTarget::MediaFwd, "+10", false, 9.0),
    ] {
        let h = st.hover.as_ref() == Some(&tgt);
        if h {
            fill_rr(cr, bx, cyc - 13.0, 28.0, 26.0, 7.0, t.surface_hover, 1.0);
        }
        if is_glyph {
            draw_glyph_centered(cr, lbl, bx, cyc - 13.0, 28.0, 26.0, size, if h { t.text_bright } else { t.text }, 1.0);
        } else {
            text_center_v(cr, lbl, bx + 14.0, cyc - 13.0, 26.0, size, true, if h { t.text_bright } else { t.muted }, 1.0);
        }
        hits.push(HitBox { x: bx, y: cyc - 13.0, w: 28.0, h: 26.0, target: tgt, wheel: WheelAction::None });
        bx += 36.0;
    }
}

fn signal_icon(sig: u32) -> &'static str {
    if sig >= 75 {
        "󰤨"
    } else if sig >= 50 {
        "󰤥"
    } else if sig >= 25 {
        "󰤢"
    } else {
        "󰤟"
    }
}

fn draw_wifi_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;

    // Compact frameless header (30px)
    draw_glyph_centered(cr, if s.wifi_enabled { "󰤨" } else { "󰤭" }, cx, cy, 20.0, 28.0, 15.0, if s.wifi_enabled { t.accent } else { t.muted }, 1.0);
    text_left_v(cr, "Wi-Fi", cx + 24.0, cy, 28.0, 12.5, true, t.text_bright, 1.0);
    let sub_status = if !s.wifi_enabled {
        "Off"
    } else if !s.wifi_ssid.is_empty() {
        s.wifi_ssid.as_str()
    } else {
        "Disconnected"
    };
    let sub_w = (cw - 148.0).max(50.0);
    text_left_v(cr, &fit_text(cr, sub_status, 10.5, false, sub_w), cx + 66.0, cy + 1.0, 28.0, 10.5, false, t.muted, 1.0);

    if s.wifi_enabled {
        let r = HitTarget::WifiRescan;
        let rh = st.hover.as_ref() == Some(&r);
        if rh {
            fill_rr(cr, cx + cw - 66.0, cy + 2.0, 24.0, 24.0, 7.0, t.surface_hover, 1.0);
        }
        draw_glyph_centered(cr, if s.wifi_scanning { "󰑐" } else { "󰚰" }, cx + cw - 66.0, cy + 2.0, 24.0, 24.0, 13.0, if s.wifi_scanning { t.accent } else { t.muted }, 1.0);
        hits.push(HitBox { x: cx + cw - 66.0, y: cy + 2.0, w: 24.0, h: 24.0, target: r, wheel: WheelAction::None });
    }
    let tg = HitTarget::WifiToggle;
    compact_switch(cr, cx + cw - 34.0, cy + 5.0, s.wifi_enabled, t);
    hits.push(HitBox { x: cx + cw - 34.0, y: cy + 2.0, w: 34.0, h: 24.0, target: tg, wheel: WheelAction::None });

    hairline_h(cr, cx, cy + 33.0, cw, t, 0.25);

    let mut y = cy + 40.0;
    if !s.wifi_status_msg.is_empty() {
        text_left_v(cr, &fit_text(cr, &s.wifi_status_msg, 10.5, false, cw), cx + 2.0, y, 16.0, 10.5, false, t.accent, 1.0);
        y += 22.0;
    }

    if !s.wifi_enabled {
        draw_glyph_centered(cr, "󰤭", cx + cw / 2.0 - 12.0, y + 6.0, 24.0, 24.0, 18.0, t.muted, 0.8);
        text_center_v(cr, "Wi-Fi is turned off", cx + cw / 2.0, y + 32.0, 16.0, 11.0, false, t.muted, 1.0);
        return;
    }

    if !st.wifi_selected_ssid.is_empty() {
        fill_rr(cr, cx, y, cw, 60.0, 10.0, t.surface, 0.72);
        let hdr = format!("Password · {}", st.wifi_selected_ssid);
        text_left_v(cr, &fit_text(cr, &hdr, 10.5, true, cw - 32.0), cx + 10.0, y + 4.0, 16.0, 10.5, true, t.text_bright, 1.0);

        let close_t = HitTarget::WifiPassClose;
        let close_h = st.hover.as_ref() == Some(&close_t);
        if close_h {
            fill_rr(cr, cx + cw - 24.0, y + 4.0, 18.0, 18.0, 5.0, t.surface_hover, 1.0);
        }
        draw_glyph_centered(cr, "󰅖", cx + cw - 24.0, y + 4.0, 18.0, 18.0, 11.0, t.muted, 1.0);
        hits.push(HitBox { x: cx + cw - 24.0, y: y + 4.0, w: 18.0, h: 18.0, target: close_t, wheel: WheelAction::None });

        let in_x = cx + 8.0;
        let in_y = y + 25.0;
        let in_w = (cw - 16.0 - 72.0).max(60.0);
        fill_rr(cr, in_x, in_y, in_w, 26.0, 7.0, t.bg_deep, 1.0);
        let n_chars = st.wifi_password.chars().count();
        let masked = if n_chars == 0 {
            if st.agy_cursor_on { "|".to_string() } else { "Password...".to_string() }
        } else {
            let mut dots: String = "•".repeat(n_chars.min(24));
            if st.agy_cursor_on {
                dots.push('|');
            }
            dots
        };
        let m_col = if n_chars == 0 && !st.agy_cursor_on { t.muted } else { t.text_bright };
        text_left_v(cr, &masked, in_x + 10.0, in_y, 26.0, 11.5, false, m_col, 1.0);
        hits.push(HitBox { x: in_x, y: in_y, w: in_w, h: 26.0, target: HitTarget::WifiPassInput, wheel: WheelAction::None });

        let btn_x = cx + cw - 8.0 - 66.0;
        let btn_t = HitTarget::WifiPassConnect;
        let btn_h = st.hover.as_ref() == Some(&btn_t);
        tonal_pill(cr, btn_x, in_y, 66.0, 26.0, "", "Join", true, btn_h, false, false, t, btn_t, hits);
        y += 68.0;
    }

    let list_h = 202.0;
    cr.save().ok();
    rounded_rect(cr, cx, y, cw, list_h, 8.0);
    cr.clip();
    let mut ry = y - st.wifi_scroll;
    for (i, net) in s.wifi_networks.iter().enumerate() {
        if ry + 34.0 < y {
            ry += 38.0;
            continue;
        }
        if ry > y + list_h {
            break;
        }
        let tgt = HitTarget::WifiRow(i);
        let hov = st.hover.as_ref() == Some(&tgt);
        if net.active || hov {
            fill_rr(cr, cx, ry, cw, 34.0, 8.0, if net.active { t.surface_hover } else { t.surface }, 0.9);
        }
        if net.active {
            fill_rr(cr, cx + 2.0, ry + 8.0, 3.0, 18.0, 1.5, t.accent, 1.0);
        }
        draw_glyph_centered(cr, signal_icon(net.signal), cx + 10.0, ry, 18.0, 34.0, 13.5, if net.active { t.accent } else { t.muted }, 1.0);
        let fg = if net.active { t.text_bright } else { t.text };
        let label = fit_text(cr, &net.ssid, 11.5, net.active, cw - 110.0);
        text_left_v(cr, &label, cx + 34.0, ry, 34.0, 11.5, net.active, fg, 1.0);

        let main = if net.active {
            "Connected".to_string()
        } else {
            format!("{}%", net.signal)
        };
        text_right_v(cr, &main, cx + cw - 8.0, ry, 34.0, 10.5, net.active, if net.active { t.accent } else { t.muted }, 1.0);
        if !net.active && net.saved {
            let mw = text_width(cr, &main, 10.5, false);
            text_right_v(cr, "Saved", cx + cw - 8.0 - mw - 6.0, ry, 34.0, 9.5, false, t.muted, 0.85);
        } else if !net.active && !net.saved && !net.security.is_empty() && net.security != "--" {
            let mw = text_width(cr, &main, 10.5, false);
            draw_glyph_centered(cr, "󰌾", cx + cw - 8.0 - mw - 20.0, ry, 16.0, 34.0, 10.5, t.muted, 0.8);
        }
        let hy = ry.max(y);
        let hh = (ry + 34.0).min(y + list_h) - hy;
        if hh > 4.0 {
            hits.push(HitBox { x: cx, y: hy, w: cw, h: hh, target: tgt, wheel: WheelAction::None });
        }
        ry += 38.0;
    }
    cr.restore().ok();
}

fn draw_bluetooth_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;

    draw_glyph_centered(cr, if s.bt_powered { "󰂯" } else { "󰂲" }, cx, cy, 20.0, 28.0, 15.0, if s.bt_powered { t.accent } else { t.muted }, 1.0);
    text_left_v(cr, "Bluetooth", cx + 24.0, cy, 28.0, 12.5, true, t.text_bright, 1.0);
    let status = if !s.bt_powered {
        "Off".to_string()
    } else if s.bt_connected > 0 {
        format!("{} conn", s.bt_connected)
    } else {
        "On".to_string()
    };
    text_left_v(cr, &status, cx + 96.0, cy + 1.0, 28.0, 10.5, false, t.muted, 1.0);

    if s.bt_powered {
        let pt = HitTarget::BtScan;
        let ph = st.hover.as_ref() == Some(&pt);
        let lbl = if s.bt_scanning { "Stop" } else { "Scan" };
        tonal_pill(cr, cx + cw - 98.0, cy + 2.0, 56.0, 24.0, if s.bt_scanning { "󰑐" } else { "󰐕" }, lbl, s.bt_scanning, ph, false, false, t, pt, hits);
    }
    let tg = HitTarget::BtToggle;
    compact_switch(cr, cx + cw - 34.0, cy + 5.0, s.bt_powered, t);
    hits.push(HitBox { x: cx + cw - 34.0, y: cy + 2.0, w: 34.0, h: 24.0, target: tg, wheel: WheelAction::None });

    hairline_h(cr, cx, cy + 33.0, cw, t, 0.25);

    if !s.bt_powered {
        draw_glyph_centered(cr, "󰂲", cx + cw / 2.0 - 12.0, cy + 46.0, 24.0, 24.0, 18.0, t.muted, 0.8);
        text_center_v(cr, "Bluetooth is turned off", cx + cw / 2.0, cy + 72.0, 16.0, 11.0, false, t.muted, 1.0);
        return;
    }

    let mut y = cy + 40.0;
    if !s.bt_status_msg.is_empty() {
        text_left_v(cr, &fit_text(cr, &s.bt_status_msg, 10.5, false, cw), cx + 2.0, y, 16.0, 10.5, false, t.accent, 1.0);
        y += 22.0;
    }

    let list_h = 216.0;
    cr.save().ok();
    rounded_rect(cr, cx, y, cw, list_h, 8.0);
    cr.clip();
    let mut ry = y - st.bt_scroll;

    if !s.bt_paired.is_empty() {
        if ry + 16.0 >= y && ry <= y + list_h {
            text_left_v(cr, "Saved", cx + 4.0, ry, 16.0, 10.0, true, t.muted, 0.9);
        }
        ry += 22.0;
        for (i, d) in s.bt_paired.iter().enumerate() {
            if ry + 34.0 < y {
                ry += 38.0;
                continue;
            }
            if ry > y + list_h {
                break;
            }
            let tgt = HitTarget::BtPairedRow(i);
            let hov = st.hover.as_ref() == Some(&tgt);
            if d.connected || hov {
                fill_rr(cr, cx, ry, cw, 34.0, 8.0, if d.connected { t.surface_hover } else { t.surface }, 0.9);
            }
            if d.connected {
                fill_rr(cr, cx + 2.0, ry + 8.0, 3.0, 18.0, 1.5, t.accent, 1.0);
            }
            draw_glyph_centered(cr, if d.connected { "󰂱" } else { "󰂯" }, cx + 10.0, ry, 18.0, 34.0, 13.5, if d.connected { t.accent } else { t.muted }, 1.0);
            let fg = if d.connected { t.text_bright } else { t.text };
            let nm = fit_text(cr, &d.name, 11.5, d.connected, cw - 118.0);
            text_left_v(cr, &nm, cx + 34.0, ry, 34.0, 11.5, d.connected, fg, 1.0);
            let label = if d.connected { "Connected" } else { "Connect" };
            text_right_v(cr, label, cx + cw - 32.0, ry, 34.0, 10.0, d.connected, if d.connected { t.accent } else { t.muted }, 1.0);

            let ft = HitTarget::BtForget(i);
            let fh = st.hover.as_ref() == Some(&ft);
            if fh {
                fill_rr(cr, cx + cw - 26.0, ry + 7.0, 20.0, 20.0, 5.0, t.surface_active, 1.0);
            }
            draw_glyph_centered(cr, "󰅖", cx + cw - 26.0, ry + 7.0, 20.0, 20.0, 11.0, if fh { t.red } else { t.muted }, 1.0);
            let hy = ry.max(y);
            let hh = (ry + 34.0).min(y + list_h) - hy;
            if hh > 4.0 {
                hits.push(HitBox { x: cx + 4.0, y: hy, w: cw - 34.0, h: hh, target: tgt, wheel: WheelAction::None });
                hits.push(HitBox { x: cx + cw - 26.0, y: (ry + 7.0).max(y), w: 20.0, h: 20.0, target: ft, wheel: WheelAction::None });
            }
            ry += 38.0;
        }
    }

    if !s.bt_available.is_empty() || s.bt_scanning {
        if !s.bt_paired.is_empty() {
            ry += 4.0;
        }
        if ry + 16.0 >= y && ry <= y + list_h {
            text_left_v(cr, if s.bt_scanning { "Scanning..." } else { "Available" }, cx + 4.0, ry, 16.0, 10.0, true, t.muted, 0.9);
        }
        ry += 20.0;
        for (i, d) in s.bt_available.iter().enumerate() {
            if ry + 34.0 < y {
                ry += 38.0;
                continue;
            }
            if ry > y + list_h {
                break;
            }
            let tgt = HitTarget::BtPairRow(i);
            let hov = st.hover.as_ref() == Some(&tgt);
            if hov {
                fill_rr(cr, cx, ry, cw, 34.0, 8.0, t.surface, 0.9);
            }
            draw_glyph_centered(cr, "󰂯", cx + 10.0, ry, 18.0, 34.0, 13.5, t.muted, 1.0);
            let nm = fit_text(cr, &d.name, 11.5, false, cw - 78.0);
            text_left_v(cr, &nm, cx + 34.0, ry, 34.0, 11.5, false, t.text, 1.0);
            text_right_v(cr, "Pair", cx + cw - 8.0, ry, 34.0, 10.5, true, t.accent, 1.0);
            let hy = ry.max(y);
            let hh = (ry + 34.0).min(y + list_h) - hy;
            if hh > 4.0 {
                hits.push(HitBox { x: cx, y: hy, w: cw, h: hh, target: tgt, wheel: WheelAction::None });
            }
            ry += 38.0;
        }
    }

    if s.bt_paired.is_empty() && s.bt_available.is_empty() && !s.bt_scanning {
        text_center_v(cr, "No devices yet", cx + cw / 2.0, y + 36.0, 18.0, 11.5, false, t.muted, 1.0);
        text_center_v(cr, "Click Scan above to pair", cx + cw / 2.0, y + 58.0, 16.0, 10.5, false, t.muted, 0.85);
    }
    cr.restore().ok();
}

fn draw_notifications_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;
    let n = s.notifications.len();

    draw_glyph_centered(
        cr,
        if s.dnd_enabled { "󰂛" } else { "󰂚" },
        cx,
        cy,
        20.0,
        28.0,
        15.0,
        if s.dnd_enabled { t.muted } else { t.accent },
        1.0,
    );
    text_left_v(cr, "Notifications", cx + 24.0, cy, 28.0, 12.5, true, t.text_bright, 1.0);
    let sub = if s.dnd_enabled {
        "DND".to_string()
    } else if n > 0 {
        format!("{n}")
    } else {
        String::new()
    };
    if !sub.is_empty() {
        text_left_v(cr, &sub, cx + 118.0, cy + 1.0, 28.0, 10.5, false, t.muted, 1.0);
    }

    if n > 0 {
        let tg = HitTarget::NotifClear;
        let hov = st.hover.as_ref() == Some(&tg);
        tonal_pill(cr, cx + cw - 90.0, cy + 2.0, 48.0, 24.0, "", "Clear", false, hov, false, false, t, tg, hits);
    }

    let dg = HitTarget::NotifDnd;
    compact_switch(cr, cx + cw - 34.0, cy + 5.0, s.dnd_enabled, t);
    hits.push(HitBox { x: cx + cw - 34.0, y: cy + 2.0, w: 34.0, h: 24.0, target: dg, wheel: WheelAction::None });

    hairline_h(cr, cx, cy + 33.0, cw, t, 0.25);

    if n == 0 {
        draw_glyph_centered(cr, if s.dnd_enabled { "󰂛" } else { "󰂚" }, cx, cy + 46.0, cw, 22.0, 20.0, t.muted, 0.8);
        let msg = if s.dnd_enabled { "Notifications muted" } else { "No notifications" };
        text_center_v(cr, msg, cx + cw / 2.0, cy + 72.0, 16.0, 11.0, false, t.muted, 1.0);
        return;
    }

    let list_y = cy + 40.0;
    let list_h = ((n as f64) * 64.0).min(320.0);
    cr.save().ok();
    rounded_rect(cr, cx, list_y, cw, list_h, 8.0);
    cr.clip();
    let mut ry = list_y - st.notif_scroll;
    for (i, item) in s.notifications.iter().enumerate() {
        let card_h = 58.0;
        if ry + card_h < list_y {
            ry += card_h + 6.0;
            continue;
        }
        if ry > list_y + list_h {
            break;
        }
        fill_rr(cr, cx, ry, cw, card_h, 10.0, t.surface, 0.62);
        if item.critical {
            fill_rr(cr, cx + 2.0, ry + 8.0, 3.0, card_h - 16.0, 1.5, t.red, 1.0);
        }
        let tx = cx + 12.0;
        let tw = (cw - 24.0).max(10.0);
        let app = fit_text(cr, &item.app_name, 10.0, true, tw - 66.0);
        text_left_v(cr, &app, tx, ry + 6.0, 14.0, 10.0, true, if item.critical { t.red } else { t.accent }, 1.0);
        text_right_v(cr, &item.time, cx + cw - 30.0, ry + 6.0, 14.0, 9.5, false, t.muted, 0.9);

        let del_t = HitTarget::NotifDismiss(i);
        let del_h = st.hover.as_ref() == Some(&del_t);
        if del_h {
            fill_rr(cr, cx + cw - 26.0, ry + 5.0, 18.0, 18.0, 5.0, t.surface_hover, 1.0);
        }
        draw_glyph_centered(cr, "󰅖", cx + cw - 26.0, ry + 5.0, 18.0, 18.0, 10.5, if del_h { t.text_bright } else { t.muted }, 1.0);
        if ry + 5.0 >= list_y && ry + 23.0 <= list_y + list_h {
            hits.push(HitBox { x: cx + cw - 26.0, y: ry + 5.0, w: 18.0, h: 18.0, target: del_t, wheel: WheelAction::None });
        }

        let summary = fit_text(cr, &item.summary, 11.5, true, tw);
        text_left_v(cr, &summary, tx, ry + 21.0, 16.0, 11.5, true, t.text_bright, 1.0);
        let body = fit_text(cr, &item.body, 10.5, false, tw);
        text_left_v(cr, &body, tx, ry + 37.0, 15.0, 10.5, false, t.muted, 0.95);
        ry += card_h + 6.0;
    }
    cr.restore().ok();
}

fn draw_tray_menu_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    _ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let s = st.stats;
    let title = if s.tray_menu_title.is_empty() {
        "System Tray"
    } else {
        s.tray_menu_title.as_str()
    };
    text_left_v(cr, &fit_text(cr, title, 12.0, true, cw - 8.0), cx + 4.0, cy, 24.0, 12.0, true, t.text_bright, 1.0);
    hairline_h(cr, cx, cy + 28.0, cw, t, 0.28);

    let mut y = cy + 34.0;
    if s.tray_menu_items.is_empty() {
        text_center_v(cr, "No menu items", cx + cw / 2.0, y + 6.0, 26.0, 11.0, false, t.muted, 0.9);
        return;
    }
    for item in &s.tray_menu_items {
        if item.sep {
            hairline_h(cr, cx + 6.0, y + 4.0, cw - 12.0, t, 0.22);
            y += 9.0;
            continue;
        }
        let h = 26.0;
        let tgt = HitTarget::TrayMenuClick(item.id);
        let hov = item.enabled && st.hover.as_ref() == Some(&tgt);
        if hov {
            fill_rr(cr, cx, y, cw, h, 7.0, t.surface_hover, 0.95);
        }
        let fg = if !item.enabled {
            t.muted
        } else if hov {
            t.text_bright
        } else {
            t.text
        };
        let alpha = if item.enabled { 1.0 } else { 0.48 };
        if item.checked {
            draw_glyph_centered(cr, "󰄬", cx + 6.0, y, 16.0, h, 11.5, t.accent, alpha);
        }
        let lx = cx + if item.checked { 24.0 } else { 10.0 };
        let fitted = fit_text(cr, &item.label, 11.5, false, (cw - (lx - cx) - 10.0).max(20.0));
        text_left_v(cr, &fitted, lx, y, h, 11.5, hov, fg, alpha);
        if item.enabled {
            hits.push(HitBox {
                x: cx,
                y,
                w: cw,
                h,
                target: tgt,
                wheel: WheelAction::None,
            });
        }
        y += h + 2.0;
    }
}

