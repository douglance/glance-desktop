//! Generate the app's original icon from vector geometry, then downsample it.
use image::{
    Rgba, RgbaImage,
    imageops::{FilterType, resize},
};
use std::path::Path;
fn rounded_distance(x: f32, y: f32, cx: f32, cy: f32, w: f32, h: f32, r: f32) -> f32 {
    let dx = (x - cx).abs() - w / 2. + r;
    let dy = (y - cy).abs() - h / 2. + r;
    dx.max(0.).hypot(dy.max(0.)) + dx.max(dy).min(0.) - r
}
fn segment_distance(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let vx = b.0 - a.0;
    let vy = b.1 - a.1;
    let t = ((x - a.0) * vx + (y - a.1) * vy) / (vx * vx + vy * vy);
    let t = t.clamp(0., 1.);
    (x - a.0 - t * vx).hypot(y - a.1 - t * vy)
}
fn main() {
    let directory = std::env::args().nth(1).expect("iconset directory");
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut icon = RgbaImage::new(1024, 1024);
    for (x, y, pixel) in icon.enumerate_pixels_mut() {
        let x = x as f32;
        let y = y as f32;
        let d = rounded_distance(x, y, 512., 512., 824., 824., 186.);
        let alpha = (0.5 - d).clamp(0., 1.);
        if alpha == 0. {
            continue;
        }
        let t = (x + y) / 2048.;
        let mut color = [
            (252. - 20. * t) as u8,
            (115. - 54. * t) as u8,
            (87. - 16. * t) as u8,
        ];
        let frame = rounded_distance(x, y, 482., 473., 438., 330., 36.).abs() - 14.;
        let pen = segment_distance(x, y, (434., 626.), (725., 335.)) - 24.;
        let accent = segment_distance(x, y, (641., 339.), (706., 404.)) - 11.;
        let ink = (0.5 - frame.min(pen)).clamp(0., 1.);
        for c in &mut color {
            *c = (*c as f32 * (1. - ink) + 255. * ink) as u8;
        }
        let cut = (0.5 - accent).clamp(0., 1.);
        if x > 625. && y < 420. {
            color = [
                (color[0] as f32 * (1. - cut) + 241. * cut) as u8,
                (color[1] as f32 * (1. - cut) + 77. * cut) as u8,
                (color[2] as f32 * (1. - cut) + 67. * cut) as u8,
            ];
        }
        *pixel = Rgba([color[0], color[1], color[2], (alpha * 255.) as u8]);
    }
    for size in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let name = format!(
                "icon_{size}x{size}{}.png",
                if scale == 2 { "@2x" } else { "" }
            );
            resize(&icon, size * scale, size * scale, FilterType::Lanczos3)
                .save(directory.join(name))
                .unwrap();
        }
    }
}
