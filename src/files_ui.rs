use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cairo::{Context, FontSlant, FontWeight, ImageSurface};

use crate::render::{
    draw_glyph_centered, draw_surface_fitted, draw_text, fit_text, rounded_rect, text_width,
    HitBox, HitTarget, IslandRenderState, WheelAction,
};
use crate::services::{
    default_places, file_icon_color, format_size, is_archive_file, FileViewMode, FilesPrompt,
    TRASH_DIR,
};
use crate::theme::Rgb;

/// Compute the Y offset (relative to sidebar top `body_y`) of the active item in the Left Sidebar.
/// Returns `None` if `current_dir` does not match any sidebar item.
pub fn sidebar_active_target_y(
    current_dir: &Path,
    drives: &[crate::services::DriveUsage],
    bookmarks: &[crate::theme::Bookmark],
) -> Option<f64> {
    let cur_s = current_dir.to_string_lossy();
    let places = default_places();
    let mut y = 8.0; // padding: 8px

    for p in &places {
        if cur_s == p.dir {
            return Some(y);
        }
        y += 34.0; // height 32px + 2px gap
    }

    if !drives.is_empty() {
        y += 8.0; // +2px previous gap = 10px group separation
        for d in drives {
            let d_s = d.mount.to_string_lossy();
            // Only highlight drive if it wasn't already matched by Home in Places
            if cur_s == d_s && !places.iter().any(|p| p.dir == d_s.as_ref()) {
                return Some(y);
            }
            y += 34.0;
        }
    }

    let extra_bms: Vec<&crate::theme::Bookmark> = bookmarks
        .iter()
        .filter(|b| !places.iter().any(|p| p.dir == b.dir))
        .collect();
    if !extra_bms.is_empty() {
        y += 8.0;
        for b in extra_bms {
            if cur_s == b.dir {
                return Some(y);
            }
            y += 34.0;
        }
    }

    None
}

/// Elide text in the middle if it exceeds `max_w`, preserving start and extension.
fn elide_middle(cr: &Context, text: &str, size: f64, bold: bool, max_w: f64) -> String {
    if text_width(cr, text, size, bold) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 6 {
        return fit_text(cr, text, size, bold, max_w);
    }
    let tail_len = 5.min(chars.len() / 3).max(3);
    let tail: String = chars[chars.len() - tail_len..].iter().collect();
    let mut head_len = chars.len().saturating_sub(tail_len + 1);
    while head_len > 1 {
        let head: String = chars[..head_len].iter().collect();
        let candidate = format!("{head}…{tail}");
        if text_width(cr, &candidate, size, bold) <= max_w {
            return candidate;
        }
        head_len -= 1;
    }
    fit_text(cr, text, size, bold, max_w)
}

/// Draw right-aligned text with strict tabular-nums (`tnum`) character cell alignment
/// so digits in Size and Modified columns align vertically without subpixel jitter.
fn draw_tabular_right(
    cr: &Context,
    text: &str,
    right_x: f64,
    baseline_y: f64,
    size: f64,
    col: Rgb,
    alpha: f64,
) {
    if text.is_empty() {
        return;
    }
    cr.select_font_face("JetBrainsMono Nerd Font", FontSlant::Normal, FontWeight::Normal);
    cr.set_font_size(size);
    cr.set_source_rgba(col.0, col.1, col.2, alpha);

    let cell_w = (size * 0.60).round().max(6.0);
    let chars: Vec<char> = text.chars().collect();
    let total_w = chars.len() as f64 * cell_w;
    let start_x = (right_x - total_w).round();

    let mut buf = [0u8; 4];
    for (i, ch) in chars.iter().enumerate() {
        let s = ch.encode_utf8(&mut buf);
        let cx = start_x + (i as f64) * cell_w;
        cr.move_to(cx, baseline_y);
        let _ = cr.show_text(s);
    }
}

/// Draw a 28x28 navigation/tool icon button with `radius: 10px` and tactile `scale: 0.95` on press.
#[allow(clippy::too_many_arguments)]
fn draw_icon_btn_28(
    cr: &Context,
    x: f64,
    y: f64,
    glyph: &str,
    enabled: bool,
    active_state: bool,
    target: HitTarget,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let is_hov = enabled && st.hover.as_ref() == Some(&target);
    let is_pressed = enabled && st.pressed_target.as_ref() == Some(&target);

    let scale = if is_pressed { 0.95 } else { 1.0 };
    let w = 28.0 * scale;
    let h = 28.0 * scale;
    let bx = x + (28.0 - w) * 0.5;
    let by = y + (28.0 - h) * 0.5;
    let rad = 10.0 * scale;

    if active_state {
        rounded_rect(cr, bx, by, w, h, rad);
        cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, if is_hov { 0.28 } else { 0.20 });
        let _ = cr.fill();
    } else if is_pressed {
        rounded_rect(cr, bx, by, w, h, rad);
        cr.set_source_rgba(t.surface_active.0, t.surface_active.1, t.surface_active.2, 0.95);
        let _ = cr.fill();
    } else if is_hov {
        rounded_rect(cr, bx, by, w, h, rad);
        cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.85);
        let _ = cr.fill();
    }

    let (ic_col, ic_alpha) = if !enabled {
        (t.muted, 0.35)
    } else if active_state {
        (t.accent, 1.0)
    } else if is_hov || is_pressed {
        (t.text_bright, 1.0)
    } else {
        (t.text, 0.88)
    };

    draw_glyph_centered(cr, glyph, bx, by, w, h, 14.0 * scale, ic_col, ic_alpha);

    if enabled {
        hits.push(HitBox {
            x,
            y,
            w: 28.0,
            h: 28.0,
            target,
            wheel: WheelAction::None,
        });
    }
}

/// Draw a thumbnail or fallback glyph clipped to `radius: 10px`.
#[allow(clippy::too_many_arguments)]
fn draw_grid_visual(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    item_path: &Path,
    glyph: &str,
    glyph_col: Rgb,
    thumbs: &HashMap<PathBuf, ImageSurface>,
) {
    if let Some(surf) = thumbs.get(item_path) {
        cr.save().ok();
        rounded_rect(cr, x, y, w, h, 10.0);
        cr.clip();
        draw_surface_fitted(cr, surf, x, y, w, h, 1.0);
        cr.restore().ok();
    } else {
        draw_glyph_centered(cr, glyph, x, y, w, h, 26.0, glyph_col, 0.95);
    }
}

/// 100% Full Rewrite of the File Manager UI following Noctalia / Luciole / Android 17 Expressive spec.
pub fn draw_files_popup(
    cr: &Context,
    cx: f64,
    cy: f64,
    cw: f64,
    ch: f64,
    st: &IslandRenderState<'_>,
    hits: &mut Vec<HitBox>,
) {
    let t = st.theme;
    let gap = 6.0;

    // =========================================================================
    // 1. TOP NAVIGATION & BREADCRUMB BAR (height: 38px, horizontal padding: 10px)
    // =========================================================================
    let top_h = 38.0;
    rounded_rect(cr, cx, cy, cw, top_h, 12.0);
    cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.96);
    let _ = cr.fill();

    let pad_x = 10.0;
    let btn_y = cy + (top_h - 28.0) * 0.5; // cy + 5.0

    // Left Navigation Cluster: Back, Forward, Up (28x28px, radius: 10px, scale: 0.95 on press)
    let mut lx = cx + pad_x;
    draw_icon_btn_28(
        cr,
        lx,
        btn_y,
        "󰁍",
        st.can_back,
        false,
        HitTarget::FilesNavBack,
        st,
        hits,
    );
    lx += 28.0 + 4.0;

    draw_icon_btn_28(
        cr,
        lx,
        btn_y,
        "󰁔",
        st.can_fwd,
        false,
        HitTarget::FilesNavFwd,
        st,
        hits,
    );
    lx += 28.0 + 4.0;

    let can_up = st.current_dir.parent().is_some();
    draw_icon_btn_28(
        cr,
        lx,
        btn_y,
        "󰁝",
        can_up,
        false,
        HitTarget::FilesNavUp,
        st,
        hits,
    );
    lx += 28.0 + 8.0;

    // Right Tools Cluster: Search Pill (140px -> 220px OutExpo), View Mode Toggle, Hidden Files Toggle
    let mut rx = cx + cw - pad_x;

    // Optional XDG Portal Accept/Cancel buttons when acting as system file chooser
    if let Some((_, accept_lbl)) = st.portal_info {
        let acc_w = (text_width(cr, accept_lbl, 11.5, true) + 20.0).max(56.0);
        rx -= acc_w;
        let is_acc_hov = st.hover.as_ref() == Some(&HitTarget::FilesPortalAccept);
        rounded_rect(cr, rx, cy + 4.0, acc_w, 30.0, 10.0);
        cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, if is_acc_hov { 0.95 } else { 0.82 });
        let _ = cr.fill();
        let tw = text_width(cr, accept_lbl, 11.5, true);
        draw_text(
            cr,
            accept_lbl,
            rx + (acc_w - tw) * 0.5,
            cy + 23.0,
            11.5,
            true,
            t.accent_text,
            1.0,
        );
        hits.push(HitBox {
            x: rx,
            y: cy + 4.0,
            w: acc_w,
            h: 30.0,
            target: HitTarget::FilesPortalAccept,
            wheel: WheelAction::None,
        });
        rx -= 4.0;

        rx -= 28.0;
        draw_icon_btn_28(
            cr,
            rx,
            btn_y,
            "󰅖",
            true,
            false,
            HitTarget::FilesPortalCancel,
            st,
            hits,
        );
        rx -= 6.0;
    }

    // Quick Access pin toggle button for current directory
    let cur_pinned = st
        .cfg
        .bookmarks
        .iter()
        .any(|b| b.dir == st.current_dir.to_string_lossy().as_ref());
    rx -= 28.0;
    draw_icon_btn_28(
        cr,
        rx,
        btn_y,
        if cur_pinned { "󰐃" } else { "󰐄" },
        true,
        cur_pinned,
        HitTarget::FilesTogglePin,
        st,
        hits,
    );
    rx -= 4.0;

    // Hidden files toggle button (Ctrl+H)
    rx -= 28.0;
    draw_icon_btn_28(
        cr,
        rx,
        btn_y,
        if st.show_hidden { "󰈈" } else { "󰈉" },
        true,
        st.show_hidden,
        HitTarget::FilesToggleHidden,
        st,
        hits,
    );
    rx -= 4.0;

    // View mode toggle button (List View <-> Grid View)
    rx -= 28.0;
    let view_glyph = match st.files_view_mode {
        FileViewMode::List => "󰕰",
        FileViewMode::Grid => "󰝘",
    };
    draw_icon_btn_28(
        cr,
        rx,
        btn_y,
        view_glyph,
        true,
        false,
        HitTarget::FilesViewModeToggle,
        st,
        hits,
    );
    rx -= 8.0;

    // Search Pill (Ctrl+F): 140px normal -> 220px focused (180ms OutExpo)
    let search_expand = st.files_search_expand.clamp(0.0, 1.0);
    let search_w = (140.0 + (220.0 - 140.0) * search_expand).round();
    let search_h = 30.0;
    let search_y = cy + 4.0;
    rx -= search_w;
    let search_x = rx;
    let search_r = 15.0 - 5.0 * search_expand; // morphs 15px -> 10px on focus

    let search_hov = st.hover.as_ref() == Some(&HitTarget::FilesFocusFilter);
    let search_bg = if st.filter_focused {
        t.surface_active
    } else if search_hov {
        t.surface_hover
    } else {
        t.surface_hover
    };
    let search_alpha = if st.filter_focused { 1.0 } else if search_hov { 0.92 } else { 0.72 };
    rounded_rect(cr, search_x, search_y, search_w, search_h, search_r);
    cr.set_source_rgba(search_bg.0, search_bg.1, search_bg.2, search_alpha);
    let _ = cr.fill();

    draw_glyph_centered(
        cr,
        "󰍉",
        search_x + 8.0,
        search_y,
        18.0,
        search_h,
        13.0,
        if st.filter_focused { t.accent } else { t.muted },
        0.95,
    );

    cr.save().ok();
    rounded_rect(cr, search_x + 26.0, search_y + 2.0, search_w - 34.0, search_h - 4.0, 6.0);
    cr.clip();
    if st.filter_query.is_empty() && !st.filter_focused {
        draw_text(
            cr,
            "Search…",
            search_x + 28.0,
            search_y + 19.5,
            12.0,
            false,
            t.muted,
            0.75,
        );
    } else {
        let q_fit = fit_text(cr, st.filter_query, 12.0, false, search_w - 40.0);
        draw_text(
            cr,
            &q_fit,
            search_x + 28.0,
            search_y + 19.5,
            12.0,
            false,
            t.text_bright,
            1.0,
        );
        if st.filter_focused && st.agy_cursor_on {
            let cur_x = search_x + 28.0 + text_width(cr, &q_fit, 12.0, false) + 1.5;
            rounded_rect(cr, cur_x, search_y + 7.0, 1.5, 16.0, 0.75);
            cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.95);
            let _ = cr.fill();
        }
    }
    cr.restore().ok();

    hits.push(HitBox {
        x: search_x,
        y: search_y,
        w: search_w,
        h: search_h,
        target: HitTarget::FilesFocusFilter,
        wheel: WheelAction::None,
    });

    // Interactive Breadcrumb Pill (height: 30px, radius: 15px -> morphs to 10px on focus/Ctrl+L)
    let crumb_x = lx;
    let crumb_w = (rx - 8.0 - crumb_x).max(80.0);
    let crumb_y = cy + 4.0;
    let crumb_h = 30.0;
    let path_editing = matches!(st.files_prompt, FilesPrompt::PathInput);
    let crumb_focus = st.files_crumb_focus.clamp(0.0, 1.0);
    let crumb_r = 15.0 - 5.0 * crumb_focus;

    let crumb_bg = if path_editing {
        t.surface_active
    } else {
        t.surface_hover
    };
    rounded_rect(cr, crumb_x, crumb_y, crumb_w, crumb_h, crumb_r);
    cr.set_source_rgba(
        crumb_bg.0,
        crumb_bg.1,
        crumb_bg.2,
        if path_editing { 1.0 } else { 0.78 },
    );
    let _ = cr.fill();

    if path_editing {
        cr.save().ok();
        rounded_rect(cr, crumb_x + 8.0, crumb_y + 2.0, crumb_w - 16.0, crumb_h - 4.0, 6.0);
        cr.clip();
        let disp = st.files_prompt_text;
        let tw = text_width(cr, disp, 12.0, false);
        let avail = crumb_w - 24.0;
        let text_x = if tw > avail {
            crumb_x + 10.0 - (tw - avail)
        } else {
            crumb_x + 10.0
        };
        draw_text(
            cr,
            disp,
            text_x,
            crumb_y + 19.5,
            12.0,
            false,
            t.text_bright,
            1.0,
        );
        if st.agy_cursor_on {
            let cur_x = (text_x + tw + 1.5).min(crumb_x + crumb_w - 10.0);
            rounded_rect(cr, cur_x, crumb_y + 7.0, 1.5, 16.0, 0.75);
            cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.95);
            let _ = cr.fill();
        }
        cr.restore().ok();
        hits.push(HitBox {
            x: crumb_x,
            y: crumb_y,
            w: crumb_w,
            h: crumb_h,
            target: HitTarget::FilesPromptInput,
            wheel: WheelAction::None,
        });
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/huyle".into());
        let dir_s = st.current_dir.to_string_lossy();
        let mut segs: Vec<(String, PathBuf)> = Vec::new();
        if dir_s == home || dir_s.starts_with(&format!("{home}/")) {
            segs.push(("~".to_string(), PathBuf::from(&home)));
            let rel = dir_s.strip_prefix(&home).unwrap_or("").trim_start_matches('/');
            if !rel.is_empty() {
                let mut acc = PathBuf::from(&home);
                for part in rel.split('/').filter(|s| !s.is_empty()) {
                    acc.push(part);
                    segs.push((part.to_string(), acc.clone()));
                }
            }
        } else {
            segs.push(("/".to_string(), PathBuf::from("/")));
            let mut acc = PathBuf::from("/");
            for part in dir_s.trim_start_matches('/').split('/').filter(|s| !s.is_empty()) {
                acc.push(part);
                segs.push((part.to_string(), acc.clone()));
            }
        }

        // Keep trailing segments if breadcrumb bar overflows
        while segs.len() > 3 {
            let total_est: f64 = segs
                .iter()
                .map(|(l, _)| text_width(cr, l, 12.0, false) + 22.0)
                .sum();
            if total_est <= crumb_w - 28.0 {
                break;
            }
            segs.remove(1);
        }

        cr.save().ok();
        rounded_rect(cr, crumb_x + 4.0, crumb_y + 2.0, crumb_w - 8.0, crumb_h - 4.0, 10.0);
        cr.clip();

        let mut sx = crumb_x + 6.0;
        let last_idx = segs.len().saturating_sub(1);
        for (i, (label, path)) in segs.into_iter().enumerate() {
            if i > 0 {
                draw_text(cr, "/", sx + 2.0, crumb_y + 19.5, 11.5, false, t.muted, 0.65);
                sx += 12.0;
            }
            let is_last = i == last_idx;
            let short = fit_text(cr, &label, 12.0, is_last, 130.0);
            let pw = (text_width(cr, &short, 12.0, is_last) + 14.0).max(24.0);
            let pill_y = crumb_y + 4.0;
            let pill_h = 22.0;
            let tgt = HitTarget::FilesBreadcrumb(path);
            let is_hov = st.hover.as_ref() == Some(&tgt);

            if is_hov || is_last {
                rounded_rect(cr, sx, pill_y, pw, pill_h, 11.0);
                if is_hov {
                    cr.set_source_rgba(t.surface_active.0, t.surface_active.1, t.surface_active.2, 0.95);
                } else {
                    cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.75);
                }
                let _ = cr.fill();
            }

            draw_text(
                cr,
                &short,
                sx + 7.0,
                crumb_y + 19.5,
                12.0,
                is_last,
                if is_last || is_hov { t.text_bright } else { t.text },
                if is_last || is_hov { 1.0 } else { 0.85 },
            );

            hits.push(HitBox {
                x: sx,
                y: pill_y,
                w: pw,
                h: pill_h,
                target: tgt,
                wheel: WheelAction::None,
            });
            sx += pw + 2.0;
        }
        cr.restore().ok();

        // Clicking empty space in the breadcrumb pill activates inline path input (Ctrl+L)
        let rem_x = sx.max(crumb_x + 8.0);
        let rem_w = (crumb_x + crumb_w - rem_x).max(16.0);
        hits.push(HitBox {
            x: rem_x,
            y: crumb_y,
            w: rem_w,
            h: crumb_h,
            target: HitTarget::FilesEditPath,
            wheel: WheelAction::None,
        });
    }

    // =========================================================================
    // 2. LEFT SIDEBAR - PLACES & DRIVES (width: 168px, padding: 8px)
    // =========================================================================
    let body_y = cy + top_h + gap;
    let body_h = (ch - top_h - gap).max(60.0);
    let side_w = 168.0;
    let side_pad = 8.0;

    rounded_rect(cr, cx, body_y, side_w, body_h, 16.0);
    cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.96);
    let _ = cr.fill();

    let item_x = cx + side_pad;
    let item_w = side_w - side_pad * 2.0; // 152px
    let item_h = 32.0;

    // Sliding Active Pill (Shape Morphing radius 16px -> 10px, smooth Y slide)
    if st.files_sidebar_pill_y >= 0.0 {
        let pill_y = body_y + st.files_sidebar_pill_y;
        let pill_r = st.files_sidebar_pill_r.clamp(10.0, 16.0);
        rounded_rect(cr, item_x, pill_y, item_w, item_h, pill_r);
        cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.22);
        let _ = cr.fill();
    }

    let cur_dir_s = st.current_dir.to_string_lossy();
    let places = default_places();
    let mut sy = body_y + side_pad;

    // Group 1: Places (Home, Desktop, Downloads, Documents, Pictures, Trash) - NO uppercase header!
    for place in &places {
        let p_path = PathBuf::from(&place.dir);
        let is_active = cur_dir_s == place.dir;
        let tgt = HitTarget::FilesPlace(p_path.clone());
        let is_hov = st.hover.as_ref() == Some(&tgt);
        let is_dnd = st.files_dnd_target.map(|d| d == p_path.as_path()).unwrap_or(false);

        if is_dnd {
            let dnd_col = if place.dir == TRASH_DIR { t.red } else { t.accent };
            rounded_rect(cr, item_x, sy, item_w, item_h, 10.0);
            cr.set_source_rgba(dnd_col.0, dnd_col.1, dnd_col.2, 0.30);
            let _ = cr.fill();
        } else if is_hov && !is_active {
            rounded_rect(cr, item_x, sy, item_w, item_h, 10.0);
            cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.65);
            let _ = cr.fill();
        }

        let ic_col = if is_active {
            t.accent
        } else if place.dir == TRASH_DIR {
            t.red
        } else {
            t.text
        };
        let txt_col = if is_active {
            t.text_bright
        } else if is_hov {
            t.text_bright
        } else {
            t.text
        };

        draw_glyph_centered(cr, &place.icon, item_x + 10.0, sy, 16.0, item_h, 15.0, ic_col, 0.95);
        let lbl = fit_text(cr, &place.label, 12.5, is_active, item_w - 40.0);
        draw_text(
            cr,
            &lbl,
            item_x + 34.0,
            sy + 20.5,
            12.5,
            is_active,
            txt_col,
            if is_active || is_hov { 1.0 } else { 0.88 },
        );

        hits.push(HitBox {
            x: item_x,
            y: sy,
            w: item_w,
            h: item_h,
            target: tgt,
            wheel: WheelAction::None,
        });
        sy += item_h + 2.0;
    }

    // Group 2: Drives (separated strictly by 10px vertical gap, NO uppercase header!)
    if !st.files_drive_usages.is_empty() {
        sy += 8.0; // +2px from last item = 10px group gap
        for drv in st.files_drive_usages {
            if sy + item_h > body_y + body_h - 6.0 {
                break;
            }
            let d_s = drv.mount.to_string_lossy();
            let is_active = cur_dir_s == d_s && !places.iter().any(|p| p.dir == d_s.as_ref());
            let tgt = HitTarget::FilesDrive(drv.mount.clone());
            let is_hov = st.hover.as_ref() == Some(&tgt);
            let is_dnd = st.files_dnd_target.map(|d| d == drv.mount.as_path()).unwrap_or(false);

            if is_dnd {
                rounded_rect(cr, item_x, sy, item_w, item_h, 10.0);
                cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.30);
                let _ = cr.fill();
            } else if is_hov && !is_active {
                rounded_rect(cr, item_x, sy, item_w, item_h, 10.0);
                cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.65);
                let _ = cr.fill();
            }

            let ic_col = if is_active { t.accent } else { t.blue };
            draw_glyph_centered(cr, "󰋊", item_x + 10.0, sy, 16.0, item_h, 15.0, ic_col, 0.95);

            let lbl = fit_text(cr, &drv.label, 12.5, is_active, item_w - 42.0);
            draw_text(
                cr,
                &lbl,
                item_x + 34.0,
                sy + 18.0,
                12.5,
                is_active,
                if is_active || is_hov { t.text_bright } else { t.text },
                0.92,
            );

            // Ultra-slim storage usage bar (height: 3px, radius: 1.5px) right beneath drive name
            let bar_x = item_x + 34.0;
            let bar_y = sy + 24.0;
            let bar_w = item_w - 44.0;
            let bar_h = 3.0;
            rounded_rect(cr, bar_x, bar_y, bar_w, bar_h, 1.5);
            cr.set_source_rgba(t.bg_deep.0, t.bg_deep.1, t.bg_deep.2, 0.85);
            let _ = cr.fill();

            let fill_w = (bar_w * drv.percent.clamp(0.0, 1.0)).max(3.0);
            let bar_col = if drv.percent > 0.90 { t.red } else { t.accent };
            rounded_rect(cr, bar_x, bar_y, fill_w, bar_h, 1.5);
            cr.set_source_rgba(bar_col.0, bar_col.1, bar_col.2, 0.95);
            let _ = cr.fill();

            hits.push(HitBox {
                x: item_x,
                y: sy,
                w: item_w,
                h: item_h,
                target: tgt,
                wheel: WheelAction::None,
            });
            sy += item_h + 2.0;
        }
    }

    // Optional Pinned Quick Access Bookmarks (excluding duplicates of default Places, separated by 10px gap)
    let extra_bms: Vec<&crate::theme::Bookmark> = st
        .cfg
        .bookmarks
        .iter()
        .filter(|b| !places.iter().any(|p| p.dir == b.dir))
        .collect();
    let qa_start_y = sy + 6.0;
    if !extra_bms.is_empty() || st.files_is_dragging {
        sy = qa_start_y;
        for bm in &extra_bms {
            if sy + item_h > body_y + body_h - 6.0 {
                break;
            }
            let bm_path = PathBuf::from(&bm.dir);
            let is_active = cur_dir_s == bm.dir;
            let tgt = HitTarget::FilesPlace(bm_path.clone());
            let unpin_tgt = HitTarget::FilesUnpin(bm.dir.clone());
            let is_hov = st.hover.as_ref() == Some(&tgt) || st.hover.as_ref() == Some(&unpin_tgt);
            let is_unpin_hov = st.hover.as_ref() == Some(&unpin_tgt);

            if is_hov && !is_active {
                rounded_rect(cr, item_x, sy, item_w, item_h, 10.0);
                cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.65);
                let _ = cr.fill();
            }

            draw_glyph_centered(
                cr,
                &bm.icon,
                item_x + 10.0,
                sy,
                16.0,
                item_h,
                15.0,
                if is_active { t.accent } else { t.yellow },
                0.95,
            );
            let lbl_avail = if is_hov { item_w - 58.0 } else { item_w - 40.0 };
            let lbl = fit_text(cr, &bm.label, 12.5, is_active, lbl_avail);
            draw_text(
                cr,
                &lbl,
                item_x + 34.0,
                sy + 20.5,
                12.5,
                is_active,
                if is_active || is_hov { t.text_bright } else { t.text },
                0.90,
            );

            hits.push(HitBox {
                x: item_x,
                y: sy,
                w: if is_hov { item_w - 22.0 } else { item_w },
                h: item_h,
                target: tgt,
                wheel: WheelAction::None,
            });
            if is_hov {
                let ux = item_x + item_w - 22.0;
                let uy = sy + (item_h - 18.0) / 2.0;
                draw_glyph_centered(
                    cr,
                    "󰅖",
                    ux,
                    uy,
                    18.0,
                    18.0,
                    11.0,
                    if is_unpin_hov { t.red } else { t.muted },
                    if is_unpin_hov { 1.0 } else { 0.8 },
                );
                hits.push(HitBox {
                    x: ux,
                    y: sy,
                    w: 22.0,
                    h: item_h,
                    target: unpin_tgt,
                    wheel: WheelAction::None,
                });
            }
            sy += item_h + 2.0;
        }
        // Drop zone for dragging a folder to Quick Access
        if st.files_is_dragging && sy + 28.0 <= body_y + body_h - 4.0 {
            let drop_h = (body_y + body_h - 6.0 - sy).min(36.0).max(26.0);
            let is_drop = st.files_qa_dnd_hover;
            rounded_rect(cr, item_x, sy, item_w, drop_h, 10.0);
            cr.set_source_rgba(
                if is_drop { t.accent.0 } else { t.surface_hover.0 },
                if is_drop { t.accent.1 } else { t.surface_hover.1 },
                if is_drop { t.accent.2 } else { t.surface_hover.2 },
                if is_drop { 0.24 } else { 0.45 },
            );
            let _ = cr.fill();
            draw_glyph_centered(
                cr,
                "󰐃",
                item_x + 10.0,
                sy,
                16.0,
                drop_h,
                13.0,
                t.accent,
                0.95,
            );
            draw_text(
                cr,
                "Pin Quick Access",
                item_x + 32.0,
                sy + drop_h / 2.0 + 4.0,
                11.5,
                is_drop,
                if is_drop { t.accent } else { t.muted },
                0.95,
            );
            hits.push(HitBox {
                x: item_x,
                y: sy,
                w: item_w,
                h: drop_h,
                target: HitTarget::FilesQuickAccessDrop,
                wheel: WheelAction::None,
            });
        }
    }

    // =========================================================================
    // 3. MAIN FILE VIEWPORT (surfaceContainerLow, radius: 16px, 60FPS Virtualized)
    // =========================================================================
    let view_x = cx + side_w + gap;
    let view_w = (cw - side_w - gap).max(120.0);
    let view_y = body_y;
    let view_h = body_h;

    rounded_rect(cr, view_x, view_y, view_w, view_h, 16.0);
    cr.set_source_rgba(t.bg_deep.0, t.bg_deep.1, t.bg_deep.2, 0.96);
    let _ = cr.fill();

    // Background hitbox for rubber-band selection & right-click context menu on empty area
    let status_h = 26.0;
    let content_top_pad = 6.0;
    let mut list_y = view_y + content_top_pad;
    let mut list_h = (view_h - content_top_pad - status_h - 10.0).max(40.0);

    hits.push(HitBox {
        x: view_x,
        y: view_y,
        w: view_w,
        h: view_h - status_h - 6.0,
        target: HitTarget::FilesListArea,
        wheel: WheelAction::None,
    });

    // Inline bar for Mkdir / Mkfile / SaveFilename / ConfirmDelete / ConfirmEmptyTrash
    let has_inline_bar = matches!(
        st.files_prompt,
        FilesPrompt::Mkdir
            | FilesPrompt::Mkfile
            | FilesPrompt::SaveFilename
            | FilesPrompt::ConfirmDelete(_)
            | FilesPrompt::ConfirmEmptyTrash
    );
    if has_inline_bar {
        let bar_x = view_x + 6.0;
        let bar_y = list_y;
        let bar_w = view_w - 12.0;
        let bar_h = 32.0;
        rounded_rect(cr, bar_x, bar_y, bar_w, bar_h, 8.0);
        cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.95);
        let _ = cr.fill();

        let (icon, prefix, is_confirm) = match st.files_prompt {
            FilesPrompt::Mkdir => ("󰉋", "New folder:", false),
            FilesPrompt::Mkfile => ("󰈔", "New file:", false),
            FilesPrompt::SaveFilename => ("󰆓", "Save as:", false),
            FilesPrompt::ConfirmDelete(v) => ("󰩺", if v.len() == 1 { "Delete permanently?" } else { "Delete selected items?" }, true),
            FilesPrompt::ConfirmEmptyTrash => ("󰩺", "Empty Trash permanently?", true),
            _ => ("", "", false),
        };

        draw_glyph_centered(
            cr,
            icon,
            bar_x + 10.0,
            bar_y,
            18.0,
            bar_h,
            15.0,
            if is_confirm { t.red } else { t.accent },
            1.0,
        );
        draw_text(cr, prefix, bar_x + 34.0, bar_y + 20.5, 12.0, true, t.text, 0.95);
        let p_w = text_width(cr, prefix, 12.0, true);

        if !is_confirm {
            let inp_x = bar_x + 42.0 + p_w;
            let inp_w = (bar_w - (42.0 + p_w) - 116.0).max(60.0);
            rounded_rect(cr, inp_x, bar_y + 4.0, inp_w, 24.0, 6.0);
            cr.set_source_rgba(t.bg_deep.0, t.bg_deep.1, t.bg_deep.2, 0.90);
            let _ = cr.fill();

            let txt_fit = fit_text(cr, st.files_prompt_text, 12.0, false, inp_w - 14.0);
            draw_text(cr, &txt_fit, inp_x + 7.0, bar_y + 20.5, 12.0, false, t.text_bright, 1.0);
            if st.agy_cursor_on {
                let cx_cur = inp_x + 7.0 + text_width(cr, &txt_fit, 12.0, false) + 1.5;
                rounded_rect(cr, cx_cur, bar_y + 8.0, 1.5, 16.0, 0.75);
                cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.95);
                let _ = cr.fill();
            }
            hits.push(HitBox {
                x: inp_x,
                y: bar_y + 4.0,
                w: inp_w,
                h: 24.0,
                target: HitTarget::FilesPromptInput,
                wheel: WheelAction::None,
            });
        }

        // Confirm & Cancel pills
        let ok_x = bar_x + bar_w - 106.0;
        rounded_rect(cr, ok_x, bar_y + 4.0, 50.0, 24.0, 7.0);
        let ok_col = if is_confirm { t.red } else { t.accent };
        cr.set_source_rgba(ok_col.0, ok_col.1, ok_col.2, 0.25);
        let _ = cr.fill();
        draw_text(cr, "OK", ok_x + 17.0, bar_y + 20.0, 11.5, true, ok_col, 1.0);
        hits.push(HitBox {
            x: ok_x,
            y: bar_y + 4.0,
            w: 50.0,
            h: 24.0,
            target: HitTarget::FilesPromptConfirm,
            wheel: WheelAction::None,
        });

        let can_x = bar_x + bar_w - 52.0;
        rounded_rect(cr, can_x, bar_y + 4.0, 48.0, 24.0, 7.0);
        cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.9);
        let _ = cr.fill();
        draw_text(cr, "Esc", can_x + 13.0, bar_y + 20.0, 11.5, false, t.muted, 0.95);
        hits.push(HitBox {
            x: can_x,
            y: bar_y + 4.0,
            w: 48.0,
            h: 24.0,
            target: HitTarget::FilesPromptCancel,
            wheel: WheelAction::None,
        });

        list_y += 36.0;
        list_h = (list_h - 36.0).max(40.0);
    }

    // Clip viewport for virtualized file list/grid
    cr.save().ok();
    rounded_rect(cr, view_x + 2.0, list_y, view_w - 4.0, list_h, 12.0);
    cr.clip();

    if st.entries.is_empty() {
        let empty_msg = if !st.filter_query.is_empty() {
            "No matching files"
        } else {
            "Empty folder"
        };
        let ew = text_width(cr, empty_msg, 12.5, false);
        draw_text(
            cr,
            empty_msg,
            view_x + (view_w - ew) * 0.5,
            list_y + list_h * 0.5,
            12.5,
            false,
            t.muted,
            0.65,
        );
    } else {
        match st.files_view_mode {
            // -----------------------------------------------------------------
            // MODE 1: COMPACT LIST VIEW (32px height, radius: 8px, gap: 2px)
            // -----------------------------------------------------------------
            FileViewMode::List => {
                let row_h = 32.0;
                let row_gap = 2.0;
                let stride = row_h + row_gap; // 34.0px
                let row_x = view_x + 6.0;
                let row_w = view_w - 12.0;

                let first_idx = (st.scroll_y / stride).floor().max(0.0) as usize;
                let vis_count = (list_h / stride).ceil() as usize + 2;
                let last_idx = (first_idx + vis_count).min(st.entries.len());

                for idx in first_idx..last_idx {
                    let item = &st.entries[idx];
                    let ry = list_y + (idx as f64) * stride - st.scroll_y;
                    if ry + row_h < list_y || ry > list_y + list_h {
                        continue;
                    }

                    let is_sel = st.selected_paths.contains(&item.path)
                        || (st.selected_paths.is_empty() && st.selected_idx == Some(idx));
                    let tgt = HitTarget::FilesRow(idx);
                    let is_hov = st.hover.as_ref() == Some(&tgt);
                    let is_dnd = st
                        .files_dnd_target
                        .map(|d| d == item.path.as_path())
                        .unwrap_or(false);

                    // Strictly NO 1px border, NO DropShadow/Blur! Pure tonal surface hierarchy.
                    if is_dnd {
                        rounded_rect(cr, row_x, ry, row_w, row_h, 8.0);
                        cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.32);
                        let _ = cr.fill();
                    } else if is_sel {
                        rounded_rect(cr, row_x, ry, row_w, row_h, 8.0);
                        cr.set_source_rgba(
                            t.accent.0,
                            t.accent.1,
                            t.accent.2,
                            if is_hov { 0.26 } else { 0.20 },
                        );
                        let _ = cr.fill();
                    } else if is_hov {
                        rounded_rect(cr, row_x, ry, row_w, row_h, 8.0);
                        cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.65);
                        let _ = cr.fill();
                    }

                    // [Icon 18px]
                    let ic_col = if is_sel {
                        t.accent
                    } else {
                        file_icon_color(&item.name, item.is_dir, t)
                    };
                    draw_glyph_centered(cr, item.icon, row_x + 10.0, ry, 18.0, row_h, 16.0, ic_col, 0.95);

                    // [Filename 12.5px, auto-elided] OR inline Rename input (F2)
                    let name_x = row_x + 34.0;
                    let is_renaming = matches!(st.files_prompt, FilesPrompt::Rename(ref p) if p == &item.path);
                    if is_renaming {
                        let ren_w = (row_w - 240.0).max(120.0);
                        rounded_rect(cr, name_x - 4.0, ry + 3.0, ren_w, 26.0, 6.0);
                        cr.set_source_rgba(t.surface_active.0, t.surface_active.1, t.surface_active.2, 0.98);
                        let _ = cr.fill();

                        let r_fit = fit_text(cr, st.files_prompt_text, 12.5, false, ren_w - 14.0);
                        draw_text(cr, &r_fit, name_x + 2.0, ry + 20.5, 12.5, false, t.text_bright, 1.0);
                        if st.agy_cursor_on {
                            let cur_x = name_x + 2.0 + text_width(cr, &r_fit, 12.5, false) + 1.5;
                            rounded_rect(cr, cur_x, ry + 8.0, 1.5, 16.0, 0.75);
                            cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.95);
                            let _ = cr.fill();
                        }
                    } else {
                        let max_name_w = (row_w - 34.0 - 206.0).max(80.0);
                        let disp_name = elide_middle(cr, &item.name, 12.5, is_sel, max_name_w);
                        draw_text(
                            cr,
                            &disp_name,
                            name_x,
                            ry + 20.5,
                            12.5,
                            is_sel,
                            if is_sel || is_hov { t.text_bright } else { t.text },
                            if is_sel || is_hov { 1.0 } else { 0.92 },
                        );
                    }

                    // [File Size right-aligned with tabular-nums]
                    let size_right_x = row_x + row_w - 128.0;
                    if !item.is_dir {
                        draw_tabular_right(
                            cr,
                            &item.size_label,
                            size_right_x,
                            ry + 20.5,
                            11.5,
                            if is_sel { t.text } else { t.muted },
                            if is_sel { 0.92 } else { 0.78 },
                        );
                    }

                    // [Modified Date right-aligned with tabular-nums]
                    let date_right_x = row_x + row_w - 10.0;
                    draw_tabular_right(
                        cr,
                        &item.modified_label,
                        date_right_x,
                        ry + 20.5,
                        11.5,
                        if is_sel { t.text } else { t.muted },
                        if is_sel { 0.92 } else { 0.75 },
                    );

                    hits.push(HitBox {
                        x: row_x,
                        y: ry.max(list_y),
                        w: row_w,
                        h: row_h,
                        target: tgt,
                        wheel: WheelAction::None,
                    });
                }

                // Minimalist scrollbar thumb if needed
                let total_h = (st.entries.len() as f64) * stride;
                if total_h > list_h {
                    let sb_h = ((list_h / total_h) * list_h).clamp(24.0, list_h);
                    let max_sc = (total_h - list_h).max(1.0);
                    let sb_y = list_y + (st.scroll_y / max_sc).clamp(0.0, 1.0) * (list_h - sb_h);
                    rounded_rect(cr, view_x + view_w - 4.5, sb_y, 2.5, sb_h, 1.25);
                    cr.set_source_rgba(t.muted.0, t.muted.1, t.muted.2, 0.35);
                    let _ = cr.fill();
                }
            }

            // -----------------------------------------------------------------
            // MODE 2: EXPRESSIVE GRID VIEW (92x100px, radius: 14px -> 10px)
            // -----------------------------------------------------------------
            FileViewMode::Grid => {
                let cell_w = 92.0;
                let cell_h = 100.0;
                let cols = 5usize;
                let col_gap = ((view_w - 24.0 - (cols as f64) * cell_w) / ((cols - 1) as f64)).clamp(6.0, 18.0);
                let row_gap = 10.0;
                let row_stride = cell_h + row_gap; // 110.0px
                let grid_x0 = view_x + (view_w - ((cols as f64) * cell_w + ((cols - 1) as f64) * col_gap)) * 0.5;

                let total_rows = (st.entries.len() + cols - 1) / cols;
                let first_row = (st.scroll_y / row_stride).floor().max(0.0) as usize;
                let vis_rows = (list_h / row_stride).ceil() as usize + 2;
                let last_row = (first_row + vis_rows).min(total_rows);

                for row in first_row..last_row {
                    let cy_cell = list_y + 4.0 + (row as f64) * row_stride - st.scroll_y;
                    if cy_cell + cell_h < list_y || cy_cell > list_y + list_h {
                        continue;
                    }
                    for col in 0..cols {
                        let idx = row * cols + col;
                        if idx >= st.entries.len() {
                            break;
                        }
                        let item = &st.entries[idx];
                        let cx_cell = grid_x0 + (col as f64) * (cell_w + col_gap);

                        let is_sel = st.selected_paths.contains(&item.path)
                            || (st.selected_paths.is_empty() && st.selected_idx == Some(idx));
                        let tgt = HitTarget::FilesRow(idx);
                        let is_hov = st.hover.as_ref() == Some(&tgt);
                        let is_dnd = st
                            .files_dnd_target
                            .map(|d| d == item.path.as_path())
                            .unwrap_or(false);

                        // Shape Morphing: radius 14px normal -> 10px when selected!
                        let cell_r = if is_sel || is_dnd { 10.0 } else { 14.0 };
                        rounded_rect(cr, cx_cell, cy_cell, cell_w, cell_h, cell_r);
                        if is_dnd {
                            cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.34);
                            let _ = cr.fill();
                        } else if is_sel {
                            cr.set_source_rgba(
                                t.accent.0,
                                t.accent.1,
                                t.accent.2,
                                if is_hov { 0.28 } else { 0.22 },
                            );
                            let _ = cr.fill();
                        } else if is_hov {
                            cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.78);
                            let _ = cr.fill();
                        } else {
                            cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.45);
                            let _ = cr.fill();
                        }

                        // Thumbnail (radius 10px) or glyph
                        let ic_col = if is_sel {
                            t.accent
                        } else {
                            file_icon_color(&item.name, item.is_dir, t)
                        };
                        draw_grid_visual(
                            cr,
                            cx_cell + 14.0,
                            cy_cell + 10.0,
                            64.0,
                            52.0,
                            &item.path,
                            item.icon,
                            ic_col,
                            st.files_thumbs,
                        );

                        // Filename (or inline rename)
                        let is_renaming = matches!(st.files_prompt, FilesPrompt::Rename(ref p) if p == &item.path);
                        if is_renaming {
                            rounded_rect(cr, cx_cell + 4.0, cy_cell + 66.0, cell_w - 8.0, 24.0, 6.0);
                            cr.set_source_rgba(t.surface_active.0, t.surface_active.1, t.surface_active.2, 0.98);
                            let _ = cr.fill();
                            let r_fit = fit_text(cr, st.files_prompt_text, 11.0, false, cell_w - 14.0);
                            draw_text(cr, &r_fit, cx_cell + 7.0, cy_cell + 82.0, 11.0, false, t.text_bright, 1.0);
                        } else {
                            let short = elide_middle(cr, &item.name, 11.5, is_sel, cell_w - 10.0);
                            let tw = text_width(cr, &short, 11.5, is_sel);
                            draw_text(
                                cr,
                                &short,
                                cx_cell + (cell_w - tw) * 0.5,
                                cy_cell + 82.0,
                                11.5,
                                is_sel,
                                if is_sel || is_hov { t.text_bright } else { t.text },
                                if is_sel || is_hov { 1.0 } else { 0.90 },
                            );
                        }

                        hits.push(HitBox {
                            x: cx_cell,
                            y: cy_cell.max(list_y),
                            w: cell_w,
                            h: cell_h,
                            target: tgt,
                            wheel: WheelAction::None,
                        });
                    }
                }

                let total_h = (total_rows as f64) * row_stride;
                if total_h > list_h {
                    let sb_h = ((list_h / total_h) * list_h).clamp(24.0, list_h);
                    let max_sc = (total_h - list_h).max(1.0);
                    let sb_y = list_y + (st.scroll_y / max_sc).clamp(0.0, 1.0) * (list_h - sb_h);
                    rounded_rect(cr, view_x + view_w - 4.5, sb_y, 2.5, sb_h, 1.25);
                    cr.set_source_rgba(t.muted.0, t.muted.1, t.muted.2, 0.35);
                    let _ = cr.fill();
                }
            }
        }
    }

    // Rubber-band Marquee Selection Box
    if let Some((mx, my, mw, mh)) = st.files_marquee_rect {
        if mw > 2.0 && mh > 2.0 {
            rounded_rect(cr, mx, my, mw, mh, 6.0);
            cr.set_source_rgba(t.accent.0, t.accent.1, t.accent.2, 0.18);
            let _ = cr.fill();
        }
    }

    cr.restore().ok();

    // =========================================================================
    // 4. BOTTOM STATUS PILL (height: 26px, tabular-nums, slide-up quick actions)
    // =========================================================================
    let st_x = view_x + 6.0;
    let st_y = view_y + view_h - status_h - 6.0;
    let st_w = view_w - 12.0;

    rounded_rect(cr, st_x, st_y, st_w, status_h, 13.0);
    cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.94);
    let _ = cr.fill();

    // Calculate selected count & total size of selected items (tabular-nums)
    let sel_count = if !st.selected_paths.is_empty() {
        st.selected_paths.len()
    } else if st.selected_idx.is_some() {
        1
    } else {
        0
    };

    let sel_bytes: u64 = if !st.selected_paths.is_empty() {
        st.entries
            .iter()
            .filter(|e| !e.is_dir && st.selected_paths.contains(&e.path))
            .map(|e| e.size)
            .sum()
    } else if let Some(idx) = st.selected_idx {
        st.entries
            .get(idx)
            .filter(|e| !e.is_dir)
            .map(|e| e.size)
            .unwrap_or(0)
    } else {
        0
    };

    let status_text = if sel_count > 0 {
        if sel_bytes > 0 {
            format!(
                "{} items  ·  {} selected ({})",
                st.entries.len(),
                sel_count,
                format_size(sel_bytes)
            )
        } else {
            format!("{} items  ·  {} selected", st.entries.len(), sel_count)
        }
    } else {
        format!("{} items", st.entries.len())
    };

    let slide = st.files_action_slide.clamp(0.0, 1.0);
    let left_avail_w = if slide > 0.05 {
        (st_w - 274.0 * slide).max(120.0)
    } else {
        st_w - 24.0
    };

    cr.save().ok();
    rounded_rect(cr, st_x + 4.0, st_y + 1.0, left_avail_w, status_h - 2.0, 10.0);
    cr.clip();

    draw_text(
        cr,
        &status_text,
        st_x + 12.0,
        st_y + 17.0,
        11.5,
        false,
        t.muted,
        0.92,
    );

    if !st.status_toast.is_empty() && slide < 0.25 {
        let base_w = text_width(cr, &status_text, 11.5, false);
        draw_text(
            cr,
            &format!("·  {}", st.status_toast),
            st_x + 20.0 + base_w,
            st_y + 17.0,
            11.5,
            true,
            t.accent,
            0.95 * (1.0 - slide * 4.0).clamp(0.0, 1.0),
        );
    }
    cr.restore().ok();

    // Slide-up Quick Action Buttons (Copy / Cut / Rename / Trash) when >= 1 file selected
    if slide > 0.01 {
        cr.save().ok();
        rounded_rect(cr, st_x, st_y, st_w, status_h, 13.0);
        cr.clip();

        let dy = (1.0 - slide) * 16.0;
        let actions: [(&str, &str, &'static str, bool); 4] = [
            ("󰆏", "Copy", "copy_files", false),
            ("󰆐", "Cut", "cut_files", false),
            ("󰑕", "Rename", "rename", false),
            ("󰩺", "Trash", "trash", true),
        ];

        let mut ax = st_x + st_w - 6.0;
        for &(ic, lbl, act, is_danger) in actions.iter().rev() {
            let bw = text_width(cr, lbl, 11.0, false) + 26.0;
            ax -= bw;
            let by = st_y + 3.0 + dy;
            let bh = 20.0;
            let tgt = HitTarget::FilesAction(act);
            let is_hov = st.hover.as_ref() == Some(&tgt);

            rounded_rect(cr, ax, by, bw, bh, 10.0);
            if is_danger {
                cr.set_source_rgba(
                    t.red.0,
                    t.red.1,
                    t.red.2,
                    (if is_hov { 0.30 } else { 0.16 }) * slide,
                );
            } else {
                cr.set_source_rgba(
                    t.surface_hover.0,
                    t.surface_hover.1,
                    t.surface_hover.2,
                    (if is_hov { 0.95 } else { 0.70 }) * slide,
                );
            }
            let _ = cr.fill();

            let col = if is_danger {
                t.red
            } else if is_hov {
                t.text_bright
            } else {
                t.text
            };
            draw_glyph_centered(cr, ic, ax + 6.0, by, 14.0, bh, 11.5, col, slide);
            draw_text(cr, lbl, ax + 21.0, by + 14.0, 11.0, false, col, slide);

            if sel_count > 0 {
                hits.push(HitBox {
                    x: ax,
                    y: st_y + 3.0,
                    w: bw,
                    h: bh,
                    target: tgt,
                    wheel: WheelAction::None,
                });
            }
            ax -= 4.0;
        }
        cr.restore().ok();
    }

    // =========================================================================
    // 5. CONTEXT MENU (radius: 14px, padding: 6px, no inner borders, 120ms OutCubic)
    // =========================================================================
    if let Some((mx, my, ctx_idx)) = st.files_context_menu {
        let target_item = ctx_idx
            .and_then(|i| st.entries.get(i))
            .or_else(|| st.selected_idx.and_then(|i| st.entries.get(i)));
        let is_arch = target_item
            .map(|it| !it.is_dir && is_archive_file(&it.name))
            .unwrap_or(false);
        let has_sel = sel_count > 0 || target_item.is_some();

        let pin_target_dir = target_item
            .filter(|it| it.is_dir)
            .map(|it| it.path.to_string_lossy().to_string())
            .or_else(|| {
                if target_item.is_none() {
                    Some(st.current_dir.to_string_lossy().to_string())
                } else {
                    None
                }
            });
        let pin_already = pin_target_dir
            .as_ref()
            .map(|d| st.cfg.bookmarks.iter().any(|b| b.dir == *d))
            .unwrap_or(false);

        let mut items: Vec<(&str, &str, &'static str, bool)> = vec![
            ("󰝰", "Open", "open", false),
            ("󰆍", "Open in Terminal", "term", false),
            ("󰉋", "New Folder", "mkdir", false),
        ];
        if pin_target_dir.is_some() {
            items.push((
                if pin_already { "󰐃" } else { "󰐄" },
                if pin_already { "Remove from Quick Access" } else { "Add to Quick Access" },
                "toggle_pin",
                false,
            ));
        }
        if is_arch {
            items.push(("󰛫", "Extract Here", "extract", false));
        }
        if has_sel {
            items.push(("󰛫", "Compress to .zip", "compress", false));
        }
        items.extend_from_slice(&[
            ("󰑕", "Rename", "rename", false),
            ("󰆏", "Copy", "copy_files", false),
            ("󰆐", "Cut", "cut_files", false),
            ("󰆒", "Paste", "paste_files", false),
            ("󰩺", "Move to Trash", "trash", true),
            ("󰅍", "Copy Path", "copy", false),
        ]);

        let menu_pad = 6.0;
        let row_h = 28.0;
        let menu_w = 188.0;
        let menu_h = (items.len() as f64) * row_h + menu_pad * 2.0;
        let rx = mx.clamp(cx + 6.0, (cx + cw - menu_w - 6.0).max(cx + 6.0));
        let ry = my.clamp(cy + 6.0, (cy + ch - menu_h - 6.0).max(cy + 6.0));

        let v = st.files_ctx_open.clamp(0.0, 1.0);
        let scale = 0.95 + 0.05 * v;
        let alpha = v;

        cr.save().ok();
        let center_x = rx + menu_w * 0.5;
        let center_y = ry + menu_h * 0.5;
        cr.translate(center_x, center_y);
        cr.scale(scale, scale);
        cr.translate(-center_x, -center_y);

        // Menu container: radius 14px, padding 6px, NO inner borders
        rounded_rect(cr, rx, ry, menu_w, menu_h, 14.0);
        cr.set_source_rgba(t.surface.0, t.surface.1, t.surface.2, 0.98 * alpha);
        let _ = cr.fill();

        let mut iy = ry + menu_pad;
        for &(ic, lbl, act, is_danger) in &items {
            let tgt = HitTarget::FilesContextMenuAction(act);
            let is_hov = st.hover.as_ref() == Some(&tgt);

            if is_hov {
                rounded_rect(cr, rx + menu_pad, iy, menu_w - menu_pad * 2.0, row_h, 8.0);
                if is_danger {
                    cr.set_source_rgba(t.red.0, t.red.1, t.red.2, 0.22 * alpha);
                } else {
                    cr.set_source_rgba(t.surface_hover.0, t.surface_hover.1, t.surface_hover.2, 0.92 * alpha);
                }
                let _ = cr.fill();
            }

            let col = if is_danger {
                t.red
            } else if is_hov {
                t.text_bright
            } else {
                t.text
            };
            draw_glyph_centered(cr, ic, rx + menu_pad + 8.0, iy, 16.0, row_h, 14.0, col, alpha);
            draw_text(cr, lbl, rx + menu_pad + 30.0, iy + 18.5, 12.0, false, col, alpha);

            hits.push(HitBox {
                x: rx + menu_pad,
                y: iy,
                w: menu_w - menu_pad * 2.0,
                h: row_h,
                target: tgt,
                wheel: WheelAction::None,
            });
            iy += row_h;
        }
        cr.restore().ok();
    }

    // =========================================================================
    // 6. DRAG & DROP FLOATING PILL (SHM fallback; GPU path composites overlay at 60fps)
    // =========================================================================
    if st.files_drag_in_cache && st.files_is_dragging && st.files_drag_count > 0 {
        let (dx, dy) = st.files_drag_pos;
        let (bw, bh) = drag_pill_size(cr, st.files_drag_label, st.files_drag_count);
        let bx = (dx + 12.0).clamp(cx + 4.0, cx + cw - bw - 4.0);
        let by = (dy + 12.0).clamp(cy + 4.0, cy + ch - bh - 4.0);
        draw_drag_pill_at(cr, bx, by, bw, bh, st.files_drag_label, st.files_drag_count, t);
    }
}

pub fn drag_pill_size(cr: &Context, label: &str, count: usize) -> (f64, f64) {
    let badge_txt = if count > 1 {
        format!("󰉋  {count} items")
    } else {
        format!("󰈔  {}", fit_text(cr, label, 12.0, true, 140.0))
    };
    let bw = (text_width(cr, &badge_txt, 12.0, true) + 24.0).max(84.0);
    (bw, 28.0)
}

pub fn draw_drag_pill_at(
    cr: &Context,
    bx: f64,
    by: f64,
    bw: f64,
    bh: f64,
    label: &str,
    count: usize,
    t: &crate::theme::Theme,
) {
    let badge_txt = if count > 1 {
        format!("󰉋  {count} items")
    } else {
        format!("󰈔  {}", fit_text(cr, label, 12.0, true, 140.0))
    };
    rounded_rect(cr, bx, by, bw, bh, 14.0);
    cr.set_source_rgba(t.surface_active.0, t.surface_active.1, t.surface_active.2, 0.96);
    let _ = cr.fill();
    draw_text(cr, &badge_txt, bx + 12.0, by + 18.5, 12.0, true, t.accent, 1.0);
}
