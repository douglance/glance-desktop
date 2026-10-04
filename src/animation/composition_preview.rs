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

pub struct CompositionPreview {
    worker: Worker,
    source: Option<(u64, Arc<Document>)>,
    image: Option<Arc<RenderImage>>,
    requested: Option<(Key, bool)>,
    quality: AdaptiveQuality,
}
impl CompositionPreview {
    pub fn new(notify: impl Fn() + Send + 'static) -> Self {
        let mut cached: Option<(Spec, Renderer)> = None;
        Self {
            worker: Worker::new(
                move |request| {
                    let spec = request.key.spec;
                    if cached.as_ref().is_none_or(|(old, _)| *old != spec) {
                        cached = Some((
                            spec,
                            Renderer::for_document(&request.source, Some(spec.edge)),
                        ));
                    }
                    crate::editor::render_image(
                        cached
                            .as_ref()
                            .unwrap()
                            .1
                            .frame(request.key.tick as f32 / 30. / spec.animation.seconds as f32),
                    )
                },
                notify,
            ),
            source: None,
            image: None,
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
        self.requested = None;
        self.quality = AdaptiveQuality::default();
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
        if self
            .requested
            .is_some_and(|(previous, _)| previous.spec != spec || previous.seek != seek)
            && let Some(image) = self.image.take()
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
