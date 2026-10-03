//! Looping backdrops. Shader effects share a Metal renderer; foreground stays cached.
use crate::backdrop::{Backdrop, PRESETS};
use gpui::{
    Bounds, Pixels, Rgba, Window, linear_color_stop, linear_gradient, point, px, quad, rgb, size,
};
use image::{Pixel, RgbaImage};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    #[default]
    Still,
    Flow,
    Lava,
    Stars,
    Paint,
    Liquid,
    Aurora,
    Contours,
    Prism,
}
impl Motion {
    pub const EFFECTS: [Self; 8] = [
        Self::Flow,
        Self::Stars,
        Self::Aurora,
        Self::Contours,
        Self::Paint,
        Self::Prism,
        Self::Liquid,
        Self::Lava,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Still => "Still",
            Self::Flow => "Flow",
            Self::Lava => "Lava",
            Self::Stars => "Starfield",
            Self::Paint => "Painterly",
            Self::Liquid => "Liquid",
            Self::Aurora => "Aurora",
            Self::Contours => "Contours",
            Self::Prism => "Prism",
        }
    }
    pub fn uses_shader(self) -> bool {
        !matches!(self, Self::Still | Self::Flow | Self::Stars)
    }
    pub fn suggested_preset(self) -> Option<usize> {
        match self {
            Self::Liquid => Some(1),
            Self::Lava | Self::Paint => Some(3),
            Self::Aurora => Some(0),
            Self::Contours => Some(6),
            Self::Prism => Some(2),
            _ => None,
        }
    }
}
#[derive(Clone, Copy)]
struct Disc {
    x: f32,
    y: f32,
    r: f32,
    color: [u8; 3],
    alpha: f32,
    soft: bool,
}
struct Scene {
    top: [u8; 3],
    bottom: [u8; 3],
    discs: Vec<Disc>,
}
fn color(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 * (1. - t) + b[i] as f32 * t).round() as u8)
}
fn hex(c: [u8; 3]) -> u32 {
    (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}
fn random(i: usize) -> f32 {
    let mut x = (i as u32).wrapping_mul(747796405).wrapping_add(2891336453);
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277803737);
    ((x >> 22) ^ x) as f32 / u32::MAX as f32
}
fn scene(b: Backdrop, phase: f32) -> Scene {
    let p = phase.rem_euclid(1.) * TAU;
    let (_, a, z) = PRESETS[b.preset];
    let a = color(a);
    let z = color(z);
    let cream = [255, 234, 211];
    let accent = mix(a, cream, 0.55);
    let mut s = Scene {
        top: a,
        bottom: if b.gradient { z } else { a },
        discs: vec![],
    };
    match b.motion {
        Motion::Still
        | Motion::Liquid
        | Motion::Lava
        | Motion::Paint
        | Motion::Aurora
        | Motion::Contours
        | Motion::Prism => {}
        Motion::Flow => {
            let wave = p.sin() * 0.5 + 0.5;
            s.top = mix(a, z, wave * 0.6);
            s.bottom = mix(z, accent, (p + 1.8).sin() * 0.25 + 0.3);
            for i in 0..9 {
                let q = random(i * 7 + 1) * TAU;
                let speed = if i % 3 == 0 { 2. } else { 1. };
                let x = 0.5 + 0.51 * (p * speed + q).sin();
                let y = 0.5 + 0.51 * (p + q * 1.7).cos();
                let r = 0.16 + random(i * 7 + 2) * 0.24;
                let c = [a, z, accent, cream][i % 4];
                s.discs.push(Disc {
                    x,
                    y,
                    r,
                    color: c,
                    alpha: 0.55,
                    soft: true,
                });
            }
        }
        Motion::Stars => {
            s.top = [7, 12, 29];
            s.bottom = [19, 29, 55];
            for i in 0..5 {
                s.discs.push(Disc {
                    x: 0.5 + 0.5 * (p + random(i) * TAU).sin(),
                    y: random(i + 18),
                    r: 0.3,
                    color: if i % 2 == 0 { a } else { z },
                    alpha: 0.2,
                    soft: true,
                });
            }
            for i in 0..110 {
                let x = random(i * 7 + 200);
                let y = (random(i * 7 + 201) + phase).rem_euclid(1.);
                let edge = (y * 20.).min((1. - y) * 20.).clamp(0., 1.);
                let alpha =
                    (0.45 + 0.4 * (p * (1. + (i % 3) as f32) + random(i + 70) * TAU).sin()) * edge;
                s.discs.push(Disc {
                    x: x + 0.012 * (p + random(i + 50) * TAU).sin(),
                    y,
                    r: 0.0012 + random(i * 7 + 202) * 0.0023,
                    color: [231, 241, 255],
                    alpha,
                    soft: false,
                });
            }
        }
    }
    s
}
// Match GPUI 0.2.2's four-sample Gaussian shadow integration (Apache-2.0).
// Cache normalized coverage so exports do no per-pixel exponentials.
fn soft_coverage(x: f32, y: f32) -> f32 {
    const N: usize = 256;
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let x = x.abs();
    let y = y.abs();
    if x >= 1. || y >= 1. {
        return 0.;
    }
    let table = TABLE.get_or_init(|| {
        let erf = |v: f32| {
            let a = v.abs();
            let r = 1. + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
            v.signum() * (1. - 1. / r.powi(4))
        };
        let sigma = 0.18;
        let core = 0.45;
        let mut values = vec![0.; N * N];
        for yi in 0..N {
            for xi in 0..N {
                let x = xi as f32 / (N - 1) as f32;
                let y = yi as f32 / (N - 1) as f32;
                let low = y - core;
                let high = y + core;
                let start = (-3_f32 * sigma).clamp(low, high);
                let end = (3_f32 * sigma).clamp(low, high);
                let step = (end - start) / 4.;
                let mut alpha = 0.;
                for i in 0..4 {
                    let sample = start + step * (i as f32 + 0.5);
                    let delta = -(y - sample).abs();
                    let curved = (core * core - delta * delta).max(0.).sqrt();
                    let integral = 0.5
                        * (erf((x + curved) * std::f32::consts::FRAC_1_SQRT_2 / sigma)
                            - erf((x - curved) * std::f32::consts::FRAC_1_SQRT_2 / sigma));
                    let gaussian = (-(sample * sample) / (2. * sigma * sigma)).exp()
                        / ((2. * std::f32::consts::PI).sqrt() * sigma);
                    alpha += integral * gaussian * step;
                }
                values[yi * N + xi] = alpha.clamp(0., 1.);
            }
        }
        values
    });
    let x = x * (N - 1) as f32;
    let y = y * (N - 1) as f32;
    let xi = (x as usize).min(N - 2);
    let yi = (y as usize).min(N - 2);
    let tx = x - xi as f32;
    let ty = y - yi as f32;
    let top = table[yi * N + xi] * (1. - tx) + table[yi * N + xi + 1] * tx;
    let bottom = table[(yi + 1) * N + xi] * (1. - tx) + table[(yi + 1) * N + xi + 1] * tx;
    top * (1. - ty) + bottom * ty
}
/// One preview image per editor. Retire atlas entries before replacing them,
/// and reuse paused frames so preview memory cannot grow with playback time.
#[derive(Default)]
pub struct Preview {
    image: Option<std::sync::Arc<gpui::RenderImage>>,
    key: Option<(u32, u32, usize, u32, u32, Motion)>,
}
impl Preview {
    pub fn clear(&mut self, window: &mut Window) {
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
        self.key = None;
    }
    fn paint(
        &mut self,
        b: Backdrop,
        phase: f32,
        bounds: Bounds<Pixels>,
        radius: Pixels,
        window: &mut Window,
    ) {
        let w = f32::from(bounds.size.width) * window.scale_factor();
        let h = f32::from(bounds.size.height) * window.scale_factor();
        let scale = (960. / w.max(h)).min(1.);
        let w = (w * scale).round().max(1.) as u32;
        let h = (h * scale).round().max(1.) as u32;
        let frames = b.seconds.max(2) * 30;
        let tick = (phase.rem_euclid(1.) * frames as f32).floor() as u32;
        let key = (w, h, b.preset, frames, tick, b.motion);
        if self.key != Some(key) {
            self.clear(window);
            self.image = Some(crate::render_image(crate::motion_shader::frame(
                w,
                h,
                b.preset,
                b.motion,
                tick as f32 / frames as f32,
            )));
            self.key = Some(key);
        }
        if let Some(image) = &self.image {
            let _ = window.paint_image(bounds, radius.into(), image.clone(), 0, false);
        }
    }
}
pub fn paint(
    b: Backdrop,
    phase: f32,
    bounds: Bounds<Pixels>,
    radius: Pixels,
    preview: &mut Preview,
    window: &mut Window,
) {
    if b.motion.uses_shader() {
        preview.paint(b, phase, bounds, radius, window);
        return;
    }
    let s = scene(b, phase);
    window.paint_quad(quad(
        bounds,
        px(0.),
        linear_gradient(
            180.,
            linear_color_stop(rgb(hex(s.top)), 0.),
            linear_color_stop(rgb(hex(s.bottom)), 1.),
        ),
        px(0.),
        rgb(0),
        Default::default(),
    ));
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let unit = w.min(h);
    for d in s.discs {
        let r = d.r * unit;
        let c = rgb(hex(d.color));
        if d.soft {
            // GPUI's Metal shadow shader supplies continuous Gaussian blur.
            let core = r * 0.45;
            window.paint_shadows(
                Bounds::new(
                    bounds.origin + point(px(d.x * w - core), px(d.y * h - core)),
                    size(px(core * 2.), px(core * 2.)),
                ),
                px(core).into(),
                &[gpui::BoxShadow {
                    color: Rgba { a: d.alpha, ..c }.into(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(r * 0.18),
                    spread_radius: px(0.),
                }],
            );
            continue;
        }
        window.paint_quad(quad(
            Bounds::new(
                bounds.origin + point(px(d.x * w - r), px(d.y * h - r)),
                size(px(r * 2.), px(r * 2.)),
            ),
            px(r),
            Rgba { a: d.alpha, ..c },
            px(0.),
            c,
            Default::default(),
        ));
    }
}

/// A constant foreground/shadow layer, reused by every exported frame.
pub struct Renderer {
    pub width: u32,
    pub height: u32,
    b: Backdrop,
    foreground: RgbaImage,
    outer: Vec<u8>,
}
impl Renderer {
    pub fn new(source: &RgbaImage, mut b: Backdrop, max_edge: Option<u32>) -> Self {
        let (w, h) = b.dimensions(source.dimensions());
        let scale = max_edge.map_or(1., |cap| (cap as f32 / w.max(h) as f32).min(1.));
        let source = if scale < 1. {
            std::borrow::Cow::Owned(crate::enhance::resize(
                source,
                (
                    (source.width() as f32 * scale).round().max(1.) as u32,
                    (source.height() as f32 * scale).round().max(1.) as u32,
                ),
                false,
            ))
        } else {
            std::borrow::Cow::Borrowed(source)
        };
        b.padding = (b.padding as f32 * scale).round() as u32;
        b.inner_radius = (b.inner_radius as f32 * scale).round() as u32;
        b.outer_radius = (b.outer_radius as f32 * scale).round() as u32;
        b.shadow = (b.shadow as f32 * scale).round() as u32;
        let (mut w, mut h) = b.dimensions(source.dimensions());
        if let Some(cap) = max_edge {
            w = (w.min(cap) / 2 * 2).max(2);
            h = (h.min(cap) / 2 * 2).max(2);
        }
        let mut foreground = RgbaImage::new(w, h);
        let mut outer = vec![0; w as usize * h as usize];
        let pad = b.padding as f32;
        let sw = source.width() as f32;
        let sh = source.height() as f32;
        for (x, y, p) in foreground.enumerate_pixels_mut() {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            outer[(y * w + x) as usize] = (crate::backdrop::coverage(crate::backdrop::distance(
                px,
                py,
                w as f32,
                h as f32,
                b.outer_radius as f32,
            )) * 255.)
                .round() as u8;
            if b.shadow > 0 {
                let blur = b.shadow as f32;
                let d = crate::backdrop::distance(
                    px - pad,
                    py - pad - blur * 0.25,
                    sw,
                    sh,
                    b.inner_radius as f32,
                )
                .max(0.);
                *p = image::Rgba([
                    0,
                    0,
                    0,
                    (0.22 * (-2. * (d / (blur * 0.6)).powi(2)).exp() * 255.).round() as u8,
                ]);
            }
            if x >= b.padding
                && y >= b.padding
                && x < b.padding + source.width()
                && y < b.padding + source.height()
            {
                let mut src = *source.get_pixel(x - b.padding, y - b.padding);
                src[3] = (src[3] as f32
                    * crate::backdrop::coverage(crate::backdrop::distance(
                        px - pad,
                        py - pad,
                        sw,
                        sh,
                        b.inner_radius as f32,
                    )))
                .round() as u8;
                p.blend(&src);
            }
        }
        Self {
            width: w,
            height: h,
            b,
            foreground,
            outer,
        }
    }
    pub fn frame(&self, phase: f32) -> RgbaImage {
        let s = scene(self.b, phase);
        let mut out = if self.b.motion.uses_shader() {
            crate::motion_shader::frame(
                self.width,
                self.height,
                self.b.preset,
                self.b.motion,
                phase,
            )
        } else {
            RgbaImage::new(self.width, self.height)
        };
        if !self.b.motion.uses_shader() {
            for y in 0..self.height {
                let c = mix(s.top, s.bottom, (y as f32 + 0.5) / self.height as f32);
                for x in 0..self.width {
                    let fg = self.foreground.get_pixel(x, y);
                    out.put_pixel(
                        x,
                        y,
                        if fg[3] == 255 {
                            *fg
                        } else {
                            image::Rgba([c[0], c[1], c[2], 255])
                        },
                    );
                }
            }
        }
        let unit = self.width.min(self.height) as f32;
        for d in s.discs {
            let r = d.r * unit;
            let cx = d.x * self.width as f32;
            let cy = d.y * self.height as f32;
            let x0 = (cx - r - 1.).max(0.) as u32;
            let y0 = (cy - r - 1.).max(0.) as u32;
            let x1 = (cx + r + 1.).max(0.).min(self.width as f32) as u32;
            let y1 = (cy + r + 1.).max(0.).min(self.height as f32) as u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    if self.foreground.get_pixel(x, y)[3] == 255 {
                        continue;
                    }
                    let distance = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy);
                    let coverage = if d.soft {
                        soft_coverage((x as f32 + 0.5 - cx) / r, (y as f32 + 0.5 - cy) / r)
                    } else {
                        (r + 0.5 - distance).clamp(0., 1.)
                    };
                    let alpha = (d.alpha * coverage * 255.).round() as u8;
                    if alpha > 0 {
                        out.get_pixel_mut(x, y)
                            .blend(&image::Rgba([d.color[0], d.color[1], d.color[2], alpha]));
                    }
                }
            }
        }
        for (i, ((_, _, p), fg)) in out
            .enumerate_pixels_mut()
            .zip(self.foreground.pixels())
            .enumerate()
        {
            p.blend(fg);
            p[3] = self.outer[i];
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn liquid_export_blends_transparency_and_clips_outer_corners() {
        let source = RgbaImage::from_pixel(20, 12, image::Rgba([250, 80, 30, 128]));
        let b = Backdrop {
            motion: Motion::Liquid,
            preset: 1,
            padding: 8,
            inner_radius: 0,
            outer_radius: 6,
            shadow: 0,
            ..Default::default()
        };
        let renderer = Renderer::new(&source, b, None);
        let output = renderer.frame(0.37);
        let background =
            crate::motion_shader::frame(renderer.width, renderer.height, 1, Motion::Liquid, 0.37);
        let mut expected = *background.get_pixel(18, 14);
        expected.blend(source.get_pixel(10, 6));
        // Final alpha comes from the outer frame coverage, independently of
        // image::Pixel's floating-point alpha rounding during blending.
        expected[3] = 255;
        assert_eq!(*output.get_pixel(18, 14), expected);
        assert_eq!(output.get_pixel(0, 0)[3], 0);
        assert_eq!(output.get_pixel(18, 14)[3], 255);
        assert_eq!(output.get_pixel(18, 1), background.get_pixel(18, 1));
    }
    #[test]
    fn all_effects_loop_and_foreground_is_unchanged() {
        let source = RgbaImage::from_pixel(80, 50, image::Rgba([20, 40, 60, 255]));
        for motion in Motion::EFFECTS {
            let r = Renderer::new(
                &source,
                Backdrop {
                    motion,
                    padding: 20,
                    inner_radius: 0,
                    shadow: 0,
                    ..Default::default()
                },
                None,
            );
            let a = r.frame(0.);
            let b = r.frame(0.37);
            let end = r.frame(1.);
            assert_eq!(a, end, "{} must loop", motion.label());
            assert_ne!(a, b);
            for y in 20..70 {
                for x in 20..100 {
                    assert_eq!(a.get_pixel(x, y), b.get_pixel(x, y));
                    assert_eq!(b.get_pixel(x, y), source.get_pixel(x - 20, y - 20));
                }
            }
        }
    }
    #[test]
    fn video_dimensions_are_even_and_bounded() {
        let source = RgbaImage::new(3001, 1733);
        let r = Renderer::new(&source, Backdrop::default(), Some(1920));
        assert!(r.width <= 1920 && r.height <= 1920);
        assert_eq!(r.width % 2, 0);
        assert_eq!(r.height % 2, 0);
    }
}

#[cfg(test)]
mod continuity_tests {
    use super::*;
    #[test]
    fn loop_seam_is_a_normal_animation_step() {
        let source = RgbaImage::from_pixel(40, 30, image::Rgba([80, 120, 160, 255]));
        for effect in Motion::EFFECTS {
            let b = Backdrop {
                motion: effect,
                padding: 20,
                inner_radius: 0,
                ..Default::default()
            };
            let renderer = Renderer::new(&source, b, None);
            let frames: Vec<_> = (0..20).map(|i| renderer.frame(i as f32 / 20.)).collect();
            let distance = |a: &RgbaImage, b: &RgbaImage| {
                a.as_raw()
                    .iter()
                    .zip(b.as_raw())
                    .map(|(a, b)| (*a as f32 - *b as f32).abs())
                    .sum::<f32>()
                    / a.as_raw().len() as f32
            };
            let max_step = frames
                .windows(2)
                .map(|f| distance(&f[0], &f[1]))
                .fold(0., f32::max);
            let seam = distance(frames.last().unwrap(), &frames[0]);
            assert!(
                seam <= max_step * 1.5 + 0.1,
                "{} seam={seam} max_step={max_step}",
                effect.label()
            );
            assert_eq!(renderer.frame(0.), renderer.frame(1.));
        }
    }
}
