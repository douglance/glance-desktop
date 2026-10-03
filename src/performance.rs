//! Reproducible CPU preparation benchmark; not an end-to-end display latency test.
use crate::{
    Layout,
    document::{Document, Mark, Tool},
    drawing,
    editor::render_image,
};
use image::{Rgba, RgbaImage};
use std::time::Instant;
fn samples(mut run: impl FnMut(), count: usize) -> (f64, f64) {
    for _ in 0..10 {
        run();
    }
    let mut times: Vec<f64> = (0..count)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed().as_secs_f64() * 1000.
        })
        .collect();
    times.sort_by(f64::total_cmp);
    (times[count / 2], times[count * 95 / 100])
}
#[test]
#[ignore = "run explicitly in release mode for performance measurements"]
fn drawing_preparation_benchmark() {
    crate::document::demo()
        .save("/private/tmp/pachiri-practice.png")
        .unwrap();
    let base = RgbaImage::from_pixel(1600, 900, Rgba([25, 30, 40, 255]));
    let layout = Layout {
        scale: 0.4,
        ..Default::default()
    };
    println!(
        "CPU preparation milliseconds (100 samples, 10 warmups; 4K source / 1600×900 preview)"
    );
    for count in [500, 2000, 10000] {
        let mark = Mark {
            tool: Tool::Pen,
            curve: None,
            points: (0..count)
                .map(|i| {
                    let t = i as f32 / count as f32;
                    (100. + t * 3400., 1000. + (t * 30.).sin() * 500.)
                })
                .collect(),
            color: [255, 56, 100, 255],
            width: 5.,
            text: String::new(),
        };
        let gpu = samples(
            || {
                let m = std::hint::black_box(mark.clone());
                std::hint::black_box(drawing::paths(&m, layout));
            },
            100,
        );
        let raster = samples(
            || {
                // Reproduce the former live-preview clone, compose, BGRA conversion and allocation.
                let mut doc = Document::new(base.clone());
                let mut mark = mark.clone();
                mark.points.iter_mut().for_each(|p| {
                    p.0 *= 0.4;
                    p.1 *= 0.4;
                });
                mark.width *= 0.4;
                doc.marks.push(mark);
                std::hint::black_box(render_image(doc.render(None)));
            },
            100,
        );
        println!(
            "{count} points: GPU paths p50 {:.3}, p95 {:.3}; former raster p50 {:.3}, p95 {:.3}",
            gpu.0, gpu.1, raster.0, raster.1
        );
    }
    let arrow = Mark {
        tool: Tool::Arrow,
        points: vec![(100., 1900.), (3500., 1900.)],
        curve: Some((1800., -1000.)),
        color: [255, 40, 100, 255],
        width: 9.,
        text: String::new(),
    };
    let gpu = samples(
        || {
            std::hint::black_box(drawing::paths(&arrow, layout));
        },
        1000,
    );
    println!(
        "Curved arrow GPU paths p50 {:.3}, p95 {:.3} ms",
        gpu.0, gpu.1
    );
    let mut document = Document::new(RgbaImage::from_pixel(3840, 2160, Rgba([0, 0, 0, 255])));
    let mark = Mark {
        tool: Tool::Pen,
        curve: None,
        points: vec![(10., 10.), (100., 100.)],
        color: [255, 0, 0, 255],
        width: 5.,
        text: String::new(),
    };
    let commit = samples(
        || {
            document.commit(mark.clone());
        },
        100,
    );
    println!("4K history commit p50 {:.3}, p95 {:.3}", commit.0, commit.1);
}
