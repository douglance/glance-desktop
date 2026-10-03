//! State owned by the editor, grouped by responsibility.
use super::*;
#[derive(Clone, Copy, Default)]
pub(crate) struct Layout {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) scale: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}
pub(super) struct InteractionState {
    pub(super) selected: Option<usize>,
    pub(super) drag_handle: Option<usize>,
    pub(super) object_drag: Option<(usize, (f32, f32), Mark)>,
    pub(super) tool: Tool,
    pub(super) color: [u8; 4],
    pub(super) width: f32,
    pub(super) draft: Option<Mark>,
    pub(super) text_edit: Option<text::Edit>,
    pub(super) text_session: u64,
}
pub(super) struct ViewportState {
    pub(super) canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) space_down: bool,
    pub(super) zoom_down: bool,
    pub(super) zoom: Option<f32>,
    pub(super) pan: (f32, f32),
    pub(super) pan_start: Option<Point<Pixels>>,
    pub(super) layout: Rc<Cell<Layout>>,
}
pub(super) struct PreviewState {
    pub(super) revision: u64,
    pub(super) mark_count: usize,
    pub(super) rendering: bool,
    pub(super) waiting: bool,
    pub(super) image: Arc<RenderImage>,
    pub(super) retired: Vec<Arc<RenderImage>>,
}
pub(super) struct PlaybackState {
    pub(super) epoch: std::time::Instant,
    pub(super) paused: bool,
    pub(super) position: f32,
}
pub(super) struct VideoExportState {
    pub(super) last_video: Option<std::path::PathBuf>,
    pub(super) progress: Option<u32>,
    pub(super) cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}
pub(super) struct PanelState {
    pub(super) backdrop: bool,
    pub(super) enhance: bool,
    pub(super) resize_scale: f32,
    pub(super) resize_smart: bool,
    pub(super) backdrop_drag: Option<(backdrop::Control, Bounds<Pixels>)>,
}
pub(super) struct FeedbackState {
    pub(super) status: String,
    pub(super) copy: Option<CopyFeedback>,
    pub(super) timer: Option<Task<()>>,
}
