//! Annotation appearance and shared preview/export geometry.
use crate::document::Mark;
pub type Point = (f32, f32);
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dash {
    #[default]
    Solid,
    Dashed,
    Dotted,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fill {
    #[default]
    Outline,
    Filled,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cleanup {
    #[default]
    Raw,
    Smooth,
    Adaptive,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    None,
    #[default]
    Arrow,
    Dot,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub dash: Dash,
    pub fill: Fill,
    pub radius: f32,
    pub cleanup: Cleanup,
    pub start: End,
    pub end: End,
    pub dim: f32,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            dash: Dash::Solid,
            fill: Fill::Outline,
            radius: 0.,
            cleanup: Cleanup::Raw,
            start: End::None,
            end: End::Arrow,
            dim: 0.68,
        }
    }
}
impl Style {
    pub fn validate(self) -> Result<(), String> {
        if !self.radius.is_finite()
            || !(0. ..=32768.).contains(&self.radius)
            || !self.dim.is_finite()
            || !(0. ..=0.95).contains(&self.dim)
        {
            return Err("Corner radius must be 0..32768 and dim strength 0..0.95".into());
        }
        Ok(())
    }
}
pub fn pen_points(m: &Mark) -> Vec<Point> {
    if m.style.cleanup == Cleanup::Raw || m.points.len() < 3 {
        return m.points.clone();
    }
    let mut pts = m.points.clone();
    // A bounded three-point filter retains endpoints and sample count. Keep the
    // original samples on the mark, so switching cleanup never loses detail.
    for _ in 0..2 {
        let old = pts.clone();
        for i in 1..old.len() - 1 {
            let a = old[i - 1];
            let p = old[i];
            let b = old[i + 1];
            let u = (p.0 - a.0, p.1 - a.1);
            let v = (b.0 - p.0, b.1 - p.1);
            let cosine = (u.0 * v.0 + u.1 * v.1) / (u.0.hypot(u.1) * v.0.hypot(v.1)).max(0.001);
            if m.style.cleanup == Cleanup::Adaptive && cosine < 0.5 {
                continue;
            }
            pts[i] = ((a.0 + 2. * p.0 + b.0) * 0.25, (a.1 + 2. * p.1 + b.1) * 0.25);
        }
    }
    pts
}
pub fn box_points(m: &Mark) -> Vec<Point> {
    let a = m.points[0];
    let b = *m.points.last().unwrap();
    let (l, t, r, d) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
    let radius = m.style.radius.min((r - l) * 0.5).min((d - t) * 0.5);
    if radius < 0.01 {
        return vec![(l, t), (r, t), (r, d), (l, d), (l, t)];
    }
    let mut pts = vec![];
    for (x, y, angle) in [
        (r - radius, t + radius, -90.),
        (r - radius, d - radius, 0.),
        (l + radius, d - radius, 90.),
        (l + radius, t + radius, 180.),
    ] {
        for i in 0..=12 {
            let a = (angle + i as f32 * 7.5).to_radians();
            pts.push((x + radius * a.cos(), y + radius * a.sin()));
        }
    }
    pts.push(pts[0]);
    pts
}
pub fn line_points(m: &Mark) -> Vec<Point> {
    if m.points.len() > 2 {
        m.points.clone()
    } else {
        crate::arrow::samples(m)
    }
}
pub fn heads(m: &Mark) -> Vec<[Point; 3]> {
    let mut result = vec![];
    if m.style.end == End::Arrow {
        result.push(crate::arrow::head(m));
    }
    if m.style.start == End::Arrow {
        let mut reversed = m.clone();
        reversed.points.reverse();
        result.push(crate::arrow::head(&reversed));
    }
    result
}
pub fn dots(m: &Mark) -> Vec<Point> {
    let mut pts = vec![];
    if m.style.start == End::Dot {
        pts.push(m.points[0]);
    }
    if m.style.end == End::Dot {
        pts.push(*m.points.last().unwrap());
    }
    pts
}
pub fn shaft(m: &Mark) -> Vec<Point> {
    let mut pts = line_points(m);
    let length = if m.points.len() > 2 {
        pts.windows(2)
            .map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1))
            .sum()
    } else {
        let a = m.points[0];
        let b = *m.points.last().unwrap();
        (b.0 - a.0).hypot(b.1 - a.1)
    };
    let trim = |pts: &mut Vec<Point>, end: End| {
        if end != End::Arrow {
            return;
        }
        let mut remaining = (m.width * 4.).max(16.).min(length * 0.45) * 0.8;
        while pts.len() > 1 {
            let b = pts[pts.len() - 1];
            let a = pts[pts.len() - 2];
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            if len > remaining {
                let t = (len - remaining) / len;
                *pts.last_mut().unwrap() = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                break;
            }
            remaining -= len;
            pts.pop();
        }
    };
    trim(&mut pts, m.style.end);
    pts.reverse();
    trim(&mut pts, m.style.start);
    pts.reverse();
    pts
}
/// Split paths with a continuous dash phase across corners and waypoints.
pub fn strokes(pts: &[Point], dash: Dash, width: f32) -> Vec<Vec<Point>> {
    if dash == Dash::Solid {
        return vec![pts.to_vec()];
    }
    let on = if dash == Dash::Dotted {
        0.01
    } else {
        width.max(1.) * 3.
    };
    let off = width.max(1.) * 3.;
    let mut result = vec![];
    let mut active = true;
    let mut remaining = on;
    let mut current = vec![];
    for pair in pts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        if len < 0.001 {
            continue;
        }
        let mut pos = 0.;
        while pos < len {
            let step = remaining.min(len - pos);
            let at = |d: f32| (a.0 + (b.0 - a.0) * d / len, a.1 + (b.1 - a.1) * d / len);
            if active {
                if current.is_empty() {
                    current.push(at(pos));
                }
                current.push(at(pos + step));
            }
            pos += step;
            remaining -= step;
            if remaining < 0.0001 {
                if active && !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
                active = !active;
                remaining = if active { on } else { off };
            }
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    if result.is_empty() && pts.len() == 1 {
        result.push(pts.to_vec());
    }
    result
}

/// Straight-alpha source-over with integer rounding; keeps an opaque destination
/// opaque (the image crate's float conversion can truncate alpha to 254).
pub fn blend(destination: &mut image::Rgba<u8>, source: image::Rgba<u8>) {
    let sa = source[3] as u32;
    let da = destination[3] as u32;
    let alpha = sa * 255 + da * (255 - sa);
    if alpha == 0 {
        return;
    }
    for c in 0..3 {
        let n = source[c] as u32 * sa * 255 + destination[c] as u32 * da * (255 - sa);
        destination[c] = ((n + alpha / 2) / alpha) as u8;
    }
    destination[3] = ((alpha + 127) / 255) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Tool};
    fn mark(tool: Tool) -> Mark {
        Mark {
            tool,
            style: Default::default(),
            points: vec![(20., 20.), (100., 100.)],
            curve: None,
            color: [255, 0, 0, 255],
            width: 4.,
            text: String::new(),
        }
    }
    #[test]
    fn filled_rounded_box_opacity_and_selection_match_export() {
        let mut m = mark(Tool::Rectangle);
        m.style.fill = Fill::Filled;
        m.style.radius = 20.;
        m.color[3] = 128;
        let mut d = Document::new(image::RgbaImage::from_pixel(
            120,
            120,
            image::Rgba([0, 0, 0, 255]),
        ));
        d.commit(m.clone());
        let image = d.export();
        assert_eq!(image.get_pixel(60, 60).0, [128, 0, 0, 255]);
        assert_eq!(image.get_pixel(20, 20).0, [0, 0, 0, 255]);
        assert!(m.hit((60., 60.), 0.));
        assert!(!m.hit((20., 20.), 0.));
        assert!(
            !crate::drawing::paths(
                &m,
                crate::Layout {
                    scale: 1.,
                    ..Default::default()
                }
            )
            .is_empty()
        );
    }
    #[test]
    fn translucent_stroke_joints_blend_once_and_dashes_have_gaps() {
        let mut m = mark(Tool::Pen);
        m.points = vec![(10., 30.), (40., 30.), (100., 30.)];
        m.color[3] = 128;
        let mut d = Document::new(image::RgbaImage::from_pixel(
            120,
            60,
            image::Rgba([0, 0, 0, 255]),
        ));
        d.commit(m);
        let image = d.export();
        assert_eq!(image.get_pixel(40, 30).0, [128, 0, 0, 255]);
        let mut m = mark(Tool::Arrow);
        m.points = vec![(10., 30.), (110., 30.)];
        m.style.end = End::None;
        m.style.dash = Dash::Dashed;
        d.marks = vec![m];
        let image = d.export();
        assert_eq!(image.get_pixel(14, 30).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(28, 30).0, [0, 0, 0, 255]);
        assert!(image.get_pixel(40, 30)[0] > 0);
    }
    #[test]
    fn cleanup_is_reversible_and_adaptive_preserves_corners() {
        let mut m = mark(Tool::Pen);
        m.points = vec![(10., 10.), (40., 10.), (40., 40.)];
        let original = m.points.clone();
        m.style.cleanup = Cleanup::Smooth;
        assert_ne!(pen_points(&m), original);
        m.style.cleanup = Cleanup::Adaptive;
        assert_eq!(pen_points(&m), original);
        m.style.cleanup = Cleanup::Raw;
        assert_eq!(pen_points(&m), original);
        assert_eq!(m.points, original);
        m.points = vec![(0., 20.), (10., 21.), (20., 19.), (30., 20.), (40., 21.)];
        m.style.cleanup = Cleanup::Adaptive;
        assert_ne!(pen_points(&m), m.points);
    }
    #[test]
    fn waypoint_arrow_geometry_and_ends_are_editable() {
        let mut m = mark(Tool::Arrow);
        m.points = vec![(10., 80.), (50., 20.), (100., 80.)];
        m.style.start = End::Dot;
        assert_eq!(line_points(&m), m.points);
        assert_eq!(crate::arrow::handle_at(&m, (50., 20.), 2.), Some(1));
        crate::arrow::drag(&mut m, Some(1), (0., 10.), false);
        assert_eq!(m.points[1], (50., 30.));
        assert_eq!(heads(&m).len(), 1);
        assert_eq!(dots(&m), vec![(10., 80.)]);
        let mut d = Document::new(image::RgbaImage::from_pixel(
            120,
            100,
            image::Rgba([0, 0, 0, 255]),
        ));
        d.commit(m);
        let image = d.export();
        assert_eq!(image.get_pixel(50, 30).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(10, 80).0, [255, 0, 0, 255]);
    }
    #[test]
    fn legacy_marks_keep_defaults_and_invalid_style_is_rejected() {
        let mut value = serde_json::to_value(mark(Tool::Arrow)).unwrap();
        value.as_object_mut().unwrap().remove("style");
        let m: Mark = serde_json::from_value(value).unwrap();
        assert_eq!(m.style, Style::default());
        let mut bad = m.clone();
        bad.style.radius = f32::NAN;
        assert!(crate::document::actions::validate_mark(&bad).is_err());
        bad = m;
        bad.style.dim = 1.;
        assert!(crate::document::actions::validate_mark(&bad).is_err());
    }
}
