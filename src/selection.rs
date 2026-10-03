//! Object geometry in physical image pixels; pointer motion never rasterizes the image.
use crate::document::{Document, Mark, Tool};
use ab_glyph::{Font, ScaleFont};

impl Mark {
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if let Some(c) = &mut self.curve {
            c.0 += dx;
            c.1 += dy;
        }
        for p in &mut self.points {
            p.0 += dx;
            p.1 += dy;
        }
    }
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let a = self.points.first().copied().unwrap_or_default();
        if self.tool == Tool::Text {
            let em = (self.width * 7.).max(1.);
            static FONT: std::sync::OnceLock<Option<ab_glyph::FontArc>> =
                std::sync::OnceLock::new();
            let font = FONT.get_or_init(|| {
                std::fs::read("/System/Library/Fonts/Supplemental/Arial.ttf")
                    .ok()
                    .and_then(|b| ab_glyph::FontArc::try_from_vec(b).ok())
            });
            let w = font
                .as_ref()
                .map_or(self.text.chars().count() as f32 * em * 0.65, |f| {
                    let scaled = f.as_scaled(
                        em * f.height_unscaled() / f.units_per_em().unwrap_or(f.height_unscaled()),
                    );
                    let mut previous = None;
                    let mut width = 0.;
                    for c in self.text.chars() {
                        let id = f.glyph_id(c);
                        if let Some(p) = previous {
                            width += scaled.kern(p, id);
                        }
                        width += scaled.h_advance(id);
                        previous = Some(id);
                    }
                    width
                });
            return (a.0, a.1, a.0 + w.max(1.), a.1 + em);
        }
        if self.tool == Tool::Counter {
            let r = (self.width * 3.6).max(1.);
            return (a.0 - r, a.1 - r, a.0 + r, a.1 + r);
        }
        let mut b = (a.0, a.1, a.0, a.1);
        for p in &self.points {
            b.0 = b.0.min(p.0);
            b.1 = b.1.min(p.1);
            b.2 = b.2.max(p.0);
            b.3 = b.3.max(p.1);
        }
        let pad = self.width / 2.;
        if self.tool == Tool::Arrow {
            for p in crate::arrow::samples(self)
                .into_iter()
                .chain(crate::arrow::head(self))
            {
                b.0 = b.0.min(p.0);
                b.1 = b.1.min(p.1);
                b.2 = b.2.max(p.0);
                b.3 = b.3.max(p.1);
            }
        }
        (b.0 - pad, b.1 - pad, b.2 + pad, b.3 + pad)
    }
    pub fn hit(&self, p: (f32, f32), tolerance: f32) -> bool {
        if self.points.is_empty() {
            return false;
        }
        let b = self.bounds();
        if p.0 < b.0 - tolerance
            || p.1 < b.1 - tolerance
            || p.0 > b.2 + tolerance
            || p.1 > b.3 + tolerance
        {
            return false;
        }
        let a = self.points[0];
        let z = *self.points.last().unwrap();
        let t = tolerance + self.width / 2.;
        match self.tool {
            Tool::Pen => {
                self.points.windows(2).any(|s| distance(p, s[0], s[1]) <= t)
                    || distance(p, a, a) <= t
            }
            Tool::Arrow => {
                let head = crate::arrow::head(self);
                crate::arrow::samples(self)
                    .windows(2)
                    .any(|s| distance(p, s[0], s[1]) <= t)
                    || crate::arrow::inside_triangle(p, head)
                    || (0..3).any(|i| distance(p, head[i], head[(i + 1) % 3]) <= tolerance)
            }
            Tool::Rectangle => [
                (a, (z.0, a.1)),
                ((z.0, a.1), z),
                (z, (a.0, z.1)),
                ((a.0, z.1), a),
            ]
            .into_iter()
            .any(|(a, b)| distance(p, a, b) <= t),
            Tool::Counter => (p.0 - a.0).hypot(p.1 - a.1) <= self.width * 3.6 + tolerance,
            Tool::Select | Tool::Crop => false,
            _ => true,
        }
    }
}
fn distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let d = (b.0 - a.0, b.1 - a.1);
    let t = if d.0 * d.0 + d.1 * d.1 > 0. {
        ((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / (d.0 * d.0 + d.1 * d.1)
    } else {
        0.
    }
    .clamp(0., 1.);
    (p.0 - a.0 - t * d.0).hypot(p.1 - a.1 - t * d.1)
}
impl Document {
    pub fn pick(&self, p: (f32, f32), tolerance: f32) -> Option<usize> {
        self.marks.iter().rposition(|m| m.hit(p, tolerance))
    }
    pub fn delete_mark(&mut self, index: usize) {
        if index < self.marks.len() {
            self.remember();
            self.marks.remove(index);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mark(tool: Tool, points: Vec<(f32, f32)>) -> Mark {
        Mark {
            tool,
            curve: None,
            points,
            color: [255, 0, 0, 255],
            width: 3.,
            text: "Hello".into(),
        }
    }
    #[test]
    fn picks_topmost_and_ignores_empty_box_and_stroke_space() {
        let mut d = Document::new(image::RgbaImage::new(100, 100));
        d.commit(mark(Tool::Rectangle, vec![(10., 10.), (90., 90.)]));
        assert_eq!(d.pick((50., 50.), 3.), None);
        d.commit(mark(Tool::Pen, vec![(0., 0.), (90., 90.)]));
        assert_eq!(d.pick((50., 50.), 3.), Some(1));
        assert_eq!(d.pick((10., 80.), 3.), Some(0));
        assert_eq!(d.pick((20., 70.), 3.), None);
        d.commit(mark(Tool::Text, vec![(45., 45.)]));
        assert_eq!(d.pick((50., 50.), 3.), Some(2));
    }
    #[test]
    fn move_delete_and_history_preserve_base_and_export() {
        let mut d = Document::new(image::RgbaImage::from_pixel(
            100,
            100,
            image::Rgba([0, 0, 0, 255]),
        ));
        d.commit(mark(Tool::Pen, vec![(10., 10.), (20., 10.)]));
        let base = d.base.clone();
        d.remember();
        d.marks[0].translate(20., 30.);
        assert_eq!(d.export().get_pixel(35, 40), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(d.export().get_pixel(15, 10), &image::Rgba([0, 0, 0, 255]));
        d.undo();
        assert_eq!(d.marks[0].points[0], (10., 10.));
        d.redo();
        assert_eq!(d.marks[0].points[0], (30., 40.));
        d.delete_mark(0);
        assert!(d.marks.is_empty());
        d.undo();
        assert_eq!(d.marks.len(), 1);
        assert!(std::sync::Arc::ptr_eq(&base, &d.base));
    }
}
