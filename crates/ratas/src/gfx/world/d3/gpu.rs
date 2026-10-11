//! The land and the creatures go to the video card in their own buffers
//! (miniquad, past macroquad's batching): the chunks of land once, when they
//! are built, and the creatures every frame in one stream. One shader lights
//! them: in the low-poly look by the old faceted shade of the sun, in the
//! detailed looks per pixel, with the sky and the ground as ambient light,
//! highlights, the hero's torch on the faces turned to it, rippling water
//! and haze in the distance. Brush strokes are painted over everything, and
//! the materials (materials.rs) are laid over the models from three sides,
//! their relief lit, their gloss shining; the detailed look carves the
//! models (each face lit flat), the high-poly look keeps them smooth. Every
//! look darkens by the map of darkness (the night, the dark of dungeons,
//! what the hero does not see).

use macroquad::prelude::*;
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferId, BufferLayout, BufferSource, BufferType,
    BufferUsage, Comparison, Equation, PassAction, Pipeline, PipelineParams, ShaderMeta,
    UniformBlockLayout, UniformDesc, UniformType, UniformsSource, VertexAttribute, VertexFormat,
};

use super::lowpoly::Batch;
use super::materials;

/// The vertex shader (without its version line, see `sources`).
const VERTEX: &str = r#"
attribute vec3 position;
attribute vec4 color0;
attribute vec4 normal;
attribute vec4 extra;
attribute vec4 local;
varying lowp vec4 color;
varying highp vec3 wpos;
varying mediump vec3 nrm;
varying mediump vec2 mat;
varying highp vec4 tpos;
varying mediump vec2 tmat;
uniform mat4 Mvp;
uniform highp vec4 Sun;
void main() {
    vec3 p = position;
    float sway = extra.x / 255.0;
    if (sway > 0.0) {
        float t = Sun.w;
        float ph = p.x * 0.8 + p.z * 0.55;
        p.x += sin(t * 1.9 + ph) * 0.05 * sway;
        p.z += cos(t * 1.4 + ph * 1.3) * 0.04 * sway;
    }
    wpos = p;
    nrm = (normal.xyz - 128.0) / 127.0;
    mat = vec2(normal.w / 255.0, extra.y);
    color = color0 / 255.0;
    // materials stay on a creature as it walks and turns, on the land
    // where they lie (swaying with what sways)
    if (local.w > 0.5) {
        tpos = vec4((local.xyz - 32768.0) / 1024.0, (local.w - 1.0) / 65534.0 * 6.2831853);
    } else {
        tpos = vec4(position, -10.0);
    }
    tmat = vec2(extra.z, extra.w / 16.0);
    gl_Position = Mvp * vec4(p, 1.0);
}"#;

/// The fragment shader (without its version line, see `sources`).
const FRAGMENT: &str = r#"
precision mediump float;
varying lowp vec4 color;
varying highp vec3 wpos;
varying mediump vec3 nrm;
varying mediump vec2 mat;
varying highp vec4 tpos;
varying mediump vec2 tmat;
uniform sampler2D Dark;
uniform sampler2D Detail;
uniform sampler2D Atlas;
uniform highp vec4 DarkRect;
uniform vec4 Tint;
uniform highp vec4 Torch;
uniform vec4 TorchColor;
uniform highp vec4 Eye;
uniform highp vec4 Sun;
uniform vec4 Fog;
uniform highp vec4 FogRange;
#ifdef MODERN
vec3 turn(vec3 v, float a) {
    float c = cos(a);
    float s = sin(a);
    return vec3(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}
// a material's tile at uv (its slopes over the screen given: the tile
// repeats inside its slot of the picture)
vec4 stuff(float id, vec2 uv, vec2 dx, vec2 dy) {
    float k = id - 1.0;
    vec2 cell = vec2(mod(k, COLS), floor(k / COLS));
    vec2 base = (cell * SLOT + GUTTER) / SIZE;
    float span = TILE / SIZE;
    return textureGrad(Atlas, base + fract(uv) * span, dx * span, dy * span);
}
#endif
void main() {
#ifdef MODERN
    vec3 dpx = dFdx(wpos);
    vec3 dpy = dFdy(wpos);
    vec3 tp = tpos.xyz * tmat.y;
    vec3 tdx = dFdx(tp);
    vec3 tdy = dFdy(tp);
#endif
    float d = TEX(Dark, (wpos.xz - DarkRect.xy) * DarkRect.zw).a;
    vec3 c = color.rgb;
    float glow = mat.x;
    float flags = floor(mat.y + 0.5);
    float unlit = mod(flags, 2.0);
    float shiny = mod(floor(flags / 2.0), 2.0);
    float water = mod(floor(flags / 4.0), 2.0);
    float soft = mod(floor(flags / 8.0), 2.0);
    float halo = mod(floor(flags / 16.0), 2.0);
    float rim = mod(floor(flags / 32.0), 2.0);
    float alpha = color.a;
    vec3 tl = wpos - vec3(Torch.x, TorchColor.w, Torch.y);
    float r = length(tl.xz) / max(Torch.z, 0.001);
    float t = Torch.w * max(0.0, 1.0 - r * r);
    vec3 n = nrm;
    float nl = length(n);
    vec3 lit;
    if (unlit > 0.5 || nl < 0.3) {
        lit = c * Tint.rgb + c * TorchColor.rgb * t;
        if (halo + rim > 0.5 && nl > 0.3) {
            // a body of light: bright where it is deep (halo) or at its
            // edges (rim, a bubble)
            float f = abs(dot(nrm / nl, normalize(Eye.xyz - wpos)));
            alpha *= halo > 0.5 ? 0.1 + 0.9 * f * f : 0.12 + 0.88 * pow(1.0 - f, 2.0);
        }
    } else if (Eye.w < 0.5) {
        n /= nl;
        float s = dot(n, Sun.xyz);
        vec3 base = c * clamp(0.56 + 0.44 * max(s, -0.3) + 0.08 * max(n.y, 0.0), 0.3, 1.08);
        lit = base * Tint.rgb + base * TorchColor.rgb * t;
    } else {
        n /= nl;
#ifdef MODERN
        // carved: every face is lit flat, as a chisel left it (not in the
        // smooth high-poly look)
        if (soft < 0.5 && Eye.w < 1.5) {
            vec3 f = cross(dpx, dpy);
            if (dot(f, f) > 1e-14) {
                f = normalize(f);
                n = dot(f, n) < 0.0 ? -f : f;
            }
        }
#endif
        vec3 v = normalize(Eye.xyz - wpos);
        float gloss = 0.0;
        if (water > 0.5) {
            float tt = Sun.w;
            vec2 q = wpos.xz;
            n = normalize(n + vec3(
                sin(q.x * 3.1 + tt * 1.3) * 0.07 + sin((q.x + q.y) * 5.3 + tt * 2.1) * 0.04,
                0.0,
                cos(q.y * 3.7 + tt * 1.1) * 0.07 + sin((q.x - q.y) * 4.7 - tt * 1.7) * 0.04));
        } else {
            float paint = 1.0;
#ifdef MODERN
            float id = floor(tmat.x + 0.5);
            if (id > 0.5) {
                // the material, fading out where its tile shrinks too small
                float lod = log2(max(max(length(tdx), length(tdy)) * TILE, 1e-4));
                float near = 1.0 - smoothstep(3.3, 4.3, lod);
                if (near > 0.0) {
                    bool own = tpos.w > -5.0;
                    vec3 ln = own ? turn(n, tpos.w) : n;
                    vec4 s;
                    vec3 pert;
                    if (id > 16.5) {
                        // the ground, seen from above: grass, soil or sand
                        // by its colour
                        vec4 gr = stuff(13.0, tp.xz, tdx.xz, tdy.xz);
                        vec4 so = stuff(14.0, tp.xz * 0.86 + 0.37, tdx.xz * 0.86, tdy.xz * 0.86);
                        vec4 sa = stuff(15.0, tp.xz * 1.1 + 0.61, tdx.xz * 1.1, tdy.xz * 1.1);
                        float lum = dot(c, vec3(0.3, 0.55, 0.15));
                        float wg = clamp((c.g - max(c.r, c.b)) * 9.0, 0.0, 1.0);
                        float ws = clamp((lum - 0.5) * 4.0, 0.0, 1.0) * (1.0 - wg);
                        float wo = max(1.0 - wg - ws, 0.0);
                        s = (gr * wg + so * wo + sa * ws) / max(wg + wo + ws, 0.001);
                        pert = vec3(1.0 - s.g * 2.0, 0.0, 1.0 - s.b * 2.0);
                    } else {
                        vec3 w = pow(abs(ln), vec3(4.0));
                        w /= dot(w, vec3(1.0));
                        vec4 none = vec4(0.5, 0.5, 0.5, 0.0);
                        vec4 sx = w.x > 0.02 ? stuff(id, tp.zy, tdx.zy, tdy.zy) : none;
                        vec4 sy = w.y > 0.02 ? stuff(id, tp.xz, tdx.xz, tdy.xz) : none;
                        vec4 sz = w.z > 0.02 ? stuff(id, tp.xy, tdx.xy, tdy.xy) : none;
                        s = sx * w.x + sy * w.y + sz * w.z;
                        vec2 gx = sx.gb * 2.0 - 1.0;
                        vec2 gy = sy.gb * 2.0 - 1.0;
                        vec2 gz = sz.gb * 2.0 - 1.0;
                        pert = w.x * vec3(0.0, -gx.y, -gx.x)
                            + w.y * vec3(-gy.x, 0.0, -gy.y)
                            + w.z * vec3(-gz.x, -gz.y, 0.0);
                    }
                    if (own) {
                        pert = turn(pert, -tpos.w);
                    }
                    n = normalize(n + pert * 0.85 * near);
                    c *= mix(1.0, 0.42 + 1.16 * s.r, near);
                    gloss = s.a * near;
                    paint = 1.0 - 0.55 * near;
                }
            }
#endif
            // brush strokes painted over everything
            vec3 an = abs(nrm / nl);
            vec2 uv = an.y > 0.6 ? wpos.xz : (an.x > an.z ? wpos.zy : wpos.xy);
            float det = TEX(Detail, uv * 0.7).r * 0.65 + TEX(Detail, uv * 0.19 + vec2(0.31, 0.77)).r * 0.35;
            c *= 1.0 + (0.4 * det - 0.2) * paint;
        }
        if (dot(n, v) < 0.0) {
            n = -n;
        }
        // the sun wraps a little around, warm; the sky fills the shade, cool
        float s = max((dot(n, Sun.xyz) + 0.2) / 1.2, 0.0);
        float hemi = 0.5 + 0.5 * n.y;
        vec3 amb = mix(vec3(0.3, 0.26, 0.22), vec3(0.42, 0.48, 0.6), hemi);
        vec3 light = amb + vec3(0.74, 0.66, 0.5) * s;
        vec3 h = normalize(Sun.xyz + v);
        float sharp = max(max(shiny, water), gloss * 0.6);
        float sp = pow(max(dot(n, h), 0.0), 10.0 + 60.0 * sharp);
        float strength = 0.03 + 0.25 * gloss + 0.5 * shiny * (0.5 + gloss) + 0.8 * water;
        vec3 spec = vec3(1.0, 0.95, 0.85) * sp * strength * s;
        // a cool rim on the edges turned away
        float rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * 0.2;
        spec += c * rim * vec3(0.75, 0.85, 1.0);
        // the torch lights what is turned to it
        t *= 0.3 + 0.7 * max(dot(n, normalize(-tl)), 0.0);
        lit = (c * light + spec) * Tint.rgb + c * TorchColor.rgb * t;
        // painted colours: richer, a little more contrast
        float lum = dot(lit, vec3(0.299, 0.587, 0.114));
        lit = mix(vec3(lum), lit, 1.06);
        lit = (lit - 0.5) * 1.06 + 0.5;
        if (Fog.w > 0.0) {
            float f = clamp((length(wpos - Eye.xyz) - FogRange.x) / max(FogRange.y - FogRange.x, 0.01), 0.0, 1.0);
            lit = mix(lit, Fog.rgb * Tint.rgb, f * f * Fog.w);
        }
    }
    lit = mix(lit, c, glow);
    lit *= 1.0 - d * (1.0 - 0.55 * glow);
    if (d > 0.985) {
        lit = vec3(0.0);
    }
    FRAG_OUT = vec4(lit, alpha);
}"#;

/// The shaders for a GLSL version: 150 (desktop OpenGL 3.2+, with carved
/// faces and materials) or 100 (old cards and the web: smooth faces, paint
/// only).
fn sources(modern: bool) -> (String, String) {
    if modern {
        let consts = format!(
            "#define COLS {:.1}\n#define SLOT {:.1}\n#define GUTTER {:.1}\n#define SIZE {:.1}\n#define TILE {:.1}\n",
            materials::COLS as f32,
            materials::SLOT as f32,
            materials::GUTTER as f32,
            materials::SIZE as f32,
            materials::TILE as f32,
        );
        (
            format!("#version 150\n#define attribute in\n#define varying out\n{VERTEX}"),
            format!(
                "#version 150\n#define varying in\n#define TEX texture\n#define MODERN 1\n{consts}out vec4 frag_out;\n#define FRAG_OUT frag_out\n{FRAGMENT}"
            ),
        )
    } else {
        (
            format!("#version 100\n{VERTEX}"),
            format!(
                "#version 100\n#define TEX texture2D\n#define FRAG_OUT gl_FragColor\n{FRAGMENT}"
            ),
        )
    }
}

/// A tile of brush strokes (grey around the middle), seamless, to paint
/// over the colours of the detailed look.
fn strokes(n: usize) -> Vec<u8> {
    let mut v = vec![0.5f32; n * n];
    let mut seed = 0x2545_F491u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed & 0xFFFF) as f32 / 65535.0
    };
    let at = |x: f32, y: f32| -> usize {
        let (x, y) = (
            x.rem_euclid(n as f32) as usize,
            y.rem_euclid(n as f32) as usize,
        );
        y * n + x
    };
    // soft blotches
    for _ in 0..70 {
        let (cx, cy, r, k) = (
            rnd() * n as f32,
            rnd() * n as f32,
            6.0 + rnd() * 18.0,
            (rnd() - 0.5) * 0.16,
        );
        let ri = r as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
                if d < 1.0 {
                    v[at(cx + dx as f32, cy + dy as f32)] += k * (1.0 - d * d);
                }
            }
        }
    }
    // short strokes of a brush
    for _ in 0..(n * n / 22) {
        let (x, y) = (rnd() * n as f32, rnd() * n as f32);
        let a = rnd() * std::f32::consts::TAU;
        let len = 3.0 + rnd() * 7.0;
        let k = (rnd() - 0.5) * 0.34;
        let w = if rnd() < 0.3 { 1 } else { 0 };
        for i in 0..len as i32 {
            let (px, py) = (x + a.cos() * i as f32, y + a.sin() * i as f32);
            let fade = 1.0 - i as f32 / len * 0.6;
            for o in 0..=w {
                let j = at(px - a.sin() * o as f32, py + a.cos() * o as f32);
                v[j] += k * fade;
            }
        }
    }
    // blur once, so the strokes are paint and not pixels
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let mut sum = 0.0;
            for (dx, dy, w) in [
                (0, 0, 4.0),
                (1, 0, 2.0),
                (-1, 0, 2.0),
                (0, 1, 2.0),
                (0, -1, 2.0),
                (1, 1, 1.0),
                (-1, 1, 1.0),
                (1, -1, 1.0),
                (-1, -1, 1.0),
            ] {
                sum += v[at(x as f32 + dx as f32, y as f32 + dy as f32)] * w;
            }
            let g = ((sum / 16.0).clamp(0.0, 1.0) * 255.0) as u8;
            let k = (y * n + x) * 4;
            out[k..k + 4].copy_from_slice(&[g, g, g, 255]);
        }
    }
    out
}

/// The light of a frame, as the shader takes it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub mvp: Mat4,
    /// the corner of the map of darkness and 1 / its size, in tiles
    pub dark_rect: Vec4,
    /// day, dusk, night
    pub tint: Vec4,
    /// the hero's x, z, the torch's radius and strength
    pub torch: Vec4,
    /// its colour, and the height of the flame
    pub torch_color: Vec4,
    /// the camera, and the look (0 low-poly, 1 detailed, 2 high-poly)
    pub eye: Vec4,
    /// toward the sun, and the time (for wind and water)
    pub sun: Vec4,
    /// the colour of the haze and its strength
    pub fog: Vec4,
    /// where the haze begins and where it is thickest
    pub fog_range: Vec4,
}

fn meta() -> ShaderMeta {
    let f4 = |n: &str| UniformDesc::new(n, UniformType::Float4);
    ShaderMeta {
        images: vec![
            "Dark".to_string(),
            "Detail".to_string(),
            "Atlas".to_string(),
        ],
        uniforms: UniformBlockLayout {
            uniforms: vec![
                UniformDesc::new("Mvp", UniformType::Mat4),
                f4("DarkRect"),
                f4("Tint"),
                f4("Torch"),
                f4("TorchColor"),
                f4("Eye"),
                f4("Sun"),
                f4("Fog"),
                f4("FogRange"),
            ],
        },
    }
}

/// A batch put into the video card's memory.
pub struct GpuMesh {
    vb: BufferId,
    ib: BufferId,
    n: i32,
}

impl GpuMesh {
    pub fn new(b: &Batch) -> Option<GpuMesh> {
        if b.is_empty() {
            return None;
        }
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let vb = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&b.v),
        );
        let ib = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&b.i),
        );
        Some(GpuMesh {
            vb,
            ib,
            n: b.i.len() as i32,
        })
    }
}

impl Drop for GpuMesh {
    fn drop(&mut self) {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        ctx.delete_buffer(self.vb);
        ctx.delete_buffer(self.ib);
    }
}

/// The buffers of what changes every frame.
struct Stream {
    vb: BufferId,
    ib: BufferId,
    nv: usize,
    ni: usize,
}

/// Which stream buffer a batch of the frame goes through (each drawn batch
/// keeps its own, so the card never waits on one still in use).
#[derive(Clone, Copy)]
pub enum Slot {
    Creatures,
    Things,
    Light,
}

pub struct Gpu {
    pip: Pipeline,
    /// light added to the picture (spells, glows): tested against the depth
    /// of what stands, but not hiding one another
    glow: Pipeline,
    streams: [Option<Stream>; 3],
    detail: miniquad::TextureId,
    atlas: miniquad::TextureId,
}

/// Lets a texture be read sharply at a slant (where the card can).
// (off Apple platforms a texture is only ever an OpenGL one)
#[allow(irrefutable_let_patterns)]
fn anisotropic(ctx: &mut dyn miniquad::RenderingBackend, t: miniquad::TextureId) {
    use miniquad::native::gl::*;
    const MAX_ANISOTROPY: GLenum = 0x84FE;
    const BINDING_2D: GLenum = 0x8069;
    if let miniquad::RawId::OpenGl(id) = unsafe { ctx.texture_raw_id(t) } {
        unsafe {
            let mut old = 0;
            glGetIntegerv(BINDING_2D, &mut old);
            glBindTexture(GL_TEXTURE_2D, id);
            glTexParameterf(GL_TEXTURE_2D, MAX_ANISOTROPY, 8.0);
            // a card without it only says so
            while glGetError() != GL_NO_ERROR {}
            glBindTexture(GL_TEXTURE_2D, old as GLuint);
        }
    }
}

impl Gpu {
    pub fn new() -> Gpu {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let glsl = ctx.info().glsl_support;
        let compile = |ctx: &mut dyn miniquad::RenderingBackend, modern: bool| {
            let (vertex, fragment) = sources(modern);
            ctx.new_shader(
                miniquad::ShaderSource::Glsl {
                    vertex: &vertex,
                    fragment: &fragment,
                },
                meta(),
            )
        };
        let shader = match compile(ctx, glsl.v150 || glsl.v330) {
            Ok(s) => s,
            Err(_) => compile(ctx, false).expect("3d shader"),
        };
        let n = 256;
        let detail = ctx.new_texture_from_data_and_format(
            &strokes(n),
            miniquad::TextureParams {
                width: n as u32,
                height: n as u32,
                wrap: miniquad::TextureWrap::Repeat,
                mipmap_filter: miniquad::MipmapFilterMode::Linear,
                allocate_mipmaps: true,
                ..Default::default()
            },
        );
        ctx.texture_generate_mipmaps(detail);
        let size = materials::SIZE as u32;
        let atlas = ctx.new_texture_from_data_and_format(
            &materials::atlas(),
            miniquad::TextureParams {
                width: size,
                height: size,
                wrap: miniquad::TextureWrap::Clamp,
                mipmap_filter: miniquad::MipmapFilterMode::Linear,
                allocate_mipmaps: true,
                ..Default::default()
            },
        );
        ctx.texture_generate_mipmaps(atlas);
        anisotropic(ctx, atlas);
        anisotropic(ctx, detail);
        let attrs = [
            VertexAttribute::new("position", VertexFormat::Float3),
            VertexAttribute::new("color0", VertexFormat::Byte4),
            VertexAttribute::new("normal", VertexFormat::Byte4),
            VertexAttribute::new("extra", VertexFormat::Byte4),
            VertexAttribute::new("local", VertexFormat::Short4),
        ];
        let pip = ctx.new_pipeline(
            &[BufferLayout::default()],
            &attrs,
            shader,
            PipelineParams {
                depth_test: Comparison::LessOrEqual,
                depth_write: true,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );
        let glow = ctx.new_pipeline(
            &[BufferLayout::default()],
            &attrs,
            shader,
            PipelineParams {
                depth_test: Comparison::LessOrEqual,
                // (miniquad tests the depth only for pipelines that write
                // it; the writes are masked off while drawing, see `draw`)
                depth_write: true,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::One,
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                ..Default::default()
            },
        );
        Gpu {
            pip,
            glow,
            streams: [None, None, None],
            detail,
            atlas,
        }
    }

    /// Adds a batch of light to the picture: behind what stands it is hidden,
    /// but light never hides light.
    pub fn draw_light(&mut self, light: &Light, dark: &Texture2D, extra: &Batch) {
        self.run(true, light, dark, &[], extra, Slot::Light);
    }

    /// Draws meshes kept in the video card and a batch of this frame.
    pub fn draw(
        &mut self,
        light: &Light,
        dark: &Texture2D,
        meshes: &[&GpuMesh],
        extra: &Batch,
        slot: Slot,
    ) {
        self.run(false, light, dark, meshes, extra, slot);
    }

    fn run(
        &mut self,
        glow: bool,
        light: &Light,
        dark: &Texture2D,
        meshes: &[&GpuMesh],
        extra: &Batch,
        slot: Slot,
    ) {
        if meshes.is_empty() && extra.is_empty() {
            return;
        }
        let mut gl = unsafe { get_internal_gl() };
        // what macroquad gathered so far goes first
        gl.flush();
        let ctx = gl.quad_context;
        let img = dark.raw_miniquad_id();
        let (detail, atlas) = (self.detail, self.atlas);
        if !extra.is_empty() {
            let (nv, ni) = (extra.v.len(), extra.i.len());
            let stream = &mut self.streams[slot as usize];
            let fits = stream.as_ref().is_some_and(|s| s.nv >= nv && s.ni >= ni);
            if !fits {
                if let Some(s) = stream.take() {
                    ctx.delete_buffer(s.vb);
                    ctx.delete_buffer(s.ib);
                }
                let (nv, ni) = (nv.next_power_of_two(), ni.next_power_of_two());
                let vb = ctx.new_buffer(
                    BufferType::VertexBuffer,
                    BufferUsage::Stream,
                    BufferSource::empty::<super::lowpoly::V>(nv),
                );
                let ib = ctx.new_buffer(
                    BufferType::IndexBuffer,
                    BufferUsage::Stream,
                    BufferSource::empty::<u32>(ni),
                );
                *stream = Some(Stream { vb, ib, nv, ni });
            }
            let s = stream.as_ref().unwrap();
            ctx.buffer_update(s.vb, BufferSource::slice(&extra.v));
            ctx.buffer_update(s.ib, BufferSource::slice(&extra.i));
        }
        ctx.begin_default_pass(PassAction::Nothing);
        ctx.apply_pipeline(if glow { &self.glow } else { &self.pip });
        if glow {
            unsafe { miniquad::native::gl::glDepthMask(0) };
        }
        let mut first = true;
        let mut one =
            |ctx: &mut dyn miniquad::RenderingBackend, vb: BufferId, ib: BufferId, n: i32| {
                ctx.apply_bindings(&miniquad::Bindings {
                    vertex_buffers: vec![vb],
                    index_buffer: ib,
                    images: vec![img, detail, atlas],
                });
                if first {
                    ctx.apply_uniforms(UniformsSource::table(light));
                    first = false;
                }
                ctx.draw(0, n, 1);
            };
        for m in meshes {
            one(ctx, m.vb, m.ib, m.n);
        }
        if let (false, Some(s)) = (extra.is_empty(), self.streams[slot as usize].as_ref()) {
            one(ctx, s.vb, s.ib, extra.i.len() as i32);
        }
        if glow {
            unsafe { miniquad::native::gl::glDepthMask(1) };
        }
        ctx.end_render_pass();
    }
}
