//! Non-destructive focus effects; pointer handlers only change object geometry.
use crate::{
    Layout,
    document::{Mark, Tool},
};
use gpui::{Bounds, BoxShadow, RenderImage, Window, fill, point, px, quad, rgb, rgba, size};
use image::{Pixel, Rgba, RgbaImage};
use std::sync::Arc;

pub fn radius(m: &Mark) -> f32 {
    (m.width * 12.).clamp(18., 300.)
}
pub fn zoom(m: &Mark) -> f32 {
    m.text
        .parse::<f32>()
        .ok()
        .filter(|z| z.is_finite())
        .unwrap_or(2.)
        .clamp(1.5, 4.)
}
fn region(m: &Mark, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let a = m.points.first().copied().unwrap_or_default();
    let b = m.points.last().copied().unwrap_or(a);
    (
        a.0.min(b.0).clamp(0., w),
        a.1.min(b.1).clamp(0., h),
        a.0.max(b.0).clamp(0., w),
        a.1.max(b.1).clamp(0., h),
    )
}
pub fn spotlight_raster(out: &mut RgbaImage, marks: &[&Mark]) {
    let focus: Vec<_> = marks
        .iter()
        .filter(|m| m.tool == Tool::Spotlight && m.points.len() >= 2)
        .collect();
    if focus.is_empty() {
        return;
    }
    let regions: Vec<_> = focus
        .iter()
        .map(|m| region(m, out.width() as f32, out.height() as f32))
        .collect();
    for (x, y, p) in out.enumerate_pixels_mut() {
        let inside = regions.iter().any(|&(l, t, r, b)| {
            x as f32 + 0.5 >= l && x as f32 + 0.5 < r && y as f32 + 0.5 >= t && y as f32 + 0.5 < b
        });
        if !inside {
            for c in &mut p.0[..3] {
                *c = (*c as f32 * 0.32).round() as u8
            }
        }
    }
    for m in focus {
        let mut outline = (*m).clone();
        outline.tool = Tool::Rectangle;
        outline.width = 2.;
        crate::document::paint(out, &outline);
    }
}
fn sample(source: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
    let x = x.clamp(0., source.width().saturating_sub(1) as f32);
    let y = y.clamp(0., source.height().saturating_sub(1) as f32);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(source.width() - 1);
    let y1 = (y0 + 1).min(source.height() - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let samples = [
        (source.get_pixel(x0, y0), (1. - tx) * (1. - ty)),
        (source.get_pixel(x1, y0), tx * (1. - ty)),
        (source.get_pixel(x0, y1), (1. - tx) * ty),
        (source.get_pixel(x1, y1), tx * ty),
    ];
    let a: f32 = samples.iter().map(|(p, w)| p[3] as f32 * w).sum();
    Rgba(std::array::from_fn(|c| {
        if c == 3 {
            a.round() as u8
        } else if a > 0. {
            (samples
                .iter()
                .map(|(p, w)| p[c] as f32 * p[3] as f32 * w)
                .sum::<f32>()
                / a)
                .round() as u8
        } else {
            0
        }
    }))
}
pub fn tile(source: &RgbaImage, m: &Mark) -> RgbaImage {
    let r = radius(m);
    let center = m.points.first().copied().unwrap_or_default();
    let side = (r * 2.).ceil() as u32;
    let z = zoom(m);
    RgbaImage::from_fn(side, side, |x, y| {
        sample(
            source,
            center.0 + (x as f32 + 0.5 - r) / z,
            center.1 + (y as f32 + 0.5 - r) / z,
        )
    })
}
fn ring(out: &mut RgbaImage, c: (f32, f32), r: f32, color: [u8; 4], width: f32) {
    let left = (c.0 - r - width).floor().max(0.) as u32;
    let top = (c.1 - r - width).floor().max(0.) as u32;
    let right = (c.0 + r + width).ceil().max(0.).min(out.width() as f32) as u32;
    let bottom = (c.1 + r + width).ceil().max(0.).min(out.height() as f32) as u32;
    for y in top..bottom {
        for x in left..right {
            let d = ((x as f32 + 0.5 - c.0).hypot(y as f32 + 0.5 - c.1) - r).abs();
            let a = (width / 2. + 0.5 - d).clamp(0., 1.);
            if a > 0. {
                let mut p = Rgba(color);
                p[3] = (color[3] as f32 * a).round() as u8;
                out.get_pixel_mut(x, y).blend(&p);
            }
        }
    }
}
pub fn magnifier_raster(out: &mut RgbaImage, source: &RgbaImage, m: &Mark) {
    if m.points.len() < 2 {
        return;
    }
    let a = m.points[0];
    let b = *m.points.last().unwrap();
    let r = radius(m);
    let z = zoom(m);
    let mut line = m.clone();
    line.tool = Tool::Pen;
    line.width = 1.;
    line.color[3] = 255;
    crate::document::paint(out, &line);
    ring(out, a, r / z, m.color, 1.5);
    let left = (b.0 - r - 1.).floor().max(0.) as u32;
    let top = (b.1 - r - 1.).floor().max(0.) as u32;
    let right = (b.0 + r + 1.).ceil().max(0.).min(out.width() as f32) as u32;
    let bottom = (b.1 + r + 1.).ceil().max(0.).min(out.height() as f32) as u32;
    for y in top..bottom {
        for x in left..right {
            let dx = x as f32 + 0.5 - b.0;
            let dy = y as f32 + 0.5 - b.1;
            let coverage = (r + 0.5 - dx.hypot(dy)).clamp(0., 1.);
            if coverage > 0. {
                let mut p = sample(source, a.0 + dx / z, a.1 + dy / z);
                p[3] = (p[3] as f32 * coverage).round() as u8;
                out.get_pixel_mut(x, y).blend(&p);
            }
        }
    }
    ring(out, b, r, m.color, 3.);
}
pub fn paint_spotlight(m: &Mark, l: Layout, w: &mut Window) {
    let (x, y, r, b) = region(m, l.width, l.height);
    let dim = rgba(0x000000ad);
    for (x, y, width, height) in [
        (0., 0., l.width, y),
        (0., b, l.width, l.height - b),
        (0., y, x, b - y),
        (r, y, l.width - r, b - y),
    ] {
        if width > 0. && height > 0. {
            w.paint_quad(fill(
                Bounds::new(
                    point(px(l.x + x * l.scale), px(l.y + y * l.scale)),
                    size(px(width * l.scale), px(height * l.scale)),
                ),
                dim,
            ));
        }
    }
    let mut outline = m.clone();
    outline.tool = Tool::Rectangle;
    outline.width = 2.;
    for path in crate::drawing::paths(&outline, l) {
        w.paint_path(path, rgba(u32::from_be_bytes(m.color)));
    }
}
pub fn paint_lens(m: &Mark, l: Layout, image: Option<Arc<RenderImage>>, w: &mut Window) {
    if m.points.len() < 2 {
        return;
    }
    let a = m.points[0];
    let b = *m.points.last().unwrap();
    let r = radius(m) * l.scale;
    let color = rgba(u32::from_be_bytes(m.color));
    let mut line = m.clone();
    line.tool = Tool::Pen;
    line.width = 1.;
    for p in crate::drawing::paths(&line, l) {
        w.paint_path(p, color);
    }
    let map = |(x, y): (f32, f32)| point(px(l.x + x * l.scale), px(l.y + y * l.scale));
    let source_r = r / zoom(m);
    w.paint_quad(quad(
        Bounds::new(
            map(a) - point(px(source_r), px(source_r)),
            size(px(source_r * 2.), px(source_r * 2.)),
        ),
        px(source_r),
        rgba(0x00000000),
        px(1.5 * l.scale),
        color,
        Default::default(),
    ));
    let bounds = Bounds::new(map(b) - point(px(r), px(r)), size(px(r * 2.), px(r * 2.)));
    w.paint_shadows(
        bounds,
        px(r).into(),
        &[BoxShadow {
            color: rgba(0x00000038).into(),
            offset: point(px(0.), px(3.)),
            blur_radius: px(12.),
            spread_radius: px(0.),
        }],
    );
    w.paint_quad(quad(
        bounds,
        px(r),
        rgb(0xffffff),
        px(0.),
        color,
        Default::default(),
    ));
    if let Some(image) = image {
        let _ = w.paint_image(bounds, px(r).into(), image, 0, false);
    }
    w.paint_quad(quad(
        bounds,
        px(r),
        rgba(0x00000000),
        px(3. * l.scale),
        color,
        Default::default(),
    ));
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LensKey {
    revision: u64,
    x: u32,
    y: u32,
    r: u32,
    z: u32,
}
impl LensKey {
    pub fn new(revision: u64, m: &Mark) -> Self {
        let p = m.points.first().copied().unwrap_or_default();
        Self {
            revision,
            x: p.0.to_bits(),
            y: p.1.to_bits(),
            r: radius(m).to_bits(),
            z: zoom(m).to_bits(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mark(tool: Tool, a: (f32, f32), b: (f32, f32)) -> Mark {
        Mark {
            tool,
            points: vec![a, b],
            color: [255, 0, 100, 255],
            width: 2.,
            text: "2".into(),
            curve: None,
        }
    }
    #[test]
    fn spotlights_union_and_history() {
        let mut d = crate::document::Document::new(RgbaImage::from_pixel(
            100,
            100,
            Rgba([200, 200, 200, 255]),
        ));
        d.commit(mark(Tool::Spotlight, (10., 10.), (30., 30.)));
        d.commit(mark(Tool::Spotlight, (60., 60.), (80., 80.)));
        let out = d.render(None);
        assert_eq!(out.get_pixel(20, 20)[0], 200);
        assert_eq!(out.get_pixel(70, 70)[0], 200);
        assert_eq!(out.get_pixel(50, 50)[0], 64);
        d.undo();
        assert_eq!(d.render(None).get_pixel(70, 70)[0], 64);
    }
    #[test]
    fn lens_samples_undimmed_source_and_handles_independent() {
        let mut base = RgbaImage::from_pixel(160, 120, Rgba([20, 40, 60, 255]));
        for y in 15..25 {
            for x in 15..25 {
                base.put_pixel(x, y, Rgba([0, 255, 0, 255]));
            }
        }
        let mut d = crate::document::Document::new(base);
        d.commit(mark(Tool::Spotlight, (100., 80.), (150., 115.)));
        let mut lens = mark(Tool::Magnifier, (20., 20.), (100., 45.));
        d.commit(lens.clone());
        assert!(d.render(None).get_pixel(100, 45)[1] > 240);
        assert!(lens.hit((100., 45.), 1.));
        assert!(!lens.hit((150., 10.), 1.));
        assert_eq!(crate::arrow::handle_at(&lens, (100., 45.), 3.), Some(2));
        crate::arrow::drag(&mut lens, Some(2), (10., 5.), false);
        assert_eq!(lens.points[0], (20., 20.));
        assert_eq!(lens.points[1], (110., 50.));
    }
    #[test]
    fn moving_bubble_reuses_texture_key() {
        let a = mark(Tool::Magnifier, (20., 20.), (80., 40.));
        let mut b = a.clone();
        b.points[1] = (100., 80.);
        assert!(LensKey::new(1, &a) == LensKey::new(1, &b));
        b.points[0] = (30., 20.);
        assert!(LensKey::new(1, &a) != LensKey::new(1, &b));
    }
}
