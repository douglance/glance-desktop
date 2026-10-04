//! Bounded latest-frame worker for the image and backdrop together.
use super::{ImageAnimation, Renderer, preview::quality::AdaptiveQuality};
use crate::{backdrop::Backdrop, document::Document};
use gpui::{Bounds, Pixels, RenderImage, Window, px};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Spec {
    revision: u64,
    edge: u32,
    backdrop: Backdrop,
    animation: ImageAnimation,
}
impl Spec {
    fn same_render_layout(self, other: Self) -> bool {
        let backdrop = |mut b: Backdrop| {
            b.seconds = 0;
            b
        };
        self.edge == other.edge && backdrop(self.backdrop) == backdrop(other.backdrop)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    spec: Spec,
    tick: u32,
    seek: u64,
}
#[derive(Clone)]
struct Request {
    key: Key,
    generation: u64,
    source: Arc<Document>,
}
struct Completed {
    key: Key,
    generation: u64,
    image: Arc<RenderImage>,
    elapsed: Duration,
}
#[derive(Default)]
struct Mailbox {
    signature: Option<(Spec, Option<u32>, u64)>,
    wanted: Option<Key>,
    generation: u64,
    pending: Option<Request>,
    completed: Option<Completed>,
    stopped: bool,
}
#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}
struct Worker {
    shared: Arc<Shared>,
}
impl Worker {
    fn new(
        mut render: impl FnMut(&Request) -> Arc<RenderImage> + Send + 'static,
        notify: impl Fn() + Send + 'static,
    ) -> Self {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("image-animation-preview".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut mailbox = worker.mailbox.lock().unwrap();
                        while !mailbox.stopped && mailbox.pending.is_none() {
                            mailbox = worker.wake.wait(mailbox).unwrap();
                        }
                        if mailbox.stopped {
                            break;
                        }
                        mailbox.pending.take().unwrap()
                    };
                    let start = Instant::now();
                    let image = render(&request);
                    let elapsed = start.elapsed();
                    let published = {
                        let mut mailbox = worker.mailbox.lock().unwrap();
                        if !mailbox.stopped && request.generation == mailbox.generation {
                            mailbox.completed = Some(Completed {
                                key: request.key,
                                generation: request.generation,
                                image,
                                elapsed,
                            });
                            true
                        } else {
                            false
                        }
                    };
                    if published {
                        notify();
                    }
                }
            })
            .expect("start image animation preview worker");
        Self { shared }
    }
    fn request(&self, key: Key, source: Arc<Document>, playing: bool) {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        let signature = (key.spec, (!playing).then_some(key.tick), key.seek);
        if mailbox.signature != Some(signature) {
            mailbox.generation = mailbox.generation.wrapping_add(1);
            mailbox.signature = Some(signature);
            mailbox.wanted = None;
            mailbox.completed = None;
        }
        if mailbox.wanted == Some(key) {
            return;
        }
        mailbox.wanted = Some(key);
        let generation = mailbox.generation;
        mailbox.pending = Some(Request {
            key,
            generation,
            source,
        });
        self.shared.wake.notify_one();
    }
    fn clear(&self) {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.signature.take().is_some() {
            mailbox.generation = mailbox.generation.wrapping_add(1);
        }
        mailbox.wanted = None;
        mailbox.pending = None;
        mailbox.completed = None;
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        mailbox.stopped = true;
        mailbox.pending = None;
        mailbox.completed = None;
        self.shared.wake.notify_one();
    }
}

struct Prepared {
    source: Arc<Document>,
    pixels: image::RgbaImage,
    spec: Spec,
    renderer: Renderer,
}
impl Prepared {
    fn renderer(source: &Document, pixels: &image::RgbaImage, spec: Spec) -> Renderer {
        let mut renderer =
            Renderer::with_animation(pixels, spec.backdrop, Some(spec.edge), spec.animation);
        renderer.transparent_background = source.backdrop.is_none();
        // Compile and upload before publishing the initial (often hidden) pose.
        // Otherwise the first visible entrance frame can stall after playback starts.
        renderer.prepare_preview();
        renderer
    }
    fn new(request: &Request) -> Self {
        let pixels = request.source.render(None);
        let spec = request.key.spec;
        let renderer = Self::renderer(&request.source, &pixels, spec);
        Self {
            source: request.source.clone(),
            pixels,
            spec,
            renderer,
        }
    }
    fn update(&mut self, request: &Request) {
        let pixels_changed = !Arc::ptr_eq(&self.source, &request.source)
            && (!Arc::ptr_eq(&self.source.base, &request.source.base)
                || self.source.marks != request.source.marks);
        if pixels_changed {
            self.pixels = request.source.render(None);
        }
        let spec = request.key.spec;
        if pixels_changed
            || !self.spec.same_render_layout(spec)
            || self.source.backdrop.is_none() != request.source.backdrop.is_none()
        {
            self.renderer = Self::renderer(&request.source, &self.pixels, spec);
        } else {
            // Timing/effect edits keep the expensive annotated card, resize and
            // shadow cache. They still change the worker generation via Spec.
            self.renderer.image_animation = spec.animation;
            self.renderer.b.seconds = spec.backdrop.seconds;
        }
        self.spec = spec;
        self.source = request.source.clone();
    }
}

pub struct CompositionPreview {
    worker: Worker,
    source: Option<(u64, Arc<Document>)>,
    image: Option<Arc<RenderImage>>,
    painted: Option<Key>,
    requested: Option<(Key, bool)>,
    quality: AdaptiveQuality,
}
impl CompositionPreview {
    pub fn new(notify: impl Fn() + Send + 'static) -> Self {
        let mut cached: Option<Prepared> = None;
        Self {
            worker: Worker::new(
                move |request| {
                    let spec = request.key.spec;
                    if let Some(prepared) = &mut cached {
                        prepared.update(request);
                    } else {
                        cached = Some(Prepared::new(request));
                    }
                    crate::editor::render_image(
                        cached
                            .as_ref()
                            .unwrap()
                            .renderer
                            .frame(request.key.tick as f32 / 30. / spec.animation.seconds as f32),
                    )
                },
                notify,
            ),
            source: None,
            image: None,
            painted: None,
            requested: None,
            quality: AdaptiveQuality::default(),
        }
    }
    #[cfg(test)]
    pub(crate) fn blocked_for_test(
        gate: std::sync::mpsc::Receiver<()>,
        notify: impl Fn() + Send + 'static,
    ) -> Self {
        Self {
            worker: Worker::new(
                move |_| {
                    let _ = gate.recv();
                    crate::editor::render_image(image::RgbaImage::new(2, 2))
                },
                notify,
            ),
            source: None,
            image: None,
            painted: None,
            requested: None,
            quality: AdaptiveQuality::default(),
        }
    }
    pub fn clear(&mut self, window: &mut Window) {
        self.worker.clear();
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
        self.source = None;
        self.painted = None;
        self.requested = None;
        self.quality = AdaptiveQuality::default();
    }
    pub fn ready_for(&self, revision: u64, seek: u64) -> bool {
        self.image.is_some()
            && self
                .painted
                .is_some_and(|key| key.spec.revision == revision && key.seek == seek)
    }
    pub fn source(&mut self, document: &Document, revision: u64) -> Arc<Document> {
        if self.source.as_ref().is_none_or(|(r, _)| *r != revision) {
            self.source = Some((revision, Arc::new(document.render_snapshot())));
        }
        self.source.as_ref().unwrap().1.clone()
    }
    pub fn paint(
        &mut self,
        time: f32,
        seek: u64,
        bounds: Bounds<Pixels>,
        playing: bool,
        window: &mut Window,
    ) {
        let Some((revision, document)) = self.source.clone() else {
            return;
        };
        let edge = (f32::from(bounds.size.width.max(bounds.size.height)) * window.scale_factor())
            .ceil()
            .max(2.) as u32;
        let spec = Spec {
            revision,
            edge: edge.min(self.quality.edge(playing)),
            backdrop: document.animation_backdrop(),
            animation: document.image_animation,
        };
        let mut key = Key {
            spec,
            tick: (time.max(0.) * 30.).round() as u32,
            seek,
        };
        if let Some((previous, false)) = self.requested
            && !playing
            && previous.spec == spec
            && previous.seek == seek
        {
            key.tick = previous.tick;
        }
        if self.requested.is_some_and(|(previous, _)| {
            previous.spec.revision != spec.revision
                || previous.spec.backdrop != spec.backdrop
                || previous.spec.animation != spec.animation
                || previous.seek != seek
        }) && let Some(image) = self.image.take()
        {
            let _ = window.drop_image(image);
        }
        self.requested = Some((key, playing));
        self.worker
            .request(key, self.source.as_ref().unwrap().1.clone(), playing);
        let completed = self.worker.shared.mailbox.lock().unwrap().completed.take();
        if let Some(completed) = completed {
            let generation = self.worker.shared.mailbox.lock().unwrap().generation;
            if completed.generation == generation
                && completed.key.spec == spec
                && completed.key.seek == seek
            {
                self.painted = Some(completed.key);
                if let Some(old) = self.image.replace(completed.image) {
                    let _ = window.drop_image(old);
                }
                if playing {
                    self.quality.observe(completed.elapsed);
                }
            }
        }
        if let Some(image) = &self.image {
            let _ = window.paint_image(bounds, px(0.).into(), image.clone(), 0, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    #[gpui::test]
    fn quality_changes_keep_the_painted_frame_but_revisions_clear_it(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::{prelude::*, *};
        use std::{cell::RefCell, rc::Rc};
        struct View(Rc<RefCell<CompositionPreview>>);
        impl Render for View {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let preview = self.0.clone();
                canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, window, _| {
                        preview.borrow_mut().paint(0.4, 0, bounds, true, window);
                    },
                )
                .size_full()
            }
        }
        let (release, gate) = mpsc::channel::<()>();
        let mut document = Document::new(image::RgbaImage::new(2, 2));
        document.image_animation.effect = super::super::Entrance::Tilt;
        let document = Arc::new(document);
        let image = crate::editor::render_image(image::RgbaImage::new(2, 2));
        let spec = Spec {
            revision: 0,
            edge: 960,
            backdrop: document.animation_backdrop(),
            animation: document.image_animation,
        };
        let mut preview = CompositionPreview {
            worker: Worker::new(
                move |_| {
                    let _ = gate.recv();
                    crate::editor::render_image(image::RgbaImage::new(2, 2))
                },
                || {},
            ),
            source: Some((0, document.clone())),
            image: Some(image.clone()),
            painted: Some(Key {
                spec,
                tick: 12,
                seek: 0,
            }),
            requested: Some((
                Key {
                    spec,
                    tick: 12,
                    seek: 0,
                },
                true,
            )),
            quality: AdaptiveQuality::default(),
        };
        for _ in 0..5 {
            preview.quality.observe(Duration::from_millis(30));
        }
        assert_eq!(preview.quality.edge(true), 720);
        let preview = Rc::new(RefCell::new(preview));
        let view = cx.add_window(|_, _| View(preview.clone()));
        let mut visual = VisualTestContext::from_window(*view, cx);
        visual.simulate_resize(size(px(1050.), px(600.)));
        visual.run_until_parked();
        assert!(preview.borrow().ready_for(0, 0));
        assert!(!preview.borrow().ready_for(0, 1));
        assert!(Arc::ptr_eq(
            preview.borrow().image.as_ref().unwrap(),
            &image
        ));
        view.update(&mut visual, |_, _, cx| {
            preview.borrow_mut().source = Some((1, document));
            cx.notify();
        })
        .unwrap();
        visual.run_until_parked();
        assert!(preview.borrow().image.is_none());
        assert!(!preview.borrow().ready_for(1, 0));
        drop(release);
    }

    #[test]
    fn timing_edits_reuse_prepared_pixels_but_image_edits_rebuild_them() {
        let mut document = Document::new(image::RgbaImage::from_pixel(
            64,
            40,
            image::Rgba([220, 30, 60, 255]),
        ));
        document.backdrop = Some(Backdrop {
            motion: super::super::Motion::Liquid,
            padding: 12,
            ..Default::default()
        });
        document.image_animation.effect = super::super::Entrance::Tilt;
        let request = |document: &Document, revision| Request {
            key: Key {
                spec: Spec {
                    revision,
                    edge: 96,
                    backdrop: document.animation_backdrop(),
                    animation: document.image_animation,
                },
                tick: 12,
                seek: revision,
            },
            generation: revision,
            source: Arc::new(document.render_snapshot()),
        };
        let mut prepared = Prepared::new(&request(&document, 0));
        let foreground = prepared.renderer.foreground.clone();
        document.image_animation.effect = super::super::Entrance::Pop;
        document.image_animation.seconds = 6;
        document.backdrop.as_mut().unwrap().seconds = 6;
        prepared.update(&request(&document, 1));
        assert!(Arc::ptr_eq(&prepared.renderer.foreground, &foreground));
        assert_eq!(prepared.renderer.image_animation, document.image_animation);
        assert_eq!(
            prepared.renderer.frame(0.1),
            Renderer::for_document(&document, Some(96)).frame(0.1)
        );

        document.marks.push(crate::document::Mark {
            style: Default::default(),
            tool: crate::document::Tool::Rectangle,
            curve: None,
            points: vec![(10., 10.), (40., 30.)],
            color: [20, 200, 100, 255],
            width: 3.,
            text: String::new(),
        });
        prepared.update(&request(&document, 2));
        assert!(!Arc::ptr_eq(&prepared.renderer.foreground, &foreground));
        assert_eq!(
            prepared.renderer.frame(0.5),
            Renderer::for_document(&document, Some(96)).frame(0.5)
        );

        let foreground = prepared.renderer.foreground.clone();
        document.base = Arc::new(image::RgbaImage::from_pixel(
            64,
            40,
            image::Rgba([30, 80, 220, 255]),
        ));
        prepared.update(&request(&document, 3));
        assert!(!Arc::ptr_eq(&prepared.renderer.foreground, &foreground));
        assert_eq!(
            prepared.renderer.frame(0.5),
            Renderer::for_document(&document, Some(96)).frame(0.5)
        );
    }

    #[test]
    fn worker_bounds_queue_and_rejects_frames_after_seek() {
        let (started, starts) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (notify, notifications) = mpsc::channel();
        let worker = Worker::new(
            move |r| {
                started.send(r.key).unwrap();
                gate.recv_timeout(Duration::from_secs(3)).unwrap();
                crate::editor::render_image(image::RgbaImage::new(2, 2))
            },
            move || {
                notify.send(()).unwrap();
            },
        );
        let doc = Arc::new(Document::new(image::RgbaImage::new(2, 2)));
        let key = Key {
            spec: Spec {
                revision: 0,
                edge: 2,
                backdrop: Backdrop::default(),
                animation: ImageAnimation::default(),
            },
            tick: 0,
            seek: 0,
        };
        worker.request(key, doc.clone(), true);
        assert_eq!(starts.recv_timeout(Duration::from_secs(3)).unwrap(), key);
        for tick in 1..100 {
            worker.request(Key { tick, ..key }, doc.clone(), true);
        }
        assert_eq!(
            worker
                .shared
                .mailbox
                .lock()
                .unwrap()
                .pending
                .as_ref()
                .unwrap()
                .key
                .tick,
            99
        );
        let sought = Key {
            tick: 15,
            seek: 1,
            ..key
        };
        worker.request(sought, doc.clone(), false);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(Duration::from_secs(3)).unwrap(), sought);
        assert!(notifications.try_recv().is_err());
        release.send(()).unwrap();
        notifications.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(
            worker
                .shared
                .mailbox
                .lock()
                .unwrap()
                .completed
                .as_ref()
                .unwrap()
                .key,
            sought
        );
        worker.clear();
        assert!(worker.shared.mailbox.lock().unwrap().completed.is_none());
    }
}
