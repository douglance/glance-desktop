//! Build a macOS iconset from Glance's SVG mark, preserving transparent edges.
use image::{
    Rgba, RgbaImage,
    imageops::{FilterType, resize},
};
use std::path::Path;
fn main() {
    let directory = std::env::args().nth(1).expect("iconset directory");
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let source = include_bytes!("../assets/icons/glance.svg");
    let tree = resvg::usvg::Tree::from_data(source, &resvg::usvg::Options::default())
        .expect("Glance SVG icon master");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(1024, 1024).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let png = pixmap.encode_png().unwrap();
    std::fs::write(directory.join("master.png"), &png).unwrap();
    let icon = image::load_from_memory(&png).unwrap().into_rgba32f();
    assert_eq!(icon.width(), icon.height(), "icon master must be square");
    assert!(
        icon.pixels().any(|p| p[3] == 0.),
        "icon needs transparent margins"
    );
    // Filter premultiplied colors so transparent pixels cannot add dark fringes.
    let mut icon = icon;
    for pixel in icon.pixels_mut() {
        for channel in 0..3 {
            pixel[channel] *= pixel[3];
        }
    }
    for size in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let name = format!(
                "icon_{size}x{size}{}.png",
                if scale == 2 { "@2x" } else { "" }
            );
            let small = resize(&icon, size * scale, size * scale, FilterType::Lanczos3);
            let output = RgbaImage::from_fn(small.width(), small.height(), |x, y| {
                let p = small.get_pixel(x, y);
                let a = p[3].clamp(0., 1.);
                let channel = |i| {
                    if a > 0.0001 {
                        (p[i] / a).clamp(0., 1.)
                    } else {
                        0.
                    }
                };
                Rgba([
                    (channel(0) * 255.).round() as u8,
                    (channel(1) * 255.).round() as u8,
                    (channel(2) * 255.).round() as u8,
                    (a * 255.).round() as u8,
                ])
            });
            output.save(directory.join(name)).unwrap();
        }
    }
}
