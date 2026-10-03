//! Non-destructive framing: GPU preview, full-resolution worker export.
use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub gradient: bool,
    pub preset: usize,
    pub padding: u32,
    pub inner_radius: u32,
    pub outer_radius: u32,
    pub shadow: u32,
}
pub const PRESETS: [(&str, u32, u32); 8] = [
    ("Teal", 0x32b49b, 0x147d91),
    ("Ocean", 0x63b9ff, 0x344ac7),
    ("Lavender", 0xc9acff, 0x7560ce),
    ("Sunset", 0xffc393, 0xea6684),
    ("Rose", 0xf8a8c0, 0xc9619a),
    ("Cream", 0xf3eee4, 0xd5c9b3),
    ("Slate", 0x57677b, 0x232c3d),
    ("White", 0xffffff, 0xdce4ed),
];
impl Default for Backdrop {
    fn default() -> Self {
        Self {
            gradient: false,
            preset: 0,
            padding: 64,
            inner_radius: 18,
            outer_radius: 0,
            shadow: 24,
        }
    }
}
impl Backdrop {
    pub fn dimensions(self, source: (u32, u32)) -> (u32, u32) {
        (source.0 + self.padding * 2, source.1 + self.padding * 2)
    }
    pub fn background(self) -> gpui::Background {
        let (_, from, to) = PRESETS[self.preset];
        if self.gradient {
            gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(gpui::rgb(from), 0.),
                gpui::linear_color_stop(gpui::rgb(to), 1.),
            )
        } else {
            gpui::solid_background(gpui::rgb(from))
        }
    }
    pub fn apply(self, source: &RgbaImage) -> RgbaImage {
        let (w, h) = self.dimensions(source.dimensions());
        let mut out = RgbaImage::new(w, h);
        let (_, from, to) = PRESETS[self.preset];
        let rgb = |hex: u32| [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
        let from = rgb(from);
        let to = rgb(to);
        let pad = self.padding as f32;
        let sw = source.width() as f32;
        let sh = source.height() as f32;
        let outer_radius = self.outer_radius as f32;
        let inner_radius = self.inner_radius as f32;
        for y in 0..h {
            let t = if self.gradient {
                (y as f32 + 0.5) / h as f32
            } else {
                0.
            };
            let color: [f32; 3] =
                std::array::from_fn(|c| from[c] as f32 * (1. - t) + to[c] as f32 * t);
            for x in 0..w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let outer = coverage(distance(px, py, w as f32, h as f32, outer_radius));
                if outer == 0. {
                    continue;
                }
                let mut color = color;
                if self.shadow > 0 {
                    let blur = self.shadow as f32;
                    let d =
                        distance(px - pad, py - pad - blur * 0.25, sw, sh, inner_radius).max(0.);
                    // Soft rounded-rectangle falloff, independent of screenshot alpha.
                    let opacity = 0.22 * (-2. * (d / (blur * 0.6)).powi(2)).exp();
                    for c in &mut color {
                        *c *= 1. - opacity;
                    }
                }
                if x >= self.padding
                    && y >= self.padding
                    && x < self.padding + source.width()
                    && y < self.padding + source.height()
                {
                    let src = source.get_pixel(x - self.padding, y - self.padding);
                    let alpha = src[3] as f32 / 255.
                        * coverage(distance(px - pad, py - pad, sw, sh, inner_radius));
                    for c in 0..3 {
                        color[c] = color[c] * (1. - alpha) + src[c] as f32 * alpha;
                    }
                }
                out.put_pixel(
                    x,
                    y,
                    Rgba([
                        color[0].round() as u8,
                        color[1].round() as u8,
                        color[2].round() as u8,
                        (outer * 255.).round() as u8,
                    ]),
                );
            }
        }
        out
    }
}
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0., 1.)
}
fn distance(x: f32, y: f32, w: f32, h: f32, radius: f32) -> f32 {
    let radius = radius.min(w * 0.5).min(h * 0.5);
    let qx = (x - w * 0.5).abs() - w * 0.5 + radius;
    let qy = (y - h * 0.5).abs() - h * 0.5 + radius;
    qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - radius
}

#[derive(Clone, Copy, PartialEq)]
pub enum Control {
    Padding,
    InnerRadius,
    OuterRadius,
    Shadow,
}
impl Control {
    pub fn label(self) -> &'static str {
        match self {
            Self::Padding => "Padding",
            Self::InnerRadius => "Image corners",
            Self::OuterRadius => "Backdrop corners",
            Self::Shadow => "Shadow",
        }
    }
    pub fn max(self) -> u32 {
        match self {
            Self::Padding => 200,
            Self::Shadow => 60,
            _ => 80,
        }
    }
    pub fn value(self, b: Backdrop) -> u32 {
        match self {
            Self::Padding => b.padding,
            Self::InnerRadius => b.inner_radius,
            Self::OuterRadius => b.outer_radius,
            Self::Shadow => b.shadow,
        }
    }
    pub fn set(self, b: &mut Backdrop, value: u32) {
        match self {
            Self::Padding => b.padding = value,
            Self::InnerRadius => b.inner_radius = value,
            Self::OuterRadius => b.outer_radius = value,
            Self::Shadow => b.shadow = value,
        }
    }
}
/// Mask everything outside the rounded output, including shadows and live marks.
pub fn clip_output_corners(
    bounds: gpui::Bounds<gpui::Pixels>,
    radius: f32,
    window: &mut gpui::Window,
) {
    use gpui::{PathBuilder, point, px, rgb};
    let r = radius
        .min(f32::from(bounds.size.width) * 0.5)
        .min(f32::from(bounds.size.height) * 0.5);
    if r <= 0. {
        return;
    }
    for (origin, sx, sy) in [
        (bounds.origin, 1., 1.),
        (point(bounds.right(), bounds.top()), -1., 1.),
        (point(bounds.left(), bounds.bottom()), 1., -1.),
        (point(bounds.right(), bounds.bottom()), -1., -1.),
    ] {
        let map = |x, y| origin + point(px(x * sx), px(y * sy));
        let mut path = PathBuilder::fill();
        path.move_to(origin);
        path.line_to(map(r, 0.));
        for i in 1..=24 {
            let angle = -std::f32::consts::FRAC_PI_2 * (1. + i as f32 / 24.);
            path.line_to(map(r + r * angle.cos(), r + r * angle.sin()));
        }
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, rgb(0xeff0f4));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_padding_preserves_source_and_dimensions() {
        let src = RgbaImage::from_pixel(12, 8, Rgba([255, 0, 0, 255]));
        let b = Backdrop {
            padding: 4,
            inner_radius: 0,
            shadow: 0,
            ..Default::default()
        };
        let out = b.apply(&src);
        assert_eq!(out.dimensions(), (20, 16));
        assert_eq!(out.get_pixel(0, 0), &Rgba([50, 180, 155, 255]));
        assert_eq!(out.get_pixel(4, 4), src.get_pixel(0, 0));
        assert_eq!(out.get_pixel(15, 11), src.get_pixel(11, 7));
        assert_eq!(src.dimensions(), (12, 8));
    }
    #[test]
    fn gradient_rounding_and_shadow_survive_export() {
        let src = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
        let b = Backdrop {
            gradient: true,
            padding: 10,
            outer_radius: 8,
            inner_radius: 8,
            shadow: 8,
            ..Default::default()
        };
        let out = b.apply(&src);
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        assert_ne!(out.get_pixel(20, 0), out.get_pixel(20, 39));
        assert_ne!(out.get_pixel(10, 10), src.get_pixel(0, 0));
        assert_eq!(out.get_pixel(20, 20), src.get_pixel(10, 10));
        let no_shadow = Backdrop { shadow: 0, ..b }.apply(&src);
        assert!(out.get_pixel(20, 32)[0] < no_shadow.get_pixel(20, 32)[0]);
        let mut png = std::io::Cursor::new(Vec::new());
        out.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let decoded = image::load_from_memory(png.get_ref()).unwrap().to_rgba8();
        assert_eq!(decoded, out);
    }
    #[test]
    fn transparent_source_and_zero_padding_are_composited_safely() {
        let src = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 0]));
        let b = Backdrop {
            padding: 0,
            inner_radius: 0,
            outer_radius: 0,
            shadow: 0,
            ..Default::default()
        };
        let out = b.apply(&src);
        assert_eq!(out.dimensions(), (4, 4));
        assert!(out.pixels().all(|p| *p == Rgba([50, 180, 155, 255])));
    }
}
