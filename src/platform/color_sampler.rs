//! Explicit, local screen sampling. AppKit must be called on the main thread.
#![allow(unexpected_cfgs)] // objc 0.2 macros use a legacy cargo-clippy feature.
type SampleResult = Result<Option<[u8; 3]>, String>;
pub fn sample_screen_color() -> Result<async_channel::Receiver<SampleResult>, String> {
    let (sender, receiver) = async_channel::bounded(1);
    #[cfg(target_os = "macos")]
    unsafe {
        use cocoa::base::{id, nil};
        use objc::{class, msg_send, sel, sel_impl};
        let sampler: id = msg_send![class!(NSColorSampler), new];
        let handler = block::ConcreteBlock::new(move |color: id| {
            let result = if color == nil {
                Ok(None)
            } else {
                let space: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
                let color: id = msg_send![color, colorUsingColorSpace: space];
                if color == nil {
                    Err("The sampled color could not be converted to sRGB".into())
                } else {
                    let r: f64 = msg_send![color, redComponent];
                    let g: f64 = msg_send![color, greenComponent];
                    let b: f64 = msg_send![color, blueComponent];
                    Ok(Some(
                        [r, g, b].map(|v| (v.clamp(0., 1.) * 255.).round() as u8),
                    ))
                }
            };
            let _ = sender.try_send(result);
        })
        .copy();
        let _: () = msg_send![sampler, showSamplerWithSelectionHandler: &*handler];
        // AppKit retains the sampler and copies the handler until selection/cancel.
        let _: () = msg_send![sampler, release];
    }
    #[cfg(target_os = "linux")]
    {
        super::linux::permission()?;
        std::thread::spawn(move || {
            let result = std::process::Command::new("hyprpicker")
                .args(["--format", "hex", "--no-fancy"])
                .output()
                .map_err(|e| {
                    format!(
                        "Cannot run hyprpicker: {e}. Install hyprpicker for screen color sampling."
                    )
                })
                .and_then(|output| {
                    if !output.status.success() {
                        if output.stderr.is_empty() {
                            return Ok(None);
                        }
                        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
                    }
                    let text = String::from_utf8_lossy(&output.stdout);
                    if text.trim().is_empty() {
                        return Ok(None);
                    }
                    crate::color_picker::parse_hex(&text)
                        .map(Some)
                        .ok_or_else(|| "Screen sampler returned an invalid hex color".into())
                });
            let _ = sender.send_blocking(result);
        });
    }
    Ok(receiver)
}
