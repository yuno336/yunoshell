# yunoshell — Project Handoff & 1:1 Quickshell Port Progress

> [!CAUTION]
> ## CRITICAL USER VERDICT & TECHNICAL REALITY CHECK (Read First!)
> After testing OpenCode's latest `/home/huyle/.local/bin/yunoshell` build (`Nov 16 16:15`, `~9,200` LOC) against live Quickshell (`PID 201668`):
> - **Animation is still bad / stutters** compared to Quickshell (Quickshell runs buttery 60fps; `yunoshell` feels choppy).
> - **UI looks bad and does NOT look like Quickshell at all** (broken typography/icon proportions, awkward widget spacing, missing Qt/QML-grade hover scale/color polish).
> - **DO NOT keep patching raw `(x, y)` coordinates in Cairo `src/render.rs` (`3,800+` lines).** Read the root causes below before writing more code!

### Root Causes of Why Current `yunoshell` Feels & Looks Bad
1. ~~**Fake "GPU" Path**~~ ✅ **FIXED (Nov 16 15:45):** `gpu_image` now allocates only the **island card bounding box** (typically `~900×400` max) instead of fullscreen `1536×960`. Cairo draws the island at local `(0,0)` into this small surface; GPU uploads the entire tiny texture once per frame and draws a single quad at the island's screen position. This eliminates the massive CPU clear/paint/upload overhead. **Remaining:** True GPU-side SDF geometry (rounded rects, ears, morph) still done on CPU via Cairo — see §3.3.
2. **Cairo Toy Text API (`cr.select_font_face`) Instead of Pango / HarfBuzz / Real Layout:**
   - `src/render.rs` uses Cairo's toy `select_font_face` + manual `(x, y)` offsets for 3,800+ lines. Cairo toy text has **no font fallback**, poor kerning/metrics, and mis-sizes Nerd Font glyphs vs UI text, making every popup look rough and nothing like Qt Quick's HarfBuzz + layout engine.
   - **Fix required:** Either use **Pango + Cairo (`pangocairo` + `librsvg`)** / **`cosmic-text` + GPU (`glow` / `wgpu` / `vello` / `skia`)**, AND/OR visually diff live screenshots (`yunoshell` vs `quickshell`) of every single bar edge and popup until pixel/metric parity is verified!
3. **Missing Per-Widget Hover/Transition Polish:**
   - Quickshell animates hover colors (`ColorAnimation 100-150ms`), tile/icon `scale` (`1.0 -> 1.14` spline), sliding Settings tabs, directional tab content slides (`Settings`). `yunoshell`'s `src/render.rs` still snaps most hover/state changes abruptly in cached bitmaps.
   - **✅ PARTIAL FIX (Nov 16 16:10):** Settings popup now has sliding tab indicator pill, horizontal content slide animation, and proper ActionPill/SoftSwitch components. Remaining: Launcher tile hover scale, bar icon hover color animation, other popup hover states.

---

## 1. Core Objective
Build **`yunoshell`** as a **1:1 exact clone of the active Quickshell setup** (`/home/huyle/.config/quickshell`) in pure Rust + Wayland, matching its UI, 4-edge bar + Dynamic Island morphing, Quickshell easing curves, popups, overlays, and system sync identically—while gaining single-process speed and native Wayland `wl_data_device.start_drag` (`text/uri-list`) external file drag-and-drop.

> **Production Safety:** `/home/huyle/.config/quickshell` remains untouched and running (`quickshell`) as the visual & behavioral reference until `yunoshell` reaches 100% parity.

---

## 2. Current Implementation Status (`~9,200` LOC across `Cargo.toml` + `src/`)

`cargo check` passes cleanly (`0` errors) and `/home/huyle/.local/bin/yunoshell` (`3.1 MB`, built `Nov 16 16:15`) is up to date.

### Completed Modules & Files
- **`Cargo.toml` (`14` lines):** Uses `smithay-client-toolkit 0.19`, `wayland-client 0.31` (`system`), `khronos-egl 6` (`static`), `glow 0.14`, `cairo-rs 0.18` (`png`), `chrono 0.4`, `serde` / `serde_json`.
- **`src/gpu.rs` (`382` lines) — Hybrid EGL/GLES2 GPU Presenter (`GpuIsland`):**
  - Creates a `wl_egl_window` + OpenGL ES 2.0 context (`EGL_OPENGL_ES2_BIT`) on `wayland-egl`.
  - **Island-sized texture upload:** Cairo draws into a surface sized to the island card bounding box (not fullscreen). GPU uploads the entire small texture and draws one quad at the island's screen position. Massive CPU/bandwidth reduction vs fullscreen upload.
  - Fallback to SHM path if EGL/GLES init fails; `--no-gpu` forces SHM.
- **`src/main.rs` (`2,900+` lines) — Exact Quickshell Motion Engine + 4-Edge Anchored Island + DnD:**
  - Implements Quickshell's exact Bezier curves (`spline: [0.16, 1.0, 0.3, 1.0]`, `spline_close: [0.32, 0.0, 0.15, 1.0]`, `Fade: [0.22, 1.0, 0.36, 1.0]`) via `Motion` & `Ease` for `m_width`, `m_height`, `m_content`, `m_morph`, `m_fade`.
  - Smooth workspace pill animations (`PillAnim`, `PILL_MS = 200.0`) interpolating slot, pill size, and opacity.
  - **Settings tab switch animation state** (`settings_tab_pill_progress`, `settings_tab_slide_x`) with spline easing matching Quickshell `tabSwitchAnim`.
  - Supports centered top Dynamic Island (`is_top_island`) AND bar-anchored edge popups, plus native `wl_data_device.start_drag` (`text/uri-list` + `text/plain`) with custom Cairo drag-icon surfaces.
- **`src/render.rs` (`3,800+` lines) — Bar & All 13+ Popups:**
  - **Bar (`draw_bar`):** Vertical (`left`/`right`) & horizontal (`top`/`bottom`) clusters: Yuno Launcher (`Antigravity`), Files (`󰉋`), animated `WorkspaceBar`, `RamWidget`, `ClockWidget` -> opens `Settings` tab `"time"`, `MediaBarPill`, `Bluetooth`, `Brightness`, `Volume`, `Battery`, `Wi-Fi`, `Power`.
  - **Popups Implemented:**
    - `Files` (`900×398`, 3-pane browser, breadcrumbs, filter, bookmarks, `filesR` pop-out, drag-out)
    - `Launcher` (`680px`, 8-col Launchpad with real PNG/SVG app icons, auto `1..3` rows, Dock underline pill)
    - `Settings` (`900×388`, all 5 tabs: `Time`/Calendar, `Personalize`, `Display`, `System`, `Controls`) — **Quickshell-matched UI:**
      - Top nav bar (36px) with sliding active tab indicator pill (240ms spline)
      - Horizontal content slide animation when switching tabs (230ms spline)
      - Personalize sub-tabs: Theme & Bar, MangoWM, Desktop, Lock, Login (ActionPill buttons)
      - Theme & Bar: Theme dropdown, Accent color pills, Bar Position dropdown, 2×2 switches grid
      - Right column: Bar Size, Corner Radius, Popup Opacity, Bar Opacity sliders (SmoothSlider)
    - `Media` (`356px`, MPRIS2 metadata + seek/play/next/prev)
    - `Notifications` (`440px`, list/clear/DND UI)
    - `Antigravity` (`360×386`, live streaming chat UI)
    - `Wifi` (`332px`), `Bluetooth` (`332px`), `Volume` (`312px` + audio outputs), `Battery` (`312px`), `Brightness` (`292px`), `Power` (`260px`), `Controls` (`348px`)
- **`src/services.rs` (`1,685` lines) — System Backends:**
  - MangoWM `mmsg get/watch all-tags`, RAM (`/proc/meminfo`), full Battery/CPU telemetry, `pactl` volume & audio sinks, `brightnessctl`, `nmcli` Wi-Fi list/connect/disconnect, `bluetoothctl` paired/available scan/connect/remove, MPRIS2 (`dbus-send` / `playerctl`-free D-Bus queries + seek), monitor detection (`read_monitors`), `.desktop` + PNG/SVG (`rsvg-convert`) icon loader, `agy_send` streaming helper.
- **`src/theme.rs` (`250+` lines) & `src/ipc.rs` (`113` lines):**
  - Reads/writes `/home/huyle/.config/quickshell/settings.json` (7 themes + 8 accents + bar/corner/bookmark settings).
  - Runtime `ThemePreset` struct with all theme colors for accent picker UI.
  - Unix socket `/run/user/1000/yunoshell.sock` supporting `files`, `launcher`, `settings [time|personalize|display|system|controls]`, `media`, `notifications`, `volume`, `brightness`, `battery`, `wifi`, `bluetooth`, `antigravity`, `agy ...`, `power`, `controls`, `bar-pos`, `reload-theme`, `close`, `quit`.
- **MangoWM Config (`/home/huyle/.config/mango/config.conf` `L334-337`):**
  - Already added:
    ```ini
    layer_rule=no_animation:1,no_shadow:1,no_blur:1,layer_name:yuno-bar
    layer_rule=no_animation:1,no_shadow:1,no_blur:1,layer_name:yuno-island
    layer_rule=no_animation:1,no_shadow:1,no_blur:1,layer_name:yuno-overlay
    ```

---

## 3. Remaining Work for 100% Quickshell Replacement Parity

The core Bar, GPU Island presenter (now island-sized texture), Motion curves, Settings 5-tab UI, and almost all popups are in place (`cargo check` passes with 0 errors). The remaining gaps:

### 3.1 Text Rendering — **Highest Visual Impact**
- **Problem:** `src/render.rs` uses Cairo toy text API (`select_font_face`) for all 3,300 lines of UI. No font fallback, poor kerning, Nerd Font glyphs mis-sized vs UI text.
- **Fix:** Integrate **Pango + Cairo** (`pangocairo` crate) for shaped layouts with fallback, OR **`cosmic-text`** (Rust-native, used by Cosmic DE) with GPU atlas rendering. Pango is drop-in for Cairo; `cosmic-text` needs GPU glyph atlas but gives better control.
- **Validation:** Pixel-diff every popup/bar edge against Quickshell screenshots until metrics match.

### 3.2 Per-Widget Hover/Transition Polish
- Quickshell: hover `ColorAnimation` (100–150 ms), tile/icon `scale: 1.0 → 1.14` (spline), sliding Settings tabs, directional tab content slides.
- `yunoshell`: hover snaps instantly in cached bitmaps; no per-widget animation state.
- **✅ PARTIAL FIX (Nov 16 16:10):** Settings popup now has sliding tab indicator pill (240ms spline), horizontal content slide (230ms spline), ActionPill hover color animation (120ms), SoftSwitch thumb animation (190ms spline).
- **Remaining Fix:** Add `hover_progress: f64` + `hover_target: Option<HitTarget>` to `IslandRenderState`/`BarRenderState` for Launcher tiles, bar icons, and other popups. Animate with `Motion` (same spline), interpolate colors/scale in render fns.

### 3.3 True GPU Island Geometry (Optional but Big Win)
- Current: Cairo draws rounded rects, ears, borders, clip masks on CPU every frame.
- **Fix:** Move SDF rounded rect + ear geometry + morph transforms to GLSL vertex/fragment shaders. Upload static corner/ear atlas once. Animate via shader uniforms (`u_morph`, `u_width`, `u_height`, `u_content_alpha`). Eliminates Cairo from the hot animation path entirely.

### 3.4 D-Bus Daemons
- **Notification Daemon (`org.freedesktop.Notifications`):** UI exists in `render.rs`/`SysStats.notifications`; needs D-Bus listener + `NotificationToastOverlay` popups.
- **System Tray (`TrayWidget` / SNI `StatusNotifierWatcher`):** `show_tray_on_bar` setting exists; needs SNI watcher + tray icons/menus on bar.

### 3.5 Fullscreen / Special Overlays (Quickshell-only currently)
- `LockScreen` (`ext_session_lock_v1` + PAM + lock wallpaper/clock)
- `WallpaperTransitionOverlay` & `ThemeWaveOverlay` (GLSL diagonal shaders on fullscreen `Layer::Overlay`)
- `DisplaySwitcherOverlay`
- `DesktopContextMenu` (desktop right-click layer surface)
- `PolkitDialog` & `PortalFileDialog`

### 3.6 1:1 Visual QA & Behavioral Polish
- Side-by-side screenshot diff every popup & 4-edge bar (left/right/top/bottom, floating/docked, cornerRadius > 0).
- Verify `apply-system-theme.py` triggers on `themeName` / `accentPreset` change in Settings.

---

## 4. Reference Projects in Workspace
- **Reference Quickshell:** `/home/huyle/.config/quickshell/` (live & active)
- **Standalone Rust + GTK3 File Manager:** `/home/huyle/.local/src/filesR/` -> `/home/huyle/.local/bin/filesR`
- **Original PoC:** `/home/huyle/.local/src/yunoshell-poc/` -> `/home/huyle/.local/bin/yunoshell-poc`

---

## 5. Build & Test Commands
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /home/huyle/.local/src/yunoshell
cargo build --release
install -m 755 target/release/yunoshell /home/huyle/.local/bin/yunoshell

# Test Dynamic Island popups alongside Quickshell (no duplicate bar)
yunoshell --no-bar &
yunoshell ctl settings personalize
yunoshell ctl files open
yunoshell ctl launcher open
yunoshell ctl media open
yunoshell ctl wifi open
yunoshell ctl close
yunoshell ctl quit

# Full shell swap test (reversible)
pkill quickshell; yunoshell &
# Restore Quickshell:
# yunoshell ctl quit; quickshell &
```
