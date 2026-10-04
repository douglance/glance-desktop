//! Preview-only quality control with warmup and separate down/up thresholds.
use std::time::Duration;

const EDGES: [u32; 3] = [960, 720, 480];
const SLOW_MS: f64 = 24.;
const FAST_MS: f64 = 8.;
const RECOVERY_FRAMES: u16 = 90;

pub(super) struct AdaptiveQuality {
    tier: usize,
    warmup: u8,
    average_ms: Option<f64>,
    slow_frames: u8,
    fast_frames: u16,
}
impl Default for AdaptiveQuality {
    fn default() -> Self {
        Self {
            tier: 0,
            warmup: 2,
            average_ms: None,
            slow_frames: 0,
            fast_frames: 0,
        }
    }
}
impl AdaptiveQuality {
    pub(super) fn edge(&self, playing: bool) -> u32 {
        if playing { EDGES[self.tier] } else { EDGES[0] }
    }
    pub(super) fn observe(&mut self, elapsed: Duration) -> bool {
        // Pipeline compilation and buffer allocation are one-time costs.
        if self.warmup > 0 {
            self.warmup -= 1;
            return false;
        }
        let ms = elapsed.as_secs_f64() * 1000.;
        let average = self.average_ms.map_or(ms, |old| old * 0.8 + ms * 0.2);
        self.average_ms = Some(average);
        if average > SLOW_MS {
            self.slow_frames = self.slow_frames.saturating_add(1);
        } else {
            self.slow_frames = 0;
        }
        if average < FAST_MS && ms < 12. {
            self.fast_frames = self.fast_frames.saturating_add(1);
        } else {
            self.fast_frames = 0;
        }
        let next = if (self.slow_frames >= 3 || ms > 80.) && self.tier < EDGES.len() - 1 {
            self.tier + 1
        } else if self.fast_frames >= RECOVERY_FRAMES && self.tier > 0 {
            self.tier - 1
        } else {
            return false;
        };
        *self = Self {
            tier: next,
            ..Self::default()
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(quality: &mut AdaptiveQuality, ms: u64, frames: usize) {
        for _ in 0..frames {
            quality.observe(Duration::from_millis(ms));
        }
    }
    #[test]
    fn initialization_and_isolated_spikes_do_not_reduce_quality() {
        let mut quality = AdaptiveQuality::default();
        sample(&mut quality, 200, 2);
        sample(&mut quality, 3, 10);
        sample(&mut quality, 40, 1);
        sample(&mut quality, 3, 10);
        assert_eq!(quality.edge(true), 960);
    }
    #[test]
    fn sustained_slow_frames_step_down_and_respect_the_floor() {
        let mut quality = AdaptiveQuality::default();
        sample(&mut quality, 30, 4);
        assert_eq!(quality.edge(true), 960);
        sample(&mut quality, 30, 1);
        assert_eq!(quality.edge(true), 720);
        sample(&mut quality, 30, 5);
        assert_eq!(quality.edge(true), 480);
        sample(&mut quality, 200, 100);
        assert_eq!(quality.edge(true), 480);
        assert_eq!(quality.edge(false), 960);
    }
    #[test]
    fn very_slow_rendering_steps_down_after_warmup() {
        let mut quality = AdaptiveQuality::default();
        sample(&mut quality, 100, 2);
        assert_eq!(quality.edge(true), 960);
        sample(&mut quality, 100, 1);
        assert_eq!(quality.edge(true), 720);
    }
    #[test]
    fn recovery_requires_sustained_headroom_and_does_not_oscillate() {
        let mut quality = AdaptiveQuality::default();
        sample(&mut quality, 40, 5);
        assert_eq!(quality.edge(true), 720);
        sample(&mut quality, 4, 91);
        assert_eq!(quality.edge(true), 720);
        sample(&mut quality, 4, 1);
        assert_eq!(quality.edge(true), 960);
        // Upscaling 720 -> 960 increases pixel work ~1.78x; 4ms -> 7ms
        // remains well inside the separate 24ms downshift threshold.
        sample(&mut quality, 7, 120);
        assert_eq!(quality.edge(true), 960);
    }
    #[test]
    fn borderline_samples_neither_recover_nor_flap() {
        let mut quality = AdaptiveQuality::default();
        sample(&mut quality, 30, 5);
        for _ in 0..120 {
            sample(&mut quality, 7, 1);
            sample(&mut quality, 16, 1);
        }
        assert_eq!(quality.edge(true), 720);
    }
}
