use crate::{
    backdrop::Backdrop,
    document::{Document, Mark, Tool},
};
use image::{Rgba, RgbaImage};
fn random(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as u32
}
#[test]
fn randomized_edit_sequences_roundtrip_history_and_png() {
    for case in 1..=20 {
        let mut seed = case;
        let mut d = Document::new(RgbaImage::from_fn(48, 32, |x, y| {
            Rgba([x as u8 * 3, y as u8 * 5, 90, 255])
        }));
        for step in 0..100 {
            let before = d.export();
            match random(&mut seed) % 10 {
                0..=2 => {
                    let tools = [
                        Tool::Pen,
                        Tool::Arrow,
                        Tool::Rectangle,
                        Tool::Highlight,
                        Tool::Pixelate,
                        Tool::Text,
                        Tool::Counter,
                    ];
                    let tool = tools[random(&mut seed) as usize % tools.len()];
                    let points = (0..2)
                        .map(|_| {
                            (
                                (random(&mut seed) % 100) as f32 - 25.,
                                (random(&mut seed) % 80) as f32 - 25.,
                            )
                        })
                        .collect();
                    d.commit(Mark {
                        tool,
                        curve: None,
                        points,
                        color: [200, 30, 80, 255],
                        width: (random(&mut seed) % 9 + 1) as f32,
                        text: "é日本👋".into(),
                    });
                }
                3 if !d.marks.is_empty() => {
                    let i = random(&mut seed) as usize % d.marks.len();
                    d.remember();
                    d.marks[i].translate(3., -2.);
                }
                4 if !d.marks.is_empty() => {
                    let i = random(&mut seed) as usize % d.marks.len();
                    d.delete_mark(i);
                }
                5 => {
                    d.remember();
                    d.backdrop = Some(Backdrop {
                        format: crate::backdrop::Format::Auto,
                        padding: random(&mut seed) % 10,
                        preset: random(&mut seed) as usize % 8,
                        gradient: step % 2 == 0,
                        motion: crate::animation::Motion::Still,
                        seconds: 5,
                        inner_radius: random(&mut seed) % 30,
                        outer_radius: random(&mut seed) % 20,
                        shadow: random(&mut seed) % 15,
                    });
                }
                6 => d.rotate(),
                7 => {
                    let scale = if d.base.width() > 60 || d.base.height() > 60 {
                        0.5
                    } else {
                        1.5
                    };
                    d.resize(scale, step % 2 == 0).unwrap();
                }
                8 if d.base.width() > 6 && d.base.height() > 6 => {
                    d.commit(Mark {
                        tool: Tool::Crop,
                        curve: None,
                        points: vec![
                            (1., 1.),
                            ((d.base.width() - 1) as f32, (d.base.height() - 1) as f32),
                        ],
                        color: [0; 4],
                        width: 1.,
                        text: String::new(),
                    });
                }
                _ => {
                    d.remember();
                    d.backdrop = None;
                }
            }
            let after = d.export();
            d.undo();
            assert_eq!(d.export(), before, "undo case {case}, step {step}");
            d.redo();
            assert_eq!(d.export(), after, "redo case {case}, step {step}");
            if step % 10 == 0 {
                let mut bytes = std::io::Cursor::new(Vec::new());
                after.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
                assert_eq!(
                    image::load_from_memory(bytes.get_ref()).unwrap().to_rgba8(),
                    after
                );
            }
        }
    }
}
#[test]
fn tiny_crop_edges_remain_reachable_when_zoomed_out() {
    let l = crate::Layout {
        width: 10.,
        height: 10.,
        scale: 0.1,
        ..Default::default()
    };
    assert_eq!(
        crate::navigation::endpoint(Tool::Crop, (0., 0.), (9., 9.), false, l),
        (10., 10.)
    );
    assert_eq!(
        crate::navigation::endpoint(Tool::Crop, (0., 0.), (5., 5.), false, l),
        (5., 5.)
    );
}
