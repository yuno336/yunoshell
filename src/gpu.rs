//! Hybrid EGL/OpenGL ES presenter for the island overlay.
//!
//! The popup UI is still drawn with Cairo (pixel-identical to the SHM path), but
//! instead of handing the compositor a full-screen SHM buffer every frame, only
//! the damaged sub-rectangle is uploaded as a texture and composited on the GPU.

use std::ffi::c_void;

use cairo::ImageSurface;
use glow::HasContext;
use khronos_egl as egl;

#[link(name = "wayland-egl")]
extern "C" {
    fn wl_egl_window_create(
        surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> *mut c_void;
    fn wl_egl_window_destroy(window: *mut c_void);
    fn wl_egl_window_resize(window: *mut c_void, width: i32, height: i32, dx: i32, dy: i32);
}

const VS: &str = r#"#version 100
attribute vec2 a_pos;
attribute vec2 a_uv;
uniform vec2 u_size;
varying vec2 v_uv;
varying vec2 v_pos;
void main() {
    vec2 ndc = vec2(a_pos.x / u_size.x * 2.0 - 1.0, 1.0 - a_pos.y / u_size.y * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_pos = a_pos;
}
"#;

const ISLAND_FS: &str = r#"#version 100
precision highp float;
varying vec2 v_pos;
uniform sampler2D u_tex;
uniform vec4 u_body;         // bx, by, bw, bh in physical px
uniform vec4 u_params;       // ear, radius, vertical, invert
uniform vec4 u_content_rect; // cx, cy, cw, ch in physical px
uniform vec2 u_uv_max;       // cw / tex_w, ch / tex_h
uniform vec4 u_bg;           // non-premul rgba
uniform vec4 u_border;       // non-premul rgba
uniform vec2 u_content_fx;   // alpha, scale

void main() {
    vec2 p = v_pos;
    float bx = u_body.x;
    float by = u_body.y;
    float bw = u_body.z;
    float bh = u_body.w;
    float ear = u_params.x;
    float rad = u_params.y;
    bool is_vert = u_params.z > 0.5;
    bool is_inv = u_params.w > 0.5;

    float qx;
    float qy;
    float hx;
    float depth;
    if (is_vert) {
        qx = abs(p.y - (by + bh * 0.5));
        qy = is_inv ? ((bx + bw) - p.x) : (p.x - bx);
        hx = bh * 0.5;
        depth = bw;
    } else {
        qx = abs(p.x - (bx + bw * 0.5));
        qy = is_inv ? ((by + bh) - p.y) : (p.y - by);
        hx = bw * 0.5;
        depth = bh;
    }

    vec2 p_box = vec2(qx, qy - depth * 0.5);
    vec2 half_b = vec2(hx, depth * 0.5);
    float cr = (p_box.y > 0.0) ? min(rad, min(hx, depth * 0.5)) : 0.0;
    vec2 q = abs(p_box) - half_b + cr;
    float d_body = min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - cr;

    float d = d_body;
    if (ear > 0.5) {
        float d_circle = ear - length(vec2(qx - (hx + ear), qy - ear));
        float d_ear = max(max(-qy, max(qy - ear, qx - (hx + ear))), max(hx - max(ear, 16.0) - qx, d_circle));
        d = min(d_body, d_ear);
    }

    float mask = 1.0 - smoothstep(-0.85, 0.85, d);
    if (mask <= 0.002) {
        discard;
    }

    float seam_fade = smoothstep(0.0, 2.5, qy);
    float edge_band = smoothstep(-2.4, -0.7, d) * smoothstep(0.6, -0.4, d) * seam_fade;
    vec4 bg_p = vec4(u_bg.rgb * u_bg.a, u_bg.a);
    vec4 bd_p = vec4(u_border.rgb * u_border.a, u_border.a);
    vec4 out_col = mix(bg_p, bd_p, edge_band);

    float c_alpha = u_content_fx.x;
    float c_scale = max(u_content_fx.y, 0.05);
    if (c_alpha > 0.005 && u_content_rect.z > 1.0 && u_content_rect.w > 1.0) {
        vec2 anchor = vec2(u_content_rect.x + u_content_rect.z * 0.5, u_content_rect.y);
        vec2 sp = (p - anchor) / c_scale + anchor;
        vec2 cuv = (sp - u_content_rect.xy) / u_content_rect.zw;
        if (cuv.x >= 0.0 && cuv.x <= 1.0 && cuv.y >= 0.0 && cuv.y <= 1.0) {
            vec4 tc = texture2D(u_tex, cuv * u_uv_max);
            vec4 content_p = vec4(tc.b, tc.g, tc.r, tc.a) * c_alpha;
            out_col = content_p + out_col * (1.0 - content_p.a);
        }
    }

    gl_FragColor = out_col * mask;
}
"#;

const FS: &str = r#"#version 100
precision mediump float;
uniform sampler2D u_tex;
varying vec2 v_uv;
void main() {
    // Cairo ARGB32 is BGRA in memory on little-endian hosts.
    vec4 c = texture2D(u_tex, v_uv);
    gl_FragColor = vec4(c.b, c.g, c.r, c.a);
}
"#;

const WIPE_FS: &str = r#"#version 100
precision highp float;
uniform sampler2D u_tex;
uniform float u_progress;
uniform vec2 u_aspect;
uniform vec3 u_accent;
varying vec2 v_uv;
void main() {
    vec3 c = texture2D(u_tex, v_uv).rgb;
    float d = dot(v_uv, u_aspect);
    float feather = 0.024;
    float p = clamp(u_progress, 0.0, 1.0);
    float head = mix(-feather, 1.0 + feather, p);
    float keep = smoothstep(head - feather, head + feather, d);
    float sheen = exp(-pow((d - head) / 0.006, 2.0)) * 0.22 * sin(p * 3.14159265);
    vec3 col = clamp(c * keep + u_accent * sheen, 0.0, 1.0);
    float alpha = clamp(keep + sheen * 0.65, 0.0, 1.0);
    gl_FragColor = vec4(col, alpha);
}
"#;

struct EglBundle {
    egl: egl::Instance<egl::Static>,
    display: egl::Display,
    surface: egl::Surface,
    context: egl::Context,
    window: *mut c_void,
    gl: glow::Context,
}

fn init_egl_window(
    display_ptr: *mut c_void,
    surface_ptr: *mut c_void,
    width: i32,
    height: i32,
) -> Result<EglBundle, String> {
    let width = width.max(1);
    let height = height.max(1);
    let egl = egl::Instance::new(egl::Static);
    let display = unsafe { egl.get_display(display_ptr) }
        .ok_or_else(|| "eglGetDisplay failed".to_string())?;
    egl.initialize(display)
        .map_err(|e| format!("eglInitialize: {e:?}"))?;
    egl.bind_api(egl::OPENGL_ES_API)
        .map_err(|e| format!("eglBindAPI: {e:?}"))?;

    let config_attribs = [
        egl::SURFACE_TYPE,
        egl::WINDOW_BIT,
        egl::RENDERABLE_TYPE,
        egl::OPENGL_ES2_BIT,
        egl::RED_SIZE,
        8,
        egl::GREEN_SIZE,
        8,
        egl::BLUE_SIZE,
        8,
        egl::ALPHA_SIZE,
        8,
        egl::NONE,
    ];
    let config = egl
        .choose_first_config(display, &config_attribs)
        .map_err(|e| format!("eglChooseConfig: {e:?}"))?
        .ok_or_else(|| "no EGL config".to_string())?;

    let context_attribs = [egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE];
    let context = egl
        .create_context(display, config, None, &context_attribs)
        .map_err(|e| format!("eglCreateContext: {e:?}"))?;

    let window = unsafe { wl_egl_window_create(surface_ptr, width, height) };
    if window.is_null() {
        return Err("wl_egl_window_create failed".to_string());
    }
    let surface = match unsafe { egl.create_window_surface(display, config, window, None) } {
        Ok(s) => s,
        Err(e) => {
            unsafe { wl_egl_window_destroy(window) };
            return Err(format!("eglCreateWindowSurface: {e:?}"));
        }
    };
    egl.make_current(display, Some(surface), Some(surface), Some(context))
        .map_err(|e| format!("eglMakeCurrent: {e:?}"))?;
    let _ = egl.swap_interval(display, 0);

    let gl = unsafe {
        glow::Context::from_loader_function(|name| {
            egl.get_proc_address(name)
                .map(|f| f as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    };
    Ok(EglBundle {
        egl,
        display,
        surface,
        context,
        window,
        gl,
    })
}

pub struct IslandGpuParams {
    pub bx: f32,
    pub by: f32,
    pub bw: f32,
    pub bh: f32,
    pub ear: f32,
    pub radius: f32,
    pub vertical: bool,
    pub invert: bool,
    pub cx: f32,
    pub cy: f32,
    pub cw: f32,
    pub ch: f32,
    pub content_alpha: f32,
    pub content_scale: f32,
    pub bg_rgba: [f32; 4],
    pub border_rgba: [f32; 4],
    pub drag_rect: Option<(f32, f32, f32, f32)>,
}

struct IslandUniforms {
    size: Option<glow::UniformLocation>,
    tex: Option<glow::UniformLocation>,
    body: Option<glow::UniformLocation>,
    params: Option<glow::UniformLocation>,
    content_rect: Option<glow::UniformLocation>,
    uv_max: Option<glow::UniformLocation>,
    bg: Option<glow::UniformLocation>,
    border: Option<glow::UniformLocation>,
    content_fx: Option<glow::UniformLocation>,
    a_pos: u32,
    a_uv: u32,
}

pub struct GpuIsland {
    egl: egl::Instance<egl::Static>,
    display: egl::Display,
    surface: egl::Surface,
    context: egl::Context,
    window: *mut c_void,
    gl: glow::Context,
    program: glow::Program,
    island_program: glow::Program,
    iu: IslandUniforms,
    vbo: glow::Buffer,
    texture: glow::Texture,
    tex_w: i32,
    tex_h: i32,
    texture_w: i32,
    texture_h: i32,
    uploaded_gen: u64,
    content_tex_w: i32,
    content_tex_h: i32,
    drag_tex: glow::Texture,
    drag_tex_w: i32,
    drag_tex_h: i32,
    drag_gen: u64,
    scratch: Vec<u8>,
}

impl GpuIsland {
    pub fn new(
        display_ptr: *mut c_void,
        surface_ptr: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        let b = init_egl_window(display_ptr, surface_ptr, width, height)?;
        let egl = b.egl;
        let display = b.display;
        let surface = b.surface;
        let context = b.context;
        let window = b.window;
        let gl = b.gl;

        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        }

        let program = compile_program(&gl, VS, FS)?;
        let island_program = compile_program(&gl, VS, ISLAND_FS)?;
        let iu = unsafe {
            IslandUniforms {
                size: gl.get_uniform_location(island_program, "u_size"),
                tex: gl.get_uniform_location(island_program, "u_tex"),
                body: gl.get_uniform_location(island_program, "u_body"),
                params: gl.get_uniform_location(island_program, "u_params"),
                content_rect: gl.get_uniform_location(island_program, "u_content_rect"),
                uv_max: gl.get_uniform_location(island_program, "u_uv_max"),
                bg: gl.get_uniform_location(island_program, "u_bg"),
                border: gl.get_uniform_location(island_program, "u_border"),
                content_fx: gl.get_uniform_location(island_program, "u_content_fx"),
                a_pos: gl.get_attrib_location(island_program, "a_pos").unwrap_or(0) as u32,
                a_uv: gl.get_attrib_location(island_program, "a_uv").unwrap_or(1) as u32,
            }
        };
        unsafe {
            eprintln!(
                "[yunoshell] GPU renderer: {} / {}",
                gl.get_parameter_string(glow::VENDOR),
                gl.get_parameter_string(glow::RENDERER)
            );
        }

        let vbo = unsafe {
            let vbo = gl.create_buffer().map_err(|e| format!("create_buffer: {e}"))?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_size(glow::ARRAY_BUFFER, 4 * 16, glow::DYNAMIC_DRAW);
            vbo
        };

        let init_tw = 2048;
        let init_th = 1024;
        let texture = unsafe {
            let tex = gl.create_texture().map_err(|e| format!("create_texture: {e}"))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                init_tw,
                init_th,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );
            tex
        };

        let drag_tex = unsafe {
            let tex = gl.create_texture().map_err(|e| format!("create_texture drag: {e}"))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            tex
        };

        unsafe {
            gl.viewport(0, 0, width, height);
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        Ok(Self {
            egl,
            display,
            surface,
            context,
            window,
            gl,
            program,
            island_program,
            iu,
            vbo,
            texture,
            tex_w: width,
            tex_h: height,
            texture_w: init_tw,
            texture_h: init_th,
            uploaded_gen: 0,
            content_tex_w: 0,
            content_tex_h: 0,
            drag_tex,
            drag_tex_w: 0,
            drag_tex_h: 0,
            drag_gen: 0,
            scratch: Vec::with_capacity((init_tw * init_th * 4) as usize),
        })
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.tex_w && height == self.tex_h {
            return;
        }
        unsafe {
            wl_egl_window_resize(self.window, width, height, 0, 0);
            let _ = self
                .egl
                .make_current(self.display, Some(self.surface), Some(self.surface), Some(self.context));
            self.gl.viewport(0, 0, width, height);
        }
        // Note: texture is resized in present() based on island rect, not screen size
        self.tex_w = width;
        self.tex_h = height;
    }

    pub fn clear(&mut self) {
        if !self.make_current() {
            return;
        }
        unsafe {
            self.gl.viewport(0, 0, self.tex_w, self.tex_h);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        let _ = self.egl.swap_buffers(self.display, self.surface);
    }

    /// Uploads Cairo `content_cache` ONLY when `cache_gen` changed (0 CPU copy during animation).
    pub fn upload_content_if_needed(&mut self, img: &mut ImageSurface, cache_gen: u64) {
        if self.uploaded_gen == cache_gen {
            return;
        }
        if !self.make_current() {
            return;
        }
        let rw = img.width().max(0);
        let rh = img.height().max(0);
        if rw <= 0 || rh <= 0 {
            self.uploaded_gen = cache_gen;
            self.content_tex_w = 0;
            self.content_tex_h = 0;
            return;
        }
        if rw > self.texture_w || rh > self.texture_h {
            let new_tw = (rw.max(self.texture_w).max(256) as u32).next_power_of_two().min(4096) as i32;
            let new_th = (rh.max(self.texture_h).max(256) as u32).next_power_of_two().min(4096) as i32;
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    new_tw,
                    new_th,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    None,
                );
            }
            self.texture_w = new_tw;
            self.texture_h = new_th;
        }
        let stride = img.stride() as usize;
        let row_bytes = (rw as usize) * 4;
        img.flush();
        if let Ok(data) = img.data() {
            let total_bytes = row_bytes * (rh as usize);
            if stride == row_bytes && data.len() >= total_bytes {
                unsafe {
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                    self.gl.tex_sub_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        0,
                        0,
                        rw,
                        rh,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(&data[..total_bytes]),
                    );
                }
            } else {
                self.scratch.clear();
                self.scratch.reserve(total_bytes);
                for y in 0..(rh as usize) {
                    let start = y * stride;
                    let end = start + row_bytes;
                    if end <= data.len() {
                        self.scratch.extend_from_slice(&data[start..end]);
                    }
                }
                unsafe {
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                    self.gl.tex_sub_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        0,
                        0,
                        rw,
                        rh,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(&self.scratch),
                    );
                }
            }
        }
        self.content_tex_w = rw;
        self.content_tex_h = rh;
        self.uploaded_gen = cache_gen;
    }

    /// Uploads the small floating drag pill surface once per drag operation.
    pub fn upload_drag_overlay(&mut self, img: &mut ImageSurface, gen: u64) {
        if gen == self.drag_gen && self.drag_tex_w > 0 {
            return;
        }
        if !self.make_current() {
            return;
        }
        let cw = img.width().max(1);
        let ch = img.height().max(1);
        let stride = img.stride() as usize;
        let row_bytes = (cw as usize) * 4;
        img.flush();
        if let Ok(data) = img.data() {
            self.scratch.clear();
            if stride == row_bytes && data.len() >= row_bytes * (ch as usize) {
                self.scratch.extend_from_slice(&data[..row_bytes * (ch as usize)]);
            } else {
                self.scratch.reserve(row_bytes * (ch as usize));
                for y in 0..(ch as usize) {
                    let start = y * stride;
                    let end = start + row_bytes;
                    if end <= data.len() {
                        self.scratch.extend_from_slice(&data[start..end]);
                    }
                }
            }
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.drag_tex));
                self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    cw,
                    ch,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(&self.scratch),
                );
            }
            self.drag_tex_w = cw;
            self.drag_tex_h = ch;
            self.drag_gen = gen;
        }
    }

    /// Pure GPU SDF Dynamic Island compositor for v1 concave-ear geometry + cached content texture.
    pub fn present_island(&mut self, p: &IslandGpuParams) {
        if !self.make_current() {
            return;
        }
        let (w, h) = (self.tex_w, self.tex_h);
        unsafe {
            self.gl.viewport(0, 0, w, h);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let pad = p.ear.max(2.0) + 4.0;
        let (x0, y0, x1, y1) = if p.vertical {
            (
                (p.bx - 4.0).max(0.0),
                (p.by - pad).max(0.0),
                (p.bx + p.bw + 4.0).min(w as f32),
                (p.by + p.bh + pad).min(h as f32),
            )
        } else {
            (
                (p.bx - pad).max(0.0),
                (p.by - 4.0).max(0.0),
                (p.bx + p.bw + pad).min(w as f32),
                (p.by + p.bh + 4.0).min(h as f32),
            )
        };

        if x1 > x0 && y1 > y0 {
            let uv_max_x = (self.content_tex_w as f32) / (self.texture_w.max(1) as f32);
            let uv_max_y = (self.content_tex_h as f32) / (self.texture_h.max(1) as f32);
            let verts: [f32; 16] = [
                x0, y0, 0.0, 0.0, //
                x1, y0, 1.0, 0.0, //
                x0, y1, 0.0, 1.0, //
                x1, y1, 1.0, 1.0, //
            ];
            unsafe {
                self.gl.use_program(Some(self.island_program));
                self.gl.active_texture(glow::TEXTURE0);
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                self.gl.uniform_2_f32(self.iu.size.as_ref(), w as f32, h as f32);
                self.gl.uniform_1_i32(self.iu.tex.as_ref(), 0);
                self.gl.uniform_4_f32(self.iu.body.as_ref(), p.bx, p.by, p.bw, p.bh);
                self.gl.uniform_4_f32(
                    self.iu.params.as_ref(),
                    p.ear,
                    p.radius,
                    if p.vertical { 1.0 } else { 0.0 },
                    if p.invert { 1.0 } else { 0.0 },
                );
                self.gl.uniform_4_f32(self.iu.content_rect.as_ref(), p.cx, p.cy, p.cw, p.ch);
                self.gl.uniform_2_f32(self.iu.uv_max.as_ref(), uv_max_x, uv_max_y);
                self.gl.uniform_4_f32(self.iu.bg.as_ref(), p.bg_rgba[0], p.bg_rgba[1], p.bg_rgba[2], p.bg_rgba[3]);
                self.gl.uniform_4_f32(self.iu.border.as_ref(), p.border_rgba[0], p.border_rgba[1], p.border_rgba[2], p.border_rgba[3]);
                self.gl.uniform_2_f32(self.iu.content_fx.as_ref(), p.content_alpha, p.content_scale);

                self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                self.gl.buffer_sub_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    0,
                    std::slice::from_raw_parts(verts.as_ptr() as *const u8, std::mem::size_of_val(&verts)),
                );
                self.gl.enable_vertex_attrib_array(self.iu.a_pos);
                self.gl.vertex_attrib_pointer_f32(self.iu.a_pos, 2, glow::FLOAT, false, 16, 0);
                self.gl.enable_vertex_attrib_array(self.iu.a_uv);
                self.gl.vertex_attrib_pointer_f32(self.iu.a_uv, 2, glow::FLOAT, false, 16, 8);
                self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            }
        }

        if let Some((dx, dy, dw, dh)) = p.drag_rect {
            if self.drag_tex_w > 0 && self.drag_tex_h > 0 {
                let dverts: [f32; 16] = [
                    dx, dy, 0.0, 0.0,
                    dx + dw, dy, 1.0, 0.0,
                    dx, dy + dh, 0.0, 1.0,
                    dx + dw, dy + dh, 1.0, 1.0,
                ];
                unsafe {
                    self.gl.enable(glow::BLEND);
                    self.gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
                    self.gl.use_program(Some(self.program));
                    self.gl.active_texture(glow::TEXTURE0);
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(self.drag_tex));
                    if let Some(u_size) = self.gl.get_uniform_location(self.program, "u_size") {
                        self.gl.uniform_2_f32(Some(&u_size), w as f32, h as f32);
                    }
                    if let Some(u_tex) = self.gl.get_uniform_location(self.program, "u_tex") {
                        self.gl.uniform_1_i32(Some(&u_tex), 0);
                    }
                    self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                    self.gl.buffer_sub_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        0,
                        std::slice::from_raw_parts(dverts.as_ptr() as *const u8, std::mem::size_of_val(&dverts)),
                    );
                    let a_pos = self.gl.get_attrib_location(self.program, "a_pos").unwrap_or(0);
                    let a_uv = self.gl.get_attrib_location(self.program, "a_uv").unwrap_or(1);
                    self.gl.enable_vertex_attrib_array(a_pos);
                    self.gl.vertex_attrib_pointer_f32(a_pos, 2, glow::FLOAT, false, 16, 0);
                    self.gl.enable_vertex_attrib_array(a_uv);
                    self.gl.vertex_attrib_pointer_f32(a_uv, 2, glow::FLOAT, false, 16, 8);
                    self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                    self.gl.disable(glow::BLEND);
                }
            }
        }

        let _ = self.egl.swap_buffers(self.display, self.surface);
    }

    /// Uploads the island image (sized to island bounds) and composites it at screen position.
    #[allow(dead_code)]
    pub fn present(&mut self, img: &mut ImageSurface, rect: (f64, f64, f64, f64)) {
        if !self.make_current() {
            return;
        }
        let (w, h) = (self.tex_w, self.tex_h);
        unsafe {
            self.gl.viewport(0, 0, w, h);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let x0 = rect.0.floor().max(0.0) as i32;
        let y0 = rect.1.floor().max(0.0) as i32;
        let rw = img.width().min(rect.2.ceil().max(0.0) as i32);
        let rh = img.height().min(rect.3.ceil().max(0.0) as i32);
        let x1 = x0 + rw;
        let y1 = y0 + rh;

        if rw > 0 && rh > 0 {
            // Only grow texture if island exceeds pre-allocated atlas capacity
            if rw > self.texture_w || rh > self.texture_h {
                let new_tw = (rw.max(self.texture_w).max(256) as u32).next_power_of_two().min(4096) as i32;
                let new_th = (rh.max(self.texture_h).max(256) as u32).next_power_of_two().min(4096) as i32;
                unsafe {
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                    self.gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA as i32,
                        new_tw,
                        new_th,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        None,
                    );
                }
                self.texture_w = new_tw;
                self.texture_h = new_th;
            }

            // Upload the island image safely respecting Cairo row stride
            let stride = img.stride() as usize;
            let row_bytes = (rw as usize) * 4;
            img.flush();
            if let Ok(data) = img.data() {
                self.scratch.clear();
                if stride == row_bytes && data.len() >= row_bytes * (rh as usize) {
                    self.scratch.extend_from_slice(&data[..row_bytes * (rh as usize)]);
                } else {
                    self.scratch.reserve(row_bytes * (rh as usize));
                    for y in 0..(rh as usize) {
                        let start = y * stride;
                        let end = start + row_bytes;
                        if end <= data.len() {
                            self.scratch.extend_from_slice(&data[start..end]);
                        }
                    }
                }
                unsafe {
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                    self.gl.tex_sub_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        0, 0,
                        rw,
                        rh,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(&self.scratch),
                    );
                }
            }

            // Draw quad at island's screen position with sub-texture UV coords
            let (fx0, fy0, fx1, fy1) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
            let u1 = rw as f32 / (self.texture_w.max(1) as f32);
            let v1 = rh as f32 / (self.texture_h.max(1) as f32);
            let verts: [f32; 16] = [
                fx0, fy0, 0.0, 0.0, //
                fx1, fy0, u1,  0.0, //
                fx0, fy1, 0.0, v1,  //
                fx1, fy1, u1,  v1,  //
            ];
            unsafe {
                self.gl.use_program(Some(self.program));
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                let loc = self.gl.get_uniform_location(self.program, "u_size");
                self.gl.uniform_2_f32(loc.as_ref(), w as f32, h as f32);
                let loc = self.gl.get_uniform_location(self.program, "u_tex");
                self.gl.uniform_1_i32(loc.as_ref(), 0);
                self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                self.gl.buffer_sub_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    0,
                    std::slice::from_raw_parts(
                        verts.as_ptr() as *const u8,
                        std::mem::size_of_val(&verts),
                    ),
                );
                let a_pos = self.gl.get_attrib_location(self.program, "a_pos").unwrap_or(0) as u32;
                let a_uv = self.gl.get_attrib_location(self.program, "a_uv").unwrap_or(1) as u32;
                self.gl.enable_vertex_attrib_array(a_pos);
                self.gl.vertex_attrib_pointer_f32(a_pos, 2, glow::FLOAT, false, 16, 0);
                self.gl.enable_vertex_attrib_array(a_uv);
                self.gl.vertex_attrib_pointer_f32(a_uv, 2, glow::FLOAT, false, 16, 8);
                self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            }
        }

        let _ = self.egl.swap_buffers(self.display, self.surface);
    }

    fn make_current(&self) -> bool {
        if self.egl.get_current_surface(egl::DRAW) == Some(self.surface)
            && self.egl.get_current_context() == Some(self.context)
        {
            return true;
        }
        self.egl
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .is_ok()
    }
}

impl Drop for GpuIsland {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .egl
                .make_current(self.display, None, None, None);
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_texture(self.texture);
            self.gl.delete_texture(self.drag_tex);
            self.gl.delete_program(self.program);
            self.gl.delete_program(self.island_program);
            let _ = self.egl.destroy_surface(self.display, self.surface);
            let _ = self.egl.destroy_context(self.display, self.context);
            if !self.window.is_null() {
                wl_egl_window_destroy(self.window);
            }
        }
    }
}

/// Pure GPU diagonal wipe overlay ("chéo xuống") for 60fps zero-lag theme & wallpaper switches.
pub struct GpuWipe {
    egl: egl::Instance<egl::Static>,
    display: egl::Display,
    surface: egl::Surface,
    context: egl::Context,
    window: *mut c_void,
    gl: glow::Context,
    program: glow::Program,
    vbo: glow::Buffer,
    tex: glow::Texture,
    win_w: i32,
    win_h: i32,
    u_size: Option<glow::UniformLocation>,
    u_tex: Option<glow::UniformLocation>,
    u_progress: Option<glow::UniformLocation>,
    u_aspect: Option<glow::UniformLocation>,
    u_accent: Option<glow::UniformLocation>,
    a_pos: u32,
    a_uv: u32,
}

impl GpuWipe {
    pub fn new(
        display_ptr: *mut c_void,
        surface_ptr: *mut c_void,
        win_w: i32,
        win_h: i32,
        rgb_w: i32,
        rgb_h: i32,
        rgb_bytes: &[u8],
    ) -> Result<Self, String> {
        let b = init_egl_window(display_ptr, surface_ptr, win_w, win_h)?;
        let gl = b.gl;
        let program = compile_program(&gl, VS, WIPE_FS)?;
        let (vbo, tex, u_size, u_tex, u_progress, u_aspect, u_accent, a_pos, a_uv) = unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

            let vbo = gl.create_buffer().map_err(|e| format!("create_buffer: {e}"))?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let fw = win_w.max(1) as f32;
            let fh = win_h.max(1) as f32;
            let verts: [f32; 16] = [
                0.0, 0.0, 0.0, 0.0,
                fw, 0.0, 1.0, 0.0,
                0.0, fh, 0.0, 1.0,
                fw, fh, 1.0, 1.0,
            ];
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                std::slice::from_raw_parts(verts.as_ptr() as *const u8, std::mem::size_of_val(&verts)),
                glow::STATIC_DRAW,
            );

            let tex = gl.create_texture().map_err(|e| format!("create_texture: {e}"))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGB as i32,
                rgb_w.max(1),
                rgb_h.max(1),
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                Some(rgb_bytes),
            );

            (
                vbo,
                tex,
                gl.get_uniform_location(program, "u_size"),
                gl.get_uniform_location(program, "u_tex"),
                gl.get_uniform_location(program, "u_progress"),
                gl.get_uniform_location(program, "u_aspect"),
                gl.get_uniform_location(program, "u_accent"),
                gl.get_attrib_location(program, "a_pos").unwrap_or(0) as u32,
                gl.get_attrib_location(program, "a_uv").unwrap_or(1) as u32,
            )
        };

        Ok(Self {
            egl: b.egl,
            display: b.display,
            surface: b.surface,
            context: b.context,
            window: b.window,
            gl,
            program,
            vbo,
            tex,
            win_w: win_w.max(1),
            win_h: win_h.max(1),
            u_size,
            u_tex,
            u_progress,
            u_aspect,
            u_accent,
            a_pos,
            a_uv,
        })
    }

    pub fn present(&self, progress: f32, accent: [f32; 3]) {
        if self
            .egl
            .make_current(self.display, Some(self.surface), Some(self.surface), Some(self.context))
            .is_err()
        {
            return;
        }
        let w = self.win_w as f32;
        let h = self.win_h as f32;
        let sum = (w + h).max(1.0);
        unsafe {
            self.gl.viewport(0, 0, self.win_w, self.win_h);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.use_program(Some(self.program));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));
            self.gl.uniform_2_f32(self.u_size.as_ref(), w, h);
            self.gl.uniform_1_i32(self.u_tex.as_ref(), 0);
            self.gl.uniform_1_f32(self.u_progress.as_ref(), progress);
            self.gl.uniform_2_f32(self.u_aspect.as_ref(), w / sum, h / sum);
            self.gl.uniform_3_f32(self.u_accent.as_ref(), accent[0], accent[1], accent[2]);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.enable_vertex_attrib_array(self.a_pos);
            self.gl.vertex_attrib_pointer_f32(self.a_pos, 2, glow::FLOAT, false, 16, 0);
            self.gl.enable_vertex_attrib_array(self.a_uv);
            self.gl.vertex_attrib_pointer_f32(self.a_uv, 2, glow::FLOAT, false, 16, 8);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
        let _ = self.egl.swap_buffers(self.display, self.surface);
    }
}

impl Drop for GpuWipe {
    fn drop(&mut self) {
        unsafe {
            let _ = self.egl.make_current(self.display, None, None, None);
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_texture(self.tex);
            self.gl.delete_program(self.program);
            let _ = self.egl.destroy_surface(self.display, self.surface);
            let _ = self.egl.destroy_context(self.display, self.context);
            if !self.window.is_null() {
                wl_egl_window_destroy(self.window);
            }
        }
    }
}

fn compile_program(gl: &glow::Context, vs_src: &str, fs_src: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program().map_err(|e| format!("create_program: {e}"))?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vs_src), (glow::FRAGMENT_SHADER, fs_src)] {
            let shader = gl.create_shader(kind).map_err(|e| format!("create_shader: {e}"))?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                return Err(format!("shader compile failed: {log}"));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            return Err(format!("program link failed: {log}"));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        Ok(program)
    }
}
