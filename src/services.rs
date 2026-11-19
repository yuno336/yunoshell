use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::theme::{read_settings_value, write_settings_value, Bookmark, Rgb, Theme};

#[derive(Clone, Debug)]
pub struct TagInfo {
    pub index: u32,
    pub is_active: bool,
    pub is_urgent: bool,
    pub client_count: u32,
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub active: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: String,
    pub player: String,
    pub bus: String,
    pub length: f64,
    pub position: f64,
}

impl Default for MediaInfo {
    fn default() -> Self {
        Self {
            active: false,
            playing: false,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: String::new(),
            player: String::new(),
            bus: String::new(),
            length: 0.0,
            position: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AudioOutput {
    pub active: bool,
    pub sink: String,
    pub port: String,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct WifiNetwork {
    pub active: bool,
    pub saved: bool,
    pub signal: u32,
    pub security: String,
    pub ssid: String,
}

#[derive(Clone, Debug, Default)]
pub struct BtDevice {
    pub mac: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NotifItem {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub critical: bool,
    pub time: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct MonitorInfo {
    pub name: String,
    pub active: bool,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub x: i32,
    pub y: i32,
    pub transform: String,
    pub layout_symbol: String,
    pub tag_num: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayItem {
    pub key: String,
    pub service: String,
    pub path: String,
    pub id: String,
    pub title: String,
    pub tooltip: String,
    pub icon_name: String,
    pub icon_png: String,
    pub menu_path: String,
    pub item_is_menu: bool,
    pub status: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayMenuItem {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub sep: bool,
    pub checked: bool,
}

#[derive(Clone, Debug)]
pub struct SysStats {
    pub tags: Vec<TagInfo>,
    pub ram_percent: u32,
    pub ram_used_gb: f64,
    pub ram_total_gb: f64,
    pub volume_percent: u32,
    pub volume_muted: bool,
    pub brightness_percent: u32,
    pub battery_percent: u32,
    pub battery_charging: bool,
    pub battery_present: bool,
    pub battery_status: String,
    pub battery_detail: String,
    pub power_profile: String,
    pub cpu_temp: String,
    pub cpu_freq_ghz: String,
    pub wifi_enabled: bool,
    pub wifi_ssid: String,
    pub wifi_signal: u32,
    pub wifi_networks: Vec<WifiNetwork>,
    pub wifi_scanning: bool,
    pub wifi_status_msg: String,
    pub bt_powered: bool,
    pub bt_connected: u32,
    pub bt_scanning: bool,
    pub bt_status_msg: String,
    pub bt_paired: Vec<BtDevice>,
    pub bt_available: Vec<BtDevice>,
    pub media: MediaInfo,
    pub media_players: Vec<MediaInfo>,
    pub preferred_player: String,
    pub audio_outputs: Vec<AudioOutput>,
    pub notifications: Vec<NotifItem>,
    pub dnd_enabled: bool,
    pub monitors: Vec<MonitorInfo>,
    pub cpu_percent: u32,
    pub cpu_cores: u32,
    pub cpu_model: String,
    pub cpu_history: Vec<u32>,
    pub ram_history: Vec<u32>,
    pub swap_used_gb: f64,
    pub swap_total_gb: f64,
    pub disk_percent: u32,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub net_down_kbps: f64,
    pub net_up_kbps: f64,
    pub sys_uptime: String,
    pub sys_process_count: u32,
    pub sys_load_avg: String,
    pub tray_items: Vec<TrayItem>,
    pub tray_menu_key: String,
    pub tray_menu_title: String,
    pub tray_menu_items: Vec<TrayMenuItem>,
    pub dirty: bool,
}

impl Default for SysStats {
    fn default() -> Self {
        let (cores, model) = read_cpu_info_once();
        let (disk_pct, disk_u, disk_t) = read_disk_root();
        let (uptime, procs, load) = read_uptime_and_load();
        Self {
            tags: (1..=9)
                .map(|i| TagInfo {
                    index: i,
                    is_active: i == 1,
                    is_urgent: false,
                    client_count: 0,
                })
                .collect(),
            ram_percent: 0,
            ram_used_gb: 0.0,
            ram_total_gb: 8.0,
            volume_percent: 50,
            volume_muted: false,
            brightness_percent: 60,
            battery_percent: 100,
            battery_charging: false,
            battery_present: false,
            battery_status: "Discharging".into(),
            battery_detail: String::new(),
            power_profile: "balance_power".into(),
            cpu_temp: "40°C".into(),
            cpu_freq_ghz: "0.8 GHz".into(),
            wifi_enabled: false,
            wifi_ssid: String::new(),
            wifi_signal: 0,
            wifi_networks: Vec::new(),
            wifi_scanning: false,
            wifi_status_msg: String::new(),
            bt_powered: false,
            bt_connected: 0,
            bt_scanning: false,
            bt_status_msg: String::new(),
            bt_paired: Vec::new(),
            bt_available: Vec::new(),
            media: MediaInfo::default(),
            media_players: Vec::new(),
            preferred_player: String::new(),
            audio_outputs: Vec::new(),
            notifications: Vec::new(),
            dnd_enabled: false,
            monitors: Vec::new(),
            cpu_percent: 4,
            cpu_cores: cores,
            cpu_model: model,
            cpu_history: vec![4; 24],
            ram_history: vec![30; 24],
            swap_used_gb: 0.0,
            swap_total_gb: 0.0,
            disk_percent: disk_pct,
            disk_used_gb: disk_u,
            disk_total_gb: disk_t,
            net_down_kbps: 0.0,
            net_up_kbps: 0.0,
            sys_uptime: uptime,
            sys_process_count: procs,
            sys_load_avg: load,
            tray_items: Vec::new(),
            tray_menu_key: String::new(),
            tray_menu_title: String::new(),
            tray_menu_items: Vec::new(),
            dirty: true,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AppItem {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub icon_surface: Option<cairo::ImageSurface>,
    pub terminal: bool,
}

pub const TRASH_DIR: &str = "/home/huyle/.local/share/Trash/files";
pub const TRASH_INFO_DIR: &str = "/home/huyle/.local/share/Trash/info";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipMode {
    Copy,
    Cut,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesPrompt {
    None,
    PathInput,
    Mkdir,
    Mkfile,
    SaveFilename,
    Rename(PathBuf),
    ConfirmDelete(Vec<PathBuf>),
    ConfirmEmptyTrash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileViewMode {
    Grid,
    List,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DriveUsage {
    pub label: String,
    pub mount: PathBuf,
    pub used_str: String,
    pub total_str: String,
    pub free_str: String,
    pub percent: f64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct FileItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub size: u64,
    pub size_label: String,
    pub modified_secs: i64,
    pub modified_label: String,
    pub icon: &'static str,
    pub kind_label: &'static str,
    pub mode_str: String,
}

pub fn default_places() -> Vec<Bookmark> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/huyle".into());
    vec![
        Bookmark { label: "Home".into(), dir: home.clone(), icon: "󰋜".into() },
        Bookmark { label: "Downloads".into(), dir: format!("{home}/Downloads"), icon: "󰉍".into() },
        Bookmark { label: "Documents".into(), dir: format!("{home}/Documents"), icon: "󰈙".into() },
        Bookmark { label: "Pictures".into(), dir: format!("{home}/Pictures"), icon: "󰋩".into() },
        Bookmark { label: "Trash".into(), dir: TRASH_DIR.into(), icon: "󰩺".into() },
    ]
}

pub fn file_kind_label(name: &str, is_dir: bool, is_symlink: bool) -> &'static str {
    if is_symlink {
        return "Symlink";
    }
    if is_dir {
        return "Folder";
    }
    let lower = name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "avif" => "Image",
        "mp4" | "mkv" | "webm" | "mov" | "avi" => "Video",
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" => "Audio",
        "zip" | "tar" | "gz" | "xz" | "7z" | "bz2" | "rar" => "Archive",
        "rs" | "cpp" | "c" | "h" | "hpp" | "py" | "js" | "ts" | "go" | "sh" | "json" | "toml"
        | "yaml" | "yml" | "html" | "css" => "Code",
        "pdf" | "md" | "txt" | "doc" | "docx" | "odt" => "Document",
        "iso" | "img" => "Disk Image",
        _ => "File",
    }
}

pub fn format_permissions(mode: u32) -> String {
    let rwx = |bits: u32| {
        let r = if bits & 4 != 0 { 'r' } else { '-' };
        let w = if bits & 2 != 0 { 'w' } else { '-' };
        let x = if bits & 1 != 0 { 'x' } else { '-' };
        [r, w, x]
    };
    let u = rwx((mode >> 6) & 7);
    let g = rwx((mode >> 3) & 7);
    let o = rwx(mode & 7);
    format!(
        "{}{}{}",
        u.iter().collect::<String>(),
        g.iter().collect::<String>(),
        o.iter().collect::<String>()
    )
}

pub fn read_drive_usages() -> Vec<DriveUsage> {
    let Ok(o) = Command::new("df").args(["-k", "/", "/home"]).output() else {
        return Vec::new();
    };
    let s = String::from_utf8_lossy(&o.stdout);
    let mut out = Vec::new();
    for line in s.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 6 {
            let total_kb: f64 = cols[1].parse().unwrap_or(0.0);
            let used_kb: f64 = cols[2].parse().unwrap_or(0.0);
            let free_kb: f64 = cols[3].parse().unwrap_or(0.0);
            let mount = cols[5].to_string();
            let pct = if total_kb > 0.0 {
                (used_kb / total_kb).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let label = if mount == "/" {
                "Root (/)".to_string()
            } else if mount == "/home" {
                "Home (~)".to_string()
            } else {
                mount.clone()
            };
            out.push(DriveUsage {
                label,
                mount: PathBuf::from(mount),
                used_str: format_size((used_kb * 1024.0) as u64),
                total_str: format_size((total_kb * 1024.0) as u64),
                free_str: format_size((free_kb * 1024.0) as u64),
                percent: pct,
            });
        }
    }
    out
}

pub fn spawn_detached(prog: &str, args: &[&str]) {
    let _ = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

pub fn switch_tag(idx: u32) {
    spawn_detached("mmsg", &["dispatch", &format!("view,{idx},0")]);
}

pub fn set_volume(pct: i32) {
    let clamped = pct.clamp(0, 150);
    spawn_detached("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &format!("{clamped}%")]);
}

pub fn toggle_mute() {
    spawn_detached("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"]);
}

pub fn set_brightness(pct: i32) {
    let clamped = pct.clamp(5, 100);
    spawn_detached("brightnessctl", &["set", &format!("{clamped}%")]);
}

pub fn power_action(act: &str) {
    match act {
        "lock" => spawn_detached(
            "sh",
            &[
                "-c",
                "loginctl lock-session >/dev/null 2>&1 || swaylock -f >/dev/null 2>&1 || true",
            ],
        ),
        "suspend" => spawn_detached("systemctl", &["suspend"]),
        "logout" => spawn_detached("mmsg", &["dispatch", "quit"]),
        "reboot" => spawn_detached("systemctl", &["reboot"]),
        "shutdown" => spawn_detached("systemctl", &["poweroff"]),
        _ => {}
    }
}

pub fn apply_wallpaper(path: &str, mode: &str) {
    let mut v = read_settings_value();
    if let Some(obj) = v.as_object_mut() {
        obj.insert("wallpaperPath".to_string(), Value::String(path.to_string()));
        obj.insert("wallpaperMode".to_string(), Value::String(mode.to_string()));
    }
    write_settings_value(&v);
    let script = "pkill -x swaybg 2>/dev/null || true; setsid -f swaybg -i \"$1\" -m \"$2\" >/dev/null 2>&1";
    spawn_detached("/bin/bash", &["-c", script, "_", path, mode]);
}

pub fn encode_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut out = String::from("file://");
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn is_image_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".png")
        || l.ends_with(".jpg")
        || l.ends_with(".jpeg")
        || l.ends_with(".webp")
        || l.ends_with(".gif")
        || l.ends_with(".bmp")
        || l.ends_with(".svg")
}

pub fn is_video_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".mp4")
        || l.ends_with(".mkv")
        || l.ends_with(".webm")
        || l.ends_with(".mov")
        || l.ends_with(".avi")
}

pub fn is_thumbable_file(name: &str) -> bool {
    is_image_file(name) || is_video_file(name)
}

pub fn file_icon(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "󰉋";
    }
    let l = name.to_lowercase();
    if is_image_file(&l) { return "󰸉"; }
    if l.ends_with(".mp4") || l.ends_with(".mkv") || l.ends_with(".webm") || l.ends_with(".mov") { return "󰕧"; }
    if l.ends_with(".mp3") || l.ends_with(".flac") || l.ends_with(".wav") || l.ends_with(".ogg") { return "󰎆"; }
    if l.ends_with(".zip") || l.ends_with(".tar") || l.ends_with(".gz") || l.ends_with(".xz") || l.ends_with(".7z") || l.ends_with(".rar") { return "󰛫"; }
    if l.ends_with(".pdf") { return "󰈦"; }
    if l.ends_with(".qml") || l.ends_with(".js") || l.ends_with(".ts") || l.ends_with(".py") || l.ends_with(".rs") || l.ends_with(".c") || l.ends_with(".cpp") || l.ends_with(".sh") { return "󰈮"; }
    if l.ends_with(".json") || l.ends_with(".conf") || l.ends_with(".toml") || l.ends_with(".yaml") || l.ends_with(".yml") { return "󰒓"; }
    if l.ends_with(".md") || l.ends_with(".txt") { return "󰈙"; }
    "󰈔"
}

pub fn file_icon_color(name: &str, is_dir: bool, t: &Theme) -> Rgb {
    if is_dir {
        return t.blue;
    }
    let l = name.to_lowercase();
    if is_image_file(&l) { return t.purple; }
    if l.ends_with(".mp4") || l.ends_with(".mkv") || l.ends_with(".webm") || l.ends_with(".mp3") || l.ends_with(".flac") { return t.yellow; }
    if l.ends_with(".zip") || l.ends_with(".tar") || l.ends_with(".gz") || l.ends_with(".7z") || l.ends_with(".rar") { return t.orange; }
    if l.ends_with(".qml") || l.ends_with(".js") || l.ends_with(".ts") || l.ends_with(".py") || l.ends_with(".rs") || l.ends_with(".sh") { return t.cyan; }
    if l.ends_with(".json") || l.ends_with(".conf") || l.ends_with(".toml") || l.ends_with(".yaml") { return t.green; }
    if l.ends_with(".pdf") || l.ends_with(".md") || l.ends_with(".txt") { return t.blue; }
    t.muted
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[allow(dead_code)]
pub fn is_archive_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".zip")
        || lower.ends_with(".tar")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar.xz")
        || lower.ends_with(".txz")
        || lower.ends_with(".tar.bz2")
        || lower.ends_with(".tbz2")
        || lower.ends_with(".7z")
        || lower.ends_with(".rar")
}

pub fn is_text_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    let exts = [
        ".txt", ".md", ".json", ".qml", ".js", ".ts", ".py", ".sh", ".bash", ".zsh",
        ".conf", ".ini", ".toml", ".yaml", ".yml", ".css", ".html", ".xml", ".rs",
        ".c", ".cpp", ".h", ".hpp", ".log", ".desktop", ".service", ".env", ".lock",
    ];
    exts.iter().any(|e| lower.ends_with(e))
}

pub fn format_modified(secs: i64) -> String {
    if secs <= 0 {
        return "-".to_string();
    }
    let local = secs + 7 * 3600;
    let days = local.div_euclid(86400);
    let rem = local.rem_euclid(86400);
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, hour, min)
}

#[allow(dead_code)]
pub fn list_dir_entries(dir: &Path, show_hidden: bool, filter: &str) -> Vec<FileItem> {
    list_dir_entries_sorted(dir, show_hidden, filter, SortBy::Name, true)
}

pub fn list_dir_entries_sorted(
    dir: &Path,
    show_hidden: bool,
    filter: &str,
    sort_by: SortBy,
    sort_asc: bool,
) -> Vec<FileItem> {
    let q = filter.trim().to_lowercase();
    let mut items = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                continue;
            }
            let path = ent.path();
            let sym_meta = fs::symlink_metadata(&path);
            let is_symlink = sym_meta
                .as_ref()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            let symlink_target = if is_symlink {
                fs::read_link(&path).ok().map(|t| t.to_string_lossy().to_string())
            } else {
                None
            };
            let meta = fs::metadata(&path).or(sym_meta);
            let (is_dir, size, modified_secs, mode_bits) = match meta {
                Ok(m) => {
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    (m.is_dir(), if m.is_dir() { 0 } else { m.len() }, mtime, m.permissions().mode())
                }
                Err(_) => (false, 0, 0, 0),
            };
            let kind_label = file_kind_label(&name, is_dir, is_symlink);
            let mode_str = format_permissions(mode_bits);
            items.push(FileItem {
                icon: file_icon(&name, is_dir),
                size_label: if is_dir { String::new() } else { format_size(size) },
                modified_label: format_modified(modified_secs),
                name,
                path,
                is_dir,
                is_symlink,
                symlink_target,
                size,
                modified_secs,
                kind_label,
                mode_str,
            });
        }
    }
    items.sort_by(|a, b| {
        let dir_ord = b.is_dir.cmp(&a.is_dir);
        if dir_ord != std::cmp::Ordering::Equal {
            return dir_ord;
        }
        let ord = match sort_by {
            SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortBy::Size => a
                .size
                .cmp(&b.size)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            SortBy::Modified => a
                .modified_secs
                .cmp(&b.modified_secs)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        };
        if sort_asc { ord } else { ord.reverse() }
    });
    items
}

pub fn count_dir_items(dir: &Path) -> Option<usize> {
    fs::read_dir(dir).ok().map(|rd| rd.filter_map(|e| e.ok()).count())
}

pub fn read_text_preview(path: &Path, max_lines: usize) -> Vec<String> {
    use std::io::Read;
    let Ok(mut f) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut buf = vec![0u8; 4096];
    let Ok(n) = f.read(&mut buf) else {
        return Vec::new();
    };
    buf.truncate(n);
    if buf.iter().take(512).any(|&b| b == 0) {
        return Vec::new();
    }
    let s = String::from_utf8_lossy(&buf);
    s.lines()
        .take(max_lines)
        .map(|l| l.replace('\t', "  "))
        .collect()
}

fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

pub fn unique_dest_path(dest_dir: &Path, base_name: &str) -> PathBuf {
    let candidate = dest_dir.join(base_name);
    if !candidate.exists() {
        return candidate;
    }
    let p = Path::new(base_name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| base_name.to_string());
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut i = 1;
    loop {
        let next = dest_dir.join(format!("{}_copy{}{}", stem, i, ext));
        if !next.exists() {
            return next;
        }
        i += 1;
    }
}

pub fn transfer_files(sources: &[PathBuf], dest_dir: &Path, move_mode: bool) -> Result<usize, String> {
    let dest_real = fs::canonicalize(dest_dir).unwrap_or_else(|_| dest_dir.to_path_buf());
    let mut count = 0;
    for src_path in sources {
        if !src_path.exists() {
            continue;
        }
        let src_real = fs::canonicalize(src_path).unwrap_or_else(|_| src_path.clone());
        if move_mode && src_real.parent() == Some(dest_real.as_path()) {
            continue;
        }
        if dest_real == src_real || dest_real.starts_with(&src_real) {
            continue;
        }
        let base = src_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string());
        let dst = unique_dest_path(&dest_real, &base);
        if move_mode {
            if fs::rename(&src_real, &dst).is_err() {
                copy_recursive(&src_real, &dst).map_err(|e| e.to_string())?;
                if src_real.is_dir() {
                    fs::remove_dir_all(&src_real).map_err(|e| e.to_string())?;
                } else {
                    fs::remove_file(&src_real).map_err(|e| e.to_string())?;
                }
            }
        } else {
            copy_recursive(&src_real, &dst).map_err(|e| e.to_string())?;
        }
        count += 1;
    }
    Ok(count)
}

pub fn duplicate_files(sources: &[PathBuf]) -> Result<usize, String> {
    let mut count = 0;
    for src in sources {
        let Some(parent) = src.parent() else { continue };
        let Some(base) = src.file_name().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let dst = unique_dest_path(parent, &base);
        copy_recursive(src, &dst).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

pub fn run_trash(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let _ = fs::create_dir_all(TRASH_DIR);
    let _ = fs::create_dir_all(TRASH_INFO_DIR);
    let ok = Command::new("gio")
        .arg("trash")
        .args(paths)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok && paths.iter().all(|p| !p.exists()) {
        return Ok(());
    }
    // Fallback for cross-mount / tmpfs / internal mounts where `gio trash` refuses:
    // Move directly into ~/.local/share/Trash/files and write .trashinfo
    let trash_files = Path::new(TRASH_DIR);
    let trash_info = Path::new(TRASH_INFO_DIR);
    let now_str = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    for src in paths {
        if !src.exists() && fs::symlink_metadata(src).is_err() {
            continue;
        }
        let base = src
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "trashed_item".to_string());
        let dst = unique_dest_path(trash_files, &base);
        let dst_name = dst
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| base.clone());
        if fs::rename(src, &dst).is_err() {
            copy_recursive(src, &dst).map_err(|e| e.to_string())?;
            if src.is_dir() && !src.is_symlink() {
                fs::remove_dir_all(src).map_err(|e| e.to_string())?;
            } else {
                fs::remove_file(src).map_err(|e| e.to_string())?;
            }
        }
        let info_path = trash_info.join(format!("{dst_name}.trashinfo"));
        let info_body = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            src.to_string_lossy(),
            now_str
        );
        let _ = fs::write(info_path, info_body);
    }
    Ok(())
}

pub fn delete_permanently(paths: &[PathBuf]) -> Result<usize, String> {
    let mut count = 0;
    for p in paths {
        let sym = fs::symlink_metadata(p);
        match sym {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {
                fs::remove_dir_all(p).map_err(|e| e.to_string())?;
                count += 1;
            }
            Ok(_) => {
                fs::remove_file(p).map_err(|e| e.to_string())?;
                count += 1;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(count)
}

pub fn empty_trash() -> Result<usize, String> {
    let mut count = 0;
    for dir in [TRASH_DIR, TRASH_INFO_DIR] {
        if let Ok(rd) = fs::read_dir(dir) {
            for item in rd.flatten() {
                let p = item.path();
                if p.is_dir() && !p.is_symlink() {
                    let _ = fs::remove_dir_all(&p);
                } else {
                    let _ = fs::remove_file(&p);
                }
                if dir == TRASH_DIR {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<String, String> {
    let lower = archive_path.to_string_lossy().to_lowercase();
    let stem = archive_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "extracted".to_string());
    let out_dir = unique_dest_path(dest_dir, &stem);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let out_str = out_dir.to_string_lossy().to_string();
    let arch_str = archive_path.to_string_lossy().to_string();

    let status = if lower.ends_with(".zip") {
        Command::new("unzip")
            .args(["-q", &arch_str, "-d", &out_str])
            .status()
            .or_else(|_| {
                Command::new("7z")
                    .args(["x", "-y", &format!("-o{out_str}"), &arch_str])
                    .status()
            })
    } else if lower.ends_with(".tar")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar.xz")
        || lower.ends_with(".txz")
        || lower.ends_with(".tar.bz2")
        || lower.ends_with(".tbz2")
    {
        Command::new("tar")
            .args(["-xf", &arch_str, "-C", &out_str])
            .status()
    } else {
        Command::new("7z")
            .args(["x", "-y", &format!("-o{out_str}"), &arch_str])
            .status()
    }
    .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(out_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(out_str))
    } else {
        let _ = fs::remove_dir_all(&out_dir);
        Err(format!("Extract failed ({status})"))
    }
}

pub fn compress_to_zip(paths: &[PathBuf], dest_dir: &Path) -> Result<String, String> {
    if paths.is_empty() {
        return Err("Nothing selected".to_string());
    }
    let base_name = if paths.len() == 1 {
        let n = paths[0]
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string());
        format!("{n}.zip")
    } else {
        "archive.zip".to_string()
    };
    let zip_path = unique_dest_path(dest_dir, &base_name);
    let zip_str = zip_path.to_string_lossy().to_string();

    let rel_args: Vec<String> = paths
        .iter()
        .map(|p| {
            if p.parent() == Some(dest_dir) {
                p.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.to_string_lossy().to_string())
            } else {
                p.to_string_lossy().to_string()
            }
        })
        .collect();

    let status = Command::new("7z")
        .current_dir(dest_dir)
        .arg("a")
        .arg("-tzip")
        .arg("-bd")
        .arg(&zip_str)
        .args(&rel_args)
        .stdout(Stdio::null())
        .status()
        .or_else(|_| {
            Command::new("bsdtar")
                .current_dir(dest_dir)
                .args(["-a", "-cf", &zip_str])
                .args(&rel_args)
                .status()
        })
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(zip_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(zip_str))
    } else {
        Err(format!("Compress failed ({status})"))
    }
}

pub fn wl_copy_uri_list(paths: &[PathBuf]) {
    use std::io::Write;
    if paths.is_empty() {
        return;
    }
    let body = paths
        .iter()
        .map(|p| encode_uri(p))
        .collect::<Vec<_>>()
        .join("\r\n");
    if let Ok(mut child) = Command::new("wl-copy")
        .args(["-t", "text/uri-list"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(body.as_bytes());
        }
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex_val = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn wl_paste_text() -> Option<String> {
    let out = Command::new("wl-paste")
        .args(["--no-newline"])
        .output()
        .ok()?;
    if out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

pub fn wl_paste_uri_list() -> Vec<PathBuf> {
    let Ok(out) = Command::new("wl-paste")
        .args(["--no-newline", "-t", "text/uri-list"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let mut paths = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("file://") {
            let decoded = percent_decode(rest);
            if !decoded.is_empty() {
                paths.push(PathBuf::from(decoded));
            }
        }
    }
    paths
}

pub fn set_lock_wallpaper(path: &str) {
    let mut v = read_settings_value();
    if let Some(obj) = v.as_object_mut() {
        obj.insert("lockWallpaperPath".to_string(), Value::String(path.to_string()));
        obj.insert("lockUseSameWallpaper".to_string(), Value::Bool(false));
    }
    write_settings_value(&v);
}

pub fn load_png_surface(path: &Path) -> Option<cairo::ImageSurface> {
    let mut f = fs::File::open(path).ok()?;
    cairo::ImageSurface::create_from_png(&mut f).ok()
}

pub fn load_svg_surface(path: &Path, size: i32) -> Option<cairo::ImageSurface> {
    let sz_s = size.to_string();
    let out = Command::new("rsvg-convert")
        .args(["-w", &sz_s, "-h", &sz_s])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let mut cursor = Cursor::new(out.stdout);
    cairo::ImageSurface::create_from_png(&mut cursor).ok()
}

fn find_icon_file(icon_name: &str) -> Option<PathBuf> {
    let trimmed = icon_name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let direct = Path::new(trimmed);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct.to_path_buf());
    }
    let roots = [
        "/home/huyle/.local/share/icons/hicolor",
        "/home/huyle/.local/share/flatpak/exports/share/icons/hicolor",
        "/var/lib/flatpak/exports/share/icons/hicolor",
        "/usr/share/icons/hicolor",
        "/usr/share/icons/breeze-dark",
        "/usr/share/icons/Adwaita",
        "/usr/share/icons/AdwaitaLegacy",
    ];
    let subdirs = [
        "scalable/apps",
        "scalable/places",
        "scalable/categories",
        "256x256/apps",
        "128x128/apps",
        "64x64/apps",
        "48x48/apps",
        "32x32/apps",
        "48x48/places",
        "48x48/legacy",
        "32x32/places",
        "apps/48",
        "apps/64",
        "places/48",
    ];
    let exts = ["", ".png", ".svg"];
    for root in roots {
        for sub in subdirs {
            for ext in exts {
                let p = Path::new(root).join(sub).join(format!("{trimmed}{ext}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    for dir in ["/home/huyle/.local/share/icons", "/usr/share/pixmaps"] {
        for ext in exts {
            let p = Path::new(dir).join(format!("{trimmed}{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn load_icon_surface(icon_name: &str, px: i32) -> Option<cairo::ImageSurface> {
    let path = find_icon_file(icon_name)?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    if ext == "svg" {
        load_svg_surface(&path, px)
    } else if ext == "png" {
        load_png_surface(&path)
    } else {
        None
    }
}

pub fn desktop_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. User applications dir ($XDG_DATA_HOME/applications or ~/.local/share/applications)
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("applications"));
    } else if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
    }

    // 2. User flatpak exports
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));
    }

    // 3. System applications
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs.push(PathBuf::from("/usr/local/share/applications"));

    // 4. System flatpak exports
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    // 5. XDG_DATA_DIRS if set
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for d in data_dirs.split(':') {
            if !d.is_empty() {
                dirs.push(PathBuf::from(d).join("applications"));
            }
        }
    }

    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()) && d.is_dir());
    dirs
}

pub fn app_dirs_mtimes() -> Vec<(PathBuf, Option<std::time::SystemTime>)> {
    desktop_app_dirs()
        .into_iter()
        .map(|d| {
            let mt = fs::metadata(&d).ok().and_then(|m| m.modified().ok());
            (d, mt)
        })
        .collect()
}

fn exec_binary_exists(exec: &str) -> bool {
    let mut token = "";
    for part in exec.split_whitespace() {
        let p = part.trim_matches('"').trim_matches('\'');
        if p.is_empty() || p == "env" || p.contains('=') {
            continue;
        }
        token = p;
        break;
    }
    if token.is_empty() {
        return false;
    }
    let direct = Path::new(token);
    if direct.is_absolute() {
        return direct.is_file();
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            if !dir.is_empty() && Path::new(dir).join(token).is_file() {
                return true;
            }
        }
    }
    for fallback_dir in ["/home/huyle/.local/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        if Path::new(fallback_dir).join(token).is_file() {
            return true;
        }
    }
    false
}

pub fn scan_desktop_apps() -> Vec<AppItem> {
    scan_desktop_apps_cached(&[])
}

pub fn scan_desktop_apps_cached(existing: &[AppItem]) -> Vec<AppItem> {
    let mut icon_cache: HashMap<String, Option<cairo::ImageSurface>> = HashMap::new();
    for app in existing {
        if let Some(surf) = &app.icon_surface {
            icon_cache.insert(app.icon.clone(), Some(surf.clone()));
        }
    }

    let mut by_id: HashMap<String, Option<AppItem>> = HashMap::new();
    let dirs = desktop_app_dirs();
    for dir in dirs {
        let Ok(rd) = fs::read_dir(dir) else { continue };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            let id = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if by_id.contains_key(&id) {
                continue;
            }
            let Ok(txt) = fs::read_to_string(&p) else { continue };
            let mut in_entry = false;
            let mut name = String::new();
            let mut exec = String::new();
            let mut icon = String::new();
            let mut no_display = false;
            let mut hidden = false;
            let mut terminal = false;
            let mut app_type = String::new();
            for line in txt.lines() {
                let l = line.trim();
                if l.starts_with('[') {
                    in_entry = l == "[Desktop Entry]";
                    continue;
                }
                if !in_entry || l.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = l.split_once('=') {
                    match k.trim() {
                        "Name" if name.is_empty() => name = v.trim().to_string(),
                        "Exec" if exec.is_empty() => exec = v.trim().to_string(),
                        "Icon" if icon.is_empty() => icon = v.trim().to_string(),
                        "Type" => app_type = v.trim().to_string(),
                        "NoDisplay" => no_display = v.trim().eq_ignore_ascii_case("true"),
                        "Hidden" => hidden = v.trim().eq_ignore_ascii_case("true"),
                        "Terminal" => terminal = v.trim().eq_ignore_ascii_case("true"),
                        _ => {}
                    }
                }
            }
            if no_display
                || hidden
                || name.is_empty()
                || exec.is_empty()
                || (!app_type.is_empty() && app_type != "Application")
                || !exec_binary_exists(&exec)
            {
                by_id.insert(id, None);
            } else {
                by_id.insert(id.clone(), Some(AppItem { id, name, exec, icon, icon_surface: None, terminal }));
            }
        }
    }
    let mut seen_names = HashSet::new();
    let mut list: Vec<AppItem> = by_id.into_values().flatten().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list.retain(|item| seen_names.insert(item.name.to_lowercase()));
    for item in &mut list {
        if let Some(cached) = icon_cache.get(&item.icon) {
            item.icon_surface = cached.clone();
        } else {
            let surf = load_icon_surface(&item.icon, 48);
            icon_cache.insert(item.icon.clone(), surf.clone());
            item.icon_surface = surf;
        }
    }
    list
}

pub fn launch_app(app: &AppItem) {
    let cleaned = app
        .exec
        .replace("%u", "")
        .replace("%U", "")
        .replace("%f", "")
        .replace("%F", "")
        .replace("%i", "")
        .replace("%c", "")
        .replace("%k", "");
    let cmd = cleaned.trim();
    if cmd.is_empty() {
        return;
    }
    if app.terminal {
        spawn_detached("kitty", &["-e", "sh", "-c", cmd]);
    } else {
        spawn_detached("sh", &["-c", cmd]);
    }
}

pub fn local_clock_strings(use_24h: bool) -> (String, String, String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let local = secs + 7 * 3600;
    let days = local.div_euclid(86400);
    let rem = local.rem_euclid(86400);
    let hour24 = rem / 3600;
    let min = (rem % 3600) / 60;

    let (hh_str, ampm) = if use_24h {
        (format!("{:02}", hour24), String::new())
    } else {
        let h12 = match hour24 % 12 {
            0 => 12,
            v => v,
        };
        (format!("{:02}", h12), if hour24 < 12 { "AM" } else { "PM" }.to_string())
    };
    let mm_str = format!("{:02}", min);
    let time_full = if ampm.is_empty() {
        format!("{hh_str}:{mm_str}")
    } else {
        format!("{hh_str}:{mm_str} {ampm}")
    };

    // Weekday: 1970-01-01 was Thursday (index 4 in Sun=0..Sat=6)
    let wday_idx = (days + 4).rem_euclid(7) as usize;
    let wdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let months = ["", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let date_str = format!("{}, {} {:02}", wdays[wday_idx], months[month as usize], day);

    (hh_str, mm_str, time_full, date_str)
}

fn parse_tags_line(line: &str) -> Option<Vec<TagInfo>> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let arr = v
        .get("all_tags")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.get("tags"))
        .or_else(|| v.get("tags"))
        .and_then(|t| t.as_array())?;

    let mut out = Vec::new();
    for item in arr {
        let index = item.get("index").and_then(|x| x.as_u64()).unwrap_or(1) as u32;
        let is_active = item.get("is_active").and_then(|x| x.as_bool()).unwrap_or(false);
        let is_urgent = item.get("is_urgent").and_then(|x| x.as_bool()).unwrap_or(false);
        let client_count = item.get("client_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        out.push(TagInfo { index, is_active, is_urgent, client_count });
    }
    if out.is_empty() { None } else { Some(out) }
}

fn read_ram() -> (u32, f64, f64) {
    let Ok(txt) = fs::read_to_string("/proc/meminfo") else { return (0, 0.0, 8.0) };
    let mut total_kb = 0f64;
    let mut avail_kb = 0f64;
    for l in txt.lines() {
        if let Some(rest) = l.strip_prefix("MemTotal:") {
            total_kb = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        } else if let Some(rest) = l.strip_prefix("MemAvailable:") {
            avail_kb = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        }
    }
    if total_kb <= 0.0 {
        return (0, 0.0, 8.0);
    }
    let used_kb = (total_kb - avail_kb).max(0.0);
    let pct = ((used_kb * 100.0 / total_kb).round() as u32).min(100);
    let used_gb = used_kb / 1_048_576.0;
    let total_gb = total_kb / 1_048_576.0;
    (pct, used_gb, total_gb)
}

fn read_swap() -> (f64, f64) {
    let Ok(txt) = fs::read_to_string("/proc/meminfo") else { return (0.0, 0.0) };
    let mut total_kb = 0f64;
    let mut free_kb = 0f64;
    for l in txt.lines() {
        if let Some(rest) = l.strip_prefix("SwapTotal:") {
            total_kb = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        } else if let Some(rest) = l.strip_prefix("SwapFree:") {
            free_kb = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        }
    }
    let used_kb = (total_kb - free_kb).max(0.0);
    (used_kb / 1_048_576.0, total_kb / 1_048_576.0)
}

fn read_cpu_info_once() -> (u32, String) {
    let Ok(txt) = fs::read_to_string("/proc/cpuinfo") else {
        return (4, "CPU".into());
    };
    let mut cores = 0u32;
    let mut model = String::new();
    for l in txt.lines() {
        if l.starts_with("processor") {
            cores += 1;
        } else if model.is_empty() && l.starts_with("model name") {
            if let Some((_, rhs)) = l.split_once(':') {
                model = rhs
                    .trim()
                    .replace("(R)", "")
                    .replace("(TM)", "")
                    .split(" @ ")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        }
    }
    (cores.max(1), if model.is_empty() { "CPU".into() } else { model })
}

fn read_cpu_jiffies() -> (u64, u64) {
    let Ok(txt) = fs::read_to_string("/proc/stat") else { return (0, 0) };
    for l in txt.lines() {
        if let Some(rest) = l.strip_prefix("cpu ") {
            let nums: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|s| s.parse::<u64>().ok())
                .collect();
            if nums.len() >= 4 {
                let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
                let total: u64 = nums.iter().sum();
                return (total, idle);
            }
        }
    }
    (0, 0)
}

fn read_disk_root() -> (u32, f64, f64) {
    let Ok(o) = Command::new("df").args(["-k", "/"]).output() else {
        return (0, 0.0, 0.0);
    };
    let s = String::from_utf8_lossy(&o.stdout);
    if let Some(line) = s.lines().nth(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 5 {
            let total_kb: f64 = cols[1].parse().unwrap_or(0.0);
            let used_kb: f64 = cols[2].parse().unwrap_or(0.0);
            let pct: u32 = cols[4].trim_end_matches('%').parse().unwrap_or(0);
            return (pct, used_kb / 1_048_576.0, total_kb / 1_048_576.0);
        }
    }
    (0, 0.0, 0.0)
}

fn read_net_bytes() -> (u64, u64) {
    let Ok(txt) = fs::read_to_string("/proc/net/dev") else { return (0, 0) };
    let mut rx = 0u64;
    let mut tx = 0u64;
    for l in txt.lines().skip(2) {
        let Some((iface, rest)) = l.split_once(':') else { continue };
        let iface = iface.trim();
        if iface == "lo" {
            continue;
        }
        let cols: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();
        if cols.len() >= 9 {
            rx += cols[0];
            tx += cols[8];
        }
    }
    (rx, tx)
}

fn read_uptime_and_load() -> (String, u32, String) {
    let mut uptime_s = "0m".to_string();
    if let Ok(txt) = fs::read_to_string("/proc/uptime") {
        if let Some(sec_str) = txt.split_whitespace().next() {
            if let Ok(sec_f) = sec_str.parse::<f64>() {
                let sec = sec_f as u64;
                let h = sec / 3600;
                let m = (sec % 3600) / 60;
                uptime_s = if h > 0 { format!("{h}h {m}m") } else { format!("{m}m") };
            }
        }
    }
    let mut procs = 0u32;
    let mut load = "0.00".to_string();
    if let Ok(txt) = fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = txt.split_whitespace().collect();
        if !parts.is_empty() {
            load = parts[0].to_string();
        }
        if let Some(p4) = parts.get(3) {
            if let Some((_, tot)) = p4.split_once('/') {
                procs = tot.parse().unwrap_or(0);
            }
        }
    }
    (uptime_s, procs, load)
}

fn unescape_nmcli(s: &str) -> String {
    s.replace("\\:", ":").replace("\\\\", "\\")
}

fn read_wifi() -> (bool, String, u32) {
    let enabled = Command::new("nmcli")
        .args(["-t", "-f", "WIFI", "g"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(false);
    if !enabled {
        return (false, String::new(), 0);
    }
    let mut ssid = String::new();
    let mut signal = 0u32;
    if let Ok(o) = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID,SIGNAL", "dev", "wifi"])
        .output()
    {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let mut parts = line.splitn(3, ':');
            let active = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            let sig = parts.next().unwrap_or("0");
            if active == "yes" {
                ssid = unescape_nmcli(name);
                signal = sig.trim().parse().unwrap_or(0);
                break;
            }
        }
    }
    (true, ssid, signal)
}

pub fn read_wifi_networks() -> (bool, Vec<WifiNetwork>) {
    let enabled = Command::new("nmcli")
        .args(["-t", "-f", "WIFI", "g"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(false);
    if !enabled {
        return (false, Vec::new());
    }

    let mut saved: HashSet<String> = HashSet::new();
    if let Ok(o) = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
        .output()
    {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if !line.contains("802-11-wireless") {
                continue;
            }
            if let Some((name, _)) = line.rsplit_once(':') {
                saved.insert(unescape_nmcli(name));
            }
        }
    }

    let mut map: HashMap<String, WifiNetwork> = HashMap::new();
    if let Ok(o) = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"])
        .output()
    {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let active = parts[0] == "yes";
            let security = parts[parts.len() - 1].trim().to_string();
            let signal = parts[parts.len() - 2].trim().parse().unwrap_or(0);
            let ssid = unescape_nmcli(&parts[1..parts.len() - 2].join(":")).trim().to_string();
            if ssid.is_empty() {
                continue;
            }
            let item = WifiNetwork {
                active,
                saved: saved.contains(&ssid),
                signal,
                security,
                ssid: ssid.clone(),
            };
            match map.get(&ssid) {
                Some(prev) if prev.active || (!active && prev.signal >= signal) => {}
                _ => {
                    map.insert(ssid, item);
                }
            }
        }
    }
    let mut list: Vec<WifiNetwork> = map.into_values().collect();
    list.sort_by(|a, b| {
        if a.active != b.active {
            return if a.active { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        if a.saved != b.saved {
            return if a.saved { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        b.signal.cmp(&a.signal)
    });
    (true, list)
}

pub fn wifi_rescan() {
    spawn_detached("nmcli", &["device", "wifi", "list", "--rescan", "yes"]);
}

pub fn wifi_connect(ssid: &str, password: &str, is_saved: bool) {
    if !password.is_empty() {
        spawn_detached("nmcli", &["device", "wifi", "connect", ssid, "password", password]);
    } else if is_saved {
        spawn_detached("nmcli", &["connection", "up", "id", ssid]);
    } else {
        spawn_detached("nmcli", &["device", "wifi", "connect", ssid]);
    }
}

pub fn wifi_disconnect(ssid: &str) {
    spawn_detached("nmcli", &["connection", "down", "id", ssid]);
}

fn read_bluetooth() -> (bool, u32) {
    let Ok(o) = Command::new("bluetoothctl").arg("show").output() else {
        return (false, 0);
    };
    let s = String::from_utf8_lossy(&o.stdout);
    let powered = s.lines().any(|l| l.trim() == "Powered: yes");
    let connected = Command::new("bluetoothctl")
        .args(["devices", "Connected"])
        .output()
        .ok()
        .map(|c| {
            String::from_utf8_lossy(&c.stdout)
                .lines()
                .filter(|l| l.trim_start().starts_with("Device "))
                .count() as u32
        })
        .unwrap_or(0);
    (powered, connected)
}

fn bt_list(arg: Option<&str>) -> Vec<BtDevice> {
    let mut cmd = Command::new("bluetoothctl");
    cmd.arg("devices");
    if let Some(a) = arg {
        cmd.arg(a);
    }
    let Ok(o) = cmd.output() else { return Vec::new() };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Device ")?;
            let mut it = rest.splitn(2, ' ');
            let mac = it.next()?.trim().to_string();
            let name = it.next().unwrap_or(&mac).trim().to_string();
            Some(BtDevice { mac, name, connected: false, paired: false })
        })
        .collect()
}

pub fn read_bluetooth_devices() -> (bool, Vec<BtDevice>, Vec<BtDevice>, u32) {
    let Ok(o) = Command::new("bluetoothctl").arg("show").output() else {
        return (false, Vec::new(), Vec::new(), 0);
    };
    let s = String::from_utf8_lossy(&o.stdout);
    let powered = s.lines().any(|l| l.trim() == "Powered: yes");
    if !powered {
        return (false, Vec::new(), Vec::new(), 0);
    }
    let paired_macs: HashSet<String> = bt_list(Some("Paired")).into_iter().map(|d| d.mac).collect();
    let connected_macs: HashSet<String> = bt_list(Some("Connected")).into_iter().map(|d| d.mac).collect();
    let all = bt_list(None);

    let mut paired = Vec::new();
    let mut available = Vec::new();
    let mut connected_count = 0u32;
    for mut d in all {
        d.connected = connected_macs.contains(&d.mac);
        d.paired = paired_macs.contains(&d.mac);
        if d.connected {
            connected_count += 1;
        }
        if d.paired {
            paired.push(d);
        } else {
            available.push(d);
        }
    }
    paired.sort_by(|a, b| {
        if a.connected != b.connected {
            return if a.connected { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    available.sort_by(|a, b| {
        let raw = |s: &str| s.len() == 17 && s.chars().filter(|c| *c == ':').count() == 5
            && s.chars().all(|c| c.is_ascii_hexdigit() || c == ':');
        let (ar, br) = (raw(&a.name), raw(&b.name));
        if ar != br {
            return if ar { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    (true, paired, available, connected_count)
}

pub fn bt_scan() {
    spawn_detached(
        "sh",
        &[
            "-c",
            "bluetoothctl power on >/dev/null 2>&1; bluetoothctl pairable on >/dev/null 2>&1; bluetoothctl --timeout 15 scan on >/dev/null 2>&1 || true",
        ],
    );
}

pub fn bt_connect(mac: &str, name: &str, paired: bool) {
    if paired {
        spawn_detached("bluetoothctl", &["connect", mac]);
    } else {
        let script = format!(
            "bluetoothctl pairable on >/dev/null 2>&1; bluetoothctl pair '{mac}' >/dev/null 2>&1 || true; bluetoothctl trust '{mac}' >/dev/null 2>&1 || true; bluetoothctl connect '{mac}'"
        );
        let _ = name;
        spawn_detached("sh", &["-c", &script]);
    }
}

pub fn bt_disconnect(mac: &str) {
    spawn_detached("bluetoothctl", &["disconnect", mac]);
}

pub fn bt_remove(mac: &str) {
    spawn_detached("bluetoothctl", &["remove", mac]);
}

fn dbus_get_string(dest: &str, iface: &str, prop: &str) -> Option<String> {
    let out = Command::new("dbus-send")
        .args([
            "--user",
            "--print-reply",
            &format!("--dest={dest}"),
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties.Get",
        ])
        .arg(format!("string:{iface}"))
        .arg(format!("string:{prop}"))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("string \"").map(|v| v.trim_end_matches('"')))
        .map(|v| v.to_string())
}

fn dbus_get_metadata_field(dest: &str, key: &str) -> Option<String> {
    let out = Command::new("dbus-send")
        .args([
            "--user",
            "--print-reply",
            &format!("--dest={dest}"),
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties.Get",
            "string:org.mpris.MediaPlayer2.Player",
            "string:Metadata",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let needle = format!("string \"{key}\"");
    let mut lines = s.lines();
    while let Some(line) = lines.next() {
        if line.contains(&needle) {
            for next in lines.by_ref().take(4) {
                let t = next.trim();
                if let Some(pos) = t.find("string \"") {
                    let rest = &t[pos + 8..];
                    return Some(rest.trim_end_matches('"').to_string());
                }
            }
        }
    }
    None
}

fn dbus_get_metadata_int(dest: &str, key: &str) -> Option<u64> {
    let out = Command::new("dbus-send")
        .args([
            "--user",
            "--print-reply",
            &format!("--dest={dest}"),
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties.Get",
            "string:org.mpris.MediaPlayer2.Player",
            "string:Metadata",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let needle = format!("string \"{key}\"");
    let mut lines = s.lines();
    while let Some(line) = lines.next() {
        if line.contains(&needle) {
            for next in lines.by_ref().take(4) {
                let t = next.trim();
                for prefix in ["int64 ", "uint64 ", "int32 ", "uint32 ", "x ", "t "] {
                    if let Some(v) = t.strip_prefix(prefix) {
                        if let Ok(n) = v.trim().parse::<u64>() {
                            return Some(n);
                        }
                    }
                }
            }
        }
    }
    None
}

fn dbus_get_position(dest: &str) -> u64 {
    let out = Command::new("dbus-send")
        .args([
            "--user",
            "--print-reply",
            &format!("--dest={dest}"),
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties.Get",
            "string:org.mpris.MediaPlayer2.Player",
            "string:Position",
        ])
        .output()
        .ok();
    let Some(out) = out else { return 0 };
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        let t = line.trim();
        for prefix in ["int64 ", "uint64 ", "variant ", "x ", "t "] {
            if let Some(v) = t.strip_prefix(prefix) {
                if let Ok(n) = v.trim().parse::<u64>() {
                    return n / 1_000_000;
                }
            }
        }
    }
    0
}

fn read_media_all() -> Vec<MediaInfo> {
    let Ok(o) = Command::new("busctl")
        .args(["--user", "--list", "--no-legend"])
        .output()
    else {
        return Vec::new();
    };
    let names: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
        .filter(|n| n.starts_with("org.mpris.MediaPlayer2."))
        .collect();
    let mut list = Vec::new();
    for player in names {
        let status = dbus_get_string(&player, "org.mpris.MediaPlayer2.Player", "PlaybackStatus").unwrap_or_default();
        let title = dbus_get_metadata_field(&player, "xesam:title").unwrap_or_default();
        let artist = dbus_get_metadata_field(&player, "xesam:artist").unwrap_or_default();
        let album = dbus_get_metadata_field(&player, "xesam:album").unwrap_or_default();
        let art_url = dbus_get_metadata_field(&player, "mpris:artUrl").unwrap_or_default();
        let length = dbus_get_metadata_int(&player, "mpris:length").unwrap_or(0) as f64 / 1_000_000.0;
        let position = dbus_get_position(&player) as f64;
        let source = player.trim_start_matches("org.mpris.MediaPlayer2.").to_string();
        let playing = status == "Playing";
        let paused = status == "Paused";
        if !playing && !paused && title.trim().is_empty() {
            continue;
        }
        list.push(MediaInfo {
            active: true,
            playing,
            title,
            artist,
            album,
            art_url,
            player: source,
            bus: player,
            length,
            position,
        });
    }
    list
}

fn read_media_with_pref(pref: &str) -> (MediaInfo, Vec<MediaInfo>) {
    let list = read_media_all();
    if list.is_empty() {
        return (MediaInfo::default(), list);
    }
    let chosen = list
        .iter()
        .find(|m| !pref.is_empty() && m.bus == pref)
        .or_else(|| list.iter().find(|m| m.playing))
        .or_else(|| list.first())
        .cloned()
        .unwrap_or_default();
    (chosen, list)
}

fn media_method(bus: &str, method: &str) {
    spawn_detached(
        "dbus-send",
        &[
            "--user",
            &format!("--dest={bus}"),
            "/org/mpris/MediaPlayer2",
            &format!("org.mpris.MediaPlayer2.Player.{method}"),
        ],
    );
}

fn active_bus() -> Option<String> {
    let list = read_media_all();
    list.iter()
        .find(|m| m.playing)
        .or_else(|| list.first())
        .map(|m| m.bus.clone())
}

pub fn media_toggle_play() {
    if let Some(bus) = active_bus() {
        media_method(&bus, "PlayPause");
    }
}

pub fn media_next() {
    if let Some(bus) = active_bus() {
        media_method(&bus, "Next");
    }
}

pub fn media_prev() {
    if let Some(bus) = active_bus() {
        media_method(&bus, "Previous");
    }
}

pub fn media_seek_relative(secs: i64) {
    if let Some(bus) = active_bus() {
        spawn_detached(
            "dbus-send",
            &[
                "--user",
                &format!("--dest={bus}"),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player.Seek",
                &format!("int64:{}", secs * 1_000_000),
            ],
        );
    }
}

pub fn media_set_position(bus: &str, secs: f64) {
    spawn_detached(
        "dbus-send",
        &[
            "--user",
            &format!("--dest={bus}"),
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player.SetPosition",
            "objpath:/org/mpris/MediaPlayer2/TrackList/NoTrack",
            &format!("int64:{}", (secs.max(0.0) * 1_000_000.0) as i64),
        ],
    );
}

pub fn read_audio_outputs() -> Vec<AudioOutput> {
    let default_sink = Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let Ok(o) = Command::new("pactl").args(["-f", "json", "list", "sinks"]).output() else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) else {
        return Vec::new();
    };
    let Some(sinks) = v.as_array() else { return Vec::new() };
    let mut out = Vec::new();
    for sink in sinks {
        let name = sink.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let desc = sink
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or(&name)
            .to_string();
        let active_port = sink.get("active_port").and_then(|x| x.as_str()).unwrap_or("");
        let ports = sink.get("ports").and_then(|x| x.as_array());
        let mut added = false;
        if let Some(ports) = ports {
            for p in ports {
                let pn = p.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let pd = p
                    .get("description")
                    .and_then(|x| x.as_str())
                    .unwrap_or(&pn)
                    .to_string();
                let active = !default_sink.is_empty() && name == default_sink && pn == active_port;
                out.push(AudioOutput { active, sink: name.clone(), port: pn, label: pd });
                added = true;
            }
        }
        if !added {
            let active = !default_sink.is_empty() && name == default_sink;
            out.push(AudioOutput { active, sink: name.clone(), port: String::new(), label: desc });
        }
    }
    out
}

pub fn select_audio_output(sink: &str, port: &str) {
    if !port.is_empty() {
        let script = format!(
            "pactl set-default-sink '{sink}' && pactl set-sink-port '{sink}' '{port}'"
        );
        spawn_detached("sh", &["-c", &script]);
    } else {
        spawn_detached("pactl", &["set-default-sink", sink]);
    }
}

pub fn toggle_wifi() {
    if let Ok(o) = Command::new("nmcli").args(["-t", "-f", "WIFI", "g"]).output() {
        let on = String::from_utf8_lossy(&o.stdout).trim() == "enabled";
        spawn_detached("nmcli", &["radio", "wifi", if on { "off" } else { "on" }]);
    }
}

pub fn toggle_bluetooth() {
    if let Ok(o) = Command::new("bluetoothctl").arg("show").output() {
        let on = String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.trim() == "Powered: yes");
        spawn_detached("bluetoothctl", &["power", if on { "off" } else { "on" }]);
    }
}

fn read_u64_file(path: &Path) -> u64 {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn read_battery_full() -> (bool, u32, bool, String, String, String, String, String) {
    let Ok(rd) = fs::read_dir("/sys/class/power_supply") else {
        return (false, 100, false, "Unknown".into(), "Unknown".into(), "balance_power".into(), "40°C".into(), "0.8 GHz".into());
    };
    let mut result = None;
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.starts_with("BAT") {
            continue;
        }
        let p = ent.path();
        let cap: u32 = fs::read_to_string(p.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(100);
        let status = fs::read_to_string(p.join("status"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "Unknown".into());
        let power_now = read_u64_file(&p.join("power_now"));
        let energy_now = read_u64_file(&p.join("energy_now"));
        let energy_full = read_u64_file(&p.join("energy_full"));
        let charging = status.eq_ignore_ascii_case("Charging") || status.eq_ignore_ascii_case("Full");
        let detail = if power_now > 100_000 && status.eq_ignore_ascii_case("Discharging") {
            let hours = energy_now as f64 / power_now as f64;
            let h = hours.floor() as u64;
            let m = ((hours - h as f64) * 60.0).round() as u64;
            format!("{:.1} W  •  {}h {}m remaining", power_now as f64 / 1_000_000.0, h, m)
        } else if power_now > 100_000 && status.eq_ignore_ascii_case("Charging") && energy_full > energy_now {
            let hours = (energy_full - energy_now) as f64 / power_now as f64;
            let h = hours.floor() as u64;
            let m = ((hours - h as f64) * 60.0).round() as u64;
            format!("{:.1} W  •  {}h {}m until full", power_now as f64 / 1_000_000.0, h, m)
        } else {
            status.clone()
        };
        result = Some((true, cap.min(100), charging, status, detail, power_now));
        break;
    }

    let epp = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "balance_power".into());

    let mut freq_sum = 0f64;
    let mut freq_n = 0f64;
    if let Ok(rd) = fs::read_dir("/sys/devices/system/cpu") {
        for ent in rd.flatten() {
            let n = ent.file_name().to_string_lossy().to_string();
            if !n.starts_with("cpu") || n == "cpufreq" || n == "cpuidle" {
                continue;
            }
            let f = read_u64_file(&ent.path().join("cpufreq/scaling_cur_freq"));
            if f > 0 {
                freq_sum += f as f64;
                freq_n += 1.0;
            }
        }
    }
    let cpu_freq = if freq_n > 0.0 {
        format!("{:.2} GHz", freq_sum / freq_n / 1_000_000.0)
    } else {
        "0.8 GHz".into()
    };
    let temp_raw = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(40_000.0);
    let cpu_temp = format!("{:.0}°C", temp_raw / 1000.0);

    match result {
        Some((present, cap, charging, status, detail, _pw)) => {
            (present, cap, charging, status, detail, epp, cpu_temp, cpu_freq)
        }
        None => (false, 100, false, "Unknown".into(), "Unknown".into(), epp, cpu_temp, cpu_freq),
    }
}

fn read_volume() -> (u32, bool) {
    let pct = Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split('%')
                .next()
                .and_then(|left| left.split_whitespace().last())
                .and_then(|n| n.parse::<u32>().ok())
        })
        .unwrap_or(50);
    let muted = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("yes"))
        .unwrap_or(false);
    (pct.min(150), muted)
}

fn read_brightness() -> u32 {
    Command::new("brightnessctl")
        .arg("-m")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split(',')
                .nth(3)
                .and_then(|p| p.trim_end_matches('%').parse::<u32>().ok())
        })
        .unwrap_or(60)
        .min(100)
}

pub fn read_monitors() -> Vec<MonitorInfo> {
    let Ok(o) = Command::new("mmsg").args(["get", "all-monitors"]).output() else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) else {
        return Vec::new();
    };
    let Some(arr) = v.get("monitors").and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|m| {
            Some(MonitorInfo {
                name: m.get("name")?.as_str()?.to_string(),
                active: m.get("active").and_then(|x| x.as_bool()).unwrap_or(false),
                width: m.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                height: m.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                scale: m.get("scale").and_then(|x| x.as_f64()).unwrap_or(1.0),
                x: m.get("x").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
                y: m.get("y").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
                transform: m
                    .get("transform")
                    .and_then(|x| x.as_str())
                    .unwrap_or("normal")
                    .to_string(),
                layout_symbol: m
                    .get("layout_symbol")
                    .and_then(|x| x.as_str())
                    .unwrap_or("S")
                    .to_string(),
                tag_num: m.get("tag_num").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            })
        })
        .collect()
}

pub fn refresh_stats_now(stats: &Arc<Mutex<SysStats>>) {
    if let Ok(out) = Command::new("mmsg").args(["get", "all-tags"]).output() {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(tags) = parse_tags_line(&s) {
            if let Ok(mut st) = stats.lock() {
                st.tags = tags;
            }
        }
    }
    let (ram_pct, ram_gb, ram_total) = read_ram();
    let (bat_ok, bat_pct, bat_chg, bat_status, bat_detail, power_profile, cpu_temp, cpu_freq) = read_battery_full();
    let (vol_pct, vol_muted) = read_volume();
    let bri_pct = read_brightness();
    let (wifi_on, wifi_ssid, wifi_sig) = read_wifi();
    let (wifi_enabled, wifi_networks) = read_wifi_networks();
    let (bt_on, bt_paired, bt_available, bt_n) = read_bluetooth_devices();
    let audio_outputs = read_audio_outputs();
    let pref = stats
        .lock()
        .map(|st| st.preferred_player.clone())
        .unwrap_or_default();
    let (media, media_players) = read_media_with_pref(&pref);
    let monitors = read_monitors();
    if let Ok(mut st) = stats.lock() {
        st.monitors = monitors;
        st.ram_percent = ram_pct;
        st.ram_used_gb = ram_gb;
        st.ram_total_gb = ram_total;
        st.battery_present = bat_ok;
        st.battery_percent = bat_pct;
        st.battery_charging = bat_chg;
        st.battery_status = bat_status;
        st.battery_detail = bat_detail;
        st.power_profile = power_profile;
        st.cpu_temp = cpu_temp;
        st.cpu_freq_ghz = cpu_freq;
        st.volume_percent = vol_pct;
        st.volume_muted = vol_muted;
        st.brightness_percent = bri_pct;
        st.wifi_enabled = if wifi_enabled { wifi_on } else { false };
        st.wifi_ssid = wifi_ssid;
        st.wifi_signal = wifi_sig;
        st.wifi_networks = wifi_networks;
        st.bt_powered = bt_on;
        st.bt_connected = bt_n;
        st.bt_paired = bt_paired;
        st.bt_available = bt_available;
        st.audio_outputs = audio_outputs;
        st.media = media;
        st.media_players = media_players;
        st.dirty = true;
    }
}

pub fn refresh_panel_data(stats: &Arc<Mutex<SysStats>>) {
    let (wifi_enabled, wifi_networks) = read_wifi_networks();
    let (bt_on, bt_paired, bt_available, bt_n) = read_bluetooth_devices();
    let audio_outputs = read_audio_outputs();
    if let Ok(mut st) = stats.lock() {
        st.wifi_enabled = if wifi_enabled { st.wifi_enabled } else { false };
        st.wifi_networks = wifi_networks;
        st.bt_powered = bt_on;
        st.bt_connected = bt_n;
        st.bt_paired = bt_paired;
        st.bt_available = bt_available;
        st.audio_outputs = audio_outputs;
        st.wifi_scanning = false;
        st.dirty = true;
    }
}

static TRAY_STDIN: std::sync::OnceLock<Mutex<Option<std::process::ChildStdin>>> =
    std::sync::OnceLock::new();

pub fn tray_send_cmd(cmd: &serde_json::Value) {
    let lock = TRAY_STDIN.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        if let Some(ref mut stdin) = *guard {
            let line = format!("{cmd}\n");
            let _ = stdin.write_all(line.as_bytes());
            let _ = stdin.flush();
        }
    }
}

pub fn spawn_background_services(stats: Arc<Mutex<SysStats>>) {
    // SNI StatusNotifierWatcher + DBusMenu bridge
    let stats_tray = stats.clone();
    thread::spawn(move || {
        let script_path = Path::new("/tmp/yunoshell_tray_watcher.py");
        let _ = fs::write(script_path, include_str!("../scripts/tray_watcher.py"));
        loop {
            if let Ok(mut child) = Command::new("python3")
                .arg("-u")
                .arg(script_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                let stdin = child.stdin.take();
                let lock = TRAY_STDIN.get_or_init(|| Mutex::new(None));
                if let Ok(mut g) = lock.lock() {
                    *g = stdin;
                }
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                            continue;
                        };
                        match v.get("type").and_then(|x| x.as_str()).unwrap_or("") {
                            "items" => {
                                let mut items = Vec::new();
                                if let Some(arr) = v.get("items").and_then(|x| x.as_array()) {
                                    for el in arr {
                                        items.push(TrayItem {
                                            key: el.get("key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            service: el.get("service").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            path: el.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            id: el.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            title: el.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            tooltip: el.get("tooltip").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            icon_name: el.get("icon_name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            icon_png: el.get("icon_png").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            menu_path: el.get("menu_path").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            item_is_menu: el.get("item_is_menu").and_then(|x| x.as_bool()).unwrap_or(false),
                                            status: el.get("status").and_then(|x| x.as_str()).unwrap_or("Active").to_string(),
                                        });
                                    }
                                }
                                if let Ok(mut st) = stats_tray.lock() {
                                    if st.tray_items != items {
                                        st.tray_items = items;
                                        st.dirty = true;
                                    }
                                }
                            }
                            "menu" => {
                                let key = v.get("key").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("Tray").to_string();
                                let mut entries = Vec::new();
                                if let Some(arr) = v.get("items").and_then(|x| x.as_array()) {
                                    for el in arr {
                                        entries.push(TrayMenuItem {
                                            id: el.get("id").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
                                            label: el.get("label").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                            enabled: el.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
                                            sep: el.get("sep").and_then(|x| x.as_bool()).unwrap_or(false),
                                            checked: el.get("checked").and_then(|x| x.as_bool()).unwrap_or(false),
                                        });
                                    }
                                }
                                if let Ok(mut st) = stats_tray.lock() {
                                    st.tray_menu_key = key;
                                    st.tray_menu_title = title;
                                    st.tray_menu_items = entries;
                                    st.dirty = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let Ok(mut g) = lock.lock() {
                    *g = None;
                }
                let _ = child.wait();
            }
            thread::sleep(Duration::from_millis(1200));
        }
    });

    // MangoWM tags watcher thread
    let stats_tags = stats.clone();
    thread::spawn(move || loop {
        if let Ok(out) = Command::new("mmsg").args(["get", "all-tags"]).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(tags) = parse_tags_line(&s) {
                if let Ok(mut st) = stats_tags.lock() {
                    st.tags = tags;
                    st.dirty = true;
                }
            }
        }
        if let Ok(mut child) = Command::new("mmsg")
            .args(["watch", "all-tags"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(tags) = parse_tags_line(&line) {
                        if let Ok(mut st) = stats_tags.lock() {
                            st.tags = tags;
                            st.dirty = true;
                        }
                    }
                }
            }
            let _ = child.wait();
        }
        thread::sleep(Duration::from_millis(1200));
    });

    // Periodic lightweight system poller
    let stats_sys = stats.clone();
    thread::spawn(move || {
        let mut prev_cpu = read_cpu_jiffies();
        let mut prev_net = read_net_bytes();
        let mut prev_t = std::time::Instant::now();
        loop {
            let (ram_pct, ram_gb, ram_total) = read_ram();
            let (swap_u, swap_t) = read_swap();
            let (bat_ok, bat_pct, bat_chg, bat_status, bat_detail, power_profile, cpu_temp, cpu_freq) = read_battery_full();
            let (vol_pct, vol_muted) = read_volume();
            let bri_pct = read_brightness();
            let cur_cpu = read_cpu_jiffies();
            let dt_total = cur_cpu.0.saturating_sub(prev_cpu.0);
            let dt_idle = cur_cpu.1.saturating_sub(prev_cpu.1);
            let cpu_pct = if dt_total > 0 {
                (((dt_total.saturating_sub(dt_idle)) as f64 * 100.0 / dt_total as f64).round() as u32).min(100)
            } else {
                3
            };
            prev_cpu = cur_cpu;

            let cur_net = read_net_bytes();
            let elapsed_s = prev_t.elapsed().as_secs_f64().max(0.2);
            prev_t = std::time::Instant::now();
            let down_kbps = (cur_net.0.saturating_sub(prev_net.0) as f64 / 1024.0) / elapsed_s;
            let up_kbps = (cur_net.1.saturating_sub(prev_net.1) as f64 / 1024.0) / elapsed_s;
            prev_net = cur_net;

            let (uptime, procs, load) = read_uptime_and_load();
            if let Ok(mut st) = stats_sys.lock() {
                if st.ram_percent != ram_pct
                    || st.ram_total_gb != ram_total
                    || st.battery_percent != bat_pct
                    || st.battery_charging != bat_chg
                    || st.volume_percent != vol_pct
                    || st.volume_muted != vol_muted
                    || st.brightness_percent != bri_pct
                    || st.cpu_percent != cpu_pct
                {
                    st.dirty = true;
                }
                st.ram_percent = ram_pct;
                st.ram_used_gb = ram_gb;
                st.ram_total_gb = ram_total;
                st.swap_used_gb = swap_u;
                st.swap_total_gb = swap_t;
                st.cpu_percent = cpu_pct;
                if st.cpu_history.len() >= 24 {
                    st.cpu_history.remove(0);
                }
                st.cpu_history.push(cpu_pct);
                if st.ram_history.len() >= 24 {
                    st.ram_history.remove(0);
                }
                st.ram_history.push(ram_pct);
                st.net_down_kbps = down_kbps;
                st.net_up_kbps = up_kbps;
                st.sys_uptime = uptime;
                st.sys_process_count = procs;
                st.sys_load_avg = load;
                st.battery_present = bat_ok;
                st.battery_percent = bat_pct;
                st.battery_charging = bat_chg;
                st.battery_status = bat_status;
                st.battery_detail = bat_detail;
                st.power_profile = power_profile;
                st.cpu_temp = cpu_temp;
                st.cpu_freq_ghz = cpu_freq;
                st.volume_percent = vol_pct;
                st.volume_muted = vol_muted;
                st.brightness_percent = bri_pct;
            }
            thread::sleep(Duration::from_millis(1800));
        }
    });

    // Network / Bluetooth / media poller (internet state changes slowly)
    let stats_net = stats.clone();
    thread::spawn(move || loop {
        let (wifi_on, wifi_ssid, wifi_sig) = read_wifi();
        let (bt_on, bt_n) = read_bluetooth();
        let pref = stats_net
            .lock()
            .map(|st| st.preferred_player.clone())
            .unwrap_or_default();
        let (media, media_players) = read_media_with_pref(&pref);
        if let Ok(mut st) = stats_net.lock() {
            let media_changed = st.media.active != media.active
                || st.media.playing != media.playing
                || st.media.title != media.title
                || st.media.artist != media.artist
                || st.media.album != media.album
                || st.media.art_url != media.art_url
                || st.media.player != media.player;
            if st.wifi_enabled != wifi_on
                || st.wifi_ssid != wifi_ssid
                || st.wifi_signal != wifi_sig
                || st.bt_powered != bt_on
                || st.bt_connected != bt_n
                || media_changed
            {
                st.dirty = true;
            }
            st.wifi_enabled = wifi_on;
            st.wifi_ssid = wifi_ssid;
            st.wifi_signal = wifi_sig;
            st.bt_powered = bt_on;
            st.bt_connected = bt_n;
            st.media = media;
            st.media_players = media_players;
        }
        thread::sleep(Duration::from_millis(2500));
    });

    // Heavier panel data (network list, bluetooth pairs, audio ports)
    thread::spawn(move || loop {
        let (wifi_enabled, wifi_networks) = read_wifi_networks();
        let (bt_on, bt_paired, bt_available, bt_n) = read_bluetooth_devices();
        let audio_outputs = read_audio_outputs();
        let monitors = read_monitors();
        if let Ok(mut st) = stats.lock() {
            st.wifi_enabled = if wifi_enabled { st.wifi_enabled } else { false };
            st.wifi_networks = wifi_networks;
            st.bt_powered = bt_on;
            st.bt_connected = bt_n;
            st.bt_paired = bt_paired;
            st.bt_available = bt_available;
            st.audio_outputs = audio_outputs;
            if !monitors.is_empty() {
                st.monitors = monitors;
            }
            st.dirty = true;
        }
        thread::sleep(Duration::from_millis(6000));
    });
}

// ==================== ANTIGRAVITY MINI ====================

#[derive(Clone, Debug)]
pub struct AgyMsg {
    pub user: bool,
    pub text: String,
}

pub struct AgyModel {
    pub id: &'static str,
    pub label: &'static str,
    pub efforts: &'static [(&'static str, &'static str)],
}

const EFFORTS_STD: &[(&str, &str)] = &[("low", "Low"), ("medium", "Med"), ("high", "High")];

pub const AGY_MODELS: &[AgyModel] = &[
    AgyModel { id: "gemini-3.8-flash", label: "Flash", efforts: EFFORTS_STD },
    AgyModel { id: "gemini-4-argon", label: "Argon", efforts: EFFORTS_STD },
    AgyModel { id: "claude-sonnet-5-5", label: "Sonnet", efforts: EFFORTS_STD },
    AgyModel {
        id: "gemini-4-hydrogen",
        label: "Hydrogen",
        efforts: &[("low", "Low"), ("medium", "Med"), ("high", "High"), ("xhigh", "XHigh")],
    },
    AgyModel { id: "claude-opus-5-5", label: "Opus", efforts: EFFORTS_STD },
    AgyModel { id: "gpt-oss-120b", label: "GPT-OSS", efforts: &[("medium", "Med")] },
];

#[derive(Clone, Debug)]
pub enum AgyEvent {
    Status(String),
    Delta(String),
    Done {
        conv: String,
        text: String,
        meta: String,
    },
}

/// Runs the local `agy` CLI and streams its JSON events over a channel.
pub fn agy_send(prompt: &str, model: &str, effort: &str, conv: &str) -> Receiver<AgyEvent> {
    let (tx, rx) = mpsc::channel();
    let full = format!(
        "[Antigravity Mini: Reply concisely, directly, and straight to the point without greetings or filler. Keep formatting compact and match the user's language.]\n\n{prompt}"
    );
    let conv_owned = conv.to_string();
    let model_owned = model.to_string();
    let effort_owned = effort.to_string();

    let mut cmd = Command::new("/home/huyle/.local/bin/agy");
    cmd.arg("-p")
        .arg(&full)
        .args([
            "--output-format",
            "stream-json",
            "--model",
            model,
            "--effort",
            effort,
            "--print-timeout",
            "120s",
        ])
        .current_dir("/home/huyle")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if !conv.is_empty() {
        cmd.args(["--conversation", conv]);
    }

    thread::spawn(move || {
        let Ok(mut child) = cmd.spawn() else {
            let _ = tx.send(AgyEvent::Done {
                conv: String::new(),
                text: String::new(),
                meta: "Failed to launch agy".into(),
            });
            return;
        };
        let mut final_conv = conv_owned;
        let mut final_text = String::new();
        let mut final_meta = format!("{model_owned} · {effort_owned}");
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let Ok(ev) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some(su) = ev.get("step_update") {
                    if let Some(c) = su.get("conversation_id").and_then(|x| x.as_str()) {
                        if !c.is_empty() {
                            final_conv = c.to_string();
                        }
                    }
                    match su.get("step_type").and_then(|x| x.as_str()).unwrap_or("") {
                        "thinking" => {
                            let _ = tx.send(AgyEvent::Status("Thinking...".into()));
                        }
                        "tool_call" | "command_execution" => {
                            let _ = tx.send(AgyEvent::Status("Running tool...".into()));
                        }
                        "agent_response" => {
                            let _ = tx.send(AgyEvent::Status("Streaming...".into()));
                            if let Some(d) = su.get("text_delta").and_then(|x| x.as_str()) {
                                if !d.is_empty() {
                                    let _ = tx.send(AgyEvent::Delta(d.to_string()));
                                }
                            }
                        }
                        _ => {}
                    }
                } else if let Some(res) = ev.get("result") {
                    if let Some(c) = res.get("conversation_id").and_then(|x| x.as_str()) {
                        if !c.is_empty() {
                            final_conv = c.to_string();
                        }
                    }
                    if let Some(t) = res.get("response").and_then(|x| x.as_str()) {
                        final_text = t.to_string();
                    }
                    let dur = res
                        .get("duration_seconds")
                        .and_then(|x| x.as_f64())
                        .unwrap_or(0.0);
                    final_meta = if dur > 0.0 {
                        format!("{model_owned} · {effort_owned} · {dur:.1}s")
                    } else {
                        format!("{model_owned} · {effort_owned}")
                    };
                }
            }
        }
        let _ = child.wait();
        let _ = tx.send(AgyEvent::Done {
            conv: final_conv,
            text: final_text,
            meta: final_meta,
        });
    });
    rx
}

#[derive(Clone, Debug)]
pub struct MangoSummary {
    pub gappih: i32,
    pub gappoh: i32,
    pub borderpx: i32,
    pub border_radius: i32,
    pub blur: bool,
    pub shadows: bool,
    pub animations: bool,
    pub anim_open: String,
    pub anim_close: String,
}

impl Default for MangoSummary {
    fn default() -> Self {
        Self {
            gappih: 6,
            gappoh: 6,
            borderpx: 2,
            border_radius: 10,
            blur: false,
            shadows: false,
            animations: true,
            anim_open: "slide".into(),
            anim_close: "slide".into(),
        }
    }
}

const MANGO_CONF: &str = "/home/huyle/.config/mango/config.conf";

pub fn read_mango_summary() -> MangoSummary {
    let mut out = MangoSummary::default();
    let Ok(txt) = fs::read_to_string(MANGO_CONF) else {
        return out;
    };
    for line in txt.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        match k {
            "gappih" => {
                if let Ok(n) = v.parse::<i32>() {
                    out.gappih = n;
                }
            }
            "gappoh" => {
                if let Ok(n) = v.parse::<i32>() {
                    out.gappoh = n;
                }
            }
            "borderpx" => {
                if let Ok(n) = v.parse::<i32>() {
                    out.borderpx = n;
                }
            }
            "border_radius" => {
                if let Ok(n) = v.parse::<i32>() {
                    out.border_radius = n;
                }
            }
            "blur" => out.blur = v == "1" || v.eq_ignore_ascii_case("true"),
            "shadows" => out.shadows = v == "1" || v.eq_ignore_ascii_case("true"),
            "animations" => out.animations = v == "1" || v.eq_ignore_ascii_case("true"),
            "animation_type_open" => out.anim_open = v.to_string(),
            "animation_type_close" => out.anim_close = v.to_string(),
            _ => {}
        }
    }
    out
}

pub fn set_mango_key(key: &str, val: &str) {
    let Ok(txt) = fs::read_to_string(MANGO_CONF) else {
        return;
    };
    let mut found = false;
    let mut lines: Vec<String> = txt
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with('#') {
                if let Some((k, _)) = trimmed.split_once('=') {
                    if k.trim() == key {
                        found = true;
                        return format!("{key}={val}");
                    }
                }
            }
            line.to_string()
        })
        .collect();
    if !found {
        lines.push(format!("{key}={val}"));
    }
    let _ = fs::write(MANGO_CONF, format!("{}\n", lines.join("\n")));
    spawn_detached("mmsg", &["-d", "reload_config"]);
}

pub fn trigger_system_theme_apply(cfg: &crate::theme::ShellConfig, theme: &crate::theme::Theme) {
    let py = "/home/huyle/.config/quickshell/scripts/apply-system-theme.py";
    if Path::new(py).is_file() {
        let payload = serde_json::json!({
            "themeName": cfg.theme_name,
            "label": if cfg.current_theme.label.is_empty() {
                crate::theme::theme_preset_by_name(&cfg.theme_name).label
            } else {
                cfg.current_theme.label.clone()
            },
            "bg": crate::theme::rgb_to_hex(theme.bg),
            "bgDeep": crate::theme::rgb_to_hex(theme.bg_deep),
            "surface": crate::theme::rgb_to_hex(theme.surface),
            "surfaceHover": crate::theme::rgb_to_hex(theme.surface_hover),
            "surfaceActive": crate::theme::rgb_to_hex(theme.surface_active),
            "border": crate::theme::rgb_to_hex(theme.border),
            "text": crate::theme::rgb_to_hex(theme.text),
            "textBright": crate::theme::rgb_to_hex(theme.text_bright),
            "muted": crate::theme::rgb_to_hex(theme.muted),
            "dim": crate::theme::rgb_to_hex(theme.muted),
            "accent": crate::theme::rgb_to_hex(theme.accent),
            "accentText": crate::theme::rgb_to_hex(theme.accent_text),
            "red": crate::theme::rgb_to_hex(theme.red),
            "orange": crate::theme::rgb_to_hex(theme.orange),
            "yellow": crate::theme::rgb_to_hex(theme.yellow),
            "green": crate::theme::rgb_to_hex(theme.green),
            "purple": crate::theme::rgb_to_hex(theme.purple),
            "cyan": crate::theme::rgb_to_hex(theme.cyan),
            "blue": crate::theme::rgb_to_hex(theme.blue),
        });
        spawn_detached("python3", &[py, &payload.to_string()]);
    }
}

pub fn list_wallpaper_paths(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_file() {
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                if is_image_file(name) {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out.truncate(36);
    out
}

pub fn ensure_thumb_png(path: &Path, w: i32, h: i32) -> Option<PathBuf> {
    if !path.is_file() {
        return None;
    }
    let cache_dir = Path::new("/tmp/yunoshell-thumbs");
    let _ = fs::create_dir_all(cache_dir);
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in path.to_string_lossy().as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let cache_file = cache_dir.join(format!("{hash:016x}_{mtime}_{w}x{h}.png"));
    if !cache_file.is_file() {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if is_video_file(name) {
            let vf = format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}");
            let ok = Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-ss", "00:00:01", "-i"])
                .arg(path)
                .args(["-vframes", "1", "-vf", &vf])
                .arg(&cache_file)
                .output()
                .ok()
                .map(|o| o.status.success() && cache_file.is_file())
                .unwrap_or(false);
            if !ok {
                let ok2 = Command::new("ffmpeg")
                    .args(["-y", "-loglevel", "error", "-ss", "00:00:00", "-i"])
                    .arg(path)
                    .args(["-vframes", "1", "-vf", &vf])
                    .arg(&cache_file)
                    .output()
                    .ok()
                    .map(|o| o.status.success() && cache_file.is_file())
                    .unwrap_or(false);
                if !ok2 {
                    return None;
                }
            }
        } else {
            let geom = format!("{w}x{h}^");
            let ext = format!("{w}x{h}");
            let status = Command::new("magick")
                .arg(path)
                .args([
                    "-auto-orient",
                    "-thumbnail",
                    &geom,
                    "-gravity",
                    "center",
                    "-extent",
                    &ext,
                ])
                .arg(&cache_file)
                .output()
                .ok()?
                .status;
            if !status.success() {
                return None;
            }
        }
    }
    Some(cache_file)
}

pub fn load_thumb_surface(path: &Path, w: i32, h: i32) -> Option<cairo::ImageSurface> {
    let cache_file = ensure_thumb_png(path, w, h)?;
    load_png_surface(&cache_file)
}

pub fn capture_screen_ppm() -> Option<(i32, i32, Vec<u8>)> {
    let out = Command::new("grim").args(["-t", "ppm", "-"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let data = out.stdout;
    if !data.starts_with(b"P6") {
        return None;
    }
    let mut i = 2usize;
    let mut tokens: Vec<i32> = Vec::with_capacity(3);
    while tokens.len() < 3 && i < data.len() {
        while i < data.len() {
            if data[i].is_ascii_whitespace() {
                i += 1;
            } else if data[i] == b'#' {
                while i < data.len() && data[i] != b'\n' {
                    i += 1;
                }
            } else {
                break;
            }
        }
        let start = i;
        while i < data.len() && !data[i].is_ascii_whitespace() && data[i] != b'#' {
            i += 1;
        }
        if start == i {
            return None;
        }
        let val = std::str::from_utf8(&data[start..i]).ok()?.parse::<i32>().ok()?;
        tokens.push(val);
    }
    if tokens.len() < 3 || i >= data.len() {
        return None;
    }
    // Single whitespace separator before raw RGB payload
    i += 1;
    let (w, h, _maxv) = (tokens[0], tokens[1], tokens[2]);
    let expected = (w as usize).checked_mul(h as usize)?.checked_mul(3)?;
    if data.len().saturating_sub(i) < expected {
        return None;
    }
    Some((w, h, data[i..i + expected].to_vec()))
}


