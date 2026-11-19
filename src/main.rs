mod files_ui;
mod gpu;
mod ipc;
mod render;
mod services;
mod theme;

use std::collections::HashMap;
use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use cairo::{Context, Format, ImageSurface, Operator};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::data_device_manager::data_device::{DataDevice, DataDeviceHandler};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::{DataSourceHandler, DragSource};
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::EventLoop;
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RepeatInfo};
use smithay_client_toolkit::seat::pointer::{
    CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer, BTN_LEFT,
    BTN_MIDDLE, BTN_RIGHT,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_data_device, delegate_keyboard, delegate_layer, delegate_output,
    delegate_pointer, delegate_registry, delegate_seat, delegate_shm, registry_handlers,
};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, Proxy, QueueHandle};

use crate::ipc::{send_cli_command, start_ipc_server, IpcCommand};
use crate::gpu::{GpuIsland, GpuWipe, IslandGpuParams};
use crate::render::{
    draw_bar, draw_island_frame, render_popup_content, render_settings_tab_body, rounded_rect,
    BarPillVisual, HitBox, HitTarget, IslandFrameLayout, IslandRenderState, PopupKind, WheelAction,
};
use crate::services::{
    agy_send, apply_wallpaper, bt_connect, bt_disconnect, bt_remove, bt_scan, capture_screen_ppm,
    compress_to_zip,
    count_dir_items, default_places, delete_permanently, duplicate_files, empty_trash, encode_uri,
    ensure_thumb_png, extract_archive, is_image_file, is_text_file, is_thumbable_file, launch_app,
    list_dir_entries_sorted, list_wallpaper_paths, load_png_surface, load_thumb_surface,
    media_next, media_prev, media_seek_relative, media_set_position, media_toggle_play,
    power_action, read_drive_usages, read_mango_summary, read_text_preview, refresh_panel_data,
    refresh_stats_now, run_trash, scan_desktop_apps, scan_desktop_apps_cached, app_dirs_mtimes,
    select_audio_output, set_brightness, set_lock_wallpaper, set_mango_key, set_volume,
    spawn_background_services, spawn_detached, switch_tag, toggle_bluetooth, toggle_mute,
    toggle_wifi, transfer_files, tray_send_cmd, trigger_system_theme_apply, wifi_connect,
    wifi_disconnect, wifi_rescan, wl_copy_uri_list, wl_paste_text, wl_paste_uri_list, AgyEvent,
    AgyMsg, AppItem, ClipMode, DriveUsage, FileItem, FileViewMode, FilesPrompt, MangoSummary,
    SortBy, SysStats, AGY_MODELS, TRASH_DIR,
};
use crate::theme::{
    config_mtimes, ensure_user_config_exists, load_config, read_settings_value,
    save_bookmarks, theme_from_config, theme_preset_by_name, write_settings_value, Bookmark,
    ShellConfig, Theme,
};

enum PendingWipe {
    Theme,
    Wallpaper { path: String, mode: String },
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ease {
    Open,
    Close,
    Fade,
    OutCubic,
    Linear,
    Custom([f64; 4]),
}

#[derive(Clone, Copy, Debug)]
struct Motion {
    from: f64,
    to: f64,
    start: Instant,
    dur: f64,
    delay: f64,
    ease: Ease,
}

impl Motion {
    fn new(from: f64, to: f64, dur: f64, ease: Ease) -> Self {
        Self { from, to, start: Instant::now(), dur, delay: 0.0, ease }
    }

    fn delayed(mut self, delay: f64) -> Self {
        self.delay = delay;
        self
    }

    fn value(&self) -> f64 {
        let ms = self.start.elapsed().as_secs_f64() * 1000.0 - self.delay;
        if ms <= 0.0 {
            return self.from;
        }
        let t = (ms / self.dur).clamp(0.0, 1.0);
        let e = match self.ease {
            Ease::Open => spline(t),
            Ease::Close => spline_close(t),
            Ease::Fade => cubic_bezier(0.22, 1.0, 0.36, 1.0, t),
            Ease::OutCubic => ease_out_cubic(t),
            Ease::Linear => t,
            Ease::Custom(c) => cubic_bezier(c[0], c[1], c[2], c[3], t),
        };
        self.from + (self.to - self.from) * e
    }

    fn done(&self) -> bool {
        (self.from - self.to).abs() < 1e-6
            || self.start.elapsed().as_secs_f64() * 1000.0 >= self.delay + self.dur
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn shifted_hits(src: &[HitBox], dx: f64, dy: f64) -> Vec<HitBox> {
    src.iter()
        .map(|hb| {
            let target = match &hb.target {
                HitTarget::ControlsVolSlider(sx, sw) => HitTarget::ControlsVolSlider(sx + dx, *sw),
                HitTarget::ControlsBrightSlider(sx, sw) => HitTarget::ControlsBrightSlider(sx + dx, *sw),
                HitTarget::VolumeSlider(sx, sw) => HitTarget::VolumeSlider(sx + dx, *sw),
                HitTarget::BrightnessSlider(sx, sw) => HitTarget::BrightnessSlider(sx + dx, *sw),
                HitTarget::MediaSeek(sx, sw) => HitTarget::MediaSeek(sx + dx, *sw),
                HitTarget::SettingsBarSize(sx, sw) => HitTarget::SettingsBarSize(sx + dx, *sw),
                HitTarget::SettingsCorner(sx, sw) => HitTarget::SettingsCorner(sx + dx, *sw),
                HitTarget::SettingsBarOpacity(sx, sw) => HitTarget::SettingsBarOpacity(sx + dx, *sw),
                HitTarget::SettingsPopupOpacity(sx, sw) => HitTarget::SettingsPopupOpacity(sx + dx, *sw),
                HitTarget::SettingsLockDim(sx, sw) => HitTarget::SettingsLockDim(sx + dx, *sw),
                other => other.clone(),
            };
            HitBox {
                x: hb.x + dx,
                y: hb.y + dy,
                w: hb.w,
                h: hb.h,
                target,
                wheel: hb.wheel,
            }
        })
        .collect()
}

fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// CSS cubic-bezier(x1, y1, x2, y2) with Newton-Raphson + bisection fallback.
fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;

    let sample_x = |t: f64| ((ax * t + bx) * t + cx) * t;
    let sample_y = |t: f64| ((ay * t + by) * t + cy) * t;
    let sample_dx = |t: f64| (3.0 * ax * t + 2.0 * bx) * t + cx;

    let mut t = x.clamp(0.0, 1.0);
    for _ in 0..8 {
        let dx = sample_x(t) - x;
        if dx.abs() < 1e-6 {
            return sample_y(t);
        }
        let d = sample_dx(t);
        if d.abs() < 1e-6 {
            break;
        }
        t -= dx / d;
    }
    let mut lo = 0.0;
    let mut hi = 1.0;
    t = x.clamp(0.0, 1.0);
    for _ in 0..24 {
        let v = sample_x(t);
        if (v - x).abs() < 1e-6 {
            break;
        }
        if v < x {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) / 2.0;
    }
    sample_y(t)
}

/// Silky smooth open/morph easing: soft ramp-up + long buttery deceleration tail
fn spline(t: f64) -> f64 {
    cubic_bezier(0.25, 0.96, 0.32, 1.0, t.clamp(0.0, 1.0))
}

/// Silky close easing
fn spline_close(t: f64) -> f64 {
    cubic_bezier(0.32, 0.0, 0.22, 1.0, t.clamp(0.0, 1.0))
}

const BUF_SCALE: i32 = 2;

#[derive(Clone, Debug)]
struct PillAnim {
    from: BarPillVisual,
    to: BarPillVisual,
    start: Instant,
}

const PILL_MS: f64 = 200.0;

struct ActiveDrag {
    source: DragSource,
    paths: Vec<PathBuf>,
    uri_payload: String,
    plain_payload: String,
    _icon_surface: Option<wl_surface::WlSurface>,
    _icon_pool: Option<SlotPool>,
}

fn cursor_is_over_zalo_window() -> bool {
    let Ok(pos_out) = std::process::Command::new("mmsg").args(["get", "cursorpos"]).output() else {
        return false;
    };
    let Ok(pos_v) = serde_json::from_slice::<serde_json::Value>(&pos_out.stdout) else {
        return false;
    };
    let (Some(cx), Some(cy)) = (
        pos_v.get("x").and_then(|v| v.as_f64()),
        pos_v.get("y").and_then(|v| v.as_f64()),
    ) else {
        return false;
    };
    let Ok(cli_out) = std::process::Command::new("mmsg").args(["get", "all-clients"]).output() else {
        return false;
    };
    let Ok(cli_v) = serde_json::from_slice::<serde_json::Value>(&cli_out.stdout) else {
        return false;
    };
    let Some(clients) = cli_v.get("clients").and_then(|v| v.as_array()) else {
        return false;
    };
    clients.iter().any(|c| {
        let vis = c.get("is_visible").and_then(|v| v.as_bool()).unwrap_or(false);
        let min = c.get("is_minimized").and_then(|v| v.as_bool()).unwrap_or(false);
        if !vis || min {
            return false;
        }
        let appid = c.get("appid").and_then(|v| v.as_str()).unwrap_or("").to_ascii_lowercase();
        let title = c.get("title").and_then(|v| v.as_str()).unwrap_or("").to_ascii_lowercase();
        if !appid.contains("zalo") && !title.contains("zalo") {
            return false;
        }
        let x = c.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = c.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let w = c.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let h = c.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        cx >= x && cx <= x + w && cy >= y && cy <= y + h
    })
}

fn write_zalo_drop_handoff(paths: &[PathBuf]) -> bool {
    let str_paths: Vec<String> = paths
        .iter()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    if str_paths.is_empty() {
        return false;
    }
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let payload = serde_json::json!({
        "ts_ms": ts_ms,
        "paths": str_paths,
    });
    let target = "/tmp/yunoshell-zalo-drop.json";
    let tmp = "/tmp/yunoshell-zalo-drop.json.tmp";
    if std::fs::write(tmp, payload.to_string()).is_ok() {
        return std::fs::rename(tmp, target).is_ok();
    }
    false
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PortalRequest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub current_dir: String,
    #[serde(default)]
    pub current_name: String,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub directory: bool,
    #[serde(default)]
    pub accept_label: String,
    #[serde(default)]
    pub res_file: String,
}

struct YunoShell {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    shm_state: Shm,
    data_device_manager_state: DataDeviceManagerState,
    qh: QueueHandle<Self>,

    bar_layer: Option<LayerSurface>,
    bar_pool: Option<SlotPool>,
    bar_w: u32,
    bar_h: u32,
    bar_configured: bool,
    bar_hits: Vec<HitBox>,
    yuno_icon: Option<ImageSurface>,
    tray_icons: HashMap<String, ImageSurface>,

    island_layer: LayerSurface,
    island_pool: SlotPool,
    island_configured: bool,
    island_hits: Vec<HitBox>,
    content_hits: Vec<HitBox>,
    content_cache: Option<ImageSurface>,
    content_dirty: bool,
    cache_gen: u64,
    settings_body_cache: Option<ImageSurface>,
    settings_body_hits: Vec<HitBox>,
    settings_body_dirty: bool,
    prev_island_rect: (f64, f64, f64, f64),
    cur_body_rect: (f64, f64, f64, f64),
    gpu: Option<GpuIsland>,
    gpu_failed: bool,
    gpu_image: Option<ImageSurface>,
    gpu_display: *mut c_void,
    gpu_surface: *mut c_void,
    island_frame_pending: bool,
    anim_sync_pending: bool,

    seat: Option<wl_seat::WlSeat>,
    data_device: Option<DataDevice>,
    pointer: Option<ThemedPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    cfg: ShellConfig,
    cfg_last_check: Instant,
    cfg_mtimes: (Option<SystemTime>, Option<SystemTime>),
    app_dirs_mtimes: Vec<(PathBuf, Option<SystemTime>)>,
    apps_last_check: Instant,
    theme: Theme,
    stats: Arc<Mutex<SysStats>>,
    ipc_rx: Receiver<IpcCommand>,
    exit: bool,

    active_popup: PopupKind,
    rendered_popup: PopupKind,
    island_w: u32,
    island_h: u32,
    m_width: Motion,
    m_height: Motion,
    m_content: Motion,
    m_content_scale: Motion,
    m_morph: Motion,
    m_fade: Motion,
    morph_from_size: (f64, f64),
    morph_from_pos: (f64, f64),
    closing: bool,

    hover: Option<HitTarget>,
    last_click_time: Instant,
    last_click_idx: Option<usize>,

    current_dir: PathBuf,
    back_stack: Vec<PathBuf>,
    fwd_stack: Vec<PathBuf>,
    show_hidden: bool,
    filter_query: String,
    filter_focused: bool,
    entries: Vec<FileItem>,
    selected_idx: Option<usize>,
    selected_paths: Vec<PathBuf>,
    files_sort_by: SortBy,
    files_sort_asc: bool,
    files_view_mode: FileViewMode,
    files_prompt: FilesPrompt,
    files_prompt_text: String,
    files_clipboard: Vec<PathBuf>,
    files_clip_mode: ClipMode,
    files_preview_path: PathBuf,
    files_preview_surf: Option<ImageSurface>,
    files_preview_lines: Vec<String>,
    files_preview_dir_count: Option<usize>,
    files_context_menu: Option<(f64, f64, Option<usize>)>,
    internal_dragging: bool,
    internal_drag_sources: Vec<PathBuf>,
    internal_drag_target: Option<PathBuf>,
    internal_drag_pos: (f64, f64),
    files_qa_dnd_hover: bool,
    drag_pill_cache: Option<ImageSurface>,
    drag_pill_gen: u64,
    drag_pill_key: (String, usize),
    marquee_start: Option<(f64, f64)>,
    marquee_rect: Option<(f64, f64, f64, f64)>,
    drive_usages: Vec<DriveUsage>,
    #[allow(dead_code)]
    last_drive_check: Instant,
    files_thumbs: HashMap<PathBuf, ImageSurface>,
    files_thumb_tx: Sender<(PathBuf, Option<PathBuf>)>,
    files_thumb_rx: Receiver<(PathBuf, Option<PathBuf>)>,
    files_search_anim: Motion,
    files_crumb_anim: Motion,
    files_sidebar_pill_y: Motion,
    files_sidebar_pill_r: Motion,
    files_ctx_anim: Motion,
    files_action_bar_anim: Motion,
    pressed_target: Option<HitTarget>,
    kbd_ctrl: bool,
    kbd_shift: bool,
    kbd_repeat_rate_hz: u32,
    kbd_repeat_delay_ms: u32,
    kbd_held_key: Option<(KeyEvent, Instant)>,
    scroll_y: f64,
    status_toast: String,
    toast_until: Option<Instant>,

    button_down: bool,
    press_pos: (f64, f64),
    press_serial: u32,
    pressed_file_idx: Option<usize>,
    slider_drag: Option<HitTarget>,
    drag_launched: bool,
    active_drags: Vec<ActiveDrag>,

    all_apps: Vec<AppItem>,
    filtered_apps: Vec<AppItem>,
    launcher_query: String,
    launcher_idx: usize,
    launcher_scroll: f64,

    settings_tab: usize,
    settings_prev_tab: usize,
    settings_tab_anim: Motion,
    settings_tab_slide_x: f64,
    settings_bg_target: String,
    settings_open_dropdown: String,
    wallpapers: Vec<(String, PathBuf, Option<ImageSurface>)>,
    current_preview_path: String,
    current_preview_surf: Option<ImageSurface>,
    wallpaper_prev_surf: Option<ImageSurface>,
    wallpaper_preview_anim: Motion,
    wipe_layer: Option<LayerSurface>,
    wipe_gpu: Option<GpuWipe>,
    wipe_w: u32,
    wipe_h: u32,
    wipe_configured: bool,
    wipe_rgb: Option<(i32, i32, Vec<u8>)>,
    wipe_pending: Option<PendingWipe>,
    wipe_anim: Motion,
    wipe_frame_pending: bool,
    thumb_rx: Option<Receiver<(PathBuf, Option<PathBuf>)>>,
    mango_info: MangoSummary,
    cal_offset_months: i32,
    cal_selected_key: i32,
    last_clock_sec: i64,

    agy_messages: Vec<AgyMsg>,
    agy_input: String,
    agy_busy: bool,
    agy_status: String,
    agy_meta: String,
    agy_conv: String,
    agy_scroll: f64,
    agy_rx: Option<Receiver<AgyEvent>>,
    agy_model_idx: usize,
    agy_effort_idx: usize,
    agy_cursor_on: bool,
    agy_blink_at: Instant,

    opened_by_hover: bool,
    bar_hovered: bool,
    island_card_hovered: bool,
    hover_close_at: Option<Instant>,
    last_input_region: Option<(bool, String, i32, u32, u32)>,

    wifi_selected_ssid: String,
    wifi_password: String,
    wifi_scroll: f64,
    bt_scroll: f64,
    notif_scroll: f64,
    cc_expanded: &'static str,
    media_art_url: String,
    media_art_surf: Option<ImageSurface>,
    portal_req: Option<PortalRequest>,
    portal_save_name: String,

    pill_anims: Vec<PillAnim>,
    bar_frame_pending: bool,
    started: Instant,
}

impl YunoShell {
    fn sync_bar_layout(&mut self) {
        let Some(ref bar) = self.bar_layer else { return };
        let span = self.cfg.bar_size + if self.cfg.bar_floating { 10 } else { 0 };
        let (anchor, w, h) = match self.cfg.bar_position.as_str() {
            "right" => (Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT, span, 0),
            "top" => (Anchor::TOP | Anchor::LEFT | Anchor::RIGHT, 0, span),
            "bottom" => (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT, 0, span),
            _ => (Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT, span, 0),
        };
        bar.set_anchor(anchor);
        bar.set_size(w, h);
        bar.set_exclusive_zone(span as i32);
        bar.commit();

        let top_margin = 0;
        self.island_layer.set_margin(top_margin, 0, 0, 0);
        if !self.island_configured {
            self.island_layer.commit();
        }
    }

    fn launcher_rows(&self) -> usize {
        ((self.filtered_apps.len().max(1)) + 7) / 8
    }

    fn sync_pills(&mut self) {
        let stats = match self.stats.lock() {
            Ok(s) => s.clone(),
            Err(_) => return,
        };
        let vertical = self.cfg.bar_position == "left" || self.cfg.bar_position == "right";
        let now = Instant::now();
        let mut anims: Vec<PillAnim> = Vec::with_capacity(stats.tags.len());
        for (i, tag) in stats.tags.iter().enumerate() {
            let hovered = self.hover.as_ref() == Some(&HitTarget::BarWorkspace(tag.index));
            let active = tag.is_active;
            let urgent = tag.is_urgent;
            let clients = tag.client_count > 0;
            let (slot, pw, ph) = if vertical {
                let slot = if active { 24.0 } else { 12.0 };
                let pw = if active { 10.0 } else if clients { 7.0 } else { 5.0 };
                let ph = if active { 22.0 } else if clients { 7.0 } else { 5.0 };
                (slot, pw, ph)
            } else {
                let slot = if active { 24.0 } else { 12.0 };
                let pw = if active { 24.0 } else if clients { 7.0 } else { 5.0 };
                let ph = if active { 12.0 } else if clients { 7.0 } else { 5.0 };
                (slot, pw, ph)
            };
            let alpha = if active || urgent {
                1.0
            } else if hovered {
                0.9
            } else if clients {
                0.7
            } else {
                0.3
            };
            let to = BarPillVisual { slot, pill_w: pw, pill_h: ph, alpha };
            let prev = self.pill_anims.get(i);
            if let Some(a) = prev {
                if (a.to.slot - to.slot).abs() < 1e-6
                    && (a.to.pill_w - to.pill_w).abs() < 1e-6
                    && (a.to.pill_h - to.pill_h).abs() < 1e-6
                    && (a.to.alpha - to.alpha).abs() < 1e-6
                {
                    anims.push(PillAnim { from: a.from, to: a.to, start: a.start });
                    continue;
                }
            }
            let from = match prev {
                Some(a) => {
                    let t = (now.duration_since(a.start).as_secs_f64() * 1000.0 / PILL_MS).clamp(0.0, 1.0);
                    let e = spline(t);
                    BarPillVisual {
                        slot: a.from.slot + (a.to.slot - a.from.slot) * e,
                        pill_w: a.from.pill_w + (a.to.pill_w - a.from.pill_w) * e,
                        pill_h: a.from.pill_h + (a.to.pill_h - a.from.pill_h) * e,
                        alpha: a.from.alpha + (a.to.alpha - a.from.alpha) * e,
                    }
                }
                None => to,
            };
            anims.push(PillAnim { from, to, start: now });
        }
        self.pill_anims = anims;
    }

    fn bar_pills(&self) -> Vec<BarPillVisual> {
        let now = Instant::now();
        self.pill_anims
            .iter()
            .map(|a| {
                let t = (now.duration_since(a.start).as_secs_f64() * 1000.0 / PILL_MS).clamp(0.0, 1.0);
                let e = spline(t);
                BarPillVisual {
                    slot: a.from.slot + (a.to.slot - a.from.slot) * e,
                    pill_w: a.from.pill_w + (a.to.pill_w - a.from.pill_w) * e,
                    pill_h: a.from.pill_h + (a.to.pill_h - a.from.pill_h) * e,
                    alpha: a.from.alpha + (a.to.alpha - a.from.alpha) * e,
                }
            })
            .collect()
    }

    fn pills_animating(&self) -> bool {
        let now = Instant::now();
        self.pill_anims
            .iter()
            .any(|a| now.duration_since(a.start).as_secs_f64() * 1000.0 < PILL_MS)
    }

    fn bar_needs_anim(&self) -> bool {
        if self.pills_animating() {
            return true;
        }
        if !self.cfg.show_media_on_bar {
            return false;
        }
        self.stats
            .lock()
            .map(|s| s.media.active && s.media.playing)
            .unwrap_or(false)
    }

    fn popup_target_size(&self, kind: PopupKind) -> (f64, f64) {
        let stats = self
            .stats
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        kind.target_size(
            self.launcher_rows(),
            &stats,
            !self.wifi_selected_ssid.is_empty(),
            self.cc_expanded,
        )
    }

    fn cc_section_for_popup(kind: PopupKind) -> Option<&'static str> {
        match kind {
            PopupKind::Controls => Some(""),
            PopupKind::Wifi => Some("wifi"),
            PopupKind::Bluetooth => Some("bt"),
            PopupKind::Volume => Some("audio"),
            PopupKind::Battery => Some("battery"),
            PopupKind::Brightness => Some(""),
            PopupKind::Power => Some("power"),
            _ => None,
        }
    }

    fn morph_island_inplace(&mut self, ms: f64) {
        let (bx, by, bw, bh) = self.cur_body_rect;
        if bw > 1.0 && bh > 1.0 {
            self.morph_from_size = (bw, bh);
            self.morph_from_pos = (bx, by);
            self.m_morph = Motion::new(0.0, 1.0, ms, Ease::Custom(self.cfg.anim_curve));
            self.anim_sync_pending = true;
        }
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn set_cc_expanded(&mut self, sec: &'static str) {
        if self.cc_expanded == sec {
            self.cc_expanded = "";
        } else {
            self.cc_expanded = sec;
        }
        self.wifi_scroll = 0.0;
        self.bt_scroll = 0.0;
        if self.cc_expanded != "wifi" {
            self.wifi_selected_ssid.clear();
            self.wifi_password.clear();
        }
        if matches!(self.cc_expanded, "wifi" | "bt" | "audio" | "battery") {
            let st = self.stats.clone();
            std::thread::spawn(move || {
                refresh_panel_data(&st);
            });
        }
        self.morph_island_inplace(260.0);
    }

    fn files_animating(&self) -> bool {
        self.rendered_popup == PopupKind::Files
            && (!self.files_search_anim.done()
                || !self.files_crumb_anim.done()
                || !self.files_sidebar_pill_y.done()
                || !self.files_sidebar_pill_r.done()
                || !self.files_ctx_anim.done()
                || !self.files_action_bar_anim.done())
    }

    fn island_animating(&self) -> bool {
        !self.m_width.done()
            || !self.m_height.done()
            || !self.m_content.done()
            || !self.m_content_scale.done()
            || !self.m_morph.done()
            || !self.m_fade.done()
            || (self.rendered_popup == PopupKind::Settings && !self.settings_tab_anim.done())
            || self.files_animating()
            || self.closing
    }

    fn origin_for_popup(&self, kind: PopupKind) -> (f64, f64, f64) {
        let vert = self.cfg.bar_position == "left" || self.cfg.bar_position == "right";
        let span = self.cfg.bar_size as f64 + if self.cfg.bar_floating { 10.0 } else { 0.0 };
        let sw = (self.island_w as f64).max(1536.0);
        let sh = (self.island_h as f64).max(960.0);
        if kind.is_top_island() {
            let y = if self.cfg.bar_position == "top" { span / 2.0 } else { 0.0 };
            return (sw / 2.0, y, 164.0);
        }
        if kind == PopupKind::TrayMenu {
            let active_idx = self.stats.lock().ok().and_then(|s| {
                s.tray_items.iter().position(|it| it.key == s.tray_menu_key)
            });
            if let Some(ti) = active_idx {
                if let Some(h) = self.bar_hits.iter().rev().find(|h| h.target == HitTarget::BarTray(ti)) {
                    let ow = if vert { (h.h + 18.0).max(40.0) } else { (h.w + 18.0).max(40.0) };
                    return (h.x + h.w / 2.0, h.y + h.h / 2.0, ow);
                }
            }
        }
        if kind == PopupKind::Controls {
            let specific_pk = match self.cc_expanded {
                "wifi" => Some(PopupKind::Wifi),
                "bt" => Some(PopupKind::Bluetooth),
                "audio" => Some(PopupKind::Volume),
                "battery" => Some(PopupKind::Battery),
                "power" => Some(PopupKind::Power),
                _ => None,
            };
            if let Some(pk) = specific_pk {
                if let Some(h) = self.bar_hits.iter().rev().find(|h| h.target == HitTarget::BarPopup(pk)) {
                    let ow = if vert { (h.h + 18.0).max(48.0) } else { (h.w + 20.0).max(52.0) };
                    return (h.x + h.w / 2.0, h.y + h.h / 2.0, ow);
                }
            }
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for h in &self.bar_hits {
                if let HitTarget::BarPopup(pk) = h.target {
                    if Self::cc_section_for_popup(pk).is_some() {
                        min_x = min_x.min(h.x);
                        max_x = max_x.max(h.x + h.w);
                        min_y = min_y.min(h.y);
                        max_y = max_y.max(h.y + h.h);
                    }
                }
            }
            if min_x.is_finite() {
                let ow = if vert { (max_y - min_y).clamp(48.0, 180.0) } else { (max_x - min_x).clamp(52.0, 180.0) };
                return ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0, ow);
            }
        }
        if let Some(h) = self
            .bar_hits
            .iter()
            .rev()
            .find(|h| h.target == HitTarget::BarPopup(kind))
        {
            let ow = if vert { (h.h + 18.0).max(48.0) } else { (h.w + 20.0).max(52.0) };
            return (h.x + h.w / 2.0, h.y + h.h / 2.0, ow);
        }
        // Fallback when bar_hits is empty (e.g. --no-bar mode)
        if vert {
            let x = if self.cfg.bar_position == "left" { span / 2.0 } else { sw - span / 2.0 };
            let y = match kind {
                PopupKind::Antigravity => 26.0,
                PopupKind::Power => sh - 22.0,
                PopupKind::Wifi => sh - 54.0,
                PopupKind::Battery => sh - 86.0,
                PopupKind::Volume => sh - 118.0,
                PopupKind::Brightness => sh - 150.0,
                PopupKind::Bluetooth => sh - 182.0,
                _ => sh / 2.0,
            };
            (x, y, 48.0)
        } else {
            let y = if self.cfg.bar_position == "bottom" { sh - span / 2.0 } else { span / 2.0 };
            let x = match kind {
                PopupKind::Antigravity => 26.0,
                PopupKind::Power => sw - 22.0,
                PopupKind::Wifi => sw - 54.0,
                PopupKind::Battery => sw - 86.0,
                PopupKind::Volume => sw - 118.0,
                PopupKind::Brightness => sw - 150.0,
                PopupKind::Bluetooth => sw - 182.0,
                _ => sw / 2.0,
            };
            (x, y, 52.0)
        }
    }

    fn island_layout(&self) -> Option<IslandFrameLayout> {
        let kind = self.rendered_popup;
        if kind == PopupKind::None {
            return None;
        }
        let (tw, th) = self.popup_target_size(kind);
        let me = self.m_morph.value();
        let wp = self.m_width.value();
        let hp = self.m_height.value();
        let vert_bar = self.cfg.bar_position == "left" || self.cfg.bar_position == "right";
        let attached = vert_bar && !kind.is_top_island();
        let span = self.cfg.bar_size as f64 + if self.cfg.bar_floating { 10.0 } else { 0.0 };
        let inset = if self.cfg.bar_floating {
            (self.cfg.corner_radius + 4.0).clamp(8.0, 24.0)
        } else {
            6.0
        };
        let sw = self.island_w as f64;
        let sh = self.island_h as f64;
        let edge = if self.cfg.bar_floating { 6.0 } else { 1.0 };
        let mut l = IslandFrameLayout::default();

        if attached {
            let (_, oy, ow) = self.origin_for_popup(kind);
            let start_span = th.min(ow.max(44.0));
            let body_w = (tw * hp).max(2.0);
            let body_h = (start_span + (th - start_span) * wp).max(40.0);
            let start_y = (oy - (start_span + 32.0) / 2.0)
                .clamp(inset, (sh - start_span - 32.0 - inset).max(inset));
            let final_y = (oy - (th + 32.0) / 2.0)
                .clamp(inset, (sh - th - 32.0 - inset).max(inset));
            let card_y = lerp(start_y, final_y, wp);
            l.vertical = true;
            l.invert_x = self.cfg.bar_position == "right";
            l.body_w = body_w;
            l.body_h = body_h;
            l.body_x = if l.invert_x {
                sw - span + edge - body_w
            } else {
                span - edge
            };
            l.body_y = card_y + 16.0;
        } else {
            let (ox, _, ow) = self.origin_for_popup(kind);
            let start_span = tw.min(ow.max(40.0));
            l.vertical = false;
            l.invert_y = !kind.is_top_island() && self.cfg.bar_position == "bottom";
            l.body_w = (start_span + (tw - start_span) * wp).max(2.0);
            l.body_h = (th * hp).max(2.0);
            if kind.is_top_island() {
                l.body_x = (sw - l.body_w) / 2.0;
                l.body_y = if self.cfg.bar_position == "top" {
                    span - edge
                } else {
                    0.0
                };
            } else {
                let start_card_w = start_span + 32.0;
                let final_card_w = tw + 32.0;
                let start_card_x = (ox - start_card_w / 2.0)
                    .clamp(inset, (sw - start_card_w - inset).max(inset));
                let final_card_x = (ox - final_card_w / 2.0)
                    .clamp(inset, (sw - final_card_w - inset).max(inset));
                let card_x = lerp(start_card_x, final_card_x, wp);
                l.body_x = card_x + 16.0;
                l.body_y = match self.cfg.bar_position.as_str() {
                    "top" => span - edge,
                    "bottom" => (sh - span + edge - l.body_h).max(0.0),
                    _ => 0.0,
                };
            }
        }

        if me < 0.999 {
            l.body_w = lerp(self.morph_from_size.0, tw, me);
            l.body_h = lerp(self.morph_from_size.1, th, me);
            l.body_x = lerp(self.morph_from_pos.0, l.body_x, me);
            l.body_y = lerp(self.morph_from_pos.1, l.body_y, me);
            if l.vertical && l.invert_x {
                l.body_x = sw - span + edge - l.body_w;
            }
        }
        if !l.vertical && l.invert_y {
            l.body_y = (sh - span + edge - l.body_h).max(0.0);
        }

        let cw = (tw - 36.0).max(16.0);
        let ch = (th - 34.0).max(16.0);
        if l.vertical {
            l.content_x = if l.invert_x {
                l.body_x + 17.0
            } else {
                l.body_x + l.body_w - 17.0 - cw
            };
            l.content_y = l.body_y + (l.body_h - ch) / 2.0;
            l.slide_x = if l.invert_x {
                (1.0 - hp) * 10.0
            } else {
                -(1.0 - hp) * 10.0
            };
        } else {
            l.content_x = l.body_x + (l.body_w - cw) / 2.0;
            l.content_y = if l.invert_y {
                l.body_y + l.body_h - th + 20.0
            } else {
                l.body_y + 14.0
            };
            l.slide_y = if l.invert_y {
                (1.0 - hp) * 10.0 - (1.0 - self.m_fade.value()) * 6.0
            } else {
                -(1.0 - hp) * 10.0 + (1.0 - self.m_fade.value()) * 6.0
            };
        }

        Some(l)
    }

    fn point_in_island_card(&self, px: f64, py: f64) -> bool {
        let r = self.prev_island_rect;
        if r.2 > 0.0 && r.3 > 0.0 && px >= r.0 && px <= r.0 + r.2 && py >= r.1 && py <= r.1 + r.3 {
            return true;
        }
        if self.active_popup != PopupKind::None {
            let (tw, th) = self.popup_target_size(self.active_popup);
            let span = self.cfg.bar_size as f64 + if self.cfg.bar_floating { 10.0 } else { 0.0 };
            let sw = (self.island_w as f64).max(1536.0);
            let sh = (self.island_h as f64).max(960.0);
            let inset = 6.0;
            let (ox, _, _) = self.origin_for_popup(self.active_popup);
            let cx = if self.active_popup.is_top_island() {
                (sw - tw - 32.0) / 2.0
            } else {
                (ox - (tw + 32.0) / 2.0).clamp(inset, (sw - tw - 32.0 - inset).max(inset))
            };
            let cy = if self.cfg.bar_position == "bottom" && !self.active_popup.is_top_island() {
                (sh - span - th).max(0.0)
            } else {
                span - 6.0
            };
            if px >= cx && px <= cx + tw + 32.0 && py >= cy && py <= cy + th + 16.0 {
                return true;
            }
        }
        false
    }

    fn is_hover_trigger_popup(kind: PopupKind) -> bool {
        matches!(
            kind,
            PopupKind::Bluetooth
                | PopupKind::Brightness
                | PopupKind::Volume
                | PopupKind::Battery
                | PopupKind::Wifi
                | PopupKind::Power
                | PopupKind::Controls
                | PopupKind::Settings
        )
    }

    fn invalidate_content(&mut self) {
        self.content_dirty = true;
        self.settings_body_dirty = true;
    }

    fn save_setting(&mut self, key: &str, val: serde_json::Value) {
        let mut v = read_settings_value();
        if let Some(obj) = v.as_object_mut() {
            obj.insert(key.to_string(), val);
        }
        write_settings_value(&v);
        self.cfg_mtimes = config_mtimes();
    }

    fn apply_reloaded_config(&mut self) {
        self.cfg = load_config();
        self.theme = theme_from_config(&self.cfg);
        self.drag_pill_cache = None;
        self.cfg_mtimes = config_mtimes();
        self.sync_bar_layout();
        self.sync_pills();
        self.invalidate_content();
    }

    fn set_popup(&mut self, next: PopupKind) {
        if next == PopupKind::None {
            self.opened_by_hover = false;
            self.hover_close_at = None;
            self.kbd_held_key = None;
            if self.portal_req.is_some() {
                self.cancel_portal();
            }
        }
        if next == self.active_popup {
            return;
        }
        if next == PopupKind::None && self.rendered_popup == PopupKind::None {
            self.active_popup = next;
            return;
        }
        let old_rendered = self.rendered_popup;
        let displayed = self
            .island_layout()
            .map(|l| (l.body_w, l.body_h, l.body_x, l.body_y))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        self.active_popup = next;

        if next == PopupKind::None {
            self.closing = true;
            self.filter_focused = false;
            self.files_context_menu = None;
            if !matches!(self.files_prompt, FilesPrompt::None) {
                self.files_prompt = FilesPrompt::None;
                self.files_prompt_text.clear();
            }
            self.wifi_selected_ssid.clear();
            self.wifi_password.clear();
            let close_ms = self.cfg.anim_close_ms;
            let c_close_ms = (close_ms * 0.65).clamp(80.0, 160.0);
            self.m_width = Motion::new(self.m_width.value(), 0.0, close_ms, Ease::Close);
            self.m_height = Motion::new(self.m_height.value(), 0.0, close_ms, Ease::Close);
            self.m_content = Motion::new(self.m_content.value(), 0.0, c_close_ms, Ease::Fade);
            self.m_content_scale = Motion::new(self.m_content_scale.value(), 0.96, c_close_ms, Ease::OutCubic);
            self.m_morph = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
            self.m_fade = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
            self.anim_sync_pending = true;
            self.island_layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            self.island_layer.commit();
        } else {
            self.closing = false;
            if next != PopupKind::Wifi && next != PopupKind::Controls {
                self.wifi_selected_ssid.clear();
                self.wifi_password.clear();
            }
            if next == PopupKind::Launcher {
                self.launcher_query.clear();
                let mtimes = app_dirs_mtimes();
                if mtimes != self.app_dirs_mtimes {
                    self.app_dirs_mtimes = mtimes;
                    self.all_apps = scan_desktop_apps_cached(&self.all_apps);
                }
                self.refilter_apps();
            }
            if next == PopupKind::Files {
                self.drive_usages = read_drive_usages();
                self.last_drive_check = Instant::now();
            }
            if next == PopupKind::Settings {
                self.settings_tab = 0;
                self.cal_offset_months = 0;
                self.cal_selected_key = -1;
                self.last_clock_sec = chrono::Local::now().timestamp();
                let st = self.stats.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(380));
                    refresh_stats_now(&st);
                });
            }
            if next == PopupKind::Antigravity {
                self.agy_scroll = 0.0;
                self.agy_cursor_on = true;
                self.agy_blink_at = Instant::now();
            }
            match next {
                PopupKind::Wifi | PopupKind::Bluetooth | PopupKind::Volume | PopupKind::Battery | PopupKind::Controls => {
                    if let Ok(mut st) = self.stats.lock() {
                        st.wifi_scanning = false;
                        st.bt_scanning = false;
                        st.wifi_status_msg.clear();
                        st.bt_status_msg.clear();
                    }
                    let st = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(380));
                        refresh_panel_data(&st);
                    });
                }
                PopupKind::Media | PopupKind::Notifications => {
                    let st = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(380));
                        refresh_stats_now(&st);
                    });
                }
                _ => {}
            }
            self.invalidate_content();
            let was_closed = old_rendered == PopupKind::None || self.m_width.value() < 0.05;
            let same_side = old_rendered != PopupKind::None
                && (self.cfg.bar_position == "top"
                    || old_rendered.is_top_island() == next.is_top_island());
            self.rendered_popup = next;
            let open_ms = self.cfg.anim_open_ms;
            let content_ms = self.cfg.anim_content_ms;
            let morph_ms = self.cfg.anim_morph_ms;
            let curve = Ease::Custom(self.cfg.anim_curve);
            if was_closed || !same_side {
                self.m_width = Motion::new(0.0, 1.0, (open_ms * 0.92).max(120.0), curve);
                self.m_height = Motion::new(0.0, 1.0, open_ms, curve);
                self.m_content = Motion::new(0.0, 1.0, content_ms, Ease::Fade).delayed((open_ms * 0.08).clamp(15.0, 40.0));
                self.m_content_scale = Motion::new(0.94, 1.0, content_ms, curve).delayed((open_ms * 0.06).clamp(12.0, 35.0));
                self.m_morph = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
                self.m_fade = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
            } else {
                self.morph_from_size = (displayed.0, displayed.1);
                self.morph_from_pos = (displayed.2, displayed.3);
                self.m_width = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
                self.m_height = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
                self.m_content = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
                self.m_morph = Motion::new(0.0, 1.0, morph_ms, curve);
                self.m_fade = Motion::new(0.0, 1.0, (morph_ms * 0.72).max(120.0), Ease::Fade).delayed(15.0);
                self.m_content_scale = Motion::new(0.97, 1.0, (morph_ms * 0.78).max(120.0), curve).delayed(10.0);
            }
            self.anim_sync_pending = true;
            self.island_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.island_layer.commit();
        }
        self.draw_island_surface();
        self.draw_bar_surface();
    }

    fn switch_settings_tab(&mut self, new_tab: usize) {
        let new_tab = new_tab.min(4);
        if self.rendered_popup != PopupKind::Settings || self.active_popup != PopupKind::Settings {
            self.set_popup(PopupKind::Settings);
            self.settings_prev_tab = new_tab;
            self.settings_tab = new_tab;
            self.settings_tab_anim = Motion::new(1.0, 1.0, 1.0, Ease::Linear);
            self.settings_tab_slide_x = 0.0;
        } else if new_tab != self.settings_tab {
            let direction = if new_tab >= self.settings_tab { 1.0 } else { -1.0 };
            self.settings_prev_tab = self.settings_tab;
            self.settings_tab = new_tab;
            self.settings_tab_anim = Motion::new(0.0, 1.0, 180.0, Ease::OutCubic);
            self.settings_tab_slide_x = direction * 14.0;
        }
        self.settings_open_dropdown.clear();
        if self.settings_tab == 1 {
            self.refresh_wallpapers_if_needed();
            if self.settings_bg_target == "mango" {
                self.mango_info = read_mango_summary();
            }
        }
        self.invalidate_content();
    }

    fn refresh_wallpapers_if_needed(&mut self) {
        let preview_path = match self.settings_bg_target.as_str() {
            "lockscreen" => {
                if self.cfg.lock_use_same_wallpaper {
                    self.cfg.wallpaper_path.clone()
                } else {
                    self.cfg.lock_wallpaper_path.clone()
                }
            }
            "sddm" => {
                if self.cfg.sddm_selected_path.is_empty() {
                    self.cfg.wallpaper_path.clone()
                } else {
                    self.cfg.sddm_selected_path.clone()
                }
            }
            _ => self.cfg.wallpaper_path.clone(),
        };
        if self.current_preview_path != preview_path {
            self.wallpaper_prev_surf = None;
            self.current_preview_path = preview_path.clone();
            self.current_preview_surf = load_thumb_surface(Path::new(&preview_path), 304, 160);
        }
        let dir = PathBuf::from(&self.cfg.wallpaper_dir);
        let paths = list_wallpaper_paths(&dir);
        let needs_reload = self.wallpapers.len() != paths.len()
            || self
                .wallpapers
                .iter()
                .zip(paths.iter())
                .any(|((_, p1, _), p2)| p1 != p2);
        if needs_reload {
            self.wallpapers = paths
                .iter()
                .map(|p| {
                    let name = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    (name, p.clone(), None)
                })
                .collect();
            let (tx, rx) = std::sync::mpsc::channel();
            self.thumb_rx = Some(rx);
            let load_paths: Vec<PathBuf> = paths.into_iter().take(6).collect();
            std::thread::spawn(move || {
                for p in load_paths {
                    let cached = ensure_thumb_png(&p, 280, 140);
                    if tx.send((p, cached)).is_err() {
                        break;
                    }
                }
            });
        }
    }

    fn apply_theme_now(&mut self) {
        self.cfg.current_theme = theme_preset_by_name(&self.cfg.theme_name);
        let next_theme = theme_from_config(&self.cfg);
        self.theme = next_theme.clone();
        self.drag_pill_cache = None;
        self.app_dirs_mtimes = app_dirs_mtimes();
        self.all_apps = scan_desktop_apps_cached(&self.all_apps);
        if self.rendered_popup == PopupKind::Launcher {
            self.refilter_apps();
        }
        self.invalidate_content();
        self.draw_bar_surface();
        self.draw_island_surface();
        trigger_system_theme_apply(&self.cfg, &next_theme);
    }

    fn apply_pending_wipe(&mut self, pending: PendingWipe) {
        match pending {
            PendingWipe::Theme => self.apply_theme_now(),
            PendingWipe::Wallpaper { path, mode } => {
                self.cfg.wallpaper_path = path.clone();
                self.cfg.wallpaper_mode = mode.clone();
                apply_wallpaper(&path, &mode);
                self.refresh_wallpapers_if_needed();
                self.invalidate_content();
                self.draw_island_surface();
            }
        }
    }

    fn begin_diagonal_wipe(&mut self, pending: PendingWipe) {
        // Clear any earlier unfinished wipe so grim captures clean pixels
        self.wipe_gpu = None;
        self.wipe_layer = None;
        self.wipe_configured = false;
        self.wipe_frame_pending = false;
        self.wipe_rgb = None;
        self.wipe_pending = None;

        if self.gpu_failed {
            self.apply_pending_wipe(pending);
            return;
        }
        let Some((rw, rh, rgb)) = capture_screen_ppm() else {
            self.apply_pending_wipe(pending);
            return;
        };

        let surf = self.compositor_state.create_surface(&self.qh);
        if let Ok(empty_reg) = Region::new(&self.compositor_state) {
            surf.set_input_region(Some(empty_reg.wl_region()));
        }
        let layer = self.layer_shell.create_layer_surface(
            &self.qh,
            surf,
            Layer::Overlay,
            Some("yuno-wipe"),
            None,
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();

        self.wipe_layer = Some(layer);
        self.wipe_rgb = Some((rw, rh, rgb));
        self.wipe_pending = Some(pending);
    }

    fn reload_theme_and_apply(&mut self) {
        self.begin_diagonal_wipe(PendingWipe::Theme);
    }

    fn start_wallpaper_transition(&mut self, new_path: String, mode: String) {
        if self.cfg.wallpaper_path == new_path && self.cfg.wallpaper_mode == mode {
            return;
        }
        self.begin_diagonal_wipe(PendingWipe::Wallpaper {
            path: new_path,
            mode,
        });
    }

    fn draw_wipe_surface(&mut self) {
        let Some(ref wipe_layer) = self.wipe_layer else { return };
        if self.wipe_anim.done() {
            self.wipe_gpu = None;
            self.wipe_layer = None;
            self.wipe_configured = false;
            self.wipe_frame_pending = false;
            return;
        }
        let Some(ref gpu) = self.wipe_gpu else { return };
        if !self.wipe_frame_pending {
            self.wipe_frame_pending = true;
            let surf = wipe_layer.wl_surface();
            surf.frame(&self.qh, surf.clone());
        }
        let p = self.wipe_anim.value().clamp(0.0, 1.0) as f32;
        let accent = [
            self.theme.accent.0 as f32,
            self.theme.accent.1 as f32,
            self.theme.accent.2 as f32,
        ];
        gpu.present(p, accent);
    }

    fn toggle_popup(&mut self, kind: PopupKind) {
        if self.active_popup == kind {
            self.set_popup(PopupKind::None);
        } else {
            self.set_popup(kind);
        }
    }

    fn agy_model(&self) -> &'static crate::services::AgyModel {
        &AGY_MODELS[self.agy_model_idx.min(AGY_MODELS.len() - 1)]
    }

    fn agy_effort_pair(&self) -> (&'static str, &'static str) {
        let m = self.agy_model();
        m.efforts[self.agy_effort_idx.min(m.efforts.len() - 1)]
    }

    fn agy_new_chat(&mut self) {
        self.agy_messages.clear();
        self.agy_input.clear();
        self.agy_status.clear();
        self.agy_meta.clear();
        self.agy_conv.clear();
        self.agy_scroll = 0.0;
        self.agy_busy = false;
        self.agy_rx = None;
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn send_agy(&mut self) {
        let prompt = self.agy_input.trim().to_string();
        if prompt.is_empty() || self.agy_busy {
            return;
        }
        let (effort_id, effort_label) = self.agy_effort_pair();
        let model = self.agy_model();
        self.agy_meta = format!("{} · {}", model.label, effort_label);
        self.agy_messages.push(AgyMsg { user: true, text: prompt.clone() });
        self.agy_messages.push(AgyMsg { user: false, text: String::new() });
        self.agy_busy = true;
        self.agy_status = "Thinking...".into();
        self.agy_input.clear();
        self.agy_scroll = 0.0;
        self.agy_rx = Some(agy_send(&prompt, model.id, effort_id, &self.agy_conv));
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn stop_agy(&mut self) {
        if !self.agy_busy {
            return;
        }
        self.agy_busy = false;
        self.agy_status.clear();
        self.agy_rx = None;
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn agy_backspace(&mut self) {
        if !self.agy_input.is_empty() {
            self.agy_input.pop();
            self.agy_cursor_on = true;
            self.agy_blink_at = Instant::now();
            self.invalidate_content();
            self.draw_island_surface();
        }
    }

    fn sync_files_anims(&mut self) {
        let out_expo = Ease::Custom([0.16, 1.0, 0.3, 1.0]);

        let want_search = if self.filter_focused || !self.filter_query.is_empty() {
            1.0
        } else {
            0.0
        };
        if (self.files_search_anim.to - want_search).abs() > 0.01 {
            self.files_search_anim =
                Motion::new(self.files_search_anim.value(), want_search, 180.0, out_expo);
        }

        let want_crumb = if matches!(self.files_prompt, FilesPrompt::PathInput) {
            1.0
        } else {
            0.0
        };
        if (self.files_crumb_anim.to - want_crumb).abs() > 0.01 {
            self.files_crumb_anim =
                Motion::new(self.files_crumb_anim.value(), want_crumb, 180.0, out_expo);
        }

        let has_sel = !self.selected_paths.is_empty() || self.selected_idx.is_some();
        let want_act = if has_sel { 1.0 } else { 0.0 };
        if (self.files_action_bar_anim.to - want_act).abs() > 0.01 {
            self.files_action_bar_anim = Motion::new(
                self.files_action_bar_anim.value(),
                want_act,
                160.0,
                Ease::OutCubic,
            );
        }

        match crate::files_ui::sidebar_active_target_y(
            &self.current_dir,
            &self.drive_usages,
            &self.cfg.bookmarks,
        ) {
            Some(target_y) => {
                let cur_y = self.files_sidebar_pill_y.value();
                if cur_y < 0.0 {
                    self.files_sidebar_pill_y = Motion::new(target_y, target_y, 1.0, Ease::Linear);
                    self.files_sidebar_pill_r = Motion::new(16.0, 10.0, 180.0, out_expo);
                } else if (self.files_sidebar_pill_y.to - target_y).abs() > 0.5 {
                    self.files_sidebar_pill_y = Motion::new(cur_y, target_y, 180.0, out_expo);
                    self.files_sidebar_pill_r = Motion::new(16.0, 10.0, 180.0, out_expo);
                }
            }
            None => {
                self.files_sidebar_pill_y = Motion::new(-100.0, -100.0, 1.0, Ease::Linear);
            }
        }
    }

    fn refresh_files(&mut self) {
        self.entries = list_dir_entries_sorted(
            &self.current_dir,
            self.show_hidden,
            &self.filter_query,
            self.files_sort_by,
            self.files_sort_asc,
        );
        self.selected_paths
            .retain(|sp| self.entries.iter().any(|e| &e.path == sp));
        if let Some(i) = self.selected_idx {
            if i >= self.entries.len() {
                self.selected_idx = if self.entries.is_empty() {
                    None
                } else {
                    Some(self.entries.len() - 1)
                };
            }
        }

        // Queue async thumbnail generation for visible/folder image & video files
        let missing: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|e| !e.is_dir && is_thumbable_file(&e.name) && !self.files_thumbs.contains_key(&e.path))
            .take(48)
            .map(|e| e.path.clone())
            .collect();
        if !missing.is_empty() {
            let tx = self.files_thumb_tx.clone();
            std::thread::spawn(move || {
                for p in missing {
                    let cached = ensure_thumb_png(&p, 128, 128);
                    let _ = tx.send((p, cached));
                }
            });
        }

        self.update_files_preview();
        self.invalidate_content();
    }

    fn update_files_preview(&mut self) {
        self.sync_files_anims();
        let sel = self.selected_idx.and_then(|i| self.entries.get(i));
        let target_path = sel
            .map(|e| e.path.clone())
            .unwrap_or_else(|| self.current_dir.clone());
        if self.files_preview_path == target_path {
            return;
        }
        self.files_preview_path = target_path;
        self.files_preview_surf = None;
        self.files_preview_lines.clear();
        self.files_preview_dir_count = None;

        if let Some(it) = sel {
            if it.is_dir {
                self.files_preview_dir_count = count_dir_items(&it.path);
            } else if is_text_file(&it.name) && it.size <= 512 * 1024 {
                self.files_preview_lines = read_text_preview(&it.path, 8);
            }
        }
    }

    fn navigate_files(&mut self, dir: PathBuf, record: bool) {
        if !dir.is_dir() || dir == self.current_dir {
            return;
        }
        if record {
            self.back_stack.push(self.current_dir.clone());
            self.fwd_stack.clear();
        }
        self.current_dir = dir;
        self.selected_idx = None;
        self.selected_paths.clear();
        self.files_context_menu = None;
        if !matches!(self.files_prompt, FilesPrompt::None) {
            let keep_save = matches!(self.files_prompt, FilesPrompt::SaveFilename)
                && self.portal_req.as_ref().is_some_and(|r| r.mode == "save");
            if !keep_save {
                self.files_prompt = FilesPrompt::None;
                self.files_prompt_text.clear();
            }
        }
        self.scroll_y = 0.0;
        self.files_preview_path = PathBuf::new();
        self.refresh_files();
        self.sync_files_anims();
        self.invalidate_content();
    }

    fn refilter_apps(&mut self) {
        let q = self.launcher_query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_apps = self.all_apps.clone();
        } else {
            self.filtered_apps = self
                .all_apps
                .iter()
                .filter(|a| a.name.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        self.launcher_idx = 0;
        self.launcher_scroll = 0.0;
        self.invalidate_content();
    }

    fn set_toast(&mut self, msg: &str) {
        self.status_toast = msg.to_string();
        self.toast_until = Some(Instant::now() + Duration::from_secs(2));
        self.invalidate_content();
    }

    fn update_island_input_region(&mut self) {
        let active = self.active_popup != PopupKind::None
            && !self.drag_launched
            && self.island_w > 0
            && self.island_h > 0;
        let span = if self.bar_layer.is_some() {
            (self.cfg.bar_size + if self.cfg.bar_floating { 10 } else { 0 }) as i32
        } else {
            0
        };
        let key = (
            active,
            self.cfg.bar_position.clone(),
            span,
            self.island_w,
            self.island_h,
        );
        if self.last_input_region.as_ref() == Some(&key) {
            return;
        }
        self.last_input_region = Some(key);
        if let Ok(region) = Region::new(&self.compositor_state) {
            // While a popup is open (and not actively dragging a file out), capture the
            // screen EXCEPT the bar strip so bar icons stay clickable and hoverable.
            if active {
                let w = self.island_w as i32;
                let h = self.island_h as i32;
                match self.cfg.bar_position.as_str() {
                    "top" => region.add(0, span, w, (h - span).max(1)),
                    "bottom" => region.add(0, 0, w, (h - span).max(1)),
                    "left" => region.add(span, 0, (w - span).max(1), h),
                    "right" => region.add(0, 0, (w - span).max(1), h),
                    _ => region.add(0, 0, w, h),
                }
            }
            self.island_layer
                .wl_surface()
                .set_input_region(Some(region.wl_region()));
        }
    }

    fn draw_bar_surface(&mut self) {
        if !self.bar_configured || self.bar_w == 0 || self.bar_h == 0 {
            return;
        }
        let Some(ref bar) = self.bar_layer else { return };
        let Some(ref mut pool) = self.bar_pool else { return };
        let w = self.bar_w as i32;
        let h = self.bar_h as i32;
        let pw = w * BUF_SCALE;
        let ph = h * BUF_SCALE;
        let stride = pw * 4;
        if pool.len() < (pw * ph * 4) as usize * 2 {
            let _ = pool.resize((pw * ph * 4) as usize * 3);
        }
        let Ok((buf, canvas)) = pool.create_buffer(pw, ph, stride, wl_shm::Format::Argb8888) else {
            return;
        };
        let stats = self.stats.lock().map(|s| s.clone()).unwrap_or_default();
        if let Ok(img) = unsafe {
            ImageSurface::create_for_data_unsafe(canvas.as_mut_ptr(), Format::ARgb32, pw, ph, stride)
        } {
            img.set_device_scale(BUF_SCALE as f64, BUF_SCALE as f64);
            if let Ok(cr) = Context::new(&img) {
                let pills = self.bar_pills();
                self.bar_hits = draw_bar(
                    &cr,
                    w as f64,
                    h as f64,
                    &self.cfg,
                    &self.theme,
                    &stats,
                    self.active_popup,
                    self.cc_expanded,
                    &self.hover,
                    self.yuno_icon.as_ref(),
                    &self.tray_icons,
                    &pills,
                    self.started.elapsed().as_secs_f64(),
                );
                img.flush();
            }
        }
        let surf = bar.wl_surface();
        surf.set_buffer_scale(BUF_SCALE);
        let _ = buf.attach_to(surf);
        surf.damage_buffer(0, 0, pw, ph);
        if self.bar_needs_anim() && !self.bar_frame_pending {
            self.bar_frame_pending = true;
            surf.frame(&self.qh, surf.clone());
        }
        surf.commit();
    }

    fn rebuild_content_cache_if_needed(&mut self) {
        if !self.content_dirty || self.rendered_popup == PopupKind::None {
            return;
        }
        self.content_dirty = false;
        let (target_w, target_h) = self.popup_target_size(self.rendered_popup);
        // Quickshell reserves 18px per side and 14px top/bottom for the island shell.
        let cw = (target_w - 36.0).max(16.0).ceil() as i32;
        let ch = (target_h - 34.0).max(16.0).ceil() as i32;
        let pcw = cw * BUF_SCALE;
        let pch = ch * BUF_SCALE;

        let stats = self.stats.lock().map(|s| s.clone()).unwrap_or_default();
        if self.rendered_popup == PopupKind::Media && stats.media.art_url != self.media_art_url {
            self.media_art_url = stats.media.art_url.clone();
            let raw = self.media_art_url.trim_start_matches("file://");
            self.media_art_surf = if !raw.is_empty() && Path::new(raw).is_file() {
                load_thumb_surface(Path::new(raw), 104, 104)
            } else {
                None
            };
        }
        self.update_island_input_region();
        let pill_p = self.settings_tab_anim.value();
        let slide_x = self.settings_tab_slide_x * (1.0 - pill_p);

        // If Settings tab body changed, render it once into `settings_body_cache`
        if self.rendered_popup == PopupKind::Settings && self.settings_body_dirty {
            self.settings_body_dirty = false;
            let body_w = (cw - 156).max(40);
            let body_h = ch.max(40);
            let pbw = body_w * BUF_SCALE;
            let pbh = body_h * BUF_SCALE;
            let need_body_alloc = self
                .settings_body_cache
                .as_ref()
                .map(|s| s.width() != pbw || s.height() != pbh)
                .unwrap_or(true);
            if need_body_alloc {
                if let Ok(surf) = ImageSurface::create(Format::ARgb32, pbw, pbh) {
                    surf.set_device_scale(BUF_SCALE as f64, BUF_SCALE as f64);
                    self.settings_body_cache = Some(surf);
                }
            }
            if let Some(ref bsurf) = self.settings_body_cache {
                let bstate = IslandRenderState {
                    kind: self.rendered_popup,
                    cfg: &self.cfg,
                    theme: &self.theme,
                    stats: &stats,
                    hover: &self.hover,
                    current_dir: &self.current_dir,
                    can_back: !self.back_stack.is_empty(),
                    can_fwd: !self.fwd_stack.is_empty(),
                    show_hidden: self.show_hidden,
                    filter_query: &self.filter_query,
                    filter_focused: self.filter_focused,
                    entries: &self.entries,
                    selected_idx: self.selected_idx,
                    scroll_y: self.scroll_y,
                    status_toast: &self.status_toast,
                    launcher_query: &self.launcher_query,
                    filtered_apps: &self.filtered_apps,
                    launcher_idx: self.launcher_idx,
                    launcher_scroll: self.launcher_scroll,
                    settings_tab: self.settings_tab,
                    settings_prev_tab: self.settings_prev_tab,
                    settings_tab_pill_progress: pill_p,
                    settings_tab_slide_x: slide_x,
                    settings_bg_target: &self.settings_bg_target,
                    settings_open_dropdown: &self.settings_open_dropdown,
                    wallpapers: &self.wallpapers,
                    current_preview_surf: self.current_preview_surf.as_ref(),
                    wallpaper_prev_surf: self.wallpaper_prev_surf.as_ref(),
                    wallpaper_preview_progress: self.wallpaper_preview_anim.value(),
                    mango_info: &self.mango_info,
                    cal_offset_months: self.cal_offset_months,
                    cal_selected_key: self.cal_selected_key,
                    agy_messages: &self.agy_messages,
                    agy_input: &self.agy_input,
                    agy_busy: self.agy_busy,
                    agy_status: &self.agy_status,
                    agy_meta: &self.agy_meta,
                    agy_model_idx: self.agy_model_idx,
                    agy_effort: self.agy_effort_idx,
                    agy_scroll: self.agy_scroll,
                    agy_cursor_on: self.agy_cursor_on,
                    wifi_selected_ssid: &self.wifi_selected_ssid,
                    wifi_password: &self.wifi_password,
                    wifi_scroll: self.wifi_scroll,
                    bt_scroll: self.bt_scroll,
                    notif_scroll: self.notif_scroll,
                    cc_expanded: self.cc_expanded,
                    media_art_surf: self.media_art_surf.as_ref(),
                    settings_body_cache: None,
                    selected_paths: &self.selected_paths,
                    files_sort_by: self.files_sort_by,
                    files_sort_asc: self.files_sort_asc,
                    files_prompt: &self.files_prompt,
                    files_prompt_text: &self.files_prompt_text,
                    files_clip_count: self.files_clipboard.len(),
                    files_clip_cut: self.files_clip_mode == ClipMode::Cut,
                    files_preview_surf: self.files_preview_surf.as_ref(),
                    files_preview_lines: &self.files_preview_lines,
                    files_preview_dir_count: self.files_preview_dir_count,
                    files_context_menu: self.files_context_menu,
                    portal_info: None,
                    files_view_mode: self.files_view_mode,
                    files_drive_usages: &self.drive_usages,
                    files_dnd_target: self.internal_drag_target.as_deref(),
                    files_is_dragging: self.internal_dragging,
                    files_drag_count: self.internal_drag_sources.len(),
                    files_drag_label: self.internal_drag_sources.first().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or(""),
                    files_drag_pos: self.internal_drag_pos,
                    files_marquee_rect: self.marquee_rect,
                    files_thumbs: &self.files_thumbs,
                    files_search_expand: self.files_search_anim.value(),
                    files_crumb_focus: self.files_crumb_anim.value(),
                    files_sidebar_pill_y: self.files_sidebar_pill_y.value(),
                    files_sidebar_pill_r: self.files_sidebar_pill_r.value(),
                    files_ctx_open: self.files_ctx_anim.value(),
                    files_action_slide: self.files_action_bar_anim.value(),
                    files_qa_dnd_hover: self.files_qa_dnd_hover,
                    files_drag_in_cache: false,
                    pressed_target: &self.pressed_target,
                };
                if let Ok(cr) = Context::new(bsurf) {
                    self.settings_body_hits =
                        render_settings_tab_body(&cr, body_w as f64, body_h as f64, &bstate);
                    bsurf.flush();
                }
            }
        }

        let need_cache_alloc = self
            .content_cache
            .as_ref()
            .map(|s| s.width() != pcw || s.height() != pch)
            .unwrap_or(true);
        if need_cache_alloc {
            if let Ok(surf) = ImageSurface::create(Format::ARgb32, pcw, pch) {
                surf.set_device_scale(BUF_SCALE as f64, BUF_SCALE as f64);
                self.content_cache = Some(surf);
            }
        }

        let state = IslandRenderState {
            kind: self.rendered_popup,
            cfg: &self.cfg,
            theme: &self.theme,
            stats: &stats,
            hover: &self.hover,
            current_dir: &self.current_dir,
            can_back: !self.back_stack.is_empty(),
            can_fwd: !self.fwd_stack.is_empty(),
            show_hidden: self.show_hidden,
            filter_query: &self.filter_query,
            filter_focused: self.filter_focused,
            entries: &self.entries,
            selected_idx: self.selected_idx,
            scroll_y: self.scroll_y,
            status_toast: &self.status_toast,
            launcher_query: &self.launcher_query,
            filtered_apps: &self.filtered_apps,
            launcher_idx: self.launcher_idx,
            launcher_scroll: self.launcher_scroll,
            settings_tab: self.settings_tab,
            settings_prev_tab: self.settings_prev_tab,
            settings_tab_pill_progress: pill_p,
            settings_tab_slide_x: slide_x,
            settings_bg_target: &self.settings_bg_target,
            settings_open_dropdown: &self.settings_open_dropdown,
            wallpapers: &self.wallpapers,
            current_preview_surf: self.current_preview_surf.as_ref(),
            wallpaper_prev_surf: self.wallpaper_prev_surf.as_ref(),
            wallpaper_preview_progress: self.wallpaper_preview_anim.value(),
            mango_info: &self.mango_info,
            cal_offset_months: self.cal_offset_months,
            cal_selected_key: self.cal_selected_key,
            agy_messages: &self.agy_messages,
            agy_input: &self.agy_input,
            agy_busy: self.agy_busy,
            agy_status: &self.agy_status,
            agy_meta: &self.agy_meta,
            agy_model_idx: self.agy_model_idx,
            agy_effort: self.agy_effort_idx,
            agy_scroll: self.agy_scroll,
            agy_cursor_on: self.agy_cursor_on,
            wifi_selected_ssid: &self.wifi_selected_ssid,
            wifi_password: &self.wifi_password,
            wifi_scroll: self.wifi_scroll,
            bt_scroll: self.bt_scroll,
            notif_scroll: self.notif_scroll,
            cc_expanded: self.cc_expanded,
            media_art_surf: self.media_art_surf.as_ref(),
            settings_body_cache: self.settings_body_cache.as_ref(),
            selected_paths: &self.selected_paths,
            files_sort_by: self.files_sort_by,
            files_sort_asc: self.files_sort_asc,
            files_prompt: &self.files_prompt,
            files_prompt_text: &self.files_prompt_text,
            files_clip_count: self.files_clipboard.len(),
            files_clip_cut: self.files_clip_mode == ClipMode::Cut,
            files_preview_surf: self.files_preview_surf.as_ref(),
            files_preview_lines: &self.files_preview_lines,
            files_preview_dir_count: self.files_preview_dir_count,
            files_context_menu: self.files_context_menu,
            portal_info: self.portal_req.as_ref().map(|r| (
                r.title.as_str(),
                if r.accept_label.is_empty() {
                    if r.mode == "save" { "Save" } else { "Open" }
                } else {
                    r.accept_label.as_str()
                },
            )),
            files_view_mode: self.files_view_mode,
            files_drive_usages: &self.drive_usages,
            files_dnd_target: self.internal_drag_target.as_deref(),
            files_is_dragging: self.internal_dragging,
            files_drag_count: self.internal_drag_sources.len(),
            files_drag_label: self.internal_drag_sources.first().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or(""),
            files_drag_pos: self.internal_drag_pos,
            files_marquee_rect: self.marquee_rect,
            files_thumbs: &self.files_thumbs,
            files_search_expand: self.files_search_anim.value(),
            files_crumb_focus: self.files_crumb_anim.value(),
            files_sidebar_pill_y: self.files_sidebar_pill_y.value(),
            files_sidebar_pill_r: self.files_sidebar_pill_r.value(),
            files_ctx_open: self.files_ctx_anim.value(),
            files_action_slide: self.files_action_bar_anim.value(),
            files_qa_dnd_hover: self.files_qa_dnd_hover,
            files_drag_in_cache: self.gpu.is_none(),
            pressed_target: &self.pressed_target,
        };

        if let Some(ref surf) = self.content_cache {
            if let Ok(cr) = Context::new(surf) {
                let mut hits = render_popup_content(&cr, 0.0, 0.0, cw as f64, ch as f64, &state);
                if self.rendered_popup == PopupKind::Settings && self.settings_body_cache.is_some() {
                    hits.extend(self.settings_body_hits.iter().cloned());
                }
                self.content_hits = hits;
                surf.flush();
                self.cache_gen = self.cache_gen.wrapping_add(1).max(1);
            }
        }
    }

    fn draw_island_surface(&mut self) {
        if !self.island_configured || self.island_w == 0 || self.island_h == 0 {
            return;
        }
        if !self.closing
            && ((self.rendered_popup == PopupKind::Settings && !self.settings_tab_anim.done())
                || self.files_animating())
        {
            self.content_dirty = true;
        }
        if !self.closing {
            self.rebuild_content_cache_if_needed();
        } else {
            self.content_dirty = false;
        }

        if self.anim_sync_pending {
            self.anim_sync_pending = false;
            let now = Instant::now();
            if !self.m_width.done() {
                self.m_width.start = now;
            }
            if !self.m_height.done() {
                self.m_height.start = now;
            }
            if !self.m_content.done() {
                self.m_content.start = now;
            }
            if !self.m_content_scale.done() {
                self.m_content_scale.start = now;
            }
            if !self.m_morph.done() {
                self.m_morph.start = now;
            }
            if !self.m_fade.done() {
                self.m_fade.start = now;
            }
        }

        if self.closing
            && self.m_width.done()
            && self.m_height.done()
            && self.m_content.done()
            && self.m_content_scale.done()
        {
            self.closing = false;
            self.rendered_popup = PopupKind::None;
            self.content_cache = None;
            self.content_hits.clear();
            self.island_hits.clear();
            self.hover = None;
            self.cc_expanded = "";
            self.wifi_selected_ssid.clear();
            self.wifi_password.clear();
        }

        let layout = self.island_layout();
        let w = self.island_w as i32;
        let h = self.island_h as i32;
        let pw = w * BUF_SCALE;
        let ph = h * BUF_SCALE;
        let stride = w * 4;
        self.island_layer.wl_surface().set_buffer_scale(BUF_SCALE);

        let Some(l) = layout else {
            let mut need_clear = self.prev_island_rect.2 > 0.0 && self.prev_island_rect.3 > 0.0;
            if !self.gpu_failed && self.gpu.is_none() {
                match GpuIsland::new(self.gpu_display, self.gpu_surface, pw, ph) {
                    Ok(g) => {
                        self.gpu = Some(g);
                        need_clear = true;
                    }
                    Err(e) => {
                        eprintln!("[yunoshell] GPU renderer unavailable, using SHM: {e}");
                        self.gpu_failed = true;
                        need_clear = true;
                    }
                }
            }
            self.prev_island_rect = (0.0, 0.0, 0.0, 0.0);
            self.cur_body_rect = (0.0, 0.0, 0.0, 0.0);
            self.update_island_input_region();
            if need_clear {
                if let Some(ref mut gpu) = self.gpu {
                    gpu.resize(pw, ph);
                    gpu.clear();
                } else if let Ok((buf, canvas)) =
                    self.island_pool.create_buffer(w, h, stride, wl_shm::Format::Argb8888)
                {
                    if let Ok(img) = unsafe {
                        ImageSurface::create_for_data_unsafe(
                            canvas.as_mut_ptr(),
                            Format::ARgb32,
                            w,
                            h,
                            stride,
                        )
                    } {
                        if let Ok(cr) = Context::new(&img) {
                            cr.set_operator(Operator::Source);
                            cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                            let _ = cr.paint();
                            img.flush();
                        }
                    }
                    let surf = self.island_layer.wl_surface();
                    surf.set_buffer_scale(1);
                    let _ = buf.attach_to(surf);
                    surf.damage_buffer(0, 0, w, h);
                    surf.commit();
                }
            } else {
                self.island_layer.wl_surface().commit();
            }
            return;
        };

        let stats = self.stats.lock().map(|s| s.clone()).unwrap_or_default();
        let content_alpha = self.m_content.value() * self.m_fade.value();
        if std::env::var_os("YUNO_DEBUG_LAYOUT").is_some() {
            let (tw, th) = self.popup_target_size(self.rendered_popup);
            eprintln!(
                "[layout] kind={:?} bar={} float={} island={}x{} target={:.1}x{:.1} body=({:.1},{:.1} {:.1}x{:.1}) vert={} content=({:.1},{:.1}) ch={:.1}",
                self.rendered_popup,
                self.cfg.bar_position,
                self.cfg.bar_floating,
                self.island_w,
                self.island_h,
                tw,
                th,
                l.body_x,
                l.body_y,
                l.body_w,
                l.body_h,
                l.vertical,
                l.content_x,
                l.content_y,
                th - 34.0
            );
        }
        self.update_island_input_region();
        let pill_p = self.settings_tab_anim.value();
        let slide_x = self.settings_tab_slide_x * (1.0 - pill_p);
        let state = IslandRenderState {
            kind: self.rendered_popup,
            cfg: &self.cfg,
            theme: &self.theme,
            stats: &stats,
            hover: &self.hover,
            current_dir: &self.current_dir,
            can_back: !self.back_stack.is_empty(),
            can_fwd: !self.fwd_stack.is_empty(),
            show_hidden: self.show_hidden,
            filter_query: &self.filter_query,
            filter_focused: self.filter_focused,
            entries: &self.entries,
            selected_idx: self.selected_idx,
            scroll_y: self.scroll_y,
            status_toast: &self.status_toast,
            launcher_query: &self.launcher_query,
            filtered_apps: &self.filtered_apps,
            launcher_idx: self.launcher_idx,
            launcher_scroll: self.launcher_scroll,
            settings_tab: self.settings_tab,
            settings_prev_tab: self.settings_prev_tab,
            settings_tab_pill_progress: pill_p,
            settings_tab_slide_x: slide_x,
            settings_bg_target: &self.settings_bg_target,
            settings_open_dropdown: &self.settings_open_dropdown,
            wallpapers: &self.wallpapers,
            current_preview_surf: self.current_preview_surf.as_ref(),
            wallpaper_prev_surf: self.wallpaper_prev_surf.as_ref(),
            wallpaper_preview_progress: self.wallpaper_preview_anim.value(),
            mango_info: &self.mango_info,
            cal_offset_months: self.cal_offset_months,
            cal_selected_key: self.cal_selected_key,
            agy_messages: &self.agy_messages,
            agy_input: &self.agy_input,
            agy_busy: self.agy_busy,
            agy_status: &self.agy_status,
            agy_meta: &self.agy_meta,
            agy_model_idx: self.agy_model_idx,
            agy_effort: self.agy_effort_idx,
            agy_scroll: self.agy_scroll,
            agy_cursor_on: self.agy_cursor_on,
            wifi_selected_ssid: &self.wifi_selected_ssid,
            wifi_password: &self.wifi_password,
            wifi_scroll: self.wifi_scroll,
            bt_scroll: self.bt_scroll,
            notif_scroll: self.notif_scroll,
            cc_expanded: self.cc_expanded,
            media_art_surf: self.media_art_surf.as_ref(),
            settings_body_cache: self.settings_body_cache.as_ref(),
            selected_paths: &self.selected_paths,
            files_sort_by: self.files_sort_by,
            files_sort_asc: self.files_sort_asc,
            files_prompt: &self.files_prompt,
            files_prompt_text: &self.files_prompt_text,
            files_clip_count: self.files_clipboard.len(),
            files_clip_cut: self.files_clip_mode == ClipMode::Cut,
            files_preview_surf: self.files_preview_surf.as_ref(),
            files_preview_lines: &self.files_preview_lines,
            files_preview_dir_count: self.files_preview_dir_count,
            files_context_menu: self.files_context_menu,
            portal_info: self.portal_req.as_ref().map(|r| (
                r.title.as_str(),
                if r.accept_label.is_empty() {
                    if r.mode == "save" { "Save" } else { "Open" }
                } else {
                    r.accept_label.as_str()
                },
            )),
            files_view_mode: self.files_view_mode,
            files_drive_usages: &self.drive_usages,
            files_dnd_target: self.internal_drag_target.as_deref(),
            files_is_dragging: self.internal_dragging,
            files_drag_count: self.internal_drag_sources.len(),
            files_drag_label: self.internal_drag_sources.first().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or(""),
            files_drag_pos: self.internal_drag_pos,
            files_marquee_rect: self.marquee_rect,
            files_thumbs: &self.files_thumbs,
            files_search_expand: self.files_search_anim.value(),
            files_crumb_focus: self.files_crumb_anim.value(),
            files_sidebar_pill_y: self.files_sidebar_pill_y.value(),
            files_sidebar_pill_r: self.files_sidebar_pill_r.value(),
            files_ctx_open: self.files_ctx_anim.value(),
            files_action_slide: self.files_action_bar_anim.value(),
            files_qa_dnd_hover: self.files_qa_dnd_hover,
            files_drag_in_cache: self.gpu.is_none(),
            pressed_target: &self.pressed_target,
        };

        let mut cur_rect = (0.0, 0.0, 0.0, 0.0);
        let mut gpu_drawn = false;

        // Compute integer-aligned screen damage rect to size the offscreen Cairo surface
        // and composite the GPU quad back at its exact screen position.
        let raw_rect = if l.vertical {
            (l.body_x, l.body_y - 16.0, l.body_w, l.body_h + 32.0)
        } else {
            (l.body_x - 16.0, l.body_y, l.body_w + 32.0, l.body_h)
        };
        let rx = raw_rect.0.floor().max(0.0);
        let ry = raw_rect.1.floor().max(0.0);
        let iw = ((raw_rect.0 + raw_rect.2).ceil() - rx).max(0.0) as i32;
        let ih = ((raw_rect.1 + raw_rect.3).ceil() - ry).max(0.0) as i32;
        let piw = iw * BUF_SCALE;
        let pih = ih * BUF_SCALE;
        let screen_rect = (rx, ry, iw as f64, ih as f64);
        let phys_rect = (
            rx * BUF_SCALE as f64,
            ry * BUF_SCALE as f64,
            piw as f64,
            pih as f64,
        );

        if !self.gpu_failed && self.gpu.is_none() {
            match GpuIsland::new(self.gpu_display, self.gpu_surface, pw, ph) {
                Ok(g) => self.gpu = Some(g),
                Err(e) => {
                    eprintln!("[yunoshell] GPU renderer unavailable, using SHM: {e}");
                    self.gpu_failed = true;
                }
            }
        }

        // Prepare hardware drag pill overlay before borrowing self.gpu
        let drag_info: Option<(f32, f32, f32, f32)> = if self.rendered_popup == PopupKind::Files
            && self.internal_dragging
            && !self.internal_drag_sources.is_empty()
            && !self.gpu_failed
        {
            let drag_cnt = self.internal_drag_sources.len();
            let drag_lbl = self
                .internal_drag_sources
                .first()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let key = (drag_lbl.clone(), drag_cnt);
            if self.drag_pill_cache.is_none() || self.drag_pill_key != key {
                if let Ok(tmp) = ImageSurface::create(Format::ARgb32, 32, 32) {
                    if let Ok(tcr) = Context::new(&tmp) {
                        let (bw, bh) = crate::files_ui::drag_pill_size(&tcr, &drag_lbl, drag_cnt);
                        let pbw = (bw.ceil() as i32).max(16) * BUF_SCALE;
                        let pbh = (bh.ceil() as i32).max(16) * BUF_SCALE;
                        if let Ok(psurf) = ImageSurface::create(Format::ARgb32, pbw, pbh) {
                            psurf.set_device_scale(BUF_SCALE as f64, BUF_SCALE as f64);
                            if let Ok(pcr) = Context::new(&psurf) {
                                pcr.set_operator(Operator::Source);
                                pcr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                                let _ = pcr.paint();
                                pcr.set_operator(Operator::Over);
                                crate::files_ui::draw_drag_pill_at(
                                    &pcr, 0.0, 0.0, bw.ceil(), bh.ceil(), &drag_lbl, drag_cnt, &self.theme,
                                );
                                psurf.flush();
                            }
                            self.drag_pill_cache = Some(psurf);
                            self.drag_pill_key = key;
                            self.drag_pill_gen = self.drag_pill_gen.wrapping_add(1).max(1);
                        }
                    }
                }
            }
            self.drag_pill_cache.as_ref().map(|psurf| {
                let lw = psurf.width() as f64 / BUF_SCALE as f64;
                let lh = psurf.height() as f64 / BUF_SCALE as f64;
                let sx = ((l.content_x + l.slide_x) * 2.0).round() / 2.0;
                let sy = ((l.content_y + l.slide_y) * 2.0).round() / 2.0;
                let (cw_log, ch_log) = self
                    .content_cache
                    .as_ref()
                    .map(|c| (c.width() as f64 / BUF_SCALE as f64, c.height() as f64 / BUF_SCALE as f64))
                    .unwrap_or((724.0, 420.0));
                let (dx, dy) = self.internal_drag_pos;
                let bx = (dx + 12.0).clamp(4.0, (cw_log - lw - 4.0).max(4.0));
                let by = (dy + 12.0).clamp(4.0, (ch_log - lh - 4.0).max(4.0));
                let s = BUF_SCALE as f32;
                (
                    ((sx + bx) as f32) * s,
                    ((sy + by) as f32) * s,
                    (lw as f32) * s,
                    (lh as f32) * s,
                )
            })
        } else {
            None
        };

        let animating = self.island_animating();
        if let Some(gpu) = self.gpu.as_mut() {
            if iw > 0 && ih > 0 {
                gpu.resize(pw, ph);
                let sx = ((l.content_x + l.slide_x) * 2.0).round() / 2.0;
                let sy = ((l.content_y + l.slide_y) * 2.0).round() / 2.0;
                if let Some(cache) = self.content_cache.as_mut() {
                    gpu.upload_content_if_needed(cache, self.cache_gen);
                }
                if drag_info.is_some() {
                    if let Some(pill) = self.drag_pill_cache.as_mut() {
                        gpu.upload_drag_overlay(pill, self.drag_pill_gen);
                    }
                }
                let (cw_px, ch_px) = self
                    .content_cache
                    .as_ref()
                    .map(|c| (c.width() as f32, c.height() as f32))
                    .unwrap_or((0.0, 0.0));
                let scale = BUF_SCALE as f32;
                let ear = if l.vertical {
                    16.0f64.min(l.body_h * 0.5).max(0.0)
                } else {
                    16.0f64.min(l.body_h * 0.34)
                };
                let radius = if l.vertical {
                    (l.body_w * 0.48).min(l.body_h * 0.62).min(24.0).max(0.5)
                } else {
                    self.cfg.corner_radius.max(20.0)
                };
                let invert = if l.vertical { l.invert_x } else { l.invert_y };
                let (bg_alpha, border_alpha) = if self.closing {
                    let cf = (self.m_height.value() / 0.75).clamp(0.0, 1.0);
                    let cf_eased = (cf * cf * (3.0 - 2.0 * cf)) as f32;
                    (
                        (self.cfg.popup_opacity as f32) * cf_eased,
                        0.85 * cf_eased,
                    )
                } else {
                    (self.cfg.popup_opacity as f32, 0.85)
                };
                let params = IslandGpuParams {
                    bx: (l.body_x as f32) * scale,
                    by: (l.body_y as f32) * scale,
                    bw: (l.body_w as f32) * scale,
                    bh: (l.body_h as f32) * scale,
                    ear: (ear as f32) * scale,
                    radius: (radius as f32) * scale,
                    vertical: l.vertical,
                    invert,
                    cx: (sx as f32) * scale,
                    cy: (sy as f32) * scale,
                    cw: cw_px,
                    ch: ch_px,
                    content_alpha: content_alpha.clamp(0.0, 1.0) as f32,
                    content_scale: self.m_content_scale.value().clamp(0.88, 1.0) as f32,
                    bg_rgba: [
                        self.theme.bg.0 as f32,
                        self.theme.bg.1 as f32,
                        self.theme.bg.2 as f32,
                        bg_alpha,
                    ],
                    border_rgba: [
                        self.theme.border.0 as f32,
                        self.theme.border.1 as f32,
                        self.theme.border.2 as f32,
                        border_alpha,
                    ],
                    drag_rect: drag_info,
                };
                self.island_hits = shifted_hits(&self.content_hits, sx, sy);
                self.prev_island_rect = screen_rect;
                self.cur_body_rect = (l.body_x, l.body_y, l.body_w, l.body_h);
                if animating && !self.island_frame_pending {
                    self.island_frame_pending = true;
                    let surf = self.island_layer.wl_surface();
                    surf.frame(&self.qh, surf.clone());
                }
                gpu.present_island(&params);
                gpu_drawn = true;
            }
        } else if !self.gpu_failed {
            if piw > 0 && pih > 0 {
                let needs_resize = self
                    .gpu_image
                    .as_ref()
                    .map(|img| img.width() < piw || img.height() < pih)
                    .unwrap_or(true);
                if needs_resize {
                    let cur_w = self.gpu_image.as_ref().map(|i| i.width()).unwrap_or(0);
                    let cur_h = self.gpu_image.as_ref().map(|i| i.height()).unwrap_or(0);
                    let aw = piw.max(cur_w).max(1920);
                    let ah = pih.max(cur_h).max(1024);
                    if let Ok(surf) = ImageSurface::create(Format::ARgb32, aw, ah) {
                        surf.set_device_scale(BUF_SCALE as f64, BUF_SCALE as f64);
                        self.gpu_image = Some(surf);
                    }
                }
            }
            let _ = (phys_rect, animating);
        }

        if !gpu_drawn {
        if let Ok((buf, canvas)) =
            self.island_pool.create_buffer(w, h, stride, wl_shm::Format::Argb8888)
        {
            if let Ok(img) = unsafe {
                ImageSurface::create_for_data_unsafe(
                    canvas.as_mut_ptr(),
                    Format::ARgb32,
                    w,
                    h,
                    stride,
                )
            } {
                if let Ok(cr) = Context::new(&img) {
                    cur_rect = draw_island_frame(
                        &cr,
                        &state,
                        &l,
                        self.content_cache.as_ref(),
                        content_alpha,
                    );
                    let sx = l.content_x + l.slide_x;
                    let sy = l.content_y + l.slide_y;
                    self.island_hits = shifted_hits(&self.content_hits, sx, sy);
                    img.flush();
                }
            }

            // Union damage with the previous frame to keep compositor transfer minimal.
            let (dx, dy, dw, dh) = {
                let p = self.prev_island_rect;
                let c = cur_rect;
                if c.2 <= 0.0 || c.3 <= 0.0 {
                    (0, 0, w, h)
                } else if p.2 <= 0.0 || p.3 <= 0.0 {
                    (
                        (c.0.floor() as i32 - 2).clamp(0, w - 1),
                        (c.1.floor() as i32 - 2).clamp(0, h - 1),
                        (c.2.ceil() as i32 + 4).clamp(1, w),
                        (c.3.ceil() as i32 + 4).clamp(1, h),
                    )
                } else {
                    let min_x = p.0.min(c.0);
                    let min_y = p.1.min(c.1);
                    let max_x = (p.0 + p.2).max(c.0 + c.2);
                    let max_y = (p.1 + p.3).max(c.1 + c.3);
                    (
                        (min_x.floor() as i32 - 2).clamp(0, w - 1),
                        (min_y.floor() as i32 - 2).clamp(0, h - 1),
                        ((max_x - min_x).ceil() as i32 + 4).clamp(1, w),
                        ((max_y - min_y).ceil() as i32 + 4).clamp(1, h),
                    )
                }
            };
            self.prev_island_rect = cur_rect;

            self.update_island_input_region();
            let surf = self.island_layer.wl_surface();
            let _ = buf.attach_to(surf);
            surf.damage_buffer(dx, dy, dw, dh);
            if self.island_animating() {
                surf.frame(&self.qh, surf.clone());
            }
            surf.commit();
        }
        }
    }

    fn create_drag_icon_surface(&mut self, label: &str) -> Option<(wl_surface::WlSurface, SlotPool)> {
        let iw: i32 = 210;
        let ih: i32 = 32;
        let stride = iw * 4;
        let mut pool = SlotPool::new((iw * ih * 4) as usize, &self.shm_state).ok()?;
        let (buf, canvas) = pool.create_buffer(iw, ih, stride, wl_shm::Format::Argb8888).ok()?;

        {
            let surf = unsafe {
                ImageSurface::create_for_data_unsafe(canvas.as_mut_ptr(), Format::ARgb32, iw, ih, stride)
            }
            .ok()?;
            let cr = Context::new(&surf).ok()?;
            cr.set_operator(Operator::Source);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            let _ = cr.paint();

            cr.set_operator(Operator::Over);
            rounded_rect(&cr, 1.0, 1.0, (iw - 2) as f64, (ih - 2) as f64, 9.0);
            cr.set_source_rgba(self.theme.surface.0, self.theme.surface.1, self.theme.surface.2, 0.96);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(self.theme.accent.0, self.theme.accent.1, self.theme.accent.2, 0.85);
            cr.set_line_width(1.5);
            let _ = cr.stroke();

            cr.select_font_face("JetBrainsMono Nerd Font", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(11.0);
            cr.set_source_rgb(self.theme.text_bright.0, self.theme.text_bright.1, self.theme.text_bright.2);
            cr.move_to(12.0, 20.0);
            let short = if label.chars().count() > 24 {
                format!("{}…", label.chars().take(23).collect::<String>())
            } else {
                label.to_string()
            };
            let _ = cr.show_text(&format!("󰆏 {short}"));
            surf.flush();
        }

        let wl_surf = self.compositor_state.create_surface(&self.qh);
        let _ = buf.attach_to(&wl_surf);
        wl_surf.damage_buffer(0, 0, iw, ih);
        wl_surf.commit();
        Some((wl_surf, pool))
    }

    fn apply_slider_hit(&mut self, tgt: &HitTarget, px: f64) {
        match tgt {
            HitTarget::ControlsVolSlider(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = (ratio * 150.0).round() as u32;
                if let Ok(mut st) = self.stats.lock() {
                    st.volume_percent = val;
                    st.volume_muted = false;
                }
                set_volume(val as i32);
                self.invalidate_content();
                self.draw_island_surface();
                self.draw_bar_surface();
            }
            HitTarget::VolumeSlider(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = (ratio * 100.0).round() as u32;
                if let Ok(mut st) = self.stats.lock() {
                    st.volume_percent = val;
                    st.volume_muted = false;
                }
                set_volume(val as i32);
                self.invalidate_content();
                self.draw_island_surface();
                self.draw_bar_surface();
            }
            HitTarget::ControlsBrightSlider(sx, sw) | HitTarget::BrightnessSlider(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.05, 1.0);
                let val = (ratio * 100.0).round() as u32;
                if let Ok(mut st) = self.stats.lock() {
                    st.brightness_percent = val;
                }
                set_brightness(val as i32);
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsBarSize(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = (28.0 + ratio * 20.0).round() as u32;
                if self.cfg.bar_size != val {
                    self.cfg.bar_size = val;
                    self.sync_bar_layout();
                    self.save_setting("barSize", serde_json::json!(val));
                    self.invalidate_content();
                    self.draw_bar_surface();
                    self.draw_island_surface();
                }
            }
            HitTarget::SettingsCorner(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = (ratio * 20.0).round();
                if (self.cfg.corner_radius - val).abs() > 0.4 {
                    self.cfg.corner_radius = val;
                    self.save_setting("cornerRadius", serde_json::json!(val));
                    self.invalidate_content();
                    self.draw_bar_surface();
                    self.draw_island_surface();
                }
            }
            HitTarget::SettingsBarOpacity(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = ((35.0 + ratio * 65.0).round() / 100.0).clamp(0.35, 1.0);
                if (self.cfg.bar_opacity - val).abs() > 0.005 {
                    self.cfg.bar_opacity = val;
                    self.save_setting("barOpacity", serde_json::json!(val));
                    self.invalidate_content();
                    self.draw_bar_surface();
                    self.draw_island_surface();
                }
            }
            HitTarget::SettingsPopupOpacity(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = ((55.0 + ratio * 45.0).round() / 100.0).clamp(0.55, 1.0);
                if (self.cfg.popup_opacity - val).abs() > 0.005 {
                    self.cfg.popup_opacity = val;
                    self.save_setting("popupOpacity", serde_json::json!(val));
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::SettingsLockDim(sx, sw) => {
                let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                let val = ((ratio * 85.0).round() / 100.0).clamp(0.0, 0.85);
                if (self.cfg.lock_dim_opacity - val).abs() > 0.005 {
                    self.cfg.lock_dim_opacity = val;
                    self.save_setting("lockDimOpacity", serde_json::json!(val));
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::MediaSeek(sx, sw) => {
                let (bus, length) = self
                    .stats
                    .lock()
                    .map(|s| (s.media.bus.clone(), s.media.length))
                    .unwrap_or_default();
                if length > 0.0 && !bus.is_empty() {
                    let ratio = ((px - sx) / sw).clamp(0.0, 1.0);
                    let secs = ratio * length;
                    if let Ok(mut st) = self.stats.lock() {
                        st.media.position = secs;
                    }
                    media_set_position(&bus, secs);
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            _ => {}
        }
    }

    fn resolve_save_target_path(&self, raw: &str) -> PathBuf {
        let expanded = if raw == "~" {
            "/home/huyle".to_string()
        } else if let Some(rest) = raw.strip_prefix("~/") {
            format!("/home/huyle/{rest}")
        } else {
            raw.to_string()
        };
        let p = PathBuf::from(&expanded);
        if p.is_absolute() {
            p
        } else {
            self.current_dir.join(p)
        }
    }

    fn restore_save_prompt_if_needed(&mut self) {
        if self.portal_req.as_ref().is_some_and(|r| r.mode == "save") {
            self.files_prompt = FilesPrompt::SaveFilename;
            self.files_prompt_text = self.portal_save_name.clone();
        }
    }

    fn complete_portal(&mut self, paths: Vec<PathBuf>) {
        let Some(req) = self.portal_req.take() else { return };
        let uris: Vec<String> = paths.iter().map(|p| encode_uri(p)).collect();
        let str_paths: Vec<String> = paths.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
        let ok = !str_paths.is_empty();
        let payload = serde_json::json!({
            "accepted": ok,
            "status": if ok { "ok" } else { "cancelled" },
            "paths": str_paths,
            "uris": uris,
            "filter_index": 0
        });
        let tmp = format!("{}.tmp", req.res_file);
        let _ = std::fs::write(&tmp, payload.to_string());
        let _ = std::fs::rename(&tmp, &req.res_file);
        self.portal_save_name.clear();
        self.files_prompt = FilesPrompt::None;
        self.files_prompt_text.clear();
        self.set_popup(PopupKind::None);
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn cancel_portal(&mut self) {
        let Some(req) = self.portal_req.take() else { return };
        let payload = serde_json::json!({
            "accepted": false,
            "status": "cancelled",
            "paths": [],
            "uris": []
        });
        let tmp = format!("{}.tmp", req.res_file);
        let _ = std::fs::write(&tmp, payload.to_string());
        let _ = std::fs::rename(&tmp, &req.res_file);
        self.portal_save_name.clear();
        self.files_prompt = FilesPrompt::None;
        self.files_prompt_text.clear();
        self.set_popup(PopupKind::None);
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn activate_file_item(&mut self, idx: usize) {
        let Some(item) = self.entries.get(idx).cloned() else { return };
        if item.is_dir {
            self.navigate_files(item.path, true);
            self.draw_island_surface();
        } else if self.portal_req.is_some() {
            self.complete_portal(vec![item.path]);
        } else {
            let p = item.path.to_string_lossy().to_string();
            self.set_popup(PopupKind::None);
            spawn_detached("xdg-open", &[&p]);
        }
    }

    fn effective_selected_paths(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            return self.selected_paths.clone();
        }
        if let Some(i) = self.selected_idx {
            if let Some(e) = self.entries.get(i) {
                return vec![e.path.clone()];
            }
        }
        Vec::new()
    }

    fn open_files_prompt(&mut self, mode: FilesPrompt, initial: String) {
        self.files_context_menu = None;
        self.filter_focused = false;
        self.files_prompt = mode;
        self.files_prompt_text = initial;
        self.agy_cursor_on = true;
        self.agy_blink_at = Instant::now();
        self.island_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.sync_files_anims();
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn submit_files_prompt(&mut self) {
        let prompt = std::mem::replace(&mut self.files_prompt, FilesPrompt::None);
        let text = std::mem::take(&mut self.files_prompt_text).trim().to_string();
        self.island_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

        match prompt {
            FilesPrompt::PathInput => {
                if !text.is_empty() {
                    let expanded = if text == "~" {
                        "/home/huyle".to_string()
                    } else if let Some(rest) = text.strip_prefix("~/") {
                        format!("/home/huyle/{rest}")
                    } else {
                        text
                    };
                    let p = PathBuf::from(&expanded);
                    if p.is_dir() {
                        self.navigate_files(p, true);
                    } else if let Some(parent) = p.parent() {
                        if p.exists() && parent.is_dir() {
                            self.navigate_files(parent.to_path_buf(), true);
                            if let Some(idx) = self.entries.iter().position(|e| e.path == p) {
                                self.selected_idx = Some(idx);
                                self.selected_paths = vec![p];
                                self.update_files_preview();
                            }
                        } else {
                            self.set_toast("Path not found");
                        }
                    } else {
                        self.set_toast("Path not found");
                    }
                }
            }
            FilesPrompt::Mkdir => {
                if !text.is_empty() && !text.contains('/') {
                    let target = self.current_dir.join(&text);
                    match std::fs::create_dir_all(&target) {
                        Ok(_) => {
                            self.set_toast(&format!("Created folder {text}"));
                            self.refresh_files();
                            if let Some(idx) = self.entries.iter().position(|e| e.path == target) {
                                self.selected_idx = Some(idx);
                                self.selected_paths = vec![target];
                                self.update_files_preview();
                            }
                        }
                        Err(e) => self.set_toast(&format!("Error: {e}")),
                    }
                }
            }
            FilesPrompt::Mkfile => {
                if !text.is_empty() && !text.contains('/') {
                    let target = self.current_dir.join(&text);
                    if target.exists() {
                        self.set_toast("File already exists");
                    } else {
                        match std::fs::File::create(&target) {
                            Ok(_) => {
                                self.set_toast(&format!("Created {text}"));
                                self.refresh_files();
                                if let Some(idx) = self.entries.iter().position(|e| e.path == target) {
                                    self.selected_idx = Some(idx);
                                    self.selected_paths = vec![target];
                                    self.update_files_preview();
                                }
                            }
                            Err(e) => self.set_toast(&format!("Error: {e}")),
                        }
                    }
                }
            }
            FilesPrompt::SaveFilename => {
                if !text.is_empty() {
                    let target = self.resolve_save_target_path(&text);
                    self.complete_portal(vec![target]);
                    return;
                } else {
                    self.files_prompt = FilesPrompt::SaveFilename;
                    self.set_toast("Enter a filename to save");
                }
            }
            FilesPrompt::Rename(old_path) => {
                if !text.is_empty() && !text.contains('/') {
                    if let Some(parent) = old_path.parent() {
                        let new_path = parent.join(&text);
                        if new_path != old_path {
                            match std::fs::rename(&old_path, &new_path) {
                                Ok(_) => {
                                    self.set_toast(&format!("Renamed to {text}"));
                                    self.files_preview_path = PathBuf::new();
                                    self.refresh_files();
                                    if let Some(idx) = self.entries.iter().position(|e| e.path == new_path) {
                                        self.selected_idx = Some(idx);
                                        self.selected_paths = vec![new_path];
                                        self.update_files_preview();
                                    }
                                }
                                Err(e) => self.set_toast(&format!("Rename failed: {e}")),
                            }
                        }
                    }
                }
            }
            FilesPrompt::ConfirmDelete(paths) => match delete_permanently(&paths) {
                Ok(n) => {
                    self.selected_idx = None;
                    self.selected_paths.clear();
                    self.files_preview_path = PathBuf::new();
                    self.set_toast(&format!("Deleted {n} item(s)"));
                    self.refresh_files();
                }
                Err(e) => self.set_toast(&format!("Delete failed: {e}")),
            },
            FilesPrompt::ConfirmEmptyTrash => match empty_trash() {
                Ok(n) => {
                    self.selected_idx = None;
                    self.selected_paths.clear();
                    self.files_preview_path = PathBuf::new();
                    self.set_toast(&format!("Emptied {n} item(s)"));
                    self.refresh_files();
                }
                Err(e) => self.set_toast(&format!("Empty trash failed: {e}")),
            },
            FilesPrompt::None => {}
        }
        self.restore_save_prompt_if_needed();
        self.sync_files_anims();
        self.invalidate_content();
        self.draw_island_surface();
    }

    fn execute_files_action(&mut self, act: &str) {
        self.files_context_menu = None;
        let sel = self.selected_idx.and_then(|i| self.entries.get(i)).cloned();
        let paths = self.effective_selected_paths();
        let target = sel
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| self.current_dir.clone());
        let target_s = target.to_string_lossy().to_string();

        match act {
            "open" => {
                if let Some(idx) = self.selected_idx {
                    self.activate_file_item(idx);
                }
            }
            "codium" => {
                self.set_popup(PopupKind::None);
                spawn_detached("codium", &[&target_s]);
            }
            "term" => {
                let tdir = sel
                    .as_ref()
                    .filter(|s| s.is_dir)
                    .map(|s| s.path.to_string_lossy().to_string())
                    .unwrap_or_else(|| self.current_dir.to_string_lossy().to_string());
                self.set_popup(PopupKind::None);
                spawn_detached("kitty", &["--directory", &tdir]);
            }
            "copy" => {
                spawn_detached("wl-copy", &[&target_s]);
                self.set_toast("Copied path");
                self.draw_island_surface();
            }
            "copy_files" => {
                if !paths.is_empty() {
                    wl_copy_uri_list(&paths);
                    let n = paths.len();
                    self.files_clipboard = paths;
                    self.files_clip_mode = ClipMode::Copy;
                    self.set_toast(&format!("Copied {n} item(s)"));
                    self.draw_island_surface();
                }
            }
            "cut_files" => {
                if !paths.is_empty() {
                    wl_copy_uri_list(&paths);
                    let n = paths.len();
                    self.files_clipboard = paths;
                    self.files_clip_mode = ClipMode::Cut;
                    self.set_toast(&format!("Cut {n} item(s)"));
                    self.draw_island_surface();
                }
            }
            "paste_files" => {
                let (sources, move_mode) = if !self.files_clipboard.is_empty() {
                    (
                        self.files_clipboard.clone(),
                        self.files_clip_mode == ClipMode::Cut,
                    )
                } else {
                    (wl_paste_uri_list(), false)
                };
                if sources.is_empty() {
                    self.set_toast("Clipboard is empty");
                } else {
                    let cur = self.current_dir.clone();
                    match transfer_files(&sources, &cur, move_mode) {
                        Ok(n) => {
                            if move_mode {
                                self.files_clipboard.clear();
                            }
                            self.files_preview_path = PathBuf::new();
                            self.refresh_files();
                            self.set_toast(&format!(
                                "{} {n} item(s)",
                                if move_mode { "Moved" } else { "Pasted" }
                            ));
                        }
                        Err(e) => self.set_toast(&format!("Paste failed: {e}")),
                    }
                }
                self.draw_island_surface();
            }
            "duplicate" => {
                if !paths.is_empty() {
                    match duplicate_files(&paths) {
                        Ok(n) => {
                            self.files_preview_path = PathBuf::new();
                            self.refresh_files();
                            self.set_toast(&format!("Duplicated {n} item(s)"));
                        }
                        Err(e) => self.set_toast(&format!("Duplicate failed: {e}")),
                    }
                    self.draw_island_surface();
                }
            }
            "rename" => {
                if let Some(it) = sel {
                    self.open_files_prompt(FilesPrompt::Rename(it.path), it.name);
                }
            }
            "mkdir" => {
                self.open_files_prompt(FilesPrompt::Mkdir, String::new());
            }
            "mkfile" => {
                self.open_files_prompt(FilesPrompt::Mkfile, String::new());
            }
            "trash" => {
                if !paths.is_empty() {
                    let n = paths.len();
                    match run_trash(&paths) {
                        Ok(_) => {
                            self.selected_idx = None;
                            self.selected_paths.clear();
                            self.files_preview_path = PathBuf::new();
                            self.refresh_files();
                            self.set_toast(&format!("Moved {n} item(s) to Trash"));
                        }
                        Err(e) => self.set_toast(&format!("Trash error: {e}")),
                    }
                    self.draw_island_surface();
                }
            }
            "delete_perm" => {
                if !paths.is_empty() {
                    self.open_files_prompt(FilesPrompt::ConfirmDelete(paths), String::new());
                }
            }
            "extract" => {
                if let Some(it) = sel {
                    let cur = self.current_dir.clone();
                    match extract_archive(&it.path, &cur) {
                        Ok(out) => {
                            self.files_preview_path = PathBuf::new();
                            self.refresh_files();
                            self.set_toast(&format!("Extracted to {out}"));
                        }
                        Err(e) => self.set_toast(&e),
                    }
                    self.draw_island_surface();
                }
            }
            "compress" => {
                if !paths.is_empty() {
                    let cur = self.current_dir.clone();
                    match compress_to_zip(&paths, &cur) {
                        Ok(zip_name) => {
                            self.files_preview_path = PathBuf::new();
                            self.refresh_files();
                            self.set_toast(&format!("Created {zip_name}"));
                        }
                        Err(e) => self.set_toast(&e),
                    }
                    self.draw_island_surface();
                }
            }
            "wallpaper" => {
                if let Some(it) = sel {
                    if !it.is_dir && is_image_file(&it.name) {
                        let p = it.path.to_string_lossy().to_string();
                        let mode = self.cfg.wallpaper_mode.clone();
                        self.start_wallpaper_transition(p, mode);
                        self.set_toast("Desktop wallpaper updated");
                        self.draw_island_surface();
                    }
                }
            }
            "lock_wallpaper" => {
                if let Some(it) = sel {
                    if !it.is_dir && is_image_file(&it.name) {
                        let p = it.path.to_string_lossy().to_string();
                        self.cfg.lock_wallpaper_path = p.clone();
                        self.cfg.lock_use_same_wallpaper = false;
                        set_lock_wallpaper(&p);
                        self.set_toast("Lock wallpaper updated");
                        self.draw_island_surface();
                    }
                }
            }
            "pin_selected" => {
                if let Some(it) = sel.filter(|x| x.is_dir) {
                    let dir_s = it.path.to_string_lossy().to_string();
                    if !self.cfg.bookmarks.iter().any(|b| b.dir == dir_s) {
                        self.cfg.bookmarks.push(Bookmark {
                            label: it.name,
                            dir: dir_s,
                            icon: "󰉋".into(),
                        });
                        save_bookmarks(&self.cfg.bookmarks);
                        self.set_toast("Pinned folder");
                        self.draw_island_surface();
                    }
                }
            }
            "toggle_pin" => {
                let pin_path = sel
                    .as_ref()
                    .filter(|x| x.is_dir)
                    .map(|x| x.path.clone())
                    .unwrap_or_else(|| self.current_dir.clone());
                let dir_s = pin_path.to_string_lossy().to_string();
                let is_def = default_places().iter().any(|p| p.dir == dir_s);
                if is_def {
                    self.set_toast("Built-in Place cannot be unpinned");
                } else if let Some(pos) = self.cfg.bookmarks.iter().position(|b| b.dir == dir_s) {
                    self.cfg.bookmarks.remove(pos);
                    save_bookmarks(&self.cfg.bookmarks);
                    self.set_toast("Removed from Quick Access");
                } else {
                    let label = pin_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Folder")
                        .to_string();
                    self.cfg.bookmarks.push(Bookmark {
                        label,
                        dir: dir_s,
                        icon: "󰉋".into(),
                    });
                    save_bookmarks(&self.cfg.bookmarks);
                    self.set_toast("Added to Quick Access");
                }
                self.sync_files_anims();
                self.invalidate_content();
                self.draw_island_surface();
            }
            "refresh" => {
                self.files_preview_path = PathBuf::new();
                self.refresh_files();
                self.draw_island_surface();
            }
            _ => {}
        }
    }

    fn handle_click(&mut self, tgt: HitTarget, button: u32, serial: u32, px: f64, py: f64) {
        match tgt {
            HitTarget::BarPopup(kind) => {
                if button == BTN_RIGHT {
                    match kind {
                        PopupKind::Files => {
                            self.refresh_files();
                            self.set_popup(PopupKind::Files);
                        }
                        PopupKind::Controls | PopupKind::Volume => toggle_mute(),
                        PopupKind::Bluetooth => {
                            toggle_bluetooth();
                            self.set_toast("Bluetooth toggled");
                        }
                        PopupKind::Wifi => {
                            toggle_wifi();
                            self.set_toast("Wi-Fi toggled");
                        }
                        PopupKind::Media => media_toggle_play(),
                        _ => {}
                    }
                    if matches!(kind, PopupKind::Bluetooth | PopupKind::Wifi | PopupKind::Volume) {
                        refresh_stats_now(&self.stats);
                        self.draw_bar_surface();
                    }
                } else if button == BTN_LEFT {
                    if let Some(sec) = Self::cc_section_for_popup(kind) {
                        if self.active_popup != PopupKind::Controls {
                            self.cc_expanded = sec;
                            self.opened_by_hover = false;
                            self.set_popup(PopupKind::Controls);
                        } else if self.opened_by_hover {
                            self.opened_by_hover = false;
                            if !sec.is_empty() && self.cc_expanded != sec {
                                self.set_cc_expanded(sec);
                            }
                        } else if sec.is_empty() || self.cc_expanded == sec {
                            self.set_popup(PopupKind::None);
                        } else {
                            self.set_cc_expanded(sec);
                        }
                    } else {
                        self.toggle_popup(kind);
                    }
                }
            }
            HitTarget::CcExpand(sec) => {
                self.opened_by_hover = false;
                self.set_cc_expanded(sec);
            }
            HitTarget::BarWorkspace(idx) if button == BTN_LEFT => switch_tag(idx),
            HitTarget::BarTray(idx) => {
                let item_opt = self.stats.lock().ok().and_then(|s| s.tray_items.get(idx).cloned());
                if let Some(item) = item_opt {
                    let key = item.key.clone();
                    if button == BTN_MIDDLE {
                        tray_send_cmd(&serde_json::json!({
                            "cmd": "secondary",
                            "key": key,
                            "x": px.round() as i32,
                            "y": py.round() as i32,
                        }));
                    } else if button == BTN_RIGHT || (button == BTN_LEFT && item.item_is_menu) {
                        if !item.menu_path.is_empty() {
                            let same = self.active_popup == PopupKind::TrayMenu
                                && self.stats.lock().map(|s| s.tray_menu_key == key).unwrap_or(false);
                            if same {
                                self.set_popup(PopupKind::None);
                            } else {
                                if let Ok(mut s) = self.stats.lock() {
                                    s.tray_menu_key = key.clone();
                                    s.tray_menu_title = if item.title.is_empty() { item.id.clone() } else { item.title.clone() };
                                    s.tray_menu_items.clear();
                                }
                                tray_send_cmd(&serde_json::json!({
                                    "cmd": "menu_open",
                                    "key": key,
                                    "x": px.round() as i32,
                                    "y": py.round() as i32,
                                }));
                                self.set_popup(PopupKind::TrayMenu);
                            }
                        } else {
                            tray_send_cmd(&serde_json::json!({
                                "cmd": "menu_open",
                                "key": key,
                                "x": px.round() as i32,
                                "y": py.round() as i32,
                            }));
                        }
                    } else if button == BTN_LEFT {
                        tray_send_cmd(&serde_json::json!({
                            "cmd": "activate",
                            "key": key,
                            "x": px.round() as i32,
                            "y": py.round() as i32,
                        }));
                    }
                }
            }
            HitTarget::TrayMenuClick(mid) => {
                let key = self.stats.lock().map(|s| s.tray_menu_key.clone()).unwrap_or_default();
                if !key.is_empty() {
                    tray_send_cmd(&serde_json::json!({
                        "cmd": "menu_click",
                        "key": key,
                        "id": mid,
                    }));
                }
                self.set_popup(PopupKind::None);
            }
            HitTarget::IslandClose => self.set_popup(PopupKind::None),
            HitTarget::FilesNavBack => {
                if let Some(prev) = self.back_stack.pop() {
                    self.fwd_stack.push(self.current_dir.clone());
                    self.navigate_files(prev, false);
                    self.draw_island_surface();
                }
            }
            HitTarget::FilesNavFwd => {
                if let Some(next) = self.fwd_stack.pop() {
                    self.back_stack.push(self.current_dir.clone());
                    self.navigate_files(next, false);
                    self.draw_island_surface();
                }
            }
            HitTarget::FilesNavUp => {
                if let Some(parent) = self.current_dir.parent().map(|p| p.to_path_buf()) {
                    self.navigate_files(parent, true);
                    self.draw_island_surface();
                }
            }
            HitTarget::FilesRefresh => {
                self.refresh_files();
                self.draw_island_surface();
            }
            HitTarget::FilesNavHome => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/home/huyle".into());
                self.navigate_files(PathBuf::from(home), true);
                self.draw_island_surface();
            }
            HitTarget::FilesViewModeToggle => {
                self.files_view_mode = match self.files_view_mode {
                    FileViewMode::Grid => FileViewMode::List,
                    FileViewMode::List => FileViewMode::Grid,
                };
                self.scroll_y = 0.0;
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::FilesBreadcrumb(p) | HitTarget::FilesPlace(p) | HitTarget::FilesDrive(p) => {
                self.navigate_files(p, true);
                self.draw_island_surface();
            }
            HitTarget::FilesFocusFilter => {
                self.files_context_menu = None;
                self.files_prompt = FilesPrompt::None;
                self.filter_focused = true;
                self.agy_cursor_on = true;
                self.agy_blink_at = Instant::now();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.sync_files_anims();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::FilesEditPath => {
                let cur = self.current_dir.to_string_lossy().to_string();
                self.open_files_prompt(FilesPrompt::PathInput, cur);
            }
            HitTarget::FilesSortCycle => {
                self.files_context_menu = None;
                let (next_by, next_asc) = match (self.files_sort_by, self.files_sort_asc) {
                    (SortBy::Name, true) => (SortBy::Name, false),
                    (SortBy::Name, false) => (SortBy::Size, false),
                    (SortBy::Size, false) => (SortBy::Size, true),
                    (SortBy::Size, true) => (SortBy::Modified, false),
                    (SortBy::Modified, false) => (SortBy::Modified, true),
                    (SortBy::Modified, true) => (SortBy::Name, true),
                };
                self.files_sort_by = next_by;
                self.files_sort_asc = next_asc;
                self.refresh_files();
                self.draw_island_surface();
            }
            HitTarget::FilesSortColumn(col) => {
                self.files_context_menu = None;
                if self.files_sort_by == col {
                    self.files_sort_asc = !self.files_sort_asc;
                } else {
                    self.files_sort_by = col;
                    self.files_sort_asc = col == SortBy::Name;
                }
                self.refresh_files();
                self.draw_island_surface();
            }
            HitTarget::FilesNewFolder => {
                if matches!(self.files_prompt, FilesPrompt::Mkdir) {
                    self.files_prompt = FilesPrompt::None;
                    self.files_prompt_text.clear();
                    self.sync_files_anims();
                    self.invalidate_content();
                    self.draw_island_surface();
                } else {
                    self.open_files_prompt(FilesPrompt::Mkdir, String::new());
                }
            }
            HitTarget::FilesNewFile => {
                if matches!(self.files_prompt, FilesPrompt::Mkfile) {
                    self.files_prompt = FilesPrompt::None;
                    self.files_prompt_text.clear();
                    self.sync_files_anims();
                    self.invalidate_content();
                    self.draw_island_surface();
                } else {
                    self.open_files_prompt(FilesPrompt::Mkfile, String::new());
                }
            }
            HitTarget::FilesPromptInput => {
                self.filter_focused = false;
                self.agy_cursor_on = true;
                self.agy_blink_at = Instant::now();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::FilesPromptConfirm => {
                self.submit_files_prompt();
            }
            HitTarget::FilesPromptCancel => {
                if matches!(self.files_prompt, FilesPrompt::SaveFilename)
                    && self.portal_req.as_ref().is_some_and(|r| r.mode == "save")
                {
                    self.files_prompt_text.clear();
                    self.portal_save_name.clear();
                } else {
                    self.files_prompt = FilesPrompt::None;
                    self.files_prompt_text.clear();
                    self.restore_save_prompt_if_needed();
                }
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.sync_files_anims();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::FilesEmptyTrash => {
                self.open_files_prompt(FilesPrompt::ConfirmEmptyTrash, String::new());
            }
            HitTarget::FilesContextMenuAction(act) => {
                self.execute_files_action(act);
            }
            HitTarget::FilesListArea => {
                self.filter_focused = false;
                self.sync_files_anims();
                if button == BTN_RIGHT {
                    if let Some(l) = self.island_layout() {
                        let lx = (px - (l.content_x + l.slide_x)).max(0.0);
                        let ly = (py - (l.content_y + l.slide_y)).max(0.0);
                        self.files_context_menu = Some((lx, ly, None));
                        self.files_ctx_anim = Motion::new(0.0, 1.0, 120.0, Ease::OutCubic);
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                } else if button == BTN_LEFT {
                    if self.files_context_menu.is_some() {
                        self.files_context_menu = None;
                    }
                    if !self.kbd_ctrl && !self.kbd_shift {
                        self.selected_paths.clear();
                        self.selected_idx = None;
                        self.update_files_preview();
                    }
                    self.button_down = true;
                    self.press_pos = (px, py);
                    self.pressed_file_idx = None;
                    self.marquee_start = Some((px, py));
                    self.marquee_rect = None;
                    self.internal_dragging = false;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::FilesToggleHidden => {
                self.files_context_menu = None;
                self.show_hidden = !self.show_hidden;
                self.refresh_files();
                self.draw_island_surface();
            }
            HitTarget::FilesTogglePin => {
                self.files_context_menu = None;
                let dir_s = self.current_dir.to_string_lossy().to_string();
                let is_def = default_places().iter().any(|p| p.dir == dir_s);
                if !is_def {
                    if let Some(pos) = self.cfg.bookmarks.iter().position(|b| b.dir == dir_s) {
                        self.cfg.bookmarks.remove(pos);
                        self.set_toast("Unpinned folder");
                    } else {
                        let label = self.current_dir.file_name().and_then(|s| s.to_str()).unwrap_or("Folder").to_string();
                        self.cfg.bookmarks.push(Bookmark { label, dir: dir_s, icon: "󰉋".into() });
                        self.set_toast("Pinned to Places");
                    }
                    save_bookmarks(&self.cfg.bookmarks);
                    self.sync_files_anims();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::FilesUnpin(dir_s) => {
                self.files_context_menu = None;
                self.cfg.bookmarks.retain(|b| b.dir != dir_s);
                save_bookmarks(&self.cfg.bookmarks);
                self.set_toast("Removed from Quick Access");
                self.sync_files_anims();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::FilesQuickAccessDrop => {}
            HitTarget::FilesPopOutFilesR => {
                self.set_popup(PopupKind::None);
            }
            HitTarget::FilesPortalAccept => {
                if self.portal_req.as_ref().is_some_and(|r| r.mode == "save") {
                    let typed = if matches!(self.files_prompt, FilesPrompt::SaveFilename) {
                        self.files_prompt_text.trim().to_string()
                    } else {
                        self.portal_save_name.trim().to_string()
                    };
                    if !typed.is_empty() {
                        let target = self.resolve_save_target_path(&typed);
                        self.complete_portal(vec![target]);
                    } else {
                        let file_sel: Vec<PathBuf> = self
                            .effective_selected_paths()
                            .into_iter()
                            .filter(|p| !p.is_dir())
                            .collect();
                        if !file_sel.is_empty() {
                            self.complete_portal(file_sel);
                        } else {
                            self.files_prompt = FilesPrompt::SaveFilename;
                            self.agy_cursor_on = true;
                            self.set_toast("Enter a filename to save");
                            self.invalidate_content();
                            self.draw_island_surface();
                        }
                    }
                } else {
                    let paths = self.effective_selected_paths();
                    if !paths.is_empty() {
                        self.complete_portal(paths);
                    } else if self
                        .portal_req
                        .as_ref()
                        .map(|r| r.directory || r.mode == "directory" || r.mode == "save_files")
                        .unwrap_or(false)
                    {
                        let cur = self.current_dir.clone();
                        self.complete_portal(vec![cur]);
                    } else {
                        self.set_toast("Select a file first");
                    }
                }
            }
            HitTarget::FilesPortalCancel => {
                self.cancel_portal();
            }
            HitTarget::FilesRow(idx) => {
                self.filter_focused = false;
                self.sync_files_anims();
                if button == BTN_RIGHT {
                    if let Some(item) = self.entries.get(idx).cloned() {
                        if !self.selected_paths.contains(&item.path) {
                            self.selected_idx = Some(idx);
                            self.selected_paths = vec![item.path];
                            self.update_files_preview();
                        }
                    }
                    if let Some(l) = self.island_layout() {
                        let lx = (px - (l.content_x + l.slide_x)).max(0.0);
                        let ly = (py - (l.content_y + l.slide_y)).max(0.0);
                        self.files_context_menu = Some((lx, ly, Some(idx)));
                        self.files_ctx_anim = Motion::new(0.0, 1.0, 120.0, Ease::OutCubic);
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                } else if button == BTN_LEFT {
                    self.files_context_menu = None;
                    let now = Instant::now();
                    let dbl = self.last_click_idx == Some(idx)
                        && now.duration_since(self.last_click_time) < Duration::from_millis(360)
                        && !self.kbd_ctrl
                        && !self.kbd_shift;
                    self.last_click_idx = Some(idx);
                    self.last_click_time = now;
                    if dbl {
                        self.activate_file_item(idx);
                        self.button_down = false;
                    } else {
                        if self.kbd_shift {
                            let anchor = self.selected_idx.unwrap_or(idx);
                            let (lo, hi) = if anchor <= idx { (anchor, idx) } else { (idx, anchor) };
                            self.selected_paths = self
                                .entries
                                .iter()
                                .skip(lo)
                                .take(hi - lo + 1)
                                .map(|e| e.path.clone())
                                .collect();
                            self.selected_idx = Some(idx);
                        } else if self.kbd_ctrl {
                            if let Some(item) = self.entries.get(idx) {
                                if let Some(pos) = self.selected_paths.iter().position(|p| p == &item.path) {
                                    self.selected_paths.remove(pos);
                                } else {
                                    if self.selected_paths.is_empty() {
                                        if let Some(prev_i) = self.selected_idx {
                                            if let Some(prev_e) = self.entries.get(prev_i) {
                                                if prev_i != idx {
                                                    self.selected_paths.push(prev_e.path.clone());
                                                }
                                            }
                                        }
                                    }
                                    self.selected_paths.push(item.path.clone());
                                }
                                self.selected_idx = Some(idx);
                            }
                        } else {
                            self.selected_idx = Some(idx);
                            if let Some(item) = self.entries.get(idx).cloned() {
                                self.selected_paths = vec![item.path.clone()];
                                if !item.is_dir
                                    && self.portal_req.as_ref().is_some_and(|r| r.mode == "save")
                                {
                                    self.files_prompt = FilesPrompt::SaveFilename;
                                    self.files_prompt_text = item.name.clone();
                                    self.portal_save_name = item.name;
                                }
                            }
                        }
                        self.update_files_preview();
                        self.button_down = true;
                        self.drag_launched = false;
                        self.press_pos = (px, py);
                        self.press_serial = serial;
                        self.pressed_file_idx = Some(idx);
                        self.marquee_start = None;
                        self.marquee_rect = None;
                        self.internal_dragging = false;
                        self.internal_drag_sources = self.effective_selected_paths();
                        self.internal_drag_target = None;
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                }
            }
            HitTarget::FilesAction(act) => {
                self.execute_files_action(act);
            }
            HitTarget::LauncherTile(idx) => {
                if let Some(app) = self.filtered_apps.get(idx).cloned() {
                    self.set_popup(PopupKind::None);
                    launch_app(&app);
                }
            }
            HitTarget::ControlsMuteToggle => {
                toggle_mute();
                if let Ok(mut st) = self.stats.lock() {
                    st.volume_muted = !st.volume_muted;
                }
                self.invalidate_content();
                self.draw_island_surface();
                self.draw_bar_surface();
            }
            HitTarget::ControlsVolSlider(_, _)
            | HitTarget::ControlsBrightSlider(_, _)
            | HitTarget::VolumeSlider(_, _)
            | HitTarget::BrightnessSlider(_, _)
            | HitTarget::MediaSeek(_, _)
            | HitTarget::SettingsBarSize(_, _)
            | HitTarget::SettingsCorner(_, _)
            | HitTarget::SettingsBarOpacity(_, _)
            | HitTarget::SettingsPopupOpacity(_, _)
            | HitTarget::SettingsLockDim(_, _) => {
                self.slider_drag = Some(tgt.clone());
                self.apply_slider_hit(&tgt, px);
            }
            HitTarget::SettingsTab(i) => {
                self.switch_settings_tab(i);
                self.draw_island_surface();
            }
            HitTarget::SettingsBgTarget(sub) => {
                self.settings_bg_target = sub.to_string();
                self.settings_open_dropdown.clear();
                if sub == "mango" {
                    self.mango_info = read_mango_summary();
                } else if sub != "theme" {
                    self.refresh_wallpapers_if_needed();
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsThemeDropdown => {
                self.settings_open_dropdown = if self.settings_open_dropdown == "themePreset" {
                    String::new()
                } else {
                    "themePreset".to_string()
                };
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsThemePick(name) => {
                self.cfg.theme_name = name.to_string();
                self.cfg.accent_preset = "theme".to_string();
                self.settings_open_dropdown.clear();
                self.save_setting("themeName", serde_json::json!(name));
                self.save_setting("accentPreset", serde_json::json!("theme"));
                self.reload_theme_and_apply();
            }
            HitTarget::SettingsAccent(id) => {
                self.cfg.accent_preset = id.to_string();
                self.settings_open_dropdown.clear();
                self.save_setting("accentPreset", serde_json::json!(id));
                self.reload_theme_and_apply();
            }
            HitTarget::SettingsBarPosDropdown => {
                self.settings_open_dropdown = if self.settings_open_dropdown == "barPos" {
                    String::new()
                } else {
                    "barPos".to_string()
                };
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsBarPosPick(pos) | HitTarget::SettingsBarPos(pos) => {
                self.cfg.bar_position = pos.to_string();
                self.settings_open_dropdown.clear();
                self.sync_bar_layout();
                self.save_setting("barPosition", serde_json::json!(pos));
                self.invalidate_content();
                self.draw_bar_surface();
                self.draw_island_surface();
            }
            HitTarget::SettingsWallpaperMode(mode) => {
                self.cfg.wallpaper_mode = mode.to_string();
                let wp = self.cfg.wallpaper_path.clone();
                apply_wallpaper(&wp, mode);
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsWallpaperDir(dir) => {
                self.cfg.wallpaper_dir = dir.to_string();
                self.save_setting("wallpaperDir", serde_json::json!(dir));
                self.refresh_wallpapers_if_needed();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsWallpaperPick(idx) => {
                if let Some((_, p, _)) = self.wallpapers.get(idx).cloned() {
                    let ps = p.to_string_lossy().to_string();
                    match self.settings_bg_target.as_str() {
                        "lockscreen" => {
                            self.cfg.lock_use_same_wallpaper = false;
                            self.cfg.lock_wallpaper_path = ps.clone();
                            self.save_setting("lockUseSameWallpaper", serde_json::json!(false));
                            self.save_setting("lockWallpaperPath", serde_json::json!(ps));
                        }
                        "sddm" => {
                            self.cfg.sddm_selected_path = ps;
                        }
                        _ => {
                            let mode = self.cfg.wallpaper_mode.clone();
                            self.start_wallpaper_transition(ps, mode);
                        }
                    }
                    self.refresh_wallpapers_if_needed();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::SettingsRandomWallpaper => {
                if !self.wallpapers.is_empty() {
                    let idx = (self.started.elapsed().subsec_nanos() as usize) % self.wallpapers.len();
                    let ps = self.wallpapers[idx].1.to_string_lossy().to_string();
                    let mode = self.cfg.wallpaper_mode.clone();
                    self.start_wallpaper_transition(ps, mode);
                    self.refresh_wallpapers_if_needed();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::SettingsToggleLockSame => {
                self.cfg.lock_use_same_wallpaper = !self.cfg.lock_use_same_wallpaper;
                if self.cfg.lock_use_same_wallpaper {
                    self.cfg.lock_wallpaper_path = self.cfg.wallpaper_path.clone();
                    self.save_setting("lockWallpaperPath", serde_json::json!(self.cfg.lock_wallpaper_path));
                }
                self.save_setting("lockUseSameWallpaper", serde_json::json!(self.cfg.lock_use_same_wallpaper));
                self.refresh_wallpapers_if_needed();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsToggleLockClock => {
                self.cfg.lock_show_clock = !self.cfg.lock_show_clock;
                self.save_setting("lockShowClock", serde_json::json!(self.cfg.lock_show_clock));
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsLockNow => {
                self.set_popup(PopupKind::None);
                power_action("lock");
            }
            HitTarget::SettingsBrowseWallpaper
            | HitTarget::SettingsBrowseLockWallpaper
            | HitTarget::SettingsBrowseSddmWallpaper => {
                let dir = self.cfg.wallpaper_dir.clone();
                self.navigate_files(PathBuf::from(&dir), true);
                self.set_popup(PopupKind::Files);
            }
            HitTarget::SettingsSyncSddm => {
                let target = if self.settings_bg_target == "sddm" && !self.cfg.sddm_selected_path.is_empty() {
                    self.cfg.sddm_selected_path.clone()
                } else {
                    self.cfg.wallpaper_path.clone()
                };
                let script = "/home/huyle/.config/quickshell/scripts/set-sddm-wallpaper.sh";
                if Path::new(script).is_file() {
                    spawn_detached("bash", &[script, &target]);
                    self.cfg.sddm_status = "Applying SDDM wallpaper...".into();
                    self.cfg.sddm_status_error = false;
                } else {
                    self.cfg.sddm_status = "Saved selection".into();
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsMangoToggle(key) => {
                let next_val = match key {
                    "blur" => {
                        self.mango_info.blur = !self.mango_info.blur;
                        self.mango_info.blur
                    }
                    "shadows" => {
                        self.mango_info.shadows = !self.mango_info.shadows;
                        self.mango_info.shadows
                    }
                    _ => {
                        self.mango_info.animations = !self.mango_info.animations;
                        self.mango_info.animations
                    }
                };
                set_mango_key(key, if next_val { "1" } else { "0" });
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsMangoReload => {
                spawn_detached("mmsg", &["-d", "reload_config"]);
                self.mango_info = read_mango_summary();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsCalPrev => {
                self.cal_offset_months -= 1;
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsCalNext => {
                self.cal_offset_months += 1;
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsCalToday => {
                self.cal_offset_months = 0;
                self.cal_selected_key = -1;
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsCalDay(key) => {
                self.cal_selected_key = if self.cal_selected_key == key { -1 } else { key };
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsToggle24h => {
                self.cfg.lock_24_hour = !self.cfg.lock_24_hour;
                self.save_setting("lock24Hour", serde_json::json!(self.cfg.lock_24_hour));
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsToggleFloating => {
                self.cfg.bar_floating = !self.cfg.bar_floating;
                self.sync_bar_layout();
                self.save_setting("barFloating", serde_json::json!(self.cfg.bar_floating));
                self.invalidate_content();
                self.draw_bar_surface();
            }
            HitTarget::SettingsToggleRam => {
                self.cfg.show_ram_on_bar = !self.cfg.show_ram_on_bar;
                self.save_setting("showRamOnBar", serde_json::json!(self.cfg.show_ram_on_bar));
                self.invalidate_content();
                self.draw_bar_surface();
            }
            HitTarget::SettingsToggleMedia => {
                self.cfg.show_media_on_bar = !self.cfg.show_media_on_bar;
                self.save_setting("showMediaOnBar", serde_json::json!(self.cfg.show_media_on_bar));
                self.invalidate_content();
                self.draw_bar_surface();
            }
            HitTarget::SettingsToggleTray => {
                self.cfg.show_tray_on_bar = !self.cfg.show_tray_on_bar;
                self.save_setting("showTrayOnBar", serde_json::json!(self.cfg.show_tray_on_bar));
                self.invalidate_content();
                self.draw_bar_surface();
            }
            HitTarget::SettingsTheme(i) => {
                let names = [
                    "catppuccin",
                    "nord",
                    "gruvbox",
                    "tokyonight",
                    "dracula",
                    "rosepine",
                    "everforest",
                ];
                if let Some(name) = names.get(i) {
                    self.cfg.theme_name = (*name).to_string();
                    self.save_setting("themeName", serde_json::json!(name));
                    self.reload_theme_and_apply();
                }
            }
            HitTarget::SettingsRefresh => {
                self.set_toast("Refreshing system data...");
                let stats = self.stats.clone();
                std::thread::spawn(move || refresh_stats_now(&stats));
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsOpenPower => {
                self.set_popup(PopupKind::Power);
            }
            HitTarget::SettingsOpenLotus => {
                self.set_popup(PopupKind::None);
                spawn_detached("fcitx5-configtool", &[]);
            }
            HitTarget::SettingsDisplayDropdown(id) => {
                self.settings_open_dropdown = if self.settings_open_dropdown == id {
                    String::new()
                } else {
                    id.to_string()
                };
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsDisplayModePick(_mode) => {
                self.settings_open_dropdown.clear();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsDisplayScalePick(sc) => {
                self.settings_open_dropdown.clear();
                if let Ok(mut st) = self.stats.lock() {
                    if let Some(m) = st.monitors.first_mut() {
                        m.scale = sc.parse().unwrap_or(1.25);
                    }
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsDisplayRotPick(rot) => {
                self.settings_open_dropdown.clear();
                if let Ok(mut st) = self.stats.lock() {
                    if let Some(m) = st.monitors.first_mut() {
                        m.transform = rot.to_string();
                    }
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsDisplayApply => {
                self.settings_open_dropdown.clear();
                let mon_opt = self.stats.lock().ok().and_then(|s| s.monitors.first().cloned());
                if let Some(m) = mon_opt {
                    let sc_s = format!("{:.2}", m.scale)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string();
                    spawn_detached(
                        "wlr-randr",
                        &[
                            "--output",
                            &m.name,
                            "--scale",
                            &sc_s,
                            "--transform",
                            &m.transform,
                        ],
                    );
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::SettingsPower(act) => {
                self.set_popup(PopupKind::None);
                power_action(act);
            }
            HitTarget::AgyEffort(i) => {
                if i < self.agy_model().efforts.len() {
                    self.agy_effort_idx = i;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::AgyCycleModel => {
                let cur = self.agy_effort_pair().0;
                let next = (self.agy_model_idx + 1) % AGY_MODELS.len();
                self.agy_model_idx = next;
                let efforts = AGY_MODELS[next].efforts;
                self.agy_effort_idx = efforts
                    .iter()
                    .position(|e| e.0 == cur)
                    .or_else(|| efforts.iter().position(|e| e.0 == "high" || e.0 == "medium"))
                    .unwrap_or(0);
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::AgyNewChat => self.agy_new_chat(),
            HitTarget::AgySend => {
                if self.agy_busy {
                    self.stop_agy();
                } else {
                    self.send_agy();
                }
            }
            HitTarget::VolumeMute => {
                toggle_mute();
                if let Ok(mut st) = self.stats.lock() {
                    st.volume_muted = !st.volume_muted;
                    st.dirty = true;
                }
                self.invalidate_content();
                self.draw_island_surface();
                self.draw_bar_surface();
            }
            HitTarget::VolumeOutput(i) => {
                let out = self
                    .stats
                    .lock()
                    .ok()
                    .and_then(|s| s.audio_outputs.get(i).cloned());
                if let Some(o) = out {
                    select_audio_output(&o.sink, &o.port);
                    if let Ok(mut st) = self.stats.lock() {
                        for (j, ao) in st.audio_outputs.iter_mut().enumerate() {
                            ao.active = j == i;
                        }
                        st.dirty = true;
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(350));
                        refresh_panel_data(&stats);
                    });
                }
            }
            HitTarget::MediaPlay => {
                media_toggle_play();
                if let Ok(mut st) = self.stats.lock() {
                    st.media.playing = !st.media.playing;
                    st.dirty = true;
                }
                self.invalidate_content();
                self.draw_island_surface();
                self.draw_bar_surface();
            }
            HitTarget::MediaPrev => media_prev(),
            HitTarget::MediaNext => media_next(),
            HitTarget::MediaRew => {
                media_seek_relative(-10);
                if let Ok(mut st) = self.stats.lock() {
                    st.media.position = (st.media.position - 10.0).max(0.0);
                    st.dirty = true;
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::MediaFwd => {
                media_seek_relative(10);
                if let Ok(mut st) = self.stats.lock() {
                    let max = if st.media.length > 0.0 { st.media.length } else { f64::MAX };
                    st.media.position = (st.media.position + 10.0).min(max);
                    st.dirty = true;
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::MediaPlayerChip(i) => {
                let bus = self
                    .stats
                    .lock()
                    .ok()
                    .and_then(|s| s.media_players.get(i).map(|p| p.bus.clone()));
                if let Some(bus) = bus {
                    if let Ok(mut st) = self.stats.lock() {
                        st.preferred_player = bus;
                        st.dirty = true;
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(120));
                        refresh_stats_now(&stats);
                    });
                }
            }
            HitTarget::MediaRaise => {
                let bus = self
                    .stats
                    .lock()
                    .ok()
                    .map(|s| s.media.bus.clone())
                    .unwrap_or_default();
                if !bus.is_empty() {
                    spawn_detached(
                        "dbus-send",
                        &[
                            "--print-reply",
                            &format!("--dest={bus}"),
                            "/org/mpris/MediaPlayer2",
                            "org.mpris.MediaPlayer2.Raise",
                        ],
                    );
                }
            }
            HitTarget::WifiToggle => {
                self.wifi_selected_ssid.clear();
                self.wifi_password.clear();
                toggle_wifi();
                if let Ok(mut st) = self.stats.lock() {
                    st.wifi_enabled = !st.wifi_enabled;
                    if !st.wifi_enabled {
                        st.wifi_ssid.clear();
                        st.wifi_signal = 0;
                    }
                    st.wifi_status_msg.clear();
                    st.dirty = true;
                }
                if self.rendered_popup == PopupKind::Controls {
                    self.morph_island_inplace(200.0);
                } else {
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                self.draw_bar_surface();
                let stats = self.stats.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(900));
                    refresh_panel_data(&stats);
                    refresh_stats_now(&stats);
                });
            }
            HitTarget::WifiRescan => {
                let scanning = self
                    .stats
                    .lock()
                    .map(|s| s.wifi_scanning)
                    .unwrap_or(false);
                if !scanning {
                    if let Ok(mut st) = self.stats.lock() {
                        st.wifi_scanning = true;
                        st.wifi_status_msg = "Scanning networks...".into();
                        st.dirty = true;
                    }
                    wifi_rescan();
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(2200));
                        if let Ok(mut st) = stats.lock() {
                            st.wifi_scanning = false;
                            st.wifi_status_msg.clear();
                            st.dirty = true;
                        }
                        refresh_panel_data(&stats);
                    });
                }
            }
            HitTarget::WifiRow(i) => {
                let net = self
                    .stats
                    .lock()
                    .ok()
                    .and_then(|s| s.wifi_networks.get(i).cloned());
                if let Some(net) = net {
                    if button == BTN_RIGHT {
                        self.wifi_selected_ssid = net.ssid;
                        self.wifi_password.clear();
                        self.agy_cursor_on = true;
                        self.agy_blink_at = Instant::now();
                        self.island_layer
                            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                        self.island_layer.commit();
                        if self.rendered_popup == PopupKind::Controls {
                            self.morph_island_inplace(190.0);
                        } else {
                            self.invalidate_content();
                            self.draw_island_surface();
                        }
                        return;
                    }
                    if net.active {
                        self.wifi_selected_ssid.clear();
                        self.wifi_password.clear();
                        wifi_disconnect(&net.ssid);
                        if let Ok(mut st) = self.stats.lock() {
                            st.wifi_status_msg = "Disconnecting...".into();
                            st.dirty = true;
                        }
                    } else if net.saved || net.security.is_empty() || net.security == "--" {
                        self.wifi_selected_ssid.clear();
                        self.wifi_password.clear();
                        wifi_connect(&net.ssid, "", net.saved);
                        if let Ok(mut st) = self.stats.lock() {
                            st.wifi_status_msg = format!("Connecting to {}...", net.ssid);
                            st.dirty = true;
                        }
                    } else {
                        if self.wifi_selected_ssid == net.ssid {
                            self.wifi_selected_ssid.clear();
                            self.wifi_password.clear();
                            self.island_layer
                                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                            self.island_layer.commit();
                        } else {
                            self.wifi_selected_ssid = net.ssid;
                            self.wifi_password.clear();
                            self.agy_cursor_on = true;
                            self.agy_blink_at = Instant::now();
                            self.island_layer
                                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                            self.island_layer.commit();
                        }
                        if self.rendered_popup == PopupKind::Controls {
                            self.morph_island_inplace(190.0);
                        } else {
                            self.invalidate_content();
                            self.draw_island_surface();
                        }
                        return;
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1800));
                        refresh_panel_data(&stats);
                        refresh_stats_now(&stats);
                        if let Ok(mut st) = stats.lock() {
                            st.wifi_status_msg.clear();
                            st.dirty = true;
                        }
                    });
                }
            }
            HitTarget::WifiPassClose => {
                self.wifi_selected_ssid.clear();
                self.wifi_password.clear();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.island_layer.commit();
                if self.rendered_popup == PopupKind::Controls {
                    self.morph_island_inplace(190.0);
                } else {
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            }
            HitTarget::WifiPassInput => {
                self.agy_cursor_on = true;
                self.agy_blink_at = Instant::now();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.island_layer.commit();
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::WifiPassConnect => {
                self.submit_wifi_password();
            }
            HitTarget::BtToggle => {
                toggle_bluetooth();
                if let Ok(mut st) = self.stats.lock() {
                    st.bt_powered = !st.bt_powered;
                    if !st.bt_powered {
                        st.bt_connected = 0;
                        st.bt_paired.clear();
                        st.bt_available.clear();
                    }
                    st.bt_status_msg.clear();
                    st.dirty = true;
                }
                if self.rendered_popup == PopupKind::Controls {
                    self.morph_island_inplace(200.0);
                } else {
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                self.draw_bar_surface();
                let stats = self.stats.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(900));
                    refresh_panel_data(&stats);
                    refresh_stats_now(&stats);
                });
            }
            HitTarget::BtScan => {
                let scanning = self
                    .stats
                    .lock()
                    .map(|s| s.bt_scanning)
                    .unwrap_or(false);
                if scanning {
                    spawn_detached("pkill", &["-f", "bluetoothctl.*scan"]);
                    if let Ok(mut st) = self.stats.lock() {
                        st.bt_scanning = false;
                        st.bt_status_msg.clear();
                        st.dirty = true;
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                } else {
                    if let Ok(mut st) = self.stats.lock() {
                        st.bt_scanning = true;
                        st.bt_status_msg = "Searching for nearby devices...".into();
                        st.dirty = true;
                    }
                    bt_scan();
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1500));
                        refresh_panel_data(&stats);
                        for _ in 0..5 {
                            std::thread::sleep(Duration::from_millis(2000));
                            if !stats.lock().map(|s| s.bt_scanning).unwrap_or(false) {
                                return;
                            }
                            refresh_panel_data(&stats);
                        }
                        if let Ok(mut st) = stats.lock() {
                            st.bt_scanning = false;
                            st.bt_status_msg.clear();
                            st.dirty = true;
                        }
                    });
                }
            }
            HitTarget::BtPairedRow(i) => {
                let dev = self
                    .stats
                    .lock()
                    .ok()
                    .and_then(|s| s.bt_paired.get(i).cloned());
                if let Some(d) = dev {
                    if d.connected {
                        bt_disconnect(&d.mac);
                        if let Ok(mut st) = self.stats.lock() {
                            st.bt_status_msg = "Disconnecting...".into();
                            st.dirty = true;
                        }
                    } else {
                        bt_connect(&d.mac, &d.name, true);
                        if let Ok(mut st) = self.stats.lock() {
                            st.bt_status_msg = format!("Connecting to {}...", d.name);
                            st.dirty = true;
                        }
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1600));
                        refresh_panel_data(&stats);
                        refresh_stats_now(&stats);
                        std::thread::sleep(Duration::from_millis(1400));
                        if let Ok(mut st) = stats.lock() {
                            st.bt_status_msg.clear();
                            st.dirty = true;
                        }
                    });
                }
            }
            HitTarget::BtPairRow(i) => {
                let dev = self
                    .stats
                    .lock()
                    .ok()
                    .and_then(|s| s.bt_available.get(i).cloned());
                if let Some(d) = dev {
                    bt_connect(&d.mac, &d.name, false);
                    if let Ok(mut st) = self.stats.lock() {
                        st.bt_status_msg = format!("Pairing with {}...", d.name);
                        st.dirty = true;
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(2500));
                        refresh_panel_data(&stats);
                        refresh_stats_now(&stats);
                        std::thread::sleep(Duration::from_millis(1500));
                        if let Ok(mut st) = stats.lock() {
                            st.bt_status_msg.clear();
                            st.dirty = true;
                        }
                    });
                }
            }
            HitTarget::BtForget(i) => {
                let dev = self
                    .stats
                    .lock()
                    .ok()
                    .and_then(|s| s.bt_paired.get(i).cloned());
                if let Some(d) = dev {
                    bt_remove(&d.mac);
                    if let Ok(mut st) = self.stats.lock() {
                        st.bt_status_msg = "Removing device...".into();
                        st.bt_paired.retain(|x| x.mac != d.mac);
                        st.dirty = true;
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                    let stats = self.stats.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1200));
                        refresh_panel_data(&stats);
                        if let Ok(mut st) = stats.lock() {
                            st.bt_status_msg.clear();
                            st.dirty = true;
                        }
                    });
                }
            }
            HitTarget::NotifClear => {
                if let Ok(mut st) = self.stats.lock() {
                    st.notifications.clear();
                    st.dirty = true;
                }
                self.notif_scroll = 0.0;
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::NotifDismiss(i) => {
                if let Ok(mut st) = self.stats.lock() {
                    if i < st.notifications.len() {
                        st.notifications.remove(i);
                        st.dirty = true;
                    }
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::NotifDnd => {
                if let Ok(mut st) = self.stats.lock() {
                    st.dnd_enabled = !st.dnd_enabled;
                    st.dirty = true;
                }
                self.invalidate_content();
                self.draw_island_surface();
            }
            HitTarget::PowerAction(act) => {
                self.set_popup(PopupKind::None);
                power_action(act);
            }
            _ => {}
        }
    }

    fn submit_wifi_password(&mut self) {
        if self.wifi_selected_ssid.is_empty() {
            return;
        }
        let ssid = std::mem::take(&mut self.wifi_selected_ssid);
        let pass = std::mem::take(&mut self.wifi_password);
        self.island_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.island_layer.commit();
        wifi_connect(&ssid, &pass, false);
        if let Ok(mut st) = self.stats.lock() {
            st.wifi_status_msg = format!("Connecting to {}...", ssid);
            st.dirty = true;
        }
        if self.rendered_popup == PopupKind::Controls {
            self.morph_island_inplace(190.0);
        } else {
            self.invalidate_content();
            self.draw_island_surface();
        }
        let stats = self.stats.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2200));
            refresh_panel_data(&stats);
            refresh_stats_now(&stats);
            if let Ok(mut st) = stats.lock() {
                st.wifi_status_msg.clear();
                st.dirty = true;
            }
        });
    }

    fn process_ipc_and_stats(&mut self) {
        let mut need_bar = false;
        let mut need_island = false;

        self.tick_key_repeat();

        if let Some(close_at) = self.hover_close_at {
            if Instant::now() >= close_at {
                self.hover_close_at = None;
                if self.opened_by_hover && !self.bar_hovered && !self.island_card_hovered {
                    if self.wifi_selected_ssid.is_empty()
                        && matches!(self.files_prompt, FilesPrompt::None)
                        && !self.filter_focused
                    {
                        self.set_popup(PopupKind::None);
                    }
                }
            }
        }

        if self.rendered_popup == PopupKind::Settings && self.settings_tab == 0 && !self.island_animating() {
            let sec = chrono::Local::now().timestamp();
            if sec != self.last_clock_sec {
                self.last_clock_sec = sec;
                self.invalidate_content();
                need_island = true;
            }
        }

        if self.cfg_last_check.elapsed() >= Duration::from_millis(250) {
            self.cfg_last_check = Instant::now();
            let mtimes = config_mtimes();
            if mtimes != self.cfg_mtimes {
                self.apply_reloaded_config();
                need_bar = true;
                need_island = true;
            }
        }

        if self.apps_last_check.elapsed() >= Duration::from_millis(1000) {
            self.apps_last_check = Instant::now();
            let mtimes = app_dirs_mtimes();
            if mtimes != self.app_dirs_mtimes {
                self.app_dirs_mtimes = mtimes;
                self.all_apps = scan_desktop_apps_cached(&self.all_apps);
                if self.rendered_popup == PopupKind::Launcher {
                    self.refilter_apps();
                    need_island = true;
                }
            }
        }

        if let Some(until) = self.toast_until {
            if Instant::now() >= until {
                self.status_toast.clear();
                self.toast_until = None;
                self.invalidate_content();
                need_island = true;
            }
        }

        while let Ok(cmd) = self.ipc_rx.try_recv() {
            match cmd {
                IpcCommand::Toggle(name) => self.toggle_popup(PopupKind::from_name(&name)),
                IpcCommand::Open(name) => self.set_popup(PopupKind::from_name(&name)),
                IpcCommand::OpenFilesDir(dir) => {
                    let clean = if let Some(rest) = dir.strip_prefix("file://") {
                        rest.to_string()
                    } else {
                        dir
                    };
                    let decoded = if clean.contains('%') {
                        let mut b = Vec::new();
                        let bytes = clean.as_bytes();
                        let mut i = 0;
                        while i < bytes.len() {
                            if bytes[i] == b'%' && i + 2 < bytes.len() {
                                if let Ok(val) = u8::from_str_radix(&clean[i + 1..i + 3], 16) {
                                    b.push(val);
                                    i += 3;
                                    continue;
                                }
                            }
                            b.push(bytes[i]);
                            i += 1;
                        }
                        String::from_utf8_lossy(&b).to_string()
                    } else {
                        clean
                    };
                    let expanded = if decoded == "~" {
                        "/home/huyle".to_string()
                    } else if let Some(rest) = decoded.strip_prefix("~/") {
                        format!("/home/huyle/{rest}")
                    } else {
                        decoded
                    };
                    let p = PathBuf::from(&expanded);
                    if p.is_dir() {
                        self.navigate_files(p, true);
                    } else if let Some(parent) = p.parent() {
                        if parent.is_dir() {
                            self.navigate_files(parent.to_path_buf(), true);
                            if let Some(idx) = self.entries.iter().position(|e| e.path == p) {
                                self.selected_idx = Some(idx);
                                self.selected_paths = vec![p];
                                self.update_files_preview();
                            }
                        }
                    } else {
                        self.navigate_files(PathBuf::from("/home/huyle"), true);
                    }
                    self.set_popup(PopupKind::Files);
                    self.invalidate_content();
                    need_island = true;
                }
                IpcCommand::PortalOpen(path) => {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(req) = serde_json::from_str::<PortalRequest>(&content) {
                            let start = if !req.current_dir.is_empty() && Path::new(&req.current_dir).is_dir() {
                                PathBuf::from(&req.current_dir)
                            } else {
                                PathBuf::from("/home/huyle/Downloads")
                            };
                            self.navigate_files(start, true);
                            self.selected_idx = None;
                            self.selected_paths.clear();
                            self.filter_query.clear();
                            self.filter_focused = false;
                            if req.mode == "save" {
                                self.files_prompt = FilesPrompt::SaveFilename;
                                self.files_prompt_text = req.current_name.clone();
                                self.portal_save_name = req.current_name.clone();
                            } else {
                                self.portal_save_name.clear();
                                self.files_prompt = FilesPrompt::None;
                                self.files_prompt_text.clear();
                                if !req.current_name.is_empty() {
                                    if let Some(idx) = self.entries.iter().position(|e| e.name == req.current_name) {
                                        self.selected_idx = Some(idx);
                                        self.selected_paths = vec![self.entries[idx].path.clone()];
                                        self.update_files_preview();
                                    }
                                }
                            }
                            self.portal_req = Some(req);
                            self.set_popup(PopupKind::Files);
                            self.island_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                            self.island_layer.commit();
                            self.sync_files_anims();
                            self.invalidate_content();
                            need_island = true;
                        }
                    }
                }
                IpcCommand::PortalCancel(id) => {
                    if let Some(req) = &self.portal_req {
                        if req.id == id {
                            self.cancel_portal();
                        }
                    }
                }
                IpcCommand::OpenSettingsTab(i, sub) => {
                    if let Some(sub_id) = sub {
                        self.settings_bg_target = sub_id;
                    }
                    self.switch_settings_tab(i);
                    need_island = true;
                }
                IpcCommand::OpenControlsSection(sec_name) => {
                    let sec: &'static str = match sec_name.as_str() {
                        "wifi" => "wifi",
                        "bt" | "bluetooth" => "bt",
                        "audio" | "sound" => "audio",
                        "battery" => "battery",
                        "power" => "power",
                        _ => "",
                    };
                    if self.active_popup != PopupKind::Controls {
                        self.cc_expanded = sec;
                        self.set_popup(PopupKind::Controls);
                    } else {
                        self.set_cc_expanded(sec);
                    }
                    need_island = true;
                }
                IpcCommand::Volume(arg) => {
                    let cur = self
                        .stats
                        .lock()
                        .map(|s| (s.volume_percent as i32, s.volume_muted))
                        .unwrap_or((50, false));
                    let arg_s = arg.trim();
                    if arg_s == "mute" || arg_s == "toggle-mute" {
                        toggle_mute();
                        let new_mute = !cur.1;
                        if let Ok(mut st) = self.stats.lock() {
                            st.volume_muted = new_mute;
                            st.dirty = true;
                        }
                    } else if arg_s == "mic-mute" {
                        spawn_detached("wpctl", &["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]);
                    } else {
                        let diff: Option<i32> = if let Some(n) = arg_s.strip_prefix('+') {
                            n.trim_end_matches('%').parse::<i32>().ok()
                        } else if let Some(n) = arg_s.strip_prefix('-') {
                            n.trim_end_matches('%').parse::<i32>().ok().map(|v| -v)
                        } else if arg_s == "up" {
                            Some(5)
                        } else if arg_s == "down" {
                            Some(-5)
                        } else {
                            None
                        };

                        let target = if let Some(d) = diff {
                            (cur.0 + d).clamp(0, 150)
                        } else if let Ok(val) = arg_s.trim_end_matches('%').parse::<i32>() {
                            val.clamp(0, 150)
                        } else {
                            cur.0
                        };

                        set_volume(target);
                        if let Ok(mut st) = self.stats.lock() {
                            st.volume_percent = target as u32;
                            st.volume_muted = false;
                            st.dirty = true;
                        }
                    }
                    self.invalidate_content();
                    need_bar = true;
                    need_island = true;
                }
                IpcCommand::Brightness(arg) => {
                    let cur = self
                        .stats
                        .lock()
                        .map(|s| s.brightness_percent as i32)
                        .unwrap_or(50);
                    let arg_s = arg.trim();
                    let diff: Option<i32> = if let Some(n) = arg_s.strip_prefix('+') {
                        n.trim_end_matches('%').parse::<i32>().ok()
                    } else if let Some(n) = arg_s.strip_prefix('-') {
                        n.trim_end_matches('%').parse::<i32>().ok().map(|v| -v)
                    } else if arg_s == "up" {
                        Some(5)
                    } else if arg_s == "down" {
                        Some(-5)
                    } else {
                        None
                    };

                    let target = if let Some(d) = diff {
                        (cur + d).clamp(5, 100)
                    } else if let Ok(val) = arg_s.trim_end_matches('%').parse::<i32>() {
                        val.clamp(5, 100)
                    } else {
                        cur
                    };

                    set_brightness(target);
                    if let Ok(mut st) = self.stats.lock() {
                        st.brightness_percent = target as u32;
                        st.dirty = true;
                    }
                    self.invalidate_content();
                    need_bar = true;
                    need_island = true;
                }
                IpcCommand::Agy(text) => {
                    if self.active_popup != PopupKind::Antigravity
                        || self.rendered_popup != PopupKind::Antigravity
                    {
                        self.set_popup(PopupKind::Antigravity);
                    }
                    self.agy_input = text;
                    self.send_agy();
                    need_island = true;
                }
                IpcCommand::Close => self.set_popup(PopupKind::None),
                IpcCommand::SetBarPos(pos) => {
                    if ["left", "right", "top", "bottom"].contains(&pos.as_str()) {
                        self.cfg.bar_position = pos;
                        self.sync_bar_layout();
                        need_bar = true;
                    }
                }
                IpcCommand::FilesMode(mode) => {
                    match mode.as_str() {
                        "list" => self.files_view_mode = FileViewMode::List,
                        "grid" => self.files_view_mode = FileViewMode::Grid,
                        "toggle" => {
                            self.files_view_mode = match self.files_view_mode {
                                FileViewMode::Grid => FileViewMode::List,
                                FileViewMode::List => FileViewMode::Grid,
                            };
                        }
                        _ => {}
                    }
                    self.scroll_y = 0.0;
                    self.invalidate_content();
                    need_island = true;
                }
                IpcCommand::FilesAct(raw) => {
                    let parts: Vec<&str> = raw.split_whitespace().collect();
                    let sub = parts.first().copied().unwrap_or("");
                    match sub {
                        "back" => {
                            if let Some(prev) = self.back_stack.pop() {
                                self.fwd_stack.push(self.current_dir.clone());
                                self.navigate_files(prev, false);
                            }
                        }
                        "fwd" => {
                            if let Some(next) = self.fwd_stack.pop() {
                                self.back_stack.push(self.current_dir.clone());
                                self.navigate_files(next, false);
                            }
                        }
                        "up" => {
                            if let Some(parent) = self.current_dir.parent().map(|p| p.to_path_buf()) {
                                self.navigate_files(parent, true);
                            }
                        }
                        "breadcrumb" | "nav" if parts.len() >= 2 => {
                            let p = PathBuf::from(parts[1..].join(" "));
                            self.navigate_files(p, true);
                        }
                        "path-focus" => {
                            let init = if parts.len() >= 2 {
                                parts[1..].join(" ")
                            } else {
                                self.current_dir.to_string_lossy().to_string()
                            };
                            self.open_files_prompt(FilesPrompt::PathInput, init);
                        }
                        "path-input" if parts.len() >= 2 => {
                            let target = parts[1..].join(" ");
                            self.open_files_prompt(FilesPrompt::PathInput, target);
                            self.submit_files_prompt();
                        }
                        "hidden" => {
                            self.show_hidden = !self.show_hidden;
                            self.refresh_files();
                        }
                        "search" => {
                            let q = if parts.len() >= 2 { parts[1..].join(" ") } else { String::new() };
                            self.filter_focused = true;
                            self.filter_query = q;
                            self.refresh_files();
                        }
                        "search-clear" => {
                            self.filter_focused = false;
                            self.filter_query.clear();
                            self.refresh_files();
                        }
                        "select" if parts.len() >= 2 => {
                            let names: Vec<&str> = parts[1].split(',').collect();
                            self.selected_paths.clear();
                            self.selected_idx = None;
                            for (i, e) in self.entries.iter().enumerate() {
                                if names.contains(&e.name.as_str()) {
                                    self.selected_paths.push(e.path.clone());
                                    if self.selected_idx.is_none() {
                                        self.selected_idx = Some(i);
                                    }
                                }
                            }
                            self.update_files_preview();
                        }
                        "select-all" => {
                            self.selected_paths = self.entries.iter().map(|e| e.path.clone()).collect();
                            if !self.entries.is_empty() {
                                self.selected_idx = Some(0);
                            }
                            self.update_files_preview();
                        }
                        "dblclick" if parts.len() >= 2 => {
                            let name = parts[1..].join(" ");
                            if let Some(idx) = self.entries.iter().position(|e| e.name == name) {
                                self.activate_file_item(idx);
                            }
                        }
                        "mkdir" if parts.len() >= 2 => {
                            let name = parts[1..].join(" ");
                            self.open_files_prompt(FilesPrompt::Mkdir, name);
                            self.submit_files_prompt();
                        }
                        "rename-focus" if parts.len() >= 2 => {
                            let name = parts[1..].join(" ");
                            if let Some(idx) = self.entries.iter().position(|e| e.name == name) {
                                let p = self.entries[idx].path.clone();
                                self.selected_idx = Some(idx);
                                self.selected_paths = vec![p.clone()];
                                self.open_files_prompt(FilesPrompt::Rename(p), name);
                            }
                        }
                        "rename" if parts.len() >= 3 => {
                            let old_name = parts[1];
                            let new_name = parts[2..].join(" ");
                            if let Some(idx) = self.entries.iter().position(|e| e.name == old_name) {
                                let p = self.entries[idx].path.clone();
                                self.selected_idx = Some(idx);
                                self.selected_paths = vec![p.clone()];
                                self.open_files_prompt(FilesPrompt::Rename(p), new_name);
                                self.submit_files_prompt();
                            }
                        }
                        "copy" => self.execute_files_action("copy_files"),
                        "cut" => self.execute_files_action("cut_files"),
                        "paste" => self.execute_files_action("paste_files"),
                        "trash" => self.execute_files_action("trash"),
                        "extract" => self.execute_files_action("extract"),
                        "compress" => self.execute_files_action("compress"),
                        "context-menu" => {
                            let mx: f64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(320.0);
                            let my: f64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(140.0);
                            self.files_context_menu = Some((mx, my, self.selected_idx));
                            self.files_ctx_anim = Motion::new(0.0, 1.0, 120.0, Ease::OutCubic);
                        }
                        "context-close" => {
                            self.files_context_menu = None;
                        }
                        "portal-accept" => {
                            self.handle_click(HitTarget::FilesPortalAccept, BTN_LEFT, 0, 0.0, 0.0);
                        }
                        "dump-state" => {
                            let sel_bytes: u64 = self
                                .entries
                                .iter()
                                .filter(|e| !e.is_dir && self.selected_paths.contains(&e.path))
                                .map(|e| e.size)
                                .sum();
                            let state_json = serde_json::json!({
                                "current_dir": self.current_dir.to_string_lossy(),
                                "view_mode": match self.files_view_mode { FileViewMode::List => "list", FileViewMode::Grid => "grid" },
                                "show_hidden": self.show_hidden,
                                "filter_query": self.filter_query,
                                "entries_count": self.entries.len(),
                                "entries": self.entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
                                "selected_count": self.selected_paths.len(),
                                "selected_bytes": sel_bytes,
                                "selected_names": self.selected_paths.iter().filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())).collect::<Vec<_>>(),
                            });
                            let _ = std::fs::write("/tmp/yunoshell_files_state.json", state_json.to_string());
                        }
                        _ => {}
                    }
                    self.sync_files_anims();
                    self.invalidate_content();
                    need_island = true;
                }
                IpcCommand::ReloadTheme => {
                    self.cfg = load_config();
                    self.cfg_mtimes = config_mtimes();
                    self.sync_bar_layout();
                    self.sync_pills();
                    self.begin_diagonal_wipe(PendingWipe::Theme);
                    need_bar = true;
                    need_island = true;
                }
                IpcCommand::Quit => self.exit = true,
            }
        }

        let mut stats_dirty = false;
        let island_busy = self.island_animating();
        if let Ok(mut st) = self.stats.lock() {
            if st.dirty && !island_busy {
                st.dirty = false;
                stats_dirty = true;
            }
        }
        if stats_dirty {
            if let Ok(st) = self.stats.lock() {
                for item in &st.tray_items {
                    if !item.icon_png.is_empty() && !self.tray_icons.contains_key(&item.icon_png) {
                        if let Some(surf) = load_png_surface(Path::new(&item.icon_png)) {
                            if self.tray_icons.len() > 32 {
                                self.tray_icons.clear();
                            }
                            self.tray_icons.insert(item.icon_png.clone(), surf);
                        }
                    }
                }
            }
            self.sync_pills();
            need_bar = true;
            if matches!(
                self.rendered_popup,
                PopupKind::Controls
                    | PopupKind::Volume
                    | PopupKind::Brightness
                    | PopupKind::Battery
                    | PopupKind::Wifi
                    | PopupKind::Bluetooth
                    | PopupKind::Media
                    | PopupKind::Notifications
                    | PopupKind::TrayMenu
            ) || (self.rendered_popup == PopupKind::Settings && self.settings_tab >= 2)
            {
                self.invalidate_content();
                need_island = true;
            }
        }

        // Clean up finished diagonal wipe overlay if frame callback hasn't fired
        if self.wipe_layer.is_some() && self.wipe_configured && self.wipe_anim.done() {
            self.wipe_gpu = None;
            self.wipe_layer = None;
            self.wipe_configured = false;
            self.wipe_frame_pending = false;
        }

        // Antigravity CLI stream + blinking input cursor.
        let mut agy_events: Vec<AgyEvent> = Vec::new();
        if let Some(rx) = &self.agy_rx {
            while let Ok(ev) = rx.try_recv() {
                agy_events.push(ev);
            }
        }
        let agy_open = self.rendered_popup == PopupKind::Antigravity;
        let wifi_pass_open = !self.wifi_selected_ssid.is_empty();
        let files_input_open = self.rendered_popup == PopupKind::Files
            && (self.filter_focused
                || !matches!(
                    self.files_prompt,
                    FilesPrompt::None
                        | FilesPrompt::ConfirmDelete(_)
                        | FilesPrompt::ConfirmEmptyTrash
                ));
        if ((agy_open && !self.agy_input.is_empty() && !self.agy_busy)
            || wifi_pass_open
            || files_input_open)
            && self.agy_blink_at.elapsed() >= Duration::from_millis(530)
        {
            self.agy_blink_at = Instant::now();
            self.agy_cursor_on = !self.agy_cursor_on;
            self.invalidate_content();
            need_island = true;
        }
        if !agy_events.is_empty() {
            let mut finished = false;
            for ev in agy_events {
                match ev {
                    AgyEvent::Status(s) => {
                        self.agy_status = s;
                    }
                    AgyEvent::Delta(d) => {
                        if let Some(m) = self.agy_messages.last_mut() {
                            m.text.push_str(&d);
                        }
                        self.agy_scroll = 0.0;
                    }
                    AgyEvent::Done { conv, text, meta } => {
                        if !conv.is_empty() {
                            self.agy_conv = conv;
                        }
                        if !text.is_empty() {
                            if let Some(m) = self.agy_messages.last_mut() {
                                if !m.user {
                                    m.text = text;
                                }
                            }
                        }
                        if !meta.is_empty() {
                            self.agy_meta = meta;
                        }
                        self.agy_busy = false;
                        self.agy_status.clear();
                        finished = true;
                    }
                }
            }
            if finished {
                self.agy_rx = None;
            }
            self.invalidate_content();
            need_island = true;
        }

        // Poll async wallpaper thumbnail loader
        if let Some(ref rx) = self.thumb_rx {
            let mut got_thumb = false;
            while let Ok((p, cached_opt)) = rx.try_recv() {
                if let Some(entry) = self.wallpapers.iter_mut().find(|(_, ep, _)| *ep == p) {
                    entry.2 = cached_opt.as_deref().and_then(load_png_surface);
                    got_thumb = true;
                }
            }
            if got_thumb && self.rendered_popup == PopupKind::Settings && self.settings_tab == 1 {
                self.invalidate_content();
                need_island = true;
            }
        }

        // Poll async file manager thumbnail loader
        let mut got_file_thumb = false;
        while let Ok((p, cached_opt)) = self.files_thumb_rx.try_recv() {
            if let Some(surf) = cached_opt.as_deref().and_then(load_png_surface) {
                self.files_thumbs.insert(p, surf);
                got_file_thumb = true;
            }
        }
        if got_file_thumb && self.rendered_popup == PopupKind::Files {
            self.invalidate_content();
            need_island = true;
        }

        if need_bar {
            self.draw_bar_surface();
        }
        if need_island {
            self.draw_island_surface();
        }
    }
}

impl CompositorHandler for YunoShell {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surf: &wl_surface::WlSurface, _: u32) {
        if surf == self.island_layer.wl_surface() {
            self.island_frame_pending = false;
            self.draw_island_surface();
        } else if self.bar_layer.as_ref().map(|b| b.wl_surface() == surf).unwrap_or(false) {
            self.bar_frame_pending = false;
            self.draw_bar_surface();
        } else if self.wipe_layer.as_ref().map(|w| w.wl_surface() == surf).unwrap_or(false) {
            self.wipe_frame_pending = false;
            self.draw_wipe_surface();
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for YunoShell {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for YunoShell {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if layer == &self.island_layer || self.bar_layer.as_ref() == Some(layer) {
            self.exit = true;
        }
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        if layer == &self.island_layer {
            self.island_configured = true;
            self.island_w = cfg.new_size.0.max(1);
            self.island_h = cfg.new_size.1.max(1);
            if !self.gpu_failed && self.gpu.is_none() {
                let pw = self.island_w as i32 * BUF_SCALE;
                let ph = self.island_h as i32 * BUF_SCALE;
                if let Ok(mut g) = GpuIsland::new(self.gpu_display, self.gpu_surface, pw, ph) {
                    // Warm up EGL DRI3 swapchain buffers ahead of first open.
                    g.clear();
                    g.clear();
                    self.gpu = Some(g);
                } else {
                    self.gpu_failed = true;
                }
            }
            self.invalidate_content();
            self.draw_island_surface();
        } else if let Some(ref bar) = self.bar_layer {
            if layer == bar {
                self.bar_w = cfg.new_size.0.max(self.cfg.bar_size);
                self.bar_h = cfg.new_size.1.max(self.cfg.bar_size);
                self.bar_configured = true;
                self.draw_bar_surface();
                return;
            }
        }
        if let Some(ref wipe) = self.wipe_layer {
            if layer == wipe {
                let w = cfg.new_size.0.max(self.island_w).max(1280) as i32;
                let h = cfg.new_size.1.max(self.island_h).max(800) as i32;
                self.wipe_w = w as u32;
                self.wipe_h = h as u32;
                self.wipe_configured = true;
                if self.wipe_gpu.is_none() {
                    if let Some((rw, rh, rgb)) = self.wipe_rgb.take() {
                        let sptr = wipe.wl_surface().id().as_ptr() as *mut c_void;
                        if let Ok(gw) = GpuWipe::new(self.gpu_display, sptr, w, h, rw, rh, &rgb) {
                            self.wipe_gpu = Some(gw);
                        }
                    }
                }
                if self.wipe_gpu.is_some() {
                    let delay_ms = match self.wipe_pending {
                        Some(PendingWipe::Wallpaper { .. }) => 80.0,
                        _ => 45.0,
                    };
                    self.wipe_anim = Motion::new(0.0, 1.0, 400.0, Ease::Custom([0.35, 0.05, 0.20, 1.0])).delayed(delay_ms);
                    self.wipe_frame_pending = false;
                    self.draw_wipe_surface();
                    if let Some(pending) = self.wipe_pending.take() {
                        self.apply_pending_wipe(pending);
                    }
                } else {
                    self.wipe_layer = None;
                    self.wipe_configured = false;
                    if let Some(pending) = self.wipe_pending.take() {
                        self.apply_pending_wipe(pending);
                    }
                }
            }
        }
    }
}

impl ShmHandler for YunoShell {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm_state }
}

impl SeatHandler for YunoShell {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if self.seat.is_none() {
            self.data_device = Some(self.data_device_manager_state.get_data_device(qh, &seat));
            self.seat = Some(seat.clone());
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            let surf = self.compositor_state.create_surface(qh);
            self.pointer = self
                .seat_state
                .get_pointer_with_theme(
                    qh,
                    &seat,
                    self.shm_state.wl_shm(),
                    surf,
                    ThemeSpec::System,
                )
                .ok();
        }
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self.seat_state.get_keyboard(qh, &seat, None).ok();
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer {
            self.pointer = None;
        }
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for YunoShell {
    fn pointer_frame(&mut self, conn: &Connection, qh: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            let is_island = &ev.surface == self.island_layer.wl_surface();
            let is_bar = self.bar_layer.as_ref().map(|b| b.wl_surface() == &ev.surface).unwrap_or(false);
            if !is_island && !is_bar {
                continue;
            }
            let (px, py) = ev.position;
            let hits = if is_island { &self.island_hits } else { &self.bar_hits };
            let hit_box = hits.iter().rev().find(|hb| hb.contains(px, py)).cloned();
            let hit = hit_box.as_ref().map(|hb| hb.target.clone());

            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    let is_enter = matches!(ev.kind, PointerEventKind::Enter { .. });
                    if is_enter || hit != self.hover {
                        if let Some(ref tp) = self.pointer {
                            let icon = match hit {
                                Some(
                                    HitTarget::FilesFocusFilter
                                    | HitTarget::FilesPromptInput
                                    | HitTarget::FilesEditPath
                                    | HitTarget::WifiPassInput,
                                ) => CursorIcon::Text,
                                Some(HitTarget::FilesListArea) | None => CursorIcon::Default,
                                Some(_) => CursorIcon::Pointer,
                            };
                            let _ = tp.set_cursor(conn, icon);
                        }
                    }
                    if let Some(ref sdrag) = self.slider_drag.clone() {
                        self.apply_slider_hit(sdrag, px);
                        continue;
                    }
                    if is_bar {
                        self.bar_hovered = true;
                        if let Some(HitTarget::BarPopup(kind)) = hit {
                            if Self::is_hover_trigger_popup(kind) {
                                self.hover_close_at = None;
                                let target_kind = if Self::cc_section_for_popup(kind).is_some() {
                                    PopupKind::Controls
                                } else {
                                    kind
                                };
                                if self.active_popup != target_kind {
                                    if target_kind == PopupKind::Controls {
                                        self.cc_expanded = "";
                                    }
                                    self.opened_by_hover = true;
                                    self.set_popup(target_kind);
                                }
                            } else if self.opened_by_hover && !self.island_card_hovered && self.hover_close_at.is_none() {
                                self.hover_close_at = Some(Instant::now() + Duration::from_millis(200));
                            }
                        } else if self.opened_by_hover && !self.island_card_hovered && self.hover_close_at.is_none() {
                            self.hover_close_at = Some(Instant::now() + Duration::from_millis(200));
                        }
                    } else if is_island {
                        let in_card = self.point_in_island_card(px, py);
                        self.island_card_hovered = in_card;
                        if in_card {
                            self.hover_close_at = None;
                        } else if self.opened_by_hover && !self.bar_hovered && self.hover_close_at.is_none() {
                            self.hover_close_at = Some(Instant::now() + Duration::from_millis(200));
                        }
                    }
                    if hit != self.hover {
                        let prev_hover = self.hover.clone();
                        self.hover = hit.clone();
                        if is_island && !self.island_animating() && !self.internal_dragging {
                            let only_header_hover = self.rendered_popup == PopupKind::Settings
                                && matches!(prev_hover, None | Some(HitTarget::SettingsTab(_)) | Some(HitTarget::SettingsOpenPower))
                                && matches!(self.hover, None | Some(HitTarget::SettingsTab(_)) | Some(HitTarget::SettingsOpenPower));
                            if only_header_hover {
                                self.content_dirty = true;
                            } else {
                                self.invalidate_content();
                            }
                            self.draw_island_surface();
                        } else if is_bar {
                            self.sync_pills();
                            self.draw_bar_surface();
                        }
                    }
                    if is_island && self.button_down {
                        let dx = px - self.press_pos.0;
                        let dy = py - self.press_pos.1;
                        if dx * dx + dy * dy > 49.0 {
                            if self.rendered_popup == PopupKind::Files {
                                if self.pressed_file_idx.is_some() {
                                    let was_dragging = self.internal_dragging;
                                    self.internal_dragging = true;
                                    if let Some(l) = self.island_layout() {
                                        let lx = px - (l.content_x + l.slide_x);
                                        let ly = py - (l.content_y + l.slide_y);
                                        self.internal_drag_pos = (lx, ly);
                                    }
                                    if self.internal_drag_sources.is_empty() {
                                        self.internal_drag_sources = self.effective_selected_paths();
                                    }
                                    let mut target_dir = None;
                                    let mut qa_hover = false;
                                    if let Some(ref h) = hit {
                                        match h {
                                            HitTarget::FilesRow(t_idx) => {
                                                if let Some(it) = self.entries.get(*t_idx) {
                                                    if it.is_dir && !self.internal_drag_sources.contains(&it.path) {
                                                        target_dir = Some(it.path.clone());
                                                    }
                                                }
                                            }
                                            HitTarget::FilesPlace(p) | HitTarget::FilesDrive(p) => {
                                                target_dir = Some(p.clone());
                                            }
                                            HitTarget::FilesEmptyTrash => {
                                                target_dir = Some(PathBuf::from(TRASH_DIR));
                                            }
                                            HitTarget::FilesQuickAccessDrop => {
                                                qa_hover = true;
                                            }
                                            _ => {}
                                        }
                                    }
                                    let target_changed = self.internal_drag_target != target_dir
                                        || self.files_qa_dnd_hover != qa_hover
                                        || !was_dragging;
                                    if target_changed {
                                        self.internal_drag_target = target_dir;
                                        self.files_qa_dnd_hover = qa_hover;
                                        self.invalidate_content();
                                    } else if self.gpu.is_none() || self.gpu_failed {
                                        // SHM fallback has no separate GPU overlay layer.
                                        self.invalidate_content();
                                    }

                                    // If user drags out of the island card: start Wayland DND
                                    if !self.point_in_island_card(px, py) && !self.drag_launched && self.data_device.is_some() {
                                        let drag_paths = self.internal_drag_sources.clone();
                                        let mut uri_payload = String::new();
                                        let mut plain_lines = Vec::new();
                                        for p in &drag_paths {
                                            uri_payload.push_str(&encode_uri(p));
                                            uri_payload.push_str("\r\n");
                                            plain_lines.push(p.to_string_lossy().to_string());
                                        }
                                        let plain_payload = plain_lines.join("\n");
                                        let drag_label = if drag_paths.len() > 1 {
                                            format!("{} items", drag_paths.len())
                                        } else {
                                            drag_paths.first().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("file").to_string()
                                        };
                                        let (icon_surf, icon_pool) = match self.create_drag_icon_surface(&drag_label) {
                                            Some((s, p)) => (Some(s), Some(p)),
                                            None => (None, None),
                                        };
                                        let mut started_dnd = false;
                                        if let Some(ref dd) = self.data_device {
                                            self.drag_launched = true;
                                            let source = self.data_device_manager_state.create_drag_and_drop_source(
                                                qh,
                                                ["text/uri-list", "text/plain;charset=utf-8", "text/plain"],
                                                DndAction::Copy | DndAction::Move,
                                            );
                                            source.start_drag(dd, self.island_layer.wl_surface(), icon_surf.as_ref(), self.press_serial);
                                            self.active_drags.push(ActiveDrag {
                                                source,
                                                paths: drag_paths,
                                                uri_payload,
                                                plain_payload,
                                                _icon_surface: icon_surf,
                                                _icon_pool: icon_pool,
                                            });
                                            self.button_down = false;
                                            started_dnd = true;
                                        }
                                        if started_dnd {
                                            self.update_island_input_region();
                                            self.island_layer.wl_surface().commit();
                                        }
                                    }

                                    self.draw_island_surface();
                                } else if let Some((sx, sy)) = self.marquee_start {
                                    if let Some(l) = self.island_layout() {
                                        let lx0 = sx - (l.content_x + l.slide_x);
                                        let ly0 = sy - (l.content_y + l.slide_y);
                                        let lx1 = px - (l.content_x + l.slide_x);
                                        let ly1 = py - (l.content_y + l.slide_y);
                                        let rx = lx0.min(lx1);
                                        let ry = ly0.min(ly1);
                                        let rw = (lx1 - lx0).abs();
                                        let rh = (ly1 - ly0).abs();
                                        self.marquee_rect = Some((rx, ry, rw, rh));

                                        for hb in &self.content_hits {
                                            if let HitTarget::FilesRow(idx) = hb.target {
                                                if hb.x < rx + rw && hb.x + hb.w > rx && hb.y < ry + rh && hb.y + hb.h > ry {
                                                    if let Some(it) = self.entries.get(idx) {
                                                        if !self.selected_paths.contains(&it.path) {
                                                            self.selected_paths.push(it.path.clone());
                                                        }
                                                        if self.selected_idx.is_none() {
                                                            self.selected_idx = Some(idx);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        self.invalidate_content();
                                        self.draw_island_surface();
                                    }
                                }
                            }
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    let prev_hover = self.hover.take();
                    if is_bar {
                        self.bar_hovered = false;
                        if self.opened_by_hover && !self.island_card_hovered {
                            self.hover_close_at = Some(Instant::now() + Duration::from_millis(200));
                        }
                        self.sync_pills();
                        self.draw_bar_surface();
                    } else if is_island {
                        self.island_card_hovered = false;
                        if self.opened_by_hover && !self.bar_hovered {
                            self.hover_close_at = Some(Instant::now() + Duration::from_millis(200));
                        }
                        if !self.island_animating() && prev_hover.is_some() {
                            if self.rendered_popup == PopupKind::Settings
                                && matches!(prev_hover, Some(HitTarget::SettingsTab(_)) | Some(HitTarget::SettingsOpenPower))
                            {
                                self.content_dirty = true;
                            } else {
                                self.invalidate_content();
                            }
                            self.draw_island_surface();
                        }
                    }
                }
                PointerEventKind::Press { button, serial, .. } => {
                    if let Some(tgt) = hit {
                        self.opened_by_hover = false;
                        self.hover_close_at = None;
                        self.pressed_target = Some(tgt.clone());
                        self.handle_click(tgt, button, serial, px, py);
                    } else if is_island && self.active_popup != PopupKind::None {
                        if self.point_in_island_card(px, py) {
                            self.opened_by_hover = false;
                            self.hover_close_at = None;
                            if self.files_context_menu.is_some() {
                                self.files_context_menu = None;
                                self.invalidate_content();
                                self.draw_island_surface();
                            } else if !self.settings_open_dropdown.is_empty() {
                                self.settings_open_dropdown.clear();
                                self.invalidate_content();
                                self.draw_island_surface();
                            }
                        } else {
                            self.set_popup(PopupKind::None);
                        }
                    } else if !is_island && self.active_popup != PopupKind::None {
                        self.set_popup(PopupKind::None);
                    }
                }
                PointerEventKind::Release { .. } => {
                    self.button_down = false;
                    self.pressed_file_idx = None;
                    self.slider_drag = None;
                    let had_press = self.pressed_target.take().is_some();
                    if self.internal_dragging {
                        if self.files_qa_dnd_hover {
                            let mut added = 0usize;
                            let defs = default_places();
                            for p in &self.internal_drag_sources {
                                if p.is_dir() {
                                    let ds = p.to_string_lossy().to_string();
                                    if !defs.iter().any(|d| d.dir == ds)
                                        && !self.cfg.bookmarks.iter().any(|b| b.dir == ds)
                                    {
                                        let label = p
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("Folder")
                                            .to_string();
                                        self.cfg.bookmarks.push(Bookmark {
                                            label,
                                            dir: ds,
                                            icon: "󰉋".into(),
                                        });
                                        added += 1;
                                    }
                                }
                            }
                            if added > 0 {
                                save_bookmarks(&self.cfg.bookmarks);
                                self.set_toast(&format!("Added {added} folder(s) to Quick Access"));
                            } else {
                                self.set_toast("Drop a folder to pin to Quick Access");
                            }
                        } else if let Some(target_dir) = self.internal_drag_target.take() {
                            if !self.internal_drag_sources.is_empty() {
                                let is_trash = target_dir == Path::new(TRASH_DIR);
                                if is_trash {
                                    match run_trash(&self.internal_drag_sources) {
                                        Ok(_) => {
                                            self.set_toast(&format!(
                                                "Moved {} item(s) to Trash",
                                                self.internal_drag_sources.len()
                                            ));
                                        }
                                        Err(e) => self.set_toast(&format!("Trash error: {e}")),
                                    }
                                } else {
                                    let move_mode = !self.kbd_ctrl;
                                    match transfer_files(&self.internal_drag_sources, &target_dir, move_mode) {
                                        Ok(n) => {
                                            let dest_name = target_dir
                                                .file_name()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or("folder");
                                            self.set_toast(&format!(
                                                "{} {n} item(s) to {dest_name}",
                                                if move_mode { "Moved" } else { "Copied" }
                                            ));
                                        }
                                        Err(e) => self.set_toast(&format!("Transfer error: {e}")),
                                    }
                                }
                                self.selected_paths.clear();
                                self.selected_idx = None;
                                self.refresh_files();
                            }
                        }
                        self.internal_dragging = false;
                        self.files_qa_dnd_hover = false;
                        self.internal_drag_sources.clear();
                        self.internal_drag_target = None;
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if self.marquee_start.is_some() || self.marquee_rect.is_some() {
                        self.marquee_start = None;
                        self.marquee_rect = None;
                        self.update_files_preview();
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if had_press && is_island {
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let step = if vertical.discrete != 0 {
                        (vertical.discrete as f64) * 34.0
                    } else {
                        vertical.absolute * 1.2
                    };
                    if let Some(ref hb) = hit_box {
                        match hb.wheel {
                            WheelAction::Volume => {
                                let cur = self.stats.lock().map(|s| s.volume_percent as i32).unwrap_or(50);
                                let next = (cur + if step < 0.0 { 5 } else { -5 }).clamp(0, 150);
                                if let Ok(mut st) = self.stats.lock() { st.volume_percent = next as u32; }
                                set_volume(next);
                                self.draw_bar_surface();
                                if matches!(self.rendered_popup, PopupKind::Controls | PopupKind::Volume | PopupKind::Settings) {
                                    self.invalidate_content();
                                    self.draw_island_surface();
                                }
                                continue;
                            }
                            WheelAction::Brightness => {
                                let cur = self.stats.lock().map(|s| s.brightness_percent as i32).unwrap_or(60);
                                let next = (cur + if step < 0.0 { 5 } else { -5 }).clamp(5, 100);
                                if let Ok(mut st) = self.stats.lock() { st.brightness_percent = next as u32; }
                                set_brightness(next);
                                if matches!(self.rendered_popup, PopupKind::Controls | PopupKind::Brightness | PopupKind::Settings) {
                                    self.invalidate_content();
                                    self.draw_island_surface();
                                }
                                continue;
                            }
                            WheelAction::None => {}
                        }
                    }
                    if is_island && self.rendered_popup == PopupKind::Antigravity {
                        self.agy_scroll = (self.agy_scroll + step).max(0.0);
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if is_island && self.rendered_popup == PopupKind::Files {
                        let max_y = match self.files_view_mode {
                            FileViewMode::Grid => {
                                let rows = (self.entries.len() + 4) / 5;
                                ((rows as f64) * 110.0 - 330.0).max(0.0)
                            }
                            FileViewMode::List => {
                                ((self.entries.len() as f64) * 34.0 - 330.0).max(0.0)
                            }
                        };
                        self.scroll_y = (self.scroll_y + step).clamp(0.0, max_y);
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if is_island && self.rendered_popup == PopupKind::Launcher {
                        let rows = self.launcher_rows();
                        let max_y = ((rows as f64) * 72.0 - 216.0).max(0.0);
                        self.launcher_scroll = (self.launcher_scroll + step).clamp(0.0, max_y);
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if is_island
                        && (self.rendered_popup == PopupKind::Wifi
                            || (self.rendered_popup == PopupKind::Controls && self.cc_expanded == "wifi"))
                    {
                        let n = self.stats.lock().map(|s| s.wifi_networks.len()).unwrap_or(0);
                        let (stride, view_h) = if self.rendered_popup == PopupKind::Controls {
                            (36.0, 160.0)
                        } else {
                            (38.0, 202.0)
                        };
                        let max_y = ((n as f64) * stride - 4.0 - view_h).max(0.0);
                        self.wifi_scroll = (self.wifi_scroll + step).clamp(0.0, max_y);
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if is_island
                        && (self.rendered_popup == PopupKind::Bluetooth
                            || (self.rendered_popup == PopupKind::Controls && self.cc_expanded == "bt"))
                    {
                        let (np, na) = self
                            .stats
                            .lock()
                            .map(|s| (s.bt_paired.len(), s.bt_available.len()))
                            .unwrap_or((0, 0));
                        let (hdr_h, stride, view_h) = if self.rendered_popup == PopupKind::Controls {
                            (18.0, 36.0, 160.0)
                        } else {
                            (22.0, 38.0, 194.0)
                        };
                        let mut content_h = 0.0;
                        if np > 0 {
                            content_h += hdr_h + (np as f64) * stride;
                        }
                        if na > 0 {
                            content_h += hdr_h + 4.0 + (na as f64) * stride;
                        }
                        let max_y = (content_h - view_h).max(0.0);
                        self.bt_scroll = (self.bt_scroll + step).clamp(0.0, max_y);
                        self.invalidate_content();
                        self.draw_island_surface();
                    } else if is_island && self.rendered_popup == PopupKind::Notifications {
                        let n = self.stats.lock().map(|s| s.notifications.len()).unwrap_or(0);
                        let max_y = ((n as f64) * 64.0 - 320.0).max(0.0);
                        self.notif_scroll = (self.notif_scroll + step).clamp(0.0, max_y);
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                }
            }
        }
    }
}

impl YunoShell {
    fn is_repeatable_key(&self, ev: &KeyEvent) -> bool {
        if self.kbd_ctrl {
            return matches!(ev.keysym, Keysym::BackSpace);
        }
        matches!(
            ev.keysym,
            Keysym::BackSpace
                | Keysym::Delete
                | Keysym::Left
                | Keysym::Right
                | Keysym::Up
                | Keysym::Down
                | Keysym::Page_Up
                | Keysym::Page_Down
        ) || ev
            .utf8
            .as_ref()
            .is_some_and(|t| !t.is_empty() && t.chars().all(|c| !c.is_control()))
    }

    fn tick_key_repeat(&mut self) {
        if self.kbd_repeat_rate_hz == 0 {
            self.kbd_held_key = None;
            return;
        }
        let now = Instant::now();
        let interval = Duration::from_micros((1_000_000 / self.kbd_repeat_rate_hz.max(1)) as u64);
        let mut fired = 0_u32;
        loop {
            let Some((ev, next_at)) = self.kbd_held_key.clone() else { break };
            if now < next_at {
                break;
            }
            self.kbd_held_key = Some((ev.clone(), next_at + interval));
            self.handle_key_press_event(ev);
            fired += 1;
            if fired >= 4 || self.kbd_held_key.is_none() {
                if let Some((ev2, _)) = self.kbd_held_key.clone() {
                    self.kbd_held_key = Some((ev2, now + interval));
                }
                break;
            }
        }
    }

    fn handle_key_press_event(&mut self, ev: KeyEvent) {
        if ev.keysym == Keysym::Escape {
            if !self.wifi_selected_ssid.is_empty() {
                self.wifi_selected_ssid.clear();
                self.wifi_password.clear();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.island_layer.commit();
                if self.rendered_popup == PopupKind::Controls {
                    self.morph_island_inplace(190.0);
                } else {
                    self.invalidate_content();
                    self.draw_island_surface();
                }
            } else if self.rendered_popup == PopupKind::Files && self.files_context_menu.is_some() {
                self.files_context_menu = None;
                self.invalidate_content();
                self.draw_island_surface();
            } else if self.rendered_popup == PopupKind::Files && !matches!(self.files_prompt, FilesPrompt::None) {
                if matches!(self.files_prompt, FilesPrompt::SaveFilename)
                    && self.portal_req.is_some()
                {
                    self.cancel_portal();
                    return;
                }
                self.files_prompt = FilesPrompt::None;
                self.files_prompt_text.clear();
                self.restore_save_prompt_if_needed();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.sync_files_anims();
                self.invalidate_content();
                self.draw_island_surface();
            } else if self.rendered_popup == PopupKind::Files && (self.filter_focused || !self.filter_query.is_empty()) {
                self.filter_focused = false;
                self.filter_query.clear();
                self.island_layer
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.refresh_files();
                self.sync_files_anims();
                self.draw_island_surface();
            } else if self.rendered_popup == PopupKind::Launcher && !self.launcher_query.is_empty() {
                self.launcher_query.clear();
                self.refilter_apps();
                self.draw_island_surface();
            } else {
                self.set_popup(PopupKind::None);
            }
            return;
        }

        if !self.wifi_selected_ssid.is_empty() {
            if self.kbd_ctrl && matches!(ev.keysym, Keysym::v | Keysym::V) {
                if let Some(txt) = wl_paste_text() {
                    let clean: String = txt.chars().filter(|c| !c.is_control() || *c == ' ').collect();
                    if !clean.is_empty() {
                        self.wifi_password.push_str(&clean);
                        self.agy_cursor_on = true;
                        self.agy_blink_at = Instant::now();
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                }
                return;
            }
            if self.kbd_ctrl && matches!(ev.keysym, Keysym::u | Keysym::U) {
                self.wifi_password.clear();
                self.agy_cursor_on = true;
                self.agy_blink_at = Instant::now();
                self.invalidate_content();
                self.draw_island_surface();
                return;
            }
            match ev.keysym {
                Keysym::Return | Keysym::KP_Enter => {
                    if !self.wifi_password.is_empty() {
                        self.submit_wifi_password();
                    }
                }
                Keysym::BackSpace => {
                    self.wifi_password.pop();
                    self.agy_cursor_on = true;
                    self.agy_blink_at = Instant::now();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                _ => {
                    if let Some(txt) = ev.utf8 {
                        if !txt.is_empty() && txt.chars().all(|c| !c.is_control()) {
                            self.wifi_password.push_str(&txt);
                            self.agy_cursor_on = true;
                            self.agy_blink_at = Instant::now();
                            self.invalidate_content();
                            self.draw_island_surface();
                        }
                    }
                }
            }
            return;
        }

        if self.rendered_popup == PopupKind::Antigravity {
            match ev.keysym {
                Keysym::Return | Keysym::KP_Enter => {
                    if !self.agy_busy {
                        self.send_agy();
                    }
                }
                Keysym::BackSpace => self.agy_backspace(),
                Keysym::Up => {
                    self.agy_scroll += 40.0;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Down => {
                    self.agy_scroll = (self.agy_scroll - 40.0).max(0.0);
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                _ => {
                    if let Some(txt) = ev.utf8 {
                        if !txt.is_empty() && txt.chars().all(|c| !c.is_control()) {
                            self.agy_input.push_str(&txt);
                            self.agy_cursor_on = true;
                            self.agy_blink_at = Instant::now();
                            self.invalidate_content();
                            self.draw_island_surface();
                        }
                    }
                }
            }
            return;
        }

        if self.rendered_popup == PopupKind::Launcher {
            match ev.keysym {
                Keysym::Right | Keysym::Tab if self.launcher_idx + 1 < self.filtered_apps.len() => {
                    self.launcher_idx += 1;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Left if self.launcher_idx > 0 => {
                    self.launcher_idx -= 1;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Down if self.launcher_idx + 8 < self.filtered_apps.len() => {
                    self.launcher_idx += 8;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Up if self.launcher_idx >= 8 => {
                    self.launcher_idx -= 8;
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Return | Keysym::KP_Enter => {
                    if let Some(app) = self.filtered_apps.get(self.launcher_idx).cloned() {
                        self.set_popup(PopupKind::None);
                        launch_app(&app);
                    }
                }
                Keysym::BackSpace => {
                    self.launcher_query.pop();
                    self.refilter_apps();
                    self.draw_island_surface();
                }
                _ => {
                    if let Some(txt) = ev.utf8 {
                        if txt.chars().all(|c| !c.is_control()) {
                            self.launcher_query.push_str(&txt);
                            self.refilter_apps();
                            self.draw_island_surface();
                        }
                    }
                }
            }
        } else if self.rendered_popup == PopupKind::Files {
            if !self.filter_focused && !matches!(self.files_prompt, FilesPrompt::None) {
                match ev.keysym {
                    Keysym::Return | Keysym::KP_Enter => {
                        self.submit_files_prompt();
                    }
                    Keysym::Escape => {
                        if matches!(self.files_prompt, FilesPrompt::SaveFilename)
                            && self.portal_req.is_some()
                        {
                            self.cancel_portal();
                        } else {
                            self.files_prompt = FilesPrompt::None;
                            self.files_prompt_text.clear();
                            self.invalidate_content();
                            self.draw_island_surface();
                        }
                    }
                    Keysym::BackSpace => {
                        if self.kbd_ctrl {
                            while self.files_prompt_text.ends_with(|c: char| !c.is_alphanumeric()) {
                                if self.files_prompt_text.pop().is_none() {
                                    break;
                                }
                            }
                            while self.files_prompt_text.ends_with(|c: char| c.is_alphanumeric()) {
                                self.files_prompt_text.pop();
                            }
                        } else {
                            self.files_prompt_text.pop();
                        }
                        if matches!(self.files_prompt, FilesPrompt::SaveFilename) {
                            self.portal_save_name = self.files_prompt_text.clone();
                        }
                        self.agy_cursor_on = true;
                        self.agy_blink_at = Instant::now();
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                    Keysym::u | Keysym::U if self.kbd_ctrl => {
                        self.files_prompt_text.clear();
                        if matches!(self.files_prompt, FilesPrompt::SaveFilename) {
                            self.portal_save_name.clear();
                        }
                        self.agy_cursor_on = true;
                        self.agy_blink_at = Instant::now();
                        self.invalidate_content();
                        self.draw_island_surface();
                    }
                    _ => {
                        if let Some(txt) = ev.utf8 {
                            if !txt.is_empty() && txt.chars().all(|c| !c.is_control()) {
                                self.files_prompt_text.push_str(&txt);
                                if matches!(self.files_prompt, FilesPrompt::SaveFilename) {
                                    self.portal_save_name = self.files_prompt_text.clone();
                                }
                                self.agy_cursor_on = true;
                                self.agy_blink_at = Instant::now();
                                self.invalidate_content();
                                self.draw_island_surface();
                            }
                        }
                    }
                }
                return;
            }
            if self.filter_focused {
                if ev.keysym == Keysym::BackSpace {
                    self.filter_query.pop();
                    self.refresh_files();
                    self.draw_island_surface();
                } else if ev.keysym == Keysym::Return || ev.keysym == Keysym::KP_Enter {
                    self.filter_focused = false;
                    self.island_layer
                        .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    self.invalidate_content();
                    self.draw_island_surface();
                } else if let Some(txt) = ev.utf8 {
                    if txt.chars().all(|c| !c.is_control()) {
                        self.filter_query.push_str(&txt);
                        self.refresh_files();
                        self.draw_island_surface();
                    }
                }
                return;
            }
            if self.kbd_ctrl {
                match ev.keysym {
                    Keysym::_1 => {
                        self.files_view_mode = FileViewMode::Grid;
                        self.scroll_y = 0.0;
                        self.invalidate_content();
                        self.draw_island_surface();
                        return;
                    }
                    Keysym::_2 => {
                        self.files_view_mode = FileViewMode::List;
                        self.scroll_y = 0.0;
                        self.invalidate_content();
                        self.draw_island_surface();
                        return;
                    }
                    Keysym::c | Keysym::C => {
                        if self.kbd_shift {
                            self.execute_files_action("copy");
                        } else {
                            self.execute_files_action("copy_files");
                        }
                        return;
                    }
                    Keysym::x | Keysym::X => {
                        self.execute_files_action("cut_files");
                        return;
                    }
                    Keysym::v | Keysym::V => {
                        self.execute_files_action("paste_files");
                        return;
                    }
                    Keysym::d | Keysym::D => {
                        self.execute_files_action("duplicate");
                        return;
                    }
                    Keysym::a | Keysym::A => {
                        self.selected_paths = self.entries.iter().map(|e| e.path.clone()).collect();
                        if !self.entries.is_empty() && self.selected_idx.is_none() {
                            self.selected_idx = Some(0);
                        }
                        self.set_toast(&format!("Selected {} items", self.selected_paths.len()));
                        self.sync_files_anims();
                        self.invalidate_content();
                        self.draw_island_surface();
                        return;
                    }
                    Keysym::h | Keysym::H => {
                        self.show_hidden = !self.show_hidden;
                        self.refresh_files();
                        self.draw_island_surface();
                        return;
                    }
                    Keysym::l | Keysym::L => {
                        let cur = self.current_dir.to_string_lossy().to_string();
                        self.open_files_prompt(FilesPrompt::PathInput, cur);
                        return;
                    }
                    Keysym::f | Keysym::F => {
                        self.filter_focused = true;
                        self.island_layer
                            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                        self.sync_files_anims();
                        self.invalidate_content();
                        self.draw_island_surface();
                        return;
                    }
                    Keysym::n | Keysym::N => {
                        if self.kbd_shift {
                            self.open_files_prompt(FilesPrompt::Mkdir, String::new());
                        } else {
                            self.open_files_prompt(FilesPrompt::Mkfile, String::new());
                        }
                        return;
                    }
                    Keysym::r | Keysym::R => {
                        self.execute_files_action("refresh");
                        return;
                    }
                    _ => {}
                }
            }
            match ev.keysym {
                Keysym::Down | Keysym::j if !self.entries.is_empty() => {
                    let next = self.selected_idx.map(|i| (i + 1).min(self.entries.len() - 1)).unwrap_or(0);
                    self.selected_idx = Some(next);
                    if let Some(e) = self.entries.get(next) {
                        self.selected_paths = vec![e.path.clone()];
                    }
                    self.update_files_preview();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Up | Keysym::k if !self.entries.is_empty() => {
                    let prev = self.selected_idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                    self.selected_idx = Some(prev);
                    if let Some(e) = self.entries.get(prev) {
                        self.selected_paths = vec![e.path.clone()];
                    }
                    self.update_files_preview();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Left | Keysym::h if !self.entries.is_empty() && self.files_view_mode == FileViewMode::Grid => {
                    let prev = self.selected_idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                    self.selected_idx = Some(prev);
                    if let Some(e) = self.entries.get(prev) {
                        self.selected_paths = vec![e.path.clone()];
                    }
                    self.update_files_preview();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Right | Keysym::l if !self.entries.is_empty() && self.files_view_mode == FileViewMode::Grid => {
                    let next = self.selected_idx.map(|i| (i + 1).min(self.entries.len().saturating_sub(1))).unwrap_or(0);
                    self.selected_idx = Some(next);
                    if let Some(e) = self.entries.get(next) {
                        self.selected_paths = vec![e.path.clone()];
                    }
                    self.update_files_preview();
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::F2 => {
                    self.execute_files_action("rename");
                }
                Keysym::F4 => {
                    self.execute_files_action("term");
                }
                Keysym::F5 => {
                    self.execute_files_action("refresh");
                }
                Keysym::Escape => {
                    if !matches!(self.files_prompt, FilesPrompt::None) {
                        self.files_prompt = FilesPrompt::None;
                        self.files_prompt_text.clear();
                        self.island_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    } else if self.filter_focused || !self.filter_query.is_empty() {
                        self.filter_focused = false;
                        self.filter_query.clear();
                        self.refresh_files();
                        self.island_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    } else if !self.selected_paths.is_empty() {
                        self.selected_paths.clear();
                        self.selected_idx = None;
                        self.update_files_preview();
                    }
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::Delete => {
                    if self.kbd_shift {
                        self.execute_files_action("delete_perm");
                    } else {
                        self.execute_files_action("trash");
                    }
                }
                Keysym::Return | Keysym::KP_Enter => {
                    if let Some(idx) = self.selected_idx {
                        self.activate_file_item(idx);
                    }
                }
                Keysym::BackSpace => {
                    if let Some(p) = self.current_dir.parent().map(|x| x.to_path_buf()) {
                        self.navigate_files(p, true);
                        self.draw_island_surface();
                    }
                }
                Keysym::slash => {
                    self.filter_focused = true;
                    self.island_layer
                        .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    self.invalidate_content();
                    self.draw_island_surface();
                }
                Keysym::period => {
                    self.show_hidden = !self.show_hidden;
                    self.refresh_files();
                    self.draw_island_surface();
                }
                _ => {}
            }
        }
    }
}

impl KeyboardHandler for YunoShell {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {
        self.kbd_held_key = None;
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        if self.kbd_repeat_rate_hz > 0 && self.is_repeatable_key(&ev) {
            let delay = Duration::from_millis(self.kbd_repeat_delay_ms as u64);
            self.kbd_held_key = Some((ev.clone(), Instant::now() + delay));
        } else {
            self.kbd_held_key = None;
        }
        self.handle_key_press_event(ev);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        if self.kbd_held_key.as_ref().is_some_and(|(held, _)| held.raw_code == ev.raw_code) {
            self.kbd_held_key = None;
        }
    }
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, mods: Modifiers, _: u32) {
        self.kbd_ctrl = mods.ctrl;
        self.kbd_shift = mods.shift;
    }
    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, info: RepeatInfo) {
        match info {
            RepeatInfo::Repeat { rate, delay } => {
                self.kbd_repeat_rate_hz = rate.get().max(1);
                self.kbd_repeat_delay_ms = delay.max(120);
            }
            RepeatInfo::Disable => {
                self.kbd_repeat_rate_hz = 0;
                self.kbd_held_key = None;
            }
        }
    }
}

impl DataDeviceHandler for YunoShell {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64, _: &wl_surface::WlSurface) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
}

impl DataOfferHandler for YunoShell {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl DataSourceHandler for YunoShell {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource, mime: String, write_pipe: WritePipe) {
        if let Some(drag) = self.active_drags.iter().find(|d| d.source.inner() == source) {
            let fd = OwnedFd::from(write_pipe);
            let mut f = File::from(fd);
            let payload = if mime == "text/uri-list" { drag.uri_payload.as_bytes() } else { drag.plain_payload.as_bytes() };
            let _ = f.write_all(payload);
        }
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        let mut dropped_paths: Vec<PathBuf> = Vec::new();
        if let Some(drag) = self.active_drags.iter().find(|d| d.source.inner() == source) {
            dropped_paths = drag.paths.clone();
        }
        self.active_drags.retain(|d| d.source.inner() != source);
        source.destroy();
        self.drag_launched = false;
        self.internal_dragging = false;
        self.internal_drag_sources.clear();
        self.internal_drag_target = None;
        self.update_island_input_region();
        if !dropped_paths.is_empty() && cursor_is_over_zalo_window() && write_zalo_drop_handoff(&dropped_paths) {
            self.set_toast("Sent file(s) to Zalo");
            self.set_popup(PopupKind::None);
        } else {
            self.set_toast("Drag ended");
            self.invalidate_content();
            self.draw_island_surface();
        }
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        self.drag_launched = false;
        self.internal_dragging = false;
        self.internal_drag_sources.clear();
        self.internal_drag_target = None;
        self.update_island_input_region();
        self.set_toast("Dropped!");
        self.invalidate_content();
        self.draw_island_surface();
    }
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        self.active_drags.retain(|d| d.source.inner() != source);
        source.destroy();
        self.drag_launched = false;
        self.internal_dragging = false;
        self.internal_drag_sources.clear();
        self.internal_drag_target = None;
        self.update_island_input_region();
        self.set_toast("Transfer complete");
        self.invalidate_content();
        self.draw_island_surface();
    }
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl ProvidesRegistryState for YunoShell {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(YunoShell);
delegate_output!(YunoShell);
delegate_layer!(YunoShell);
delegate_shm!(YunoShell);
delegate_seat!(YunoShell);
delegate_pointer!(YunoShell);
delegate_keyboard!(YunoShell);
delegate_data_device!(YunoShell);
delegate_registry!(YunoShell);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("ctl") {
        let mut res = send_cli_command(&args[1..]);
        if res.is_err() {
            spawn_detached("/home/huyle/.local/bin/yunoshell", &[]);
            for _ in 0..12 {
                std::thread::sleep(Duration::from_millis(40));
                if let Ok(r) = send_cli_command(&args[1..]) {
                    res = Ok(r);
                    break;
                }
            }
        }
        match res {
            Ok(resp) => {
                if !resp.is_empty() {
                    println!("{resp}");
                }
            }
            Err(e) => {
                let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("");
                let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
                if matches!(cmd, "vol" | "volume") {
                    if sub == "mute" || sub == "toggle-mute" {
                        let _ = std::process::Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).status();
                    } else if sub == "mic-mute" {
                        let _ = std::process::Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]).status();
                    } else if sub.starts_with('+') || sub == "up" {
                        let val = if sub == "up" { "5%+" } else { sub };
                        let _ = std::process::Command::new("wpctl").args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", val]).status();
                    } else if sub.starts_with('-') || sub == "down" {
                        let val = if sub == "down" { "5%-" } else { sub };
                        let _ = std::process::Command::new("wpctl").args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", val]).status();
                    }
                    return;
                } else if matches!(cmd, "bri" | "bright" | "brightness") {
                    if sub.starts_with('+') || sub == "up" {
                        let val = if sub == "up" { "5%+" } else { sub };
                        let _ = std::process::Command::new("brightnessctl").args(["set", val]).status();
                    } else if sub.starts_with('-') || sub == "down" {
                        let val = if sub == "down" { "5%-" } else { sub };
                        let _ = std::process::Command::new("brightnessctl").args(["set", val]).status();
                    }
                    return;
                }
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let no_bar = args.iter().any(|a| a == "--no-bar");
    let no_gpu = args.iter().any(|a| a == "--no-gpu");
    let start_open = args
        .windows(2)
        .find(|w| w[0] == "--open")
        .map(|w| PopupKind::from_name(&w[1]))
        .unwrap_or(PopupKind::None);
    let start_dir = args
        .windows(2)
        .find(|w| w[0] == "--dir")
        .map(|w| PathBuf::from(&w[1]))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("/home/huyle"));

    ensure_user_config_exists();
    let cfg = load_config();
    let cfg_mtimes = config_mtimes();
    let theme = theme_from_config(&cfg);
    let ipc_rx = start_ipc_server();
    let stats = Arc::new(Mutex::new(SysStats::default()));
    spawn_background_services(stats.clone());

    let conn = Connection::connect_to_env().expect("connect wayland");
    let (globals, event_queue) = registry_queue_init(&conn).expect("registry");
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<YunoShell> = EventLoop::try_new().expect("event loop");
    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .expect("wayland source");

    let compositor_state = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("zwlr_layer_shell_v1");
    let shm_state = Shm::bind(&globals, &qh).expect("wl_shm");
    let data_device_manager_state =
        DataDeviceManagerState::bind(&globals, &qh).expect("wl_data_device_manager");

    let island_surf = compositor_state.create_surface(&qh);
    let island_layer = layer_shell.create_layer_surface(&qh, island_surf, Layer::Overlay, Some("yuno-island"), None);
    island_layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    island_layer.set_size(0, 0);
    island_layer.set_exclusive_zone(-1);
    island_layer.set_keyboard_interactivity(if start_open == PopupKind::None {
        KeyboardInteractivity::None
    } else {
        KeyboardInteractivity::OnDemand
    });
    island_layer.commit();

    let bar_layer = if no_bar {
        None
    } else {
        let surf = compositor_state.create_surface(&qh);
        Some(layer_shell.create_layer_surface(&qh, surf, Layer::Overlay, Some("yuno-bar"), None))
    };
    let bar_pool = if bar_layer.is_some() {
        Some(SlotPool::new(1920 * 64 * 4 * 3, &shm_state).expect("bar pool"))
    } else {
        None
    };

    let island_pool = SlotPool::new((1920 * 1200 * 4) as usize * 3, &shm_state).expect("island pool");
    let gpu_display = conn.backend().display_ptr() as *mut c_void;
    let gpu_surface = island_layer.wl_surface().id().as_ptr() as *mut c_void;
    let yuno_icon = load_png_surface(Path::new("/home/huyle/.config/quickshell/assets/yuno-gasai-icon.png"));
    let all_apps = scan_desktop_apps();
    let initial_entries = list_dir_entries_sorted(&start_dir, false, "", SortBy::Name, true);
    let initial_sz = {
        let s = stats.lock().map(|s| s.clone()).unwrap_or_default();
        start_open.target_size(((all_apps.len().max(1)) + 7) / 8, &s, false, "")
    };
    let (files_thumb_tx, files_thumb_rx) = std::sync::mpsc::channel();

    let mut app = YunoShell {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor_state,
        layer_shell,
        shm_state,
        data_device_manager_state,
        qh,
        bar_layer,
        bar_pool,
        bar_w: 0,
        bar_h: 0,
        bar_configured: false,
        bar_hits: Vec::new(),
        yuno_icon,
        tray_icons: HashMap::new(),
        island_layer,
        island_pool,
        island_configured: false,
        island_w: 0,
        island_h: 0,
        island_hits: Vec::new(),
        content_hits: Vec::new(),
        content_cache: None,
        content_dirty: start_open != PopupKind::None,
        cache_gen: 0,
        settings_body_cache: None,
        settings_body_hits: Vec::new(),
        settings_body_dirty: true,
        prev_island_rect: (0.0, 0.0, 0.0, 0.0),
        cur_body_rect: (0.0, 0.0, 0.0, 0.0),
        gpu: None,
        gpu_failed: no_gpu,
        gpu_image: None,
        gpu_display,
        gpu_surface,
        island_frame_pending: false,
        anim_sync_pending: false,
        seat: None,
        data_device: None,
        pointer: None,
        keyboard: None,
        cfg,
        cfg_last_check: Instant::now(),
        cfg_mtimes,
        app_dirs_mtimes: app_dirs_mtimes(),
        apps_last_check: Instant::now(),
        theme,
        stats,
        ipc_rx,
        exit: false,
        active_popup: start_open,
        rendered_popup: start_open,
        m_width: Motion::new(if start_open == PopupKind::None { 0.0 } else { 1.0 }, if start_open == PopupKind::None { 0.0 } else { 1.0 }, 1.0, Ease::Linear),
        m_height: Motion::new(if start_open == PopupKind::None { 0.0 } else { 1.0 }, if start_open == PopupKind::None { 0.0 } else { 1.0 }, 1.0, Ease::Linear),
        m_content: Motion::new(if start_open == PopupKind::None { 0.0 } else { 1.0 }, if start_open == PopupKind::None { 0.0 } else { 1.0 }, 1.0, Ease::Linear),
        m_content_scale: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        m_morph: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        m_fade: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        morph_from_size: initial_sz,
        morph_from_pos: (0.0, 0.0),
        closing: false,
        hover: None,
        last_click_time: Instant::now(),
        last_click_idx: None,
        current_dir: start_dir,
        back_stack: Vec::new(),
        fwd_stack: Vec::new(),
        show_hidden: false,
        filter_query: String::new(),
        filter_focused: false,
        entries: initial_entries,
        selected_idx: None,
        selected_paths: Vec::new(),
        files_sort_by: SortBy::Name,
        files_sort_asc: true,
        files_view_mode: FileViewMode::List,
        files_prompt: FilesPrompt::None,
        files_prompt_text: String::new(),
        files_clipboard: Vec::new(),
        files_clip_mode: ClipMode::Copy,
        files_preview_path: PathBuf::new(),
        files_preview_surf: None,
        files_preview_lines: Vec::new(),
        files_preview_dir_count: None,
        files_context_menu: None,
        internal_dragging: false,
        internal_drag_sources: Vec::new(),
        internal_drag_target: None,
        internal_drag_pos: (0.0, 0.0),
        files_qa_dnd_hover: false,
        drag_pill_cache: None,
        drag_pill_gen: 0,
        drag_pill_key: (String::new(), 0),
        marquee_start: None,
        marquee_rect: None,
        drive_usages: read_drive_usages(),
        last_drive_check: Instant::now(),
        files_thumbs: HashMap::new(),
        files_thumb_tx,
        files_thumb_rx,
        files_search_anim: Motion::new(0.0, 0.0, 1.0, Ease::Linear),
        files_crumb_anim: Motion::new(0.0, 0.0, 1.0, Ease::Linear),
        files_sidebar_pill_y: Motion::new(8.0, 8.0, 1.0, Ease::Linear),
        files_sidebar_pill_r: Motion::new(10.0, 10.0, 1.0, Ease::Linear),
        files_ctx_anim: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        files_action_bar_anim: Motion::new(0.0, 0.0, 1.0, Ease::Linear),
        pressed_target: None,
        kbd_ctrl: false,
        kbd_shift: false,
        kbd_repeat_rate_hz: 28,
        kbd_repeat_delay_ms: 350,
        kbd_held_key: None,
        scroll_y: 0.0,
        status_toast: String::new(),
        toast_until: None,
        button_down: false,
        press_pos: (0.0, 0.0),
        press_serial: 0,
        pressed_file_idx: None,
        slider_drag: None,
        drag_launched: false,
        active_drags: Vec::new(),
        filtered_apps: all_apps.clone(),
        all_apps,
        launcher_query: String::new(),
        launcher_idx: 0,
        launcher_scroll: 0.0,
        settings_tab: 0,
        settings_prev_tab: 0,
        settings_tab_anim: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        settings_tab_slide_x: 0.0,
        settings_bg_target: "theme".to_string(),
        settings_open_dropdown: String::new(),
        wallpapers: Vec::new(),
        current_preview_path: String::new(),
        current_preview_surf: None,
        wallpaper_prev_surf: None,
        wallpaper_preview_anim: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        wipe_layer: None,
        wipe_gpu: None,
        wipe_w: 0,
        wipe_h: 0,
        wipe_configured: false,
        wipe_rgb: None,
        wipe_pending: None,
        wipe_anim: Motion::new(1.0, 1.0, 1.0, Ease::Linear),
        wipe_frame_pending: false,
        thumb_rx: None,
        mango_info: read_mango_summary(),
        cal_offset_months: 0,
        cal_selected_key: -1,
        last_clock_sec: 0,
        agy_messages: Vec::new(),
        agy_input: String::new(),
        agy_busy: false,
        agy_status: String::new(),
        agy_meta: String::new(),
        agy_conv: String::new(),
        agy_scroll: 0.0,
        agy_rx: None,
        agy_model_idx: 0,
        agy_effort_idx: 1,
        agy_cursor_on: true,
        agy_blink_at: Instant::now(),
        opened_by_hover: false,
        bar_hovered: false,
        island_card_hovered: false,
        hover_close_at: None,
        last_input_region: None,
        wifi_selected_ssid: String::new(),
        wifi_password: String::new(),
        wifi_scroll: 0.0,
        bt_scroll: 0.0,
        notif_scroll: 0.0,
        cc_expanded: "",
        media_art_url: String::new(),
        media_art_surf: None,
        portal_req: None,
        portal_save_name: String::new(),
        pill_anims: Vec::new(),
        bar_frame_pending: false,
        started: Instant::now(),
    };

    app.refresh_files();
    app.sync_files_anims();
    app.sync_bar_layout();
    app.sync_pills();

    while !app.exit {
        let timeout = if app.island_animating()
            || app.bar_needs_anim()
            || app.wipe_layer.is_some()
            || app.kbd_held_key.is_some()
        {
            Duration::from_millis(4)
        } else {
            Duration::from_millis(16)
        };
        let _ = event_loop.dispatch(Some(timeout), &mut app);
        app.process_ipc_and_stats();
    }
}
