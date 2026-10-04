//! Non-destructive framing and edge padding: GPU preview, full-resolution worker export.
use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Backdrop {
    pub format: Format,
    pub gradient: bool,
    pub motion: crate::animation::Motion,
    pub seconds: u32,
    pub preset: usize,
    /// Deterministic motion variation; zero preserves the original composition.
    pub seed: u32,
    /// Minimum backdrop margin, outside the expanded screenshot.
    pub padding: u32,
    pub inner_radius: u32,
    /// Nearest-edge pixel extension, inside the screenshot corners and shadow.
    pub inside_padding: u32,
    pub shadow: u32,
}
/// Output shape; platform presets retain their identity even when ratios coincide.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    #[default]
    Auto,
    Square,
    Classic,
    Photo,
    Widescreen,
    Portrait,
    Vertical,
    Youtube,
    Shorts,
    Pinterest,
}
impl Format {
    pub const ALL: [Self; 10] = [
        Self::Auto,
        Self::Square,
        Self::Classic,
        Self::Photo,
        Self::Widescreen,
        Self::Portrait,
        Self::Vertical,
        Self::Youtube,
        Self::Shorts,
        Self::Pinterest,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Square => "Square · 1:1",
            Self::Classic => "Classic · 4:3",
            Self::Photo => "Photo · 3:2",
            Self::Widescreen => "Widescreen · 16:9",
            Self::Portrait => "Portrait · 4:5",
            Self::Vertical => "Vertical · 9:16",
            Self::Youtube => "YouTube thumbnail · 16:9",
            Self::Shorts => "YouTube Shorts · 9:16",
            Self::Pinterest => "Pinterest pin · 2:3",
        }
    }
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Youtube => "YouTube · 16:9",
            Self::Shorts => "Shorts · 9:16",
            Self::Pinterest => "Pinterest · 2:3",
            _ => self.label(),
        }
    }
    pub fn ratio(self) -> Option<(u32, u32)> {
        match self {
            Self::Auto => None,
            Self::Square => Some((1, 1)),
            Self::Classic => Some((4, 3)),
            Self::Photo => Some((3, 2)),
            Self::Widescreen | Self::Youtube => Some((16, 9)),
            Self::Portrait => Some((4, 5)),
            Self::Vertical | Self::Shorts => Some((9, 16)),
            Self::Pinterest => Some((2, 3)),
        }
    }
    /// Video encoders require even dimensions. Fixed formats keep their exact ratio.
    pub fn video_dimensions(self, dimensions: (u32, u32), cap: u32) -> (u32, u32) {
        if let Some((rw, rh)) = self.ratio() {
            let unit = (dimensions.0 / rw).min(cap / rw).min(cap / rh);
            let unit = (unit / 2 * 2).max(2);
            (unit * rw, unit * rh)
        } else {
            let scale = (cap as f32 / dimensions.0.max(dimensions.1) as f32).min(1.);
            let even = |v: u32| ((v as f32 * scale).floor() as u32 / 2 * 2).max(2);
            (even(dimensions.0), even(dimensions.1))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub dimensions: (u32, u32),
    pub origin: (u32, u32),
}
impl Frame {
    pub fn centered(dimensions: (u32, u32), source: (u32, u32)) -> Self {
        Self {
            dimensions,
            origin: ((dimensions.0 - source.0) / 2, (dimensions.1 - source.1) / 2),
        }
    }
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
            format: Format::Auto,
            gradient: false,
            motion: crate::animation::Motion::Still,
            seconds: 5,
            preset: 0,
            seed: 0,
            padding: 64,
            inner_radius: 18,
            inside_padding: 0,
            shadow: 24,
        }
    }
}
impl Backdrop {
    pub fn layout(self, source: (u32, u32)) -> Frame {
        let image = self.image_dimensions(source);
        let padded = (image.0 + self.padding * 2, image.1 + self.padding * 2);
        let dimensions = if let Some((rw, rh)) = self.format.ratio() {
            let unit = padded.0.div_ceil(rw).max(padded.1.div_ceil(rh));
            (unit * rw, unit * rh)
        } else {
            padded
        };
        Frame::centered(dimensions, source)
    }
    pub fn image_dimensions(self, source: (u32, u32)) -> (u32, u32) {
        (
            source.0 + self.inside_padding * 2,
            source.1 + self.inside_padding * 2,
        )
    }
    /// Repeat the nearest edge pixel, including alpha, without changing the source.
    pub fn extend_edges(self, source: &RgbaImage) -> std::borrow::Cow<'_, RgbaImage> {
        if self.inside_padding == 0 || source.width() == 0 || source.height() == 0 {
            return std::borrow::Cow::Borrowed(source);
        }
        let (w, h) = self.image_dimensions(source.dimensions());
        std::borrow::Cow::Owned(RgbaImage::from_fn(w, h, |x, y| {
            *source.get_pixel(
                x.saturating_sub(self.inside_padding)
                    .min(source.width() - 1),
                y.saturating_sub(self.inside_padding)
                    .min(source.height() - 1),
            )
        }))
    }
    pub fn dimensions(self, source: (u32, u32)) -> (u32, u32) {
        self.layout(source).dimensions
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
        if self.motion != crate::animation::Motion::Still {
            return crate::animation::Renderer::new(source, self, None).frame(0.);
        }
        let dimensions = self.dimensions(source.dimensions());
        let source = self.extend_edges(source);
        let frame = Frame::centered(dimensions, source.dimensions());
        let (w, h) = frame.dimensions;
        let mut out = RgbaImage::new(w, h);
        let (_, from, to) = PRESETS[self.preset];
        let rgb = |hex: u32| [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
        let from = rgb(from);
        let to = rgb(to);
        let (left, top) = frame.origin;
        let (left_f, top_f) = (left as f32, top as f32);
        let sw = source.width() as f32;
        let sh = source.height() as f32;
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
                let mut color = color;
                if self.shadow > 0 {
                    let blur = self.shadow as f32;
                    let d = distance(px - left_f, py - top_f - blur * 0.25, sw, sh, inner_radius)
                        .max(0.);
                    // Soft rounded-rectangle falloff, independent of screenshot alpha.
                    let opacity = 0.22 * (-2. * (d / (blur * 0.6)).powi(2)).exp();
                    for c in &mut color {
                        *c *= 1. - opacity;
                    }
                }
                if x >= left && y >= top && x < left + source.width() && y < top + source.height() {
                    let src = source.get_pixel(x - left, y - top);
                    let alpha = src[3] as f32 / 255.
                        * coverage(distance(px - left_f, py - top_f, sw, sh, inner_radius));
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
                        255,
                    ]),
                );
            }
        }
        out
    }
}
pub(crate) fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0., 1.)
}
pub(crate) fn distance(x: f32, y: f32, w: f32, h: f32, radius: f32) -> f32 {
    let radius = radius.min(w * 0.5).min(h * 0.5);
    let qx = (x - w * 0.5).abs() - w * 0.5 + radius;
    let qy = (y - h * 0.5).abs() - h * 0.5 + radius;
    qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - radius
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Padding,
    InnerRadius,
    InsidePadding,
    Shadow,
    Duration,
}
impl Control {
    pub fn label(self) -> &'static str {
        match self {
            Self::Padding => "Outside padding",
            Self::InnerRadius => "Image corners",
            Self::InsidePadding => "Inside padding",
            Self::Shadow => "Shadow",
            Self::Duration => "Duration",
        }
    }
    pub fn min(self) -> u32 {
        if self == Self::Duration { 2 } else { 0 }
    }
    pub fn max(self) -> u32 {
        match self {
            Self::Padding | Self::InsidePadding => 200,
            Self::Shadow => 60,
            Self::Duration => 15,
            _ => 80,
        }
    }
    pub fn value(self, b: Backdrop) -> u32 {
        match self {
            Self::Padding => b.padding,
            Self::InnerRadius => b.inner_radius,
            Self::InsidePadding => b.inside_padding,
            Self::Shadow => b.shadow,
            Self::Duration => b.seconds,
        }
    }
    pub fn set(self, b: &mut Backdrop, value: u32) {
        match self {
            Self::Padding => b.padding = value,
            Self::InnerRadius => b.inner_radius = value,
            Self::InsidePadding => b.inside_padding = value,
            Self::Shadow => b.shadow = value,
            Self::Duration => b.seconds = value.clamp(2, 15),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_center_and_preserve_the_full_image_in_static_and_motion_exports() {
        let source = RgbaImage::from_fn(37, 19, |x, y| Rgba([x as u8, y as u8, 211, 255]));
        for format in Format::ALL {
            let b = Backdrop {
                format,
                padding: 7,
                inside_padding: 5,
                inner_radius: 0,
                shadow: 0,
                ..Default::default()
            };
            let frame = b.layout(source.dimensions());
            let (w, h) = frame.dimensions;
            let (left, top) = frame.origin;
            assert!(left >= 12 && top >= 12);
            assert!((w - source.width() - left).abs_diff(left) <= 1);
            assert!((h - source.height() - top).abs_diff(top) <= 1);
            if let Some((rw, rh)) = format.ratio() {
                assert_eq!(w * rh, h * rw);
            }
            for motion in std::iter::once(crate::animation::Motion::Still)
                .chain(crate::animation::Motion::EFFECTS)
            {
                let image = Backdrop { motion, ..b }.apply(&source);
                assert_eq!(image.dimensions(), frame.dimensions);
                for (x, y, pixel) in source.enumerate_pixels() {
                    assert_eq!(
                        image.get_pixel(x + left, y + top),
                        pixel,
                        "{format:?} {motion:?}"
                    );
                }
            }
        }
        let old: Backdrop = serde_json::from_value(serde_json::json!({"padding": 10})).unwrap();
        assert_eq!(old.format, Format::Auto);
        assert_eq!(old.inside_padding, 0);
    }
    #[test]
    fn capped_exports_keep_exact_formats_and_do_not_clip_the_foreground() {
        let source = RgbaImage::from_pixel(401, 257, Rgba([210, 5, 20, 255]));
        for format in Format::ALL {
            let b = Backdrop {
                format,
                padding: 31,
                inside_padding: 43,
                inner_radius: 0,
                shadow: 0,
                ..Default::default()
            };
            let renderer = crate::animation::Renderer::new(&source, b, Some(160));
            let image = renderer.frame(0.);
            let (w, h) = image.dimensions();
            assert!(w <= 160 && h <= 160);
            assert_eq!(w % 2, 0);
            assert_eq!(h % 2, 0);
            if let Some((rw, rh)) = format.ratio() {
                assert_eq!(w * rh, h * rw);
            }
            let red: Vec<_> = image
                .enumerate_pixels()
                .filter(|(_, _, p)| p[0] == 210 && p[1] == 5)
                .map(|(x, y, _)| (x, y))
                .collect();
            let min_x = red.iter().map(|p| p.0).min().unwrap();
            let max_x = red.iter().map(|p| p.0).max().unwrap();
            let min_y = red.iter().map(|p| p.1).min().unwrap();
            let max_y = red.iter().map(|p| p.1).max().unwrap();
            assert!(min_x.abs_diff(w - 1 - max_x) <= 1);
            assert!(min_y.abs_diff(h - 1 - max_y) <= 1);
        }
    }
    #[test]
    fn inside_padding_repeats_each_edge_and_corner_in_static_and_motion_exports() {
        let source = RgbaImage::from_fn(5, 3, |x, y| Rgba([x as u8 * 40, y as u8 * 60, 90, 255]));
        let original = source.clone();
        for padding in [0, 1, 4, 200] {
            let b = Backdrop {
                inside_padding: padding,
                padding: 3,
                inner_radius: 0,
                shadow: 0,
                ..Default::default()
            };
            let extended = b.extend_edges(&source);
            assert_eq!(extended.dimensions(), (5 + padding * 2, 3 + padding * 2));
            for motion in [
                crate::animation::Motion::Still,
                crate::animation::Motion::Flow,
                crate::animation::Motion::Liquid,
            ] {
                let output = Backdrop { motion, ..b }.apply(&source);
                for (x, y, pixel) in extended.enumerate_pixels() {
                    let expected = source.get_pixel(
                        x.saturating_sub(padding).min(4),
                        y.saturating_sub(padding).min(2),
                    );
                    assert_eq!(pixel, expected);
                    assert_eq!(output.get_pixel(x + 3, y + 3), expected, "{motion:?}");
                }
                assert_eq!(output.get_pixel(0, 0)[3], 255);
            }
        }
        assert_eq!(source, original);
        let transparent = RgbaImage::from_pixel(1, 1, Rgba([80, 120, 200, 100]));
        let b = Backdrop {
            inside_padding: 2,
            ..Default::default()
        };
        assert!(
            b.extend_edges(&transparent)
                .pixels()
                .all(|pixel| *pixel == transparent[(0, 0)])
        );
        assert!(matches!(
            Backdrop::default().extend_edges(&source),
            std::borrow::Cow::Borrowed(_)
        ));
    }
    #[test]
    #[ignore = "writes synthetic PNG samples for visual QA"]
    fn inside_padding_visual_qa() {
        let source = crate::document::demo();
        let path = std::path::Path::new("target/inside-padding-qa");
        std::fs::create_dir_all(path).unwrap();
        for (name, inside_padding) in [("before", 0), ("after", 40)] {
            Backdrop {
                inside_padding,
                ..Default::default()
            }
            .apply(&source)
            .save(path.join(format!("{name}.png")))
            .unwrap();
        }
    }
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
            inside_padding: 8,
            inner_radius: 8,
            shadow: 8,
            ..Default::default()
        };
        let out = b.apply(&src);
        assert_eq!(out.get_pixel(0, 0)[3], 255);
        assert_ne!(out.get_pixel(20, 0), out.get_pixel(20, 39));
        assert_ne!(out.get_pixel(10, 10), src.get_pixel(0, 0));
        assert_eq!(out.get_pixel(20, 20), src.get_pixel(10, 10));
        let no_shadow = Backdrop { shadow: 0, ..b }.apply(&src);
        assert!(out.get_pixel(28, 48)[0] < no_shadow.get_pixel(28, 48)[0]);
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
            inside_padding: 0,
            shadow: 0,
            ..Default::default()
        };
        let out = b.apply(&src);
        assert_eq!(out.dimensions(), (4, 4));
        assert!(out.pixels().all(|p| *p == Rgba([50, 180, 155, 255])));
    }
}
