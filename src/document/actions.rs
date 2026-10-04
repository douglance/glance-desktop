//! Shared document edits. The UI and MCP adapt their inputs to these values.
use super::{Document, Mark, Tool};
use crate::backdrop::{Backdrop, PRESETS};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DocumentAction {
    AddAnnotation {
        mark: Mark,
    },
    UpdateAnnotation {
        index: usize,
        mark: Mark,
    },
    MoveAnnotation {
        index: usize,
        delta: (f32, f32),
        #[serde(default = "remember_default")]
        remember: bool,
    },
    DeleteAnnotation {
        index: usize,
    },
    Crop {
        rectangle: (f32, f32, f32, f32),
    },
    Resize {
        scale: f32,
        smart: bool,
    },
    Rotate,
    SetBackdrop {
        backdrop: Option<Backdrop>,
    },
    Undo,
    Redo,
}
fn remember_default() -> bool {
    true
}

pub(crate) enum Selection {
    Keep,
    Clear,
    Select(usize),
}
pub(crate) struct EditOutcome {
    pub(crate) changed: bool,
    pub(crate) reset_view: bool,
    pub(crate) selection: Selection,
}
pub(crate) fn validate_mark(mark: &Mark) -> Result<(), String> {
    if matches!(mark.tool, Tool::Select | Tool::Crop)
        || mark.points.is_empty()
        || mark.points.len() > 2000
        || !mark.width.is_finite()
        || !(0.5..=64.).contains(&mark.width)
        || mark.text.len() > 8000
    {
        return Err("Invalid mark tool, points, width or text".into());
    }
    if mark
        .points
        .iter()
        .chain(mark.curve.iter())
        .any(|p| !p.0.is_finite() || !p.1.is_finite() || p.0.abs() > 32768. || p.1.abs() > 32768.)
    {
        return Err("Invalid mark coordinates".into());
    }
    let length: f32 = mark
        .points
        .windows(2)
        .map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1))
        .sum();
    if length > 100_000. {
        return Err("Drawing path too long".into());
    }
    if matches!(
        mark.tool,
        Tool::Arrow
            | Tool::Rectangle
            | Tool::Highlight
            | Tool::Pixelate
            | Tool::Spotlight
            | Tool::Magnifier
    ) && mark.points.len() != 2
    {
        return Err("This tool requires two endpoints".into());
    }
    Ok(())
}
impl DocumentAction {
    /// Validation happens before history or document mutation, including moves.
    pub(crate) fn apply(self, document: &mut Document) -> Result<EditOutcome, String> {
        let mut outcome = EditOutcome {
            changed: true,
            reset_view: false,
            selection: Selection::Keep,
        };
        match self {
            Self::AddAnnotation { mark } => {
                validate_mark(&mark)?;
                if document.marks.len() >= 500 {
                    return Err("Maximum 500 annotations".into());
                }
                check_points(document, mark.points.len(), 0)?;
                document.commit(mark);
                outcome.selection = Selection::Select(document.marks.len() - 1);
            }
            Self::UpdateAnnotation { index, mark } => {
                let old = document
                    .marks
                    .get(index)
                    .ok_or("Annotation no longer exists")?;
                validate_mark(&mark)?;
                check_points(document, mark.points.len(), old.points.len())?;
                outcome.changed = old != &mark;
                if outcome.changed {
                    document.remember();
                    document.marks[index] = mark;
                }
                outcome.selection = Selection::Select(index);
            }
            Self::MoveAnnotation {
                index,
                delta,
                remember,
            } => {
                let mut mark = document
                    .marks
                    .get(index)
                    .ok_or("Annotation no longer exists")?
                    .clone();
                mark.translate(delta.0, delta.1);
                validate_mark(&mark)?;
                outcome.changed = delta != (0., 0.);
                if outcome.changed {
                    if remember {
                        document.remember();
                    }
                    document.marks[index] = mark;
                }
                outcome.selection = Selection::Select(index);
            }
            Self::DeleteAnnotation { index } => {
                if index >= document.marks.len() {
                    return Err("Annotation no longer exists".into());
                }
                document.delete_mark(index);
                outcome.selection = Selection::Clear;
            }
            Self::Crop {
                rectangle: (x, y, width, height),
            } => {
                if ![x, y, width, height].iter().all(|n| n.is_finite())
                    || x < 0.
                    || y < 0.
                    || width < 2.
                    || height < 2.
                    || x + width > document.base.width() as f32
                    || y + height > document.base.height() as f32
                {
                    return Err("Crop must lie inside the source image and be at least 2×2".into());
                }
                document.commit(Mark {
                    tool: Tool::Crop,
                    curve: None,
                    points: vec![(x, y), (x + width, y + height)],
                    color: [0; 4],
                    width: 1.,
                    text: String::new(),
                });
                outcome.reset_view = true;
                outcome.selection = Selection::Clear;
            }
            Self::Resize { scale, smart } => {
                if !scale.is_finite() || !(0.1..=4.).contains(&scale) {
                    return Err("Resize scale must be 0.1..4".into());
                }
                document.resize(scale, smart)?;
                outcome.reset_view = true;
                outcome.selection = Selection::Clear;
            }
            Self::Rotate => {
                document.rotate();
                outcome.reset_view = true;
                outcome.selection = Selection::Clear;
            }
            Self::SetBackdrop { backdrop } => {
                if let Some(b) = backdrop
                    && (b.preset >= PRESETS.len()
                        || b.padding > 512
                        || b.inner_radius > 256
                        || b.inside_padding > 512
                        || b.shadow > 128
                        || !(2..=15).contains(&b.seconds))
                {
                    return Err("Backdrop values out of range".into());
                }
                outcome.changed = document.backdrop != backdrop;
                if outcome.changed {
                    document.remember();
                    document.backdrop = backdrop;
                }
            }
            Self::Undo => {
                outcome.changed = !document.undo.is_empty();
                document.undo();
                outcome.selection = Selection::Clear;
            }
            Self::Redo => {
                outcome.changed = !document.redo.is_empty();
                document.redo();
                outcome.selection = Selection::Clear;
            }
        }
        Ok(outcome)
    }
}
fn check_points(document: &Document, added: usize, removed: usize) -> Result<(), String> {
    if document.marks.iter().map(|m| m.points.len()).sum::<usize>() - removed + added > 10000 {
        return Err("Maximum 10000 drawing points".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_edits_preserve_document_and_history() {
        let mut document = Document::new(image::RgbaImage::new(100, 80));
        let mark = Mark {
            tool: Tool::Arrow,
            points: vec![(10., 10.), (50., 40.)],
            curve: Some((30., 5.)),
            color: [255; 4],
            width: 3.,
            text: String::new(),
        };
        DocumentAction::AddAnnotation { mark: mark.clone() }
            .apply(&mut document)
            .unwrap();
        for edit in [
            DocumentAction::MoveAnnotation {
                index: 0,
                delta: (f32::NAN, 0.),
                remember: true,
            },
            DocumentAction::DeleteAnnotation { index: 9 },
            DocumentAction::Crop {
                rectangle: (0., 0., f32::INFINITY, 20.),
            },
            DocumentAction::SetBackdrop {
                backdrop: Some(Backdrop {
                    preset: PRESETS.len(),
                    ..Default::default()
                }),
            },
        ] {
            assert!(edit.apply(&mut document).is_err());
            assert_eq!(document.marks, vec![mark.clone()]);
            assert_eq!(document.base.dimensions(), (100, 80));
            assert!(document.backdrop.is_none());
        }
        DocumentAction::Undo.apply(&mut document).unwrap();
        assert!(document.marks.is_empty());
    }
}
