//! View transforms and constrained geometry. These operations never touch image pixels.
use crate::{Layout, document::Tool};

pub fn anchored_zoom(
    scale: f32,
    pan: (f32, f32),
    center: (f32, f32),
    anchor: (f32, f32),
    factor: f32,
) -> Option<(f32, (f32, f32))> {
    if !scale.is_finite() || scale <= 0. || !factor.is_finite() || factor <= 0. {
        return None;
    }
    let next = (scale * factor).clamp(0.01, 8.);
    let ratio = next / scale;
    Some((
        next,
        (
            anchor.0 - center.0 - (anchor.0 - center.0 - pan.0) * ratio,
            anchor.1 - center.1 - (anchor.1 - center.1 - pan.1) * ratio,
        ),
    ))
}
pub fn endpoint(
    tool: Tool,
    start: (f32, f32),
    mut p: (f32, f32),
    shift: bool,
    layout: Layout,
) -> (f32, f32) {
    let edge = if tool == Tool::Crop { 0. } else { 1. };
    let w = layout.width - edge;
    let h = layout.height - edge;
    if tool == Tool::Crop && layout.scale > 0. {
        let tolerance = 8. / layout.scale;
        for (value, edge) in [(&mut p.0, w), (&mut p.1, h)] {
            if value.abs() <= tolerance {
                *value = 0.;
            } else if (*value - edge).abs() <= tolerance {
                *value = edge;
            }
        }
    }
    if !shift {
        return p;
    }
    let dx = p.0 - start.0;
    let dy = p.1 - start.1;
    if tool == Tool::Arrow {
        let step = std::f32::consts::FRAC_PI_4;
        let angle = (dy.atan2(dx) / step).round() * step;
        let len = dx.hypot(dy);
        let dx = len * angle.cos();
        let dy = len * angle.sin();
        let mut factor = 1_f32;
        for (delta, origin, edge) in [(dx, start.0, w), (dy, start.1, h)] {
            if delta.abs() > 0.0001 {
                factor = factor.min(if delta > 0. {
                    (edge - origin) / delta
                } else {
                    -origin / delta
                });
            }
        }
        (start.0 + dx * factor, start.1 + dy * factor)
    } else if matches!(
        tool,
        Tool::Rectangle | Tool::Crop | Tool::Highlight | Tool::Pixelate
    ) {
        let sx = if dx < 0. { -1. } else { 1. };
        let sy = if dy < 0. { -1. } else { 1. };
        let side = dx
            .abs()
            .max(dy.abs())
            .min(if sx > 0. { w - start.0 } else { start.0 })
            .min(if sy > 0. { h - start.1 } else { start.1 });
        (start.0 + side * sx, start.1 + side * sy)
    } else {
        p
    }
}
pub fn translation(start: (f32, f32), p: (f32, f32), shift: bool) -> (f32, f32) {
    let d = (p.0 - start.0, p.1 - start.1);
    if shift {
        if d.0.abs() > d.1.abs() {
            (d.0, 0.)
        } else {
            (0., d.1)
        }
    } else {
        d
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_keeps_anchor_pixel_stationary_and_obeys_limits() {
        let pan = (34., -17.);
        let center = (600., 400.);
        let anchor = (321., 234.);
        let old = 0.035;
        let point = (
            (anchor.0 - center.0 - pan.0) / old,
            (anchor.1 - center.1 - pan.1) / old,
        );
        let (next, pan) = anchored_zoom(old, pan, center, anchor, 2.).unwrap();
        assert!((center.0 + pan.0 + point.0 * next - anchor.0).abs() < 0.001);
        assert!((center.1 + pan.1 + point.1 * next - anchor.1).abs() < 0.001);
        assert_eq!(
            anchored_zoom(0.02, (0., 0.), center, anchor, 0.001)
                .unwrap()
                .0,
            0.01
        );
        assert_eq!(
            anchored_zoom(4., (0., 0.), center, anchor, 100.).unwrap().0,
            8.
        );
        assert!(anchored_zoom(old, pan, center, anchor, f32::NAN).is_none());
    }
    #[test]
    fn constrained_shapes_stay_square_and_arrows_snap() {
        let l = Layout {
            width: 100.,
            height: 100.,
            scale: 1.,
            ..Default::default()
        };
        let p = endpoint(Tool::Crop, (80., 80.), (95., 60.), true, l);
        assert_eq!(p, (100., 60.));
        let p = endpoint(Tool::Arrow, (10., 10.), (75., 65.), true, l);
        assert!(((p.0 - 10.) - (p.1 - 10.)).abs() < 0.001);
        assert_eq!(
            endpoint(Tool::Crop, (40., 40.), (95., 4.), false, l),
            (100., 0.)
        );
        assert_eq!(translation((0., 0.), (20., 4.), true), (20., 0.));
    }
}
