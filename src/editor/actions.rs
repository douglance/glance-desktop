//! Application intent, independent of buttons, key events, GPUI, and transports.
use crate::{
    animation::Motion,
    backdrop::{Backdrop, Control},
    document::Tool,
};
use std::path::PathBuf;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    Edit {
        edit: crate::document::actions::DocumentAction,
    },
    /// Prepared by a worker; optimistic revision checking prevents lost edits.
    #[serde(skip)]
    ApplyPreparedDocument {
        document: Box<crate::document::Document>,
        revision: u64,
        replace: bool,
    },
    Show,
    Capture {
        area: bool,
    },
    OpenImage,
    OpenPath {
        path: PathBuf,
    },
    SaveImage,
    CopyImage,
    CopyRemote,
    PasteImage,
    // Contextual editing commands operate on text while an inline edit is open.
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Delete,
    DuplicateSelection,
    SelectTool {
        tool: Tool,
    },
    SetColor {
        color: [u8; 4],
    },
    SetStrokeWidth {
        width: f32,
    },
    CycleStrokeWidth,
    CycleMagnifierZoom,
    NudgeSelection {
        delta: (f32, f32),
        remember: bool,
    },
    Fit,
    ActualSize,
    Zoom {
        factor: f32,
    },
    ZoomAt {
        factor: f32,
        anchor: (f32, f32),
    },
    ToggleBackdrop,
    ToggleEnhance,
    ClosePanel {
        panel: Panel,
    },
    SetResizeScale {
        scale: f32,
    },
    ToggleSmartResize,
    ApplyResize,
    Resize {
        scale: f32,
        smart: bool,
    },
    Rotate,
    SetBackdrop {
        backdrop: Option<Backdrop>,
    },
    SetBackdropFill {
        gradient: bool,
    },
    SelectMotion {
        motion: Motion,
    },
    SetBackdropPreset {
        preset: usize,
    },
    SetBackdropControl {
        control: Control,
        value: u32,
    },
    BeginBackdropAdjustment {
        control: Control,
        track: (f32, f32, f32, f32),
        position: (f32, f32),
    },
    TogglePlayback,
    ExportAnimation {
        format: AnimationFormat,
    },
    CancelExport,
    RevealExport,
    CommitText,
    Cancel,
    Help,
    Quit,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Panel {
    Backdrop,
    Enhance,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnimationFormat {
    Mp4,
    Gif,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct ActionReceipt {
    pub(crate) revision: u64,
    /// Accepted background work has started; this is not a completion receipt.
    pub(crate) operation_id: Option<u64>,
}
