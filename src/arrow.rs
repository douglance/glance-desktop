//! Shared quadratic arrow geometry, handles, and antialiased export.
use crate::{
    Layout,
    document::{Mark, Tool},
};
type Point = (f32, f32);
pub fn at(m: &Mark, t: f32) -> Point {
    let a = m.points[0];
    let b = *m.points.last().unwrap();
    let c = m.curve.unwrap_or(((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5));
    let u = 1. - t;
    (
        u * u * a.0 + 2. * u * t * c.0 + t * t * b.0,
        u * u * a.1 + 2. * u * t * c.1 + t * t * b.1,
    )
}
pub fn samples(m: &Mark) -> Vec<Point> {
    let steps = if m.curve.is_some() { 64 } else { 1 };
    (0..=steps)
        .map(|i| at(m, i as f32 / steps as f32))
        .collect()
}
/// Trim only the hidden tail of the shaft, preserving the original quadratic.
pub fn shaft_end(m: &Mark) -> f32 {
    let h = head(m);
    let inset = (h[0].0 - (h[1].0 + h[2].0) * 0.5).hypot(h[0].1 - (h[1].1 + h[2].1) * 0.5) * 0.8;
    let mut length = 0.;
    let mut previous = at(m, 1.);
    for i in (0..128).rev() {
        let t = i as f32 / 128.;
        let p = at(m, t);
        length += (p.0 - previous.0).hypot(p.1 - previous.1);
        if length >= inset {
            return t;
        }
        previous = p;
    }
    0.
}
pub fn head(m: &Mark) -> [Point; 3] {
    let a = m.points[0];
    let b = *m.points.last().unwrap();
    let c = m.curve.unwrap_or(a);
    let d = (b.0 - c.0, b.1 - c.1);
    let n = d.0.hypot(d.1);
    let d = if n > 0.001 {
        (d.0 / n, d.1 / n)
    } else {
        (1., 0.)
    };
    let len = (m.width * 4.)
        .max(16.)
        .min((b.0 - a.0).hypot(b.1 - a.1) * 0.45);
    let half = len * 0.42;
    [
        b,
        (b.0 - d.0 * len - d.1 * half, b.1 - d.1 * len + d.0 * half),
        (b.0 - d.0 * len + d.1 * half, b.1 - d.1 * len - d.0 * half),
    ]
}
pub fn inside_triangle(p: Point, h: [Point; 3]) -> bool {
    let cross = |a: Point, b: Point| (p.0 - b.0) * (a.1 - b.1) - (a.0 - b.0) * (p.1 - b.1);
    let s = [cross(h[0], h[1]), cross(h[1], h[2]), cross(h[2], h[0])];
    (s.iter().all(|v| *v >= 0.) || s.iter().all(|v| *v <= 0.))
        && ((h[1].0 - h[0].0) * (h[2].1 - h[0].1) - (h[1].1 - h[0].1) * (h[2].0 - h[0].0)).abs()
            > 0.001
}
pub fn handles(m: &Mark) -> [Point; 3] {
    [m.points[0], at(m, 0.5), *m.points.last().unwrap()]
}
pub fn handle_at(m: &Mark, p: Point, tolerance: f32) -> Option<usize> {
    if m.tool != Tool::Arrow || m.points.len() < 2 {
        return None;
    }
    // Endpoints win when a short arrow's handles overlap.
    [0, 2, 1].into_iter().find(|i| {
        let h = handles(m)[*i];
        (p.0 - h.0).hypot(p.1 - h.1) <= tolerance
    })
}
pub fn drag(m: &mut Mark, handle: Option<usize>, delta: Point, shift: bool) {
    if delta == (0., 0.) {
        return;
    }
    let p = handle
        .map(|i| {
            let h = handles(m)[i];
            (h.0 + delta.0, h.1 + delta.1)
        })
        .unwrap_or_default();
    match handle {
        Some(0 | 2) => {
            let index = if handle == Some(0) {
                0
            } else {
                m.points.len() - 1
            };
            let fixed = if index == 0 {
                *m.points.last().unwrap()
            } else {
                m.points[0]
            };
            let p = if shift {
                let d = (p.0 - fixed.0, p.1 - fixed.1);
                let a = (d.1.atan2(d.0) / std::f32::consts::FRAC_PI_4).round()
                    * std::f32::consts::FRAC_PI_4;
                let len = d.0.hypot(d.1);
                (fixed.0 + a.cos() * len, fixed.1 + a.sin() * len)
            } else {
                p
            };
            m.points[index] = p;
        }
        Some(1) => {
            let a = m.points[0];
            let b = *m.points.last().unwrap();
            m.curve = Some((2. * p.0 - (a.0 + b.0) * 0.5, 2. * p.1 - (a.1 + b.1) * 0.5));
        }
        _ => m.translate(delta.0, delta.1),
    }
}
pub fn paint_handles(m: &Mark, l: Layout, w: &mut gpui::Window) {
    use gpui::*;
    for p in handles(m) {
        w.paint_quad(quad(
            Bounds::new(
                point(px(l.x + p.0 * l.scale - 4.), px(l.y + p.1 * l.scale - 4.)),
                size(px(8.), px(8.)),
            ),
            px(2.),
            rgb(0xffffff),
            px(1.),
            rgb(0x64748b),
            Default::default(),
        ));
    }
}
pub fn raster(out: &mut image::RgbaImage, m: &Mark) {
    use image::{GrayImage, Luma, Pixel, Rgba, imageops};
    use imageproc::{
        drawing::{draw_filled_circle_mut, draw_polygon_mut},
        point::Point,
    };
    let b = m.bounds();
    let x = b.0.floor().max(0.) as u32;
    let y = b.1.floor().max(0.) as u32;
    let right = (b.2.ceil() + 1.).max(0.).min(out.width() as f32) as u32;
    let bottom = (b.3.ceil() + 1.).max(0.).min(out.height() as f32) as u32;
    if x >= right || y >= bottom {
        return;
    }
    let mut mask = GrayImage::new((right - x) * 2, (bottom - y) * 2);
    let map = |p: (f32, f32)| ((p.0 - x as f32) * 2., (p.1 - y as f32) * 2.);
    let t = shaft_end(m);
    let steps = if m.curve.is_some() { 64 } else { 1 };
    let pts: Vec<_> = (0..=steps)
        .map(|i| at(m, t * i as f32 / steps as f32))
        .collect();
    let radius = m.width.round().max(1.) as i32;
    let h = head(m);
    for pair in pts.windows(2) {
        let a = map(pair[0]);
        let b = map(pair[1]);
        let steps = (b.0 - a.0).hypot(b.1 - a.1).ceil().max(1.) as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            draw_filled_circle_mut(
                &mut mask,
                (
                    (a.0 + (b.0 - a.0) * t).round() as i32,
                    (a.1 + (b.1 - a.1) * t).round() as i32,
                ),
                radius,
                Luma([255]),
            );
        }
    }
    let polygon = h.map(|p| {
        let p = map(p);
        Point::new(p.0.round() as i32, p.1.round() as i32)
    });
    if polygon[0] != polygon[1] && polygon[1] != polygon[2] && polygon[0] != polygon[2] {
        draw_polygon_mut(&mut mask, &polygon, Luma([255]));
    }
    let small = imageops::resize(&mask, right - x, bottom - y, imageops::FilterType::Triangle);
    for (dx, dy, p) in small.enumerate_pixels() {
        if p[0] == 0 {
            continue;
        }
        let mut color = m.color;
        color[3] = ((color[3] as u16 * p[0] as u16 + 127) / 255) as u8;
        out.get_pixel_mut(x + dx, y + dy).blend(&Rgba(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mark() -> Mark {
        Mark {
            tool: Tool::Arrow,
            points: vec![(20., 100.), (180., 100.)],
            curve: Some((100., -20.)),
            width: 5.,
            color: [255, 40, 100, 255],
            text: String::new(),
        }
    }
    #[test]
    fn curved_export_and_transforms_preserve_geometry() {
        let mut d = crate::document::Document::new(image::RgbaImage::from_pixel(
            200,
            140,
            image::Rgba([0, 0, 0, 255]),
        ));
        d.commit(mark());
        let image = d.export();
        if let Ok(path) = std::env::var("PACHIRI_QA_IMAGE") {
            image.save(path).unwrap();
        }
        assert_eq!(image.get_pixel(100, 40).0, [255, 40, 100, 255]);
        assert_eq!(image.get_pixel(100, 100).0, [0, 0, 0, 255]);
        assert!(
            image.pixels().any(|p| p[0] > 0 && p[0] < 255),
            "antialiased edges"
        );
        d.resize(2., false).unwrap();
        assert_eq!(d.marks[0].curve, Some((200., -40.)));
        assert_eq!(at(&d.marks[0], 0.5), (200., 80.));
        let mut crop = mark();
        crop.tool = Tool::Crop;
        crop.points = vec![(10., 20.), (390., 270.)];
        d.commit(crop);
        assert_eq!(at(&d.marks[0], 0.5), (190., 60.));
        d.undo();
        assert_eq!(at(&d.marks[0], 0.5), (200., 80.));
    }
    #[test]
    fn handles_are_zoom_independent_and_endpoints_win() {
        let m = mark();
        assert_eq!(handle_at(&m, (100., 40.), 7.), Some(1));
        assert_eq!(handle_at(&m, (24., 100.), 7.), Some(0));
        assert_eq!(handle_at(&m, (24., 100.), 3.5), None);
        let mut m = m;
        m.points = vec![(1., 1.), (2., 1.)];
        m.curve = None;
        assert_eq!(handle_at(&m, (1., 1.), 7.), Some(0));
    }
    #[test]
    fn handle_grab_offset_does_not_jump_and_shift_snaps() {
        let mut m = mark();
        let original = m.clone();
        drag(&mut m, Some(1), (0., 0.), false);
        assert_eq!(m.curve, original.curve);
        drag(&mut m, Some(2), (2., 3.), false);
        assert_eq!(m.points[1], (182., 103.));
        assert_eq!(m.points[0], original.points[0]);
        m.curve = None;
        drag(&mut m, Some(2), (-10., 70.), true);
        let a = m.points[0];
        let b = m.points[1];
        let angle = (b.1 - a.1).atan2(b.0 - a.0) / std::f32::consts::FRAC_PI_4;
        assert!((angle - angle.round()).abs() < 0.001);
    }
    #[test]
    fn zero_length_arrow_does_not_panic() {
        let mut m = mark();
        m.points = vec![(10., 10.), (10., 10.)];
        m.curve = None;
        let mut image = image::RgbaImage::new(30, 30);
        raster(&mut image, &m);
        assert!(!inside_triangle((10., 10.), head(&m)));
        assert!(
            crate::drawing::paths(
                &m,
                Layout {
                    scale: 1.,
                    ..Default::default()
                }
            )
            .len()
                <= 2
        );
    }
}
