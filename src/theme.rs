use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

pub const SETTINGS_PATH: &str = "/home/huyle/.config/quickshell/settings.json";
pub const USER_CONFIG_PATH: &str = "/home/huyle/.config/yunoshell/config.json";

#[derive(Clone, Copy, Debug)]
pub struct Rgb(pub f64, pub f64, pub f64);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub label: String,
    pub dir: String,
    pub icon: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ShellConfig {
    pub theme_name: String,
    pub accent_preset: String,
    pub bar_position: String,
    pub bar_size: u32,
    pub bar_opacity: f64,
    pub popup_opacity: f64,
    pub bar_floating: bool,
    pub corner_radius: f64,
    pub show_ram_on_bar: bool,
    pub show_media_on_bar: bool,
    pub show_tray_on_bar: bool,
    pub lock_24_hour: bool,
    pub lock_show_clock: bool,
    pub lock_use_same_wallpaper: bool,
    pub lock_wallpaper_path: String,
    pub lock_dim_opacity: f64,
    pub wallpaper_dir: String,
    pub wallpaper_path: String,
    pub wallpaper_mode: String,
    pub bookmarks: Vec<Bookmark>,
    // Hot-reloadable Animation & Theme overrides (~/.config/yunoshell/config.json or settings.json)
    pub anim_open_ms: f64,
    pub anim_close_ms: f64,
    pub anim_content_ms: f64,
    pub anim_morph_ms: f64,
    pub anim_curve: [f64; 4],
    pub color_overrides: Map<String, Value>,
    // Runtime-only, not persisted
    pub current_theme: ThemePreset,
    pub sddm_applying: bool,
    pub sddm_status: String,
    pub sddm_status_error: bool,
    pub sddm_theme: String,
    pub sddm_current_bg: String,
    pub sddm_selected_path: String,
    pub sddm_bg_revision: u32,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct ThemePreset {
    pub label: String,
    pub accent: String,
    pub red: String,
    pub orange: String,
    pub yellow: String,
    pub green: String,
    pub purple: String,
    pub cyan: String,
    pub blue: String,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct Theme {
    pub bg: Rgb,
    pub bg_deep: Rgb,
    pub surface: Rgb,
    pub surface_hover: Rgb,
    pub surface_active: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub text_bright: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub accent_text: Rgb,
    pub red: Rgb,
    pub orange: Rgb,
    pub yellow: Rgb,
    pub green: Rgb,
    pub purple: Rgb,
    pub cyan: Rgb,
    pub blue: Rgb,
}

pub fn hex_to_rgb(hex: &str) -> Rgb {
    let s = hex.trim_start_matches('#');
    if s.len() < 6 {
        return Rgb(0.18, 0.2, 0.25);
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(30) as f64 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(30) as f64 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(46) as f64 / 255.0;
    Rgb(r, g, b)
}

fn darken(c: Rgb, factor: f64) -> Rgb {
    Rgb(
        (c.0 * factor).clamp(0.0, 1.0),
        (c.1 * factor).clamp(0.0, 1.0),
        (c.2 * factor).clamp(0.0, 1.0),
    )
}

pub fn rgb_to_hex(c: Rgb) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.0.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.1.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.2.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

pub fn ensure_user_config_exists() {
    let p = Path::new(USER_CONFIG_PATH);
    if p.exists() {
        return;
    }
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let default_cfg = serde_json::json!({
        "_note": "Auto-reloaded live on save (<=250ms). Overrides /home/huyle/.config/quickshell/settings.json without rebuilding.",
        "animOpenMs": 320,
        "animCloseMs": 240,
        "animContentMs": 240,
        "animMorphMs": 280,
        "animCurve": "smooth",
        "colors": {}
    });
    if let Ok(pretty) = serde_json::to_string_pretty(&default_cfg) {
        let _ = fs::write(p, pretty);
    }
}

pub fn config_mtimes() -> (Option<SystemTime>, Option<SystemTime>) {
    let m1 = fs::metadata(SETTINGS_PATH).and_then(|m| m.modified()).ok();
    let m2 = fs::metadata(USER_CONFIG_PATH).and_then(|m| m.modified()).ok();
    (m1, m2)
}

pub fn read_settings_value() -> Value {
    fs::read_to_string(SETTINGS_PATH)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn read_merged_config_value() -> Value {
    let mut base = read_settings_value();
    if let Ok(raw) = fs::read_to_string(USER_CONFIG_PATH) {
        if let Ok(Value::Object(ovr)) = serde_json::from_str::<Value>(&raw) {
            if let Some(base_obj) = base.as_object_mut() {
                for (k, v) in ovr {
                    if !k.starts_with('_') {
                        base_obj.insert(k, v);
                    }
                }
            }
        }
    }
    base
}

pub fn write_settings_value(val: &Value) {
    if let Some(parent) = Path::new(SETTINGS_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(pretty) = serde_json::to_string_pretty(val) {
        let _ = fs::write(SETTINGS_PATH, pretty);
    }
}

fn parse_anim_curve(v: Option<&Value>) -> [f64; 4] {
    match v {
        Some(Value::String(s)) => match s.to_lowercase().as_str() {
            "soft" => [0.25, 0.96, 0.32, 1.0],
            "expo" => [0.16, 1.0, 0.3, 1.0],
            "snappy" => [0.2, 0.85, 0.25, 1.0],
            _ => [0.22, 1.0, 0.36, 1.0],
        },
        Some(Value::Array(arr)) if arr.len() >= 4 => [
            arr[0].as_f64().unwrap_or(0.22).clamp(0.0, 1.0),
            arr[1].as_f64().unwrap_or(1.0).clamp(0.0, 1.4),
            arr[2].as_f64().unwrap_or(0.36).clamp(0.0, 1.0),
            arr[3].as_f64().unwrap_or(1.0).clamp(0.0, 1.4),
        ],
        _ => [0.22, 1.0, 0.36, 1.0],
    }
}

pub fn load_config() -> ShellConfig {
    ensure_user_config_exists();
    let v = read_merged_config_value();
    let get_str = |k: &str, def: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or(def)
            .to_string()
    };
    let bookmarks = v
        .get("fileBookmarks")
        .and_then(|x| serde_json::from_value::<Vec<Bookmark>>(x.clone()).ok())
        .unwrap_or_default();
    let color_overrides = v
        .get("colors")
        .and_then(|x| x.as_object())
        .cloned()
        .unwrap_or_default();

    let theme_name = get_str("themeName", "catppuccin");
    let current_theme = theme_preset_by_name(&theme_name);

    ShellConfig {
        theme_name,
        accent_preset: get_str("accentPreset", "theme"),
        bar_position: get_str("barPosition", "left"),
        bar_size: v.get("barSize").and_then(|x| x.as_u64()).unwrap_or(34).clamp(26, 56) as u32,
        bar_opacity: v.get("barOpacity").and_then(|x| x.as_f64()).unwrap_or(0.97).clamp(0.4, 1.0),
        popup_opacity: v.get("popupOpacity").and_then(|x| x.as_f64()).unwrap_or(0.98).clamp(0.55, 1.0),
        bar_floating: v.get("barFloating").and_then(|x| x.as_bool()).unwrap_or(false),
        corner_radius: v.get("cornerRadius").and_then(|x| x.as_f64()).unwrap_or(0.0).clamp(0.0, 28.0),
        show_ram_on_bar: v.get("showRamOnBar").and_then(|x| x.as_bool()).unwrap_or(true),
        show_media_on_bar: v.get("showMediaOnBar").and_then(|x| x.as_bool()).unwrap_or(true),
        show_tray_on_bar: v.get("showTrayOnBar").and_then(|x| x.as_bool()).unwrap_or(true),
        lock_24_hour: v.get("lock24Hour").and_then(|x| x.as_bool()).unwrap_or(true),
        lock_show_clock: v.get("lockShowClock").and_then(|x| x.as_bool()).unwrap_or(true),
        lock_use_same_wallpaper: v.get("lockUseSameWallpaper").and_then(|x| x.as_bool()).unwrap_or(true),
        lock_wallpaper_path: get_str("lockWallpaperPath", "/home/huyle/.config/mango/wallpaper.jpg"),
        lock_dim_opacity: v.get("lockDimOpacity").and_then(|x| x.as_f64()).unwrap_or(0.45).clamp(0.0, 0.85),
        wallpaper_dir: get_str("wallpaperDir", "/home/huyle/Pictures"),
        wallpaper_path: get_str("wallpaperPath", "/home/huyle/.config/mango/wallpaper.jpg"),
        wallpaper_mode: get_str("wallpaperMode", "fill"),
        bookmarks,
        anim_open_ms: v.get("animOpenMs").and_then(|x| x.as_f64()).unwrap_or(320.0).clamp(80.0, 1200.0),
        anim_close_ms: v.get("animCloseMs").and_then(|x| x.as_f64()).unwrap_or(240.0).clamp(60.0, 1000.0),
        anim_content_ms: v.get("animContentMs").and_then(|x| x.as_f64()).unwrap_or(240.0).clamp(50.0, 800.0),
        anim_morph_ms: v.get("animMorphMs").and_then(|x| x.as_f64()).unwrap_or(280.0).clamp(60.0, 1000.0),
        anim_curve: parse_anim_curve(v.get("animCurve")),
        color_overrides,
        current_theme,
        sddm_applying: false,
        sddm_status: String::new(),
        sddm_status_error: false,
        sddm_theme: "minimal".into(),
        sddm_current_bg: "/usr/share/sddm/themes/minimal/background.jpg".into(),
        sddm_selected_path: String::new(),
        sddm_bg_revision: 0,
    }
}

pub fn theme_preset_by_name(name: &str) -> ThemePreset {
    match name {
        "nord" => ThemePreset {
            label: "Nord".into(),
            accent: "#c6d0f5".into(),
            red: "#bf616a".into(),
            orange: "#d08770".into(),
            yellow: "#ebcb8b".into(),
            green: "#a3be8c".into(),
            purple: "#b48ead".into(),
            cyan: "#88c0d0".into(),
            blue: "#81a1c1".into(),
        },
        "catppuccin" => ThemePreset {
            label: "Catppuccin".into(),
            accent: "#cba6f7".into(),
            red: "#f38ba8".into(),
            orange: "#fab387".into(),
            yellow: "#f9e2af".into(),
            green: "#a6e3a1".into(),
            purple: "#cba6f7".into(),
            cyan: "#89dceb".into(),
            blue: "#89b4fa".into(),
        },
        "gruvbox" => ThemePreset {
            label: "Gruvbox".into(),
            accent: "#fabd2f".into(),
            red: "#fb4934".into(),
            orange: "#fe8019".into(),
            yellow: "#fabd2f".into(),
            green: "#b8bb26".into(),
            purple: "#d3869b".into(),
            cyan: "#8ec07c".into(),
            blue: "#83a598".into(),
        },
        "tokyonight" => ThemePreset {
            label: "Tokyo Night".into(),
            accent: "#7aa2f7".into(),
            red: "#f7768e".into(),
            orange: "#ff9e64".into(),
            yellow: "#e0af68".into(),
            green: "#9ece6a".into(),
            purple: "#bb9af7".into(),
            cyan: "#7dcfff".into(),
            blue: "#7aa2f7".into(),
        },
        "dracula" => ThemePreset {
            label: "Dracula".into(),
            accent: "#bd93f9".into(),
            red: "#ff5555".into(),
            orange: "#ffb86c".into(),
            yellow: "#f1fa8c".into(),
            green: "#50fa7b".into(),
            purple: "#bd93f9".into(),
            cyan: "#8be9fd".into(),
            blue: "#6272a4".into(),
        },
        "rosepine" => ThemePreset {
            label: "Rosé Pine".into(),
            accent: "#ebbcba".into(),
            red: "#eb6f92".into(),
            orange: "#f6c177".into(),
            yellow: "#f6c177".into(),
            green: "#9ccfd8".into(),
            purple: "#c4a7e7".into(),
            cyan: "#9ccfd8".into(),
            blue: "#31748f".into(),
        },
        "everforest" => ThemePreset {
            label: "Everforest".into(),
            accent: "#a7c080".into(),
            red: "#e67e80".into(),
            orange: "#e69875".into(),
            yellow: "#dbbc7f".into(),
            green: "#a7c080".into(),
            purple: "#d699b6".into(),
            cyan: "#83c092".into(),
            blue: "#7fbbb3".into(),
        },
        _ => ThemePreset {
            label: "Catppuccin".into(),
            accent: "#cba6f7".into(),
            red: "#f38ba8".into(),
            orange: "#fab387".into(),
            yellow: "#f9e2af".into(),
            green: "#a6e3a1".into(),
            purple: "#cba6f7".into(),
            cyan: "#89dceb".into(),
            blue: "#89b4fa".into(),
        },
    }
}

pub fn save_bookmarks(bookmarks: &[Bookmark]) {
    let mut v = read_settings_value();
    if let Some(obj) = v.as_object_mut() {
        if let Ok(val) = serde_json::to_value(bookmarks) {
            obj.insert("fileBookmarks".to_string(), val);
        }
    }
    write_settings_value(&v);
}

pub fn theme_from_config(cfg: &ShellConfig) -> Theme {
    let (bg, surf, hover, active, border, text, bright, muted, def_acc, def_acc_txt, red, orange, yellow, green, purple, cyan, blue) =
        match cfg.theme_name.as_str() {
            "nord" => (
                "#2e3440", "#323846", "#3e4658", "#4c566a", "#434c5e", "#c6d0f5", "#eceff4", "#9099ab",
                "#c6d0f5", "#2e3440", "#bf616a", "#d08770", "#ebcb8b", "#a3be8c", "#b48ead", "#88c0d0", "#81a1c1",
            ),
            "gruvbox" => (
                "#282828", "#32302f", "#423e3c", "#504945", "#504945", "#ebdbb2", "#fbf1c7", "#a89984",
                "#fabd2f", "#282828", "#fb4934", "#fe8019", "#fabd2f", "#b8bb26", "#d3869b", "#8ec07c", "#83a598",
            ),
            "tokyonight" => (
                "#1a1b26", "#222536", "#2f344d", "#414868", "#333952", "#c0caf5", "#e0e7ff", "#8897d6",
                "#7aa2f7", "#1a1b26", "#f7768e", "#ff9e64", "#e0af68", "#9ece6a", "#bb9af7", "#7dcfff", "#7aa2f7",
            ),
            "dracula" => (
                "#282a36", "#313442", "#414457", "#535770", "#44475a", "#f8f8f2", "#ffffff", "#9ea7cc",
                "#bd93f9", "#282a36", "#ff5555", "#ffb86c", "#f1fa8c", "#50fa7b", "#bd93f9", "#8be9fd", "#6272a4",
            ),
            "rosepine" => (
                "#191724", "#211f30", "#2d2a42", "#403d52", "#403d52", "#e0def4", "#faf4ed", "#908caa",
                "#ebbcba", "#191724", "#eb6f92", "#f6c177", "#f6c177", "#9ccfd8", "#c4a7e7", "#9ccfd8", "#31748f",
            ),
            "everforest" => (
                "#2d353b", "#343f44", "#414b50", "#4f585e", "#475258", "#d3c6aa", "#ede4ce", "#859289",
                "#a7c080", "#2d353b", "#e67e80", "#e69875", "#dbbc7f", "#a7c080", "#d699b6", "#83c092", "#7fbbb3",
            ),
            _ => (
                "#1e1e2e", "#262638", "#36384f", "#45475a", "#45475a", "#cdd6f4", "#f5e0dc", "#a6adc8",
                "#cba6f7", "#1e1e2e", "#f38ba8", "#fab387", "#f9e2af", "#a6e3a1", "#cba6f7", "#89dceb", "#89b4fa",
            ),
        };

    let acc = match cfg.accent_preset.as_str() {
        "lavender" => "#c6d0f5",
        "frost" => "#88c0d0",
        "blue" => "#81a1c1",
        "sage" => "#a3be8c",
        "amber" => "#ebcb8b",
        "rose" => "#bf616a",
        "snow" => "#eceff4",
        _ => def_acc,
    };
    let acc_txt = if cfg.accent_preset == "theme" { def_acc_txt } else { bg };
    let ovr = |key: &str, default: Rgb| -> Rgb {
        cfg.color_overrides
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| s.starts_with('#') && s.len() >= 7)
            .map(hex_to_rgb)
            .unwrap_or(default)
    };
    let bg_rgb = ovr("bg", hex_to_rgb(bg));

    Theme {
        bg: bg_rgb,
        bg_deep: ovr("bgDeep", darken(bg_rgb, 0.82)),
        surface: ovr("surface", hex_to_rgb(surf)),
        surface_hover: ovr("surfaceHover", hex_to_rgb(hover)),
        surface_active: ovr("surfaceActive", hex_to_rgb(active)),
        border: ovr("border", hex_to_rgb(border)),
        text: ovr("text", hex_to_rgb(text)),
        text_bright: ovr("textBright", hex_to_rgb(bright)),
        muted: ovr("muted", hex_to_rgb(muted)),
        accent: ovr("accent", hex_to_rgb(acc)),
        accent_text: ovr("accentText", hex_to_rgb(acc_txt)),
        red: ovr("red", hex_to_rgb(red)),
        orange: ovr("orange", hex_to_rgb(orange)),
        yellow: ovr("yellow", hex_to_rgb(yellow)),
        green: ovr("green", hex_to_rgb(green)),
        purple: ovr("purple", hex_to_rgb(purple)),
        cyan: ovr("cyan", hex_to_rgb(cyan)),
        blue: ovr("blue", hex_to_rgb(blue)),
    }
}
