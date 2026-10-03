//! Local resizing: alpha-aware Lanczos and bounded, edge-adaptive sharpening.
use image::{
    RgbaImage,
    imageops::{self, FilterType},
};

pub fn dimensions(source: (u32, u32), scale: f32) -> Result<(u32, u32), String> {
    if !scale.is_finite() || !(0.1..=4.).contains(&scale) {
        return Err("Choose a scale between 10% and 400%.".into());
    }
    let w = (source.0 as f32 * scale).round().max(1.) as u32;
    let h = (source.1 as f32 * scale).round().max(1.) as u32;
    if w > 16000 || h > 16000 || u64::from(w) * u64::from(h) > 64_000_000 {
        return Err("The resized image exceeds 16,000 pixels per side or 64 megapixels. Choose a smaller scale.".into());
    }
    Ok((w, h))
}
pub fn resize(source: &RgbaImage, target: (u32, u32), smart: bool) -> RgbaImage {
    // Filter premultiplied colors to prevent dark/colored fringes around transparency.
    let mut premultiplied = source.clone();
    for p in premultiplied.pixels_mut() {
        for c in 0..3 {
            p[c] = (u16::from(p[c]) * u16::from(p[3]) / 255) as u8;
        }
    }
    let mut out = imageops::resize(&premultiplied, target.0, target.1, FilterType::Lanczos3);
    for p in out.pixels_mut() {
        for c in 0..3 {
            p[c] = if p[3] == 0 {
                0
            } else {
                (u32::from(p[c]) * 255 / u32::from(p[3])).min(255) as u8
            };
        }
    }
    if smart && (target.0 > source.width() || target.1 > source.height()) {
        let blur = imageops::blur(&out, 0.85);
        for (p, low) in out.pixels_mut().zip(blur.pixels()) {
            if p[3] < 250 {
                continue;
            }
            let detail = (0..3)
                .map(|c| (p[c] as f32 - low[c] as f32).abs())
                .fold(0., f32::max);
            // Flat regions stay flat; cap corrections to avoid halos and noise amplification.
            let amount = ((detail - 1.5) / 12.).clamp(0., 0.6);
            for c in 0..3 {
                let delta = ((p[c] as f32 - low[c] as f32) * amount).clamp(-10., 10.);
                p[c] = (p[c] as f32 + delta).round().clamp(0., 255.) as u8;
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    #[test]
    fn limits_and_flat_pixels_are_preserved() {
        assert_eq!(dimensions((100, 50), 2.).unwrap(), (200, 100));
        assert!(dimensions((16000, 16000), 4.).is_err());
        assert!(dimensions((10, 10), f32::NAN).is_err());
        let src = RgbaImage::from_pixel(4, 4, Rgba([70, 90, 110, 255]));
        assert!(
            resize(&src, (16, 16), true)
                .pixels()
                .all(|p| *p == Rgba([70, 90, 110, 255]))
        );
    }
    #[test]
    fn sharpening_is_bounded_and_alpha_edges_have_no_black_fringe() {
        let mut src = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        for x in 0..4 {
            for y in 0..8 {
                src.put_pixel(x, y, Rgba([60, 60, 60, 255]));
            }
        }
        let plain = resize(&src, (32, 32), false);
        let smart = resize(&src, (32, 32), true);
        assert_ne!(plain, smart);
        assert!(
            plain
                .pixels()
                .zip(smart.pixels())
                .all(|(a, b)| a[3] == b[3] && (0..3).all(|c| a[c].abs_diff(b[c]) <= 10))
        );
        let mut alpha = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 0]));
        alpha.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        assert!(
            resize(&alpha, (8, 8), true)
                .pixels()
                .filter(|p| p[3] > 0)
                .all(|p| p[0] >= 254)
        );
    }
}
