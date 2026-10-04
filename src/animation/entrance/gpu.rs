//! Cached Metal sampling on the preview/export worker, never the UI thread.
use super::{Entrance, Projection};
use image::RgbaImage;
use metal::*;
use std::{cell::RefCell, sync::Arc};

#[repr(C)]
struct Uniforms {
    width: u32,
    height: u32,
    diagonal: u32,
    threshold: f32,
    edge: f32,
    bounds: [f32; 4],
    inverse: [[f32; 3]; 3],
    center: [f32; 2],
    opacity: f32,
}
struct Shader {
    device: Device,
    queue: CommandQueue,
    pipeline: ComputePipelineState,
    source: Option<(Arc<RgbaImage>, Buffer)>,
    output: Option<Buffer>,
}
impl Shader {
    fn new() -> Result<Self, String> {
        objc::rc::autoreleasepool(|| {
            let device = Device::system_default().ok_or("Metal device unavailable")?;
            let options = CompileOptions::new();
            options.set_fast_math_enabled(false);
            let library = device
                .new_library_with_source(include_str!("../../shaders/entrance.metal"), &options)?;
            let function = library.get_function("image_entrance", None)?;
            Ok(Self {
                pipeline: device.new_compute_pipeline_state_with_function(&function)?,
                queue: device.new_command_queue(),
                device,
                source: None,
                output: None,
            })
        })
    }
    fn frame(&mut self, source: &Arc<RgbaImage>, u: &Uniforms) -> Result<RgbaImage, String> {
        objc::rc::autoreleasepool(|| {
            let length = u.width as u64 * u.height as u64 * 4;
            if self
                .source
                .as_ref()
                .is_none_or(|(old, _)| !Arc::ptr_eq(old, source))
            {
                self.source = Some((
                    source.clone(),
                    self.device.new_buffer_with_data(
                        source.as_raw().as_ptr().cast(),
                        length,
                        MTLResourceOptions::StorageModeShared,
                    ),
                ));
            }
            if self.output.as_ref().is_none_or(|b| b.length() != length) {
                self.output = Some(
                    self.device
                        .new_buffer(length, MTLResourceOptions::StorageModeShared),
                );
            }
            let output = self.output.as_ref().unwrap();
            let command = self.queue.new_command_buffer();
            let encoder = command.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(&self.pipeline);
            encoder.set_buffer(0, Some(output), 0);
            encoder.set_buffer(1, Some(&self.source.as_ref().unwrap().1), 0);
            encoder.set_bytes(
                2,
                std::mem::size_of::<Uniforms>() as u64,
                u as *const Uniforms as *const _,
            );
            encoder.dispatch_threads(
                MTLSize::new(u.width as u64, u.height as u64, 1),
                MTLSize::new(16, 16, 1),
            );
            encoder.end_encoding();
            command.commit();
            command.wait_until_completed();
            if command.status() != MTLCommandBufferStatus::Completed {
                return Err("Image animation Metal command failed".into());
            }
            let pixels = unsafe {
                std::slice::from_raw_parts(output.contents().cast::<u8>(), length as usize)
            };
            RgbaImage::from_raw(u.width, u.height, pixels.to_vec())
                .ok_or("Invalid image animation frame".into())
        })
    }
}
thread_local! {
    static SHADER: RefCell<Result<Shader, String>> = RefCell::new(Shader::new());
}
pub(super) fn frame(
    source: &Arc<RgbaImage>,
    effect: Entrance,
    bounds: (f32, f32, f32, f32),
    p: f32,
) -> Result<RgbaImage, String> {
    let projection = Projection::new(effect, p, bounds);
    let u = Uniforms {
        width: source.width(),
        height: source.height(),
        diagonal: u32::from(effect == Entrance::Diagonal),
        threshold: 2. * p * p * (3. - 2. * p),
        edge: (bounds.2.recip().powi(2) + bounds.3.recip().powi(2)).sqrt(),
        bounds: [bounds.0, bounds.1, bounds.2, bounds.3],
        inverse: projection.inverse,
        center: [projection.center.0, projection.center.1],
        opacity: (p * if effect == Entrance::Pop { 5. } else { 4. }).min(1.),
    };
    SHADER.with(|shader| match &mut *shader.borrow_mut() {
        Ok(shader) => shader.frame(source, &u),
        Err(error) => Err(error.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "measures real Metal versus CPU sampling on synthetic cards; no files or network"]
    fn foreground_sampling_benchmark() {
        use std::time::Instant;
        let source = Arc::new(RgbaImage::from_pixel(
            960,
            640,
            image::Rgba([40, 140, 230, 240]),
        ));
        let bounds = (80., 60., 800., 520.);
        for effect in [Entrance::Diagonal, Entrance::Pop, Entrance::Tilt] {
            let animation = super::super::ImageAnimation {
                effect,
                delay_ms: 0,
                ..Default::default()
            };
            // Compile/upload before measuring steady-state sampling.
            frame(&source, effect, bounds, 0.3).unwrap();
            let started = Instant::now();
            for i in 0..8 {
                std::hint::black_box(
                    frame(&source, effect, bounds, 0.25 + i as f32 * 0.08).unwrap(),
                );
            }
            let gpu = started.elapsed().as_secs_f64() * 1000. / 8.;
            let started = Instant::now();
            for i in 0..8 {
                std::hint::black_box(super::super::foreground(
                    &source,
                    animation,
                    bounds,
                    0.25 + i as f32 * 0.08,
                ));
            }
            let cpu = started.elapsed().as_secs_f64() * 1000. / 8.;
            println!("{effect:?} 960×640 foreground: CPU {cpu:.2} ms, Metal {gpu:.2} ms");
        }
        let source = image::RgbaImage::from_pixel(640, 420, image::Rgba([40, 140, 230, 255]));
        for effect in [Entrance::Diagonal, Entrance::Pop, Entrance::Tilt] {
            let renderer = crate::animation::Renderer::with_animation(
                &source,
                crate::backdrop::Backdrop {
                    motion: crate::animation::Motion::Liquid,
                    padding: 160,
                    ..Default::default()
                },
                Some(960),
                super::super::ImageAnimation {
                    effect,
                    delay_ms: 0,
                    ..Default::default()
                },
            );
            renderer.frame(0.05);
            let started = Instant::now();
            for i in 0..8 {
                std::hint::black_box(renderer.frame(0.05 + i as f32 * 0.016));
            }
            println!(
                "{effect:?} composed {}×{}: {:.2} ms/frame",
                renderer.width,
                renderer.height,
                started.elapsed().as_secs_f64() * 1000. / 8.
            );
        }
    }
    #[test]
    fn metal_matches_cpu_sampling_and_refreshes_cached_pixels() {
        let sources = [
            Arc::new(RgbaImage::from_fn(96, 64, |x, y| {
                image::Rgba([x as u8 * 2, y as u8 * 3, 70, if x < 8 { 0 } else { 210 }])
            })),
            Arc::new(RgbaImage::from_pixel(
                96,
                64,
                image::Rgba([30, 170, 200, 255]),
            )),
        ];
        for source in sources {
            for effect in [Entrance::Diagonal, Entrance::Pop, Entrance::Tilt] {
                for p in [0.01, 0.2, 0.5, 0.85, 0.999] {
                    let bounds = (8., 6., 80., 50.);
                    let actual = frame(&source, effect, bounds, p).unwrap();
                    let animation = super::super::ImageAnimation {
                        effect,
                        delay_ms: 0,
                        ..Default::default()
                    };
                    let expected = super::super::foreground(&source, animation, bounds, p);
                    assert!(
                        actual
                            .as_raw()
                            .iter()
                            .zip(expected.as_raw())
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "{effect:?}, p={p}"
                    );
                }
            }
        }
    }
}
