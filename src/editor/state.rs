//! State owned by the editor, grouped by responsibility.
use super::feedback::CopyFeedback;
use crate::{
    arrow,
    backdrop::Control,
    document::{Mark, Tool},
    navigation, text,
};
use gpui::{Bounds, Pixels, Point, RenderImage, Task};
use std::{cell::Cell, rc::Rc, sync::Arc};
#[derive(Clone, Copy, Default)]
pub(crate) struct Layout {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) scale: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}
pub(super) struct InteractionState {
    pub(super) gesture: Gesture,
    pub(super) selected: Option<usize>,
    /// Other members of the selection; selected remains the primary inspector target.
    pub(super) selected_others: Vec<usize>,
    pub(super) tool: Tool,
    pub(super) color: [u8; 4],
    pub(super) width: f32,
    pub(super) defaults: [ToolSettings; 11],
    pub(super) crop_ratio: Option<f32>,
    pub(super) next_counter: Option<u32>,
    pub(super) text_edit: Option<text::Edit>,
    pub(super) text_session: u64,
}
pub(super) struct ViewportState {
    pub(super) canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) space_down: bool,
    pub(super) zoom_down: bool,
    pub(super) zoom: Option<f32>,
    pub(super) pan: (f32, f32),
    pub(super) layout: Rc<Cell<Layout>>,
}
pub(super) struct PreviewState {
    pub(super) inside_padding: u32,
    pub(super) lens: Option<(crate::effects::LensKey, Arc<RenderImage>)>,
    pub(super) lens_wanted: Option<crate::effects::LensKey>,
    pub(super) lens_rendering: bool,
    pub(super) revision: u64,
    pub(super) mark_count: usize,
    pub(super) rendering: bool,
    pub(super) waiting: bool,
    pub(super) image: Arc<RenderImage>,
    pub(super) retired: Vec<Arc<RenderImage>>,
}
pub(super) struct PlaybackState {
    pub(super) motion_preview: Rc<std::cell::RefCell<crate::animation::Preview>>,
    pub(super) composition_preview: Rc<std::cell::RefCell<crate::animation::CompositionPreview>>,
    pub(super) seek: u64,
    pub(super) epoch: std::time::Instant,
    pub(super) paused: bool,
    pub(super) preparing: bool,
    pub(super) position: f32,
}
pub(super) struct VideoExportState {
    pub(super) last_video: Option<std::path::PathBuf>,
    pub(super) progress: Option<u32>,
    pub(super) cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}
pub(super) struct PanelState {
    pub(super) sampling_color: Option<usize>,
    pub(super) sampling_tool_color: bool,
    pub(super) backdrop_disabled: Option<crate::backdrop::Backdrop>,
    pub(super) popup: Option<super::panels::Popup>,
    pub(super) popup_index: usize,
    pub(super) backdrop: bool,
    pub(super) enhance: bool,
    pub(super) animation: bool,
    pub(super) resize_scale: f32,
    pub(super) resize_smart: bool,
}
pub(super) struct FeedbackState {
    pub(super) status: String,
    pub(super) copy: Option<CopyFeedback>,
    pub(super) timer: Option<Task<()>>,
}

/// A pointer can perform only one canvas or panel gesture at a time.
#[derive(Default)]
pub(super) enum Gesture {
    #[default]
    Idle,
    Drawing(Mark),
    MovingAnnotation(AnnotationDrag),
    MovingSelection(Vec<AnnotationDrag>),
    Selecting {
        origin: (f32, f32),
        current: (f32, f32),
        additive: bool,
        previous: Vec<usize>,
    },
    EditingArrow {
        drag: AnnotationDrag,
        handle: usize,
    },
    Panning(Point<Pixels>),
    AdjustingBackdrop(Control, Bounds<Pixels>),
    AdjustingAnimation(crate::animation::AnimationControl, Bounds<Pixels>),
}
pub(super) struct AnnotationDrag {
    pub(super) index: usize,
    pub(super) origin: (f32, f32),
    pub(super) original: Mark,
    pub(super) moved: Mark,
}
impl AnnotationDrag {
    pub(super) fn update(&mut self, point: (f32, f32), shift: bool, handle: Option<usize>) {
        self.moved = self.original.clone();
        let delta = navigation::translation(self.origin, point, shift && handle.is_none());
        arrow::drag(&mut self.moved, handle, delta, shift);
    }
}
impl Gesture {
    pub(super) fn is_active(&self) -> bool {
        !matches!(self, Self::Idle)
    }
    pub(super) fn drag(&self) -> Option<(&AnnotationDrag, Option<usize>)> {
        match self {
            Self::MovingAnnotation(drag) => Some((drag, None)),
            Self::EditingArrow { drag, handle } => Some((drag, Some(*handle))),
            _ => None,
        }
    }
    pub(super) fn first_drag_index(&self) -> Option<usize> {
        match self {
            Self::MovingSelection(drags) => drags.iter().map(|d| d.index).min(),
            _ => self.drag().map(|(d, _)| d.index),
        }
    }
    pub(super) fn moved_mark(&self, index: usize) -> Option<&Mark> {
        match self {
            Self::MovingSelection(drags) => {
                drags.iter().find(|d| d.index == index).map(|d| &d.moved)
            }
            _ => self
                .drag()
                .filter(|(d, _)| d.index == index)
                .map(|(d, _)| &d.moved),
        }
    }
    pub(super) fn draft(&self) -> Option<&Mark> {
        if let Self::Drawing(mark) = self {
            Some(mark)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ToolSettings {
    pub(super) color: [u8; 4],
    pub(super) width: f32,
    pub(super) style: crate::style::Style,
    pub(super) magnification: f32,
}
impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            color: [255, 56, 100, 255],
            width: 5.,
            style: Default::default(),
            magnification: 2.,
        }
    }
}
