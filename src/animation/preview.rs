//! Latest-frame mailbox: shader execution and image conversion stay off the UI thread.
use super::Motion;
use crate::backdrop::Backdrop;
use gpui::{Bounds, Pixels, RenderImage, Window};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
pub(super) mod quality;
use quality::AdaptiveQuality;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spec {
    width: u32,
    height: u32,
    preset: usize,
    seed: u32,
    colors: Option<[[u8; 3]; 2]>,
    frames: u32,
    motion: Motion,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    spec: Spec,
    tick: u32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Signature {
    spec: Spec,
    frozen_tick: Option<u32>,
}
#[derive(Clone, Copy)]
struct Request {
    key: Key,
    generation: u64,
}
struct Completed {
    request: Request,
    image: Arc<RenderImage>,
    render_time: Duration,
}
#[derive(Default)]
struct Mailbox {
    signature: Option<Signature>,
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
        mut render: impl FnMut(Key) -> Arc<RenderImage> + Send + 'static,
        notify: impl Fn() + Send + 'static,
    ) -> Self {
        let shared = Arc::new(Shared::default());
        let thread_state = shared.clone();
        std::thread::Builder::new()
            .name("motion-preview".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut mailbox = thread_state.mailbox.lock().unwrap();
                        while !mailbox.stopped && mailbox.pending.is_none() {
                            mailbox = thread_state.wake.wait(mailbox).unwrap();
                        }
                        if mailbox.stopped {
                            break;
                        }
                        mailbox.pending.take().unwrap()
                    };
                    // Persistent thread keeps Metal's thread-local pipeline and
                    // output buffer alive. No mailbox lock is held while rendering.
                    let start = Instant::now();
                    let image = render(request.key);
                    let render_time = start.elapsed();
                    let published = {
                        let mut mailbox = thread_state.mailbox.lock().unwrap();
                        if !mailbox.stopped && mailbox.generation == request.generation {
                            mailbox.completed = Some(Completed {
                                request,
                                image,
                                render_time,
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
            .expect("start motion preview worker");
        Self { shared }
    }
    fn request(&self, key: Key, playing: bool) {
        let signature = Signature {
            spec: key.spec,
            frozen_tick: (!playing).then_some(key.tick),
        };
        let mut mailbox = self.shared.mailbox.lock().unwrap();
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
        // Replace rather than enqueue: one running frame and at most one
        // latest request, irrespective of display refresh or GPU speed.
        mailbox.pending = Some(Request {
            key,
            generation: mailbox.generation,
        });
        self.shared.wake.notify_one();
    }
    fn take_completed(&self) -> Option<Completed> {
        self.shared.mailbox.lock().unwrap().completed.take()
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
        // Never join here: a GPU command may still be finishing. The thread
        // exits after that command without retaining the editor or window.
    }
}

/// Only image painting and atlas retirement run on the UI thread.
pub struct Preview {
    worker: Worker,
    image: Option<Arc<RenderImage>>,
    requested: Option<(Key, bool)>,
    base: Option<Spec>,
    quality: AdaptiveQuality,
}
impl Preview {
    pub fn new(notify: impl Fn() + Send + 'static) -> Self {
        Self {
            worker: Worker::new(
                |key| {
                    crate::editor::render_image(crate::motion_shader::frame_with_colors(
                        key.spec.width,
                        key.spec.height,
                        key.spec.preset,
                        key.spec.motion,
                        key.tick as f32 / key.spec.frames as f32,
                        key.spec.colors,
                        key.spec.seed,
                    ))
                },
                notify,
            ),
            image: None,
            requested: None,
            base: None,
            quality: AdaptiveQuality::default(),
        }
    }
    fn retire_image(&mut self, window: &mut Window) {
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
    }
    pub fn clear(&mut self, window: &mut Window) {
        self.worker.clear();
        self.retire_image(window);
        self.requested = None;
        self.base = None;
        self.quality = AdaptiveQuality::default();
    }
    pub fn suspend(&mut self) {
        self.worker.clear();
        self.requested = None;
    }
    pub fn paint_cached(&self, b: Backdrop, bounds: Bounds<Pixels>, window: &mut Window) {
        if self.base.is_some_and(|base| {
            base.motion == b.motion
                && base.preset == b.preset
                && base.seed == b.seed
                && base.colors == b.colors
        }) && let Some(image) = &self.image
        {
            let _ = window.paint_image(bounds, gpui::px(0.).into(), image.clone(), 0, false);
        }
    }
    fn select_key(&mut self, spec: Spec, tick: u32, playing: bool) -> Key {
        let key = match self.requested {
            // A busy/inactive window's clock may still advance. Completion
            // notifications must not start another frame while suspended.
            Some((previous, false)) if !playing && previous.spec == spec => previous,
            _ => Key { spec, tick },
        };
        self.requested = Some((key, playing));
        key
    }
    pub(super) fn paint(
        &mut self,
        b: Backdrop,
        phase: f32,
        bounds: Bounds<Pixels>,
        radius: Pixels,
        playing: bool,
        window: &mut Window,
    ) {
        let w = f32::from(bounds.size.width) * window.scale_factor();
        let h = f32::from(bounds.size.height) * window.scale_factor();
        let dimensions = |edge: u32| {
            let scale = (edge as f32 / w.max(h)).min(1.);
            (
                (w * scale).round().max(1.) as u32,
                (h * scale).round().max(1.) as u32,
            )
        };
        let (width, height) = dimensions(960);
        let base = Spec {
            width,
            height,
            preset: b.preset,
            seed: b.seed,
            colors: b.colors,
            frames: b.seconds.max(2) * 30,
            motion: b.motion,
        };
        if self.base != Some(base) {
            self.retire_image(window);
            self.quality = AdaptiveQuality::default();
            self.base = Some(base);
        }
        let (width, height) = dimensions(self.quality.edge(playing));
        let spec = Spec {
            width,
            height,
            ..base
        };
        let tick = (phase.rem_euclid(1.) * spec.frames as f32).floor() as u32;
        let key = self.select_key(spec, tick, playing);
        self.worker.request(key, playing);
        if let Some(completed) = self.worker.take_completed() {
            debug_assert_eq!(completed.request.key.spec, spec);
            self.retire_image(window);
            self.image = Some(completed.image);
            if playing {
                self.quality.observe(completed.render_time);
            }
        }
        // Keep the previous valid frame visible while the next one renders.
        if let Some(image) = &self.image {
            let _ = window.paint_image(bounds, radius.into(), image.clone(), 0, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    const TIMEOUT: Duration = Duration::from_secs(3);
    fn key(tick: u32, motion: Motion) -> Key {
        Key {
            spec: Spec {
                width: 1,
                height: 1,
                preset: 1,
                seed: 0,
                colors: None,
                frames: 150,
                motion,
            },
            tick,
        }
    }
    fn blocked_worker() -> (
        Worker,
        mpsc::Receiver<Key>,
        mpsc::Sender<()>,
        mpsc::Receiver<()>,
    ) {
        let (started, starts) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (notify, notifications) = mpsc::channel();
        let worker = Worker::new(
            move |key| {
                started.send(key).unwrap();
                gate.recv_timeout(TIMEOUT).unwrap();
                crate::editor::render_image(image::RgbaImage::new(1, 1))
            },
            move || {
                let _ = notify.send(());
            },
        );
        (worker, starts, release, notifications)
    }
    #[test]
    fn real_shader_worker_publishes_matching_bgra_frames_off_the_caller_thread() {
        let caller = std::thread::current().id();
        let (notify, notifications) = mpsc::channel();
        let preview = Preview::new(move || {
            let _ = notify.send(std::thread::current().id());
        });
        for motion in Motion::EFFECTS.into_iter().filter(|m| m.uses_shader()) {
            let spec = Spec {
                width: 128,
                height: 72,
                preset: motion.suggested_preset().unwrap_or(0),
                seed: 0,
                colors: None,
                frames: 150,
                motion,
            };
            let key = Key { spec, tick: 37 };
            preview.worker.request(key, false);
            assert_ne!(notifications.recv_timeout(TIMEOUT).unwrap(), caller);
            let completed = preview.worker.take_completed().unwrap();
            assert_eq!(completed.request.key, key);
            let expected = crate::editor::render_image(crate::motion_shader::frame(
                spec.width,
                spec.height,
                spec.preset,
                motion,
                37. / 150.,
            ));
            assert_eq!(
                completed.image.as_bytes(0),
                expected.as_bytes(0),
                "{motion:?}"
            );
        }
    }
    #[test]
    fn seed_changes_reject_in_flight_frames_and_preserve_a_paused_tick() {
        let (worker, starts, release, notifications) = blocked_worker();
        let first = key(7, Motion::Prism);
        worker.request(first, false);
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), first);
        let mut changed = first;
        changed.spec.seed = 42;
        worker.request(changed, false);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), changed);
        assert!(worker.take_completed().is_none());
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(worker.take_completed().unwrap().request.key, changed);
    }

    #[test]
    fn slow_render_does_not_block_requests_and_only_latest_frame_is_queued() {
        let (worker, starts, release, notifications) = blocked_worker();
        let first = key(0, Motion::Liquid);
        worker.request(first, true);
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), first);
        for tick in 1..100 {
            worker.request(key(tick, Motion::Liquid), true);
        }
        let latest = key(99, Motion::Liquid);
        assert_eq!(
            worker.shared.mailbox.lock().unwrap().pending.unwrap().key,
            latest
        );
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(worker.take_completed().unwrap().request.key, first);
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), latest);
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(worker.take_completed().unwrap().request.key, latest);
        worker.request(latest, true);
        assert!(worker.shared.mailbox.lock().unwrap().pending.is_none());
    }
    #[test]
    fn changing_custom_colors_rejects_in_flight_palette_frame() {
        let (worker, starts, release, notifications) = blocked_worker();
        let old = key(0, Motion::Liquid);
        worker.request(old, false);
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), old);
        let mut colored = old;
        colored.spec.colors = Some([[255, 0, 0], [0, 0, 255]]);
        worker.request(colored, false);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), colored);
        assert!(worker.take_completed().is_none());
        assert!(notifications.try_recv().is_err());
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(worker.take_completed().unwrap().request.key, colored);
    }
    #[test]
    fn switching_away_and_back_rejects_the_old_generation() {
        let (worker, starts, release, notifications) = blocked_worker();
        let frame = key(0, Motion::Liquid);
        worker.request(frame, true);
        starts.recv_timeout(TIMEOUT).unwrap();
        worker.request(key(0, Motion::Prism), true);
        worker.request(frame, true);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), frame);
        assert!(notifications.try_recv().is_err());
        assert!(worker.take_completed().is_none());
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(worker.take_completed().unwrap().request.generation, 3);
    }
    #[test]
    fn pause_rejects_in_flight_playback_and_renders_the_frozen_phase_once() {
        let (worker, starts, release, notifications) = blocked_worker();
        worker.request(key(0, Motion::Liquid), true);
        starts.recv_timeout(TIMEOUT).unwrap();
        let frozen = key(4, Motion::Liquid);
        worker.request(frozen, false);
        worker.request(frozen, false);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), frozen);
        assert!(notifications.try_recv().is_err());
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(worker.take_completed().unwrap().request.key, frozen);
        worker.request(frozen, false);
        assert!(worker.shared.mailbox.lock().unwrap().pending.is_none());
    }
    #[test]
    fn composition_handoff_keeps_backdrop_and_rejects_in_flight_work() {
        let (worker, starts, release, notifications) = blocked_worker();
        let first = key(7, Motion::Liquid);
        worker.request(first, true);
        starts.recv_timeout(TIMEOUT).unwrap();
        let image = crate::editor::render_image(image::RgbaImage::new(1, 1));
        let mut preview = Preview {
            worker,
            image: Some(image.clone()),
            requested: Some((first, true)),
            base: Some(first.spec),
            quality: AdaptiveQuality::default(),
        };
        preview.suspend();
        assert!(Arc::ptr_eq(preview.image.as_ref().unwrap(), &image));
        assert!(
            preview
                .worker
                .shared
                .mailbox
                .lock()
                .unwrap()
                .pending
                .is_none()
        );
        let resumed = key(20, Motion::Liquid);
        preview.worker.request(resumed, false);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(TIMEOUT).unwrap(), resumed);
        assert!(preview.worker.take_completed().is_none());
        assert!(notifications.try_recv().is_err());
        release.send(()).unwrap();
        notifications.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(
            preview.worker.take_completed().unwrap().request.key,
            resumed
        );
    }
    #[test]
    fn suspended_redraws_hold_the_phase_until_playback_resumes() {
        let (worker, _, _, _) = blocked_worker();
        let mut preview = Preview {
            worker,
            image: None,
            requested: None,
            base: None,
            quality: AdaptiveQuality::default(),
        };
        let spec = key(0, Motion::Liquid).spec;
        assert_eq!(preview.select_key(spec, 0, true).tick, 0);
        assert_eq!(preview.select_key(spec, 4, false).tick, 4);
        for tick in 5..100 {
            assert_eq!(preview.select_key(spec, tick, false).tick, 4);
        }
        assert_eq!(preview.select_key(spec, 100, true).tick, 100);
        assert_eq!(preview.select_key(spec, 101, false).tick, 101);
        let changed = key(0, Motion::Prism).spec;
        assert_eq!(preview.select_key(changed, 102, false).tick, 102);
    }
    #[test]
    fn clear_and_drop_cancel_queued_work_without_waiting_for_renderer() {
        let (worker, starts, release, notifications) = blocked_worker();
        worker.request(key(0, Motion::Liquid), true);
        starts.recv_timeout(TIMEOUT).unwrap();
        worker.request(key(1, Motion::Liquid), true);
        worker.clear();
        assert!(worker.shared.mailbox.lock().unwrap().pending.is_none());
        drop(worker);
        release.send(()).unwrap();
        assert_eq!(
            notifications.recv_timeout(TIMEOUT),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
        assert!(starts.recv_timeout(TIMEOUT).is_err());
    }
}
