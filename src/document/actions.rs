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
    /// Atomic group edit, validated before changing history or any annotation.
    EditAnnotations {
        updates: Vec<(usize, Mark)>,
        additions: Vec<Mark>,
        deletions: Vec<usize>,
        remember: bool,
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
    SetImageAnimation {
        animation: crate::animation::ImageAnimation,
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
    SelectMany(Vec<usize>),
}
pub(crate) struct EditOutcome {
    pub(crate) changed: bool,
    pub(crate) reset_view: bool,
    pub(crate) selection: Selection,
}
pub(crate) fn validate_mark(mark: &Mark) -> Result<(), String> {
    mark.style.validate()?;
    if matches!(mark.tool, Tool::Select | Tool::Crop)
        || mark.points.is_empty()
        || mark.points.len() > 2000
        || !mark.width.is_finite()
        // Stored geometry scales with the image and can lie outside toolbar
        // settings after resizing. Keep it bounded and editable.
        || mark.width <= 0.
        || mark.width > 32768.
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
        Tool::Rectangle | Tool::Highlight | Tool::Pixelate | Tool::Spotlight | Tool::Magnifier
    ) && mark.points.len() != 2
    {
        return Err("This tool requires two endpoints".into());
    }
    if mark.tool == Tool::Arrow && !(2..=32).contains(&mark.points.len()) {
        return Err("Lines require 2..32 points".into());
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
            Self::EditAnnotations {
                updates,
                additions,
                mut deletions,
                remember,
            } => {
                let mut indices = std::collections::BTreeSet::new();
                for (index, mark) in &updates {
                    if *index >= document.marks.len() || !indices.insert(*index) {
                        return Err("Invalid or repeated annotation index".into());
                    }
                    validate_mark(mark)?;
                }
                for index in &deletions {
                    if *index >= document.marks.len() || !indices.insert(*index) {
                        return Err("Invalid or repeated annotation index".into());
                    }
                }
                for mark in &additions {
                    validate_mark(mark)?;
                }
                let count = document.marks.len() - deletions.len() + additions.len();
                if count > 500 {
                    return Err("Maximum 500 annotations".into());
                }
                let added = updates.iter().map(|(_, m)| m.points.len()).sum::<usize>()
                    + additions.iter().map(|m| m.points.len()).sum::<usize>();
                let removed = indices
                    .iter()
                    .map(|i| document.marks[*i].points.len())
                    .sum();
                check_points(document, added, removed)?;
                outcome.changed = !additions.is_empty()
                    || !deletions.is_empty()
                    || updates.iter().any(|(i, m)| document.marks[*i] != *m);
                if outcome.changed {
                    if remember {
                        document.remember();
                    }
                    for (index, mark) in updates {
                        document.marks[index] = mark;
                    }
                    deletions.sort_unstable();
                    for index in deletions.iter().rev() {
                        document.marks.remove(*index);
                    }
                    let first = document.marks.len();
                    document.marks.extend(additions);
                    if first < document.marks.len() {
                        outcome.selection =
                            Selection::SelectMany((first..document.marks.len()).collect());
                    } else if !deletions.is_empty() {
                        outcome.selection = Selection::Clear;
                    }
                }
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
                    style: Default::default(),
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
                let mut animation = document.image_animation;
                if let Some(b) = backdrop {
                    animation.seconds = b.seconds;
                    animation.validate()?;
                }
                outcome.changed = document.backdrop != backdrop;
                if outcome.changed {
                    document.remember();
                    document.backdrop = backdrop;
                    document.image_animation = animation;
                }
            }
            Self::SetImageAnimation { animation } => {
                animation.validate()?;
                outcome.changed = document.image_animation != animation;
                if outcome.changed {
                    document.remember();
                    document.image_animation = animation;
                    if let Some(b) = &mut document.backdrop {
                        b.seconds = animation.seconds;
                    }
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
    fn resized_annotation_sizes_remain_editable_and_invalid_resize_is_atomic() {
        let mut d = Document::new(image::RgbaImage::new(200, 200));
        DocumentAction::AddAnnotation {
            mark: Mark {
                style: crate::style::Style {
                    radius: 128.,
                    ..Default::default()
                },
                tool: Tool::Rectangle,
                points: vec![(20., 20.), (180., 180.)],
                curve: None,
                width: 32.,
                color: [255; 4],
                text: String::new(),
            },
        }
        .apply(&mut d)
        .unwrap();
        DocumentAction::Resize {
            scale: 4.,
            smart: false,
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.marks[0].width, 128.);
        assert_eq!(d.marks[0].style.radius, 512.);
        DocumentAction::MoveAnnotation {
            index: 0,
            delta: (1., 1.),
            remember: true,
        }
        .apply(&mut d)
        .unwrap();
        let mut mark = d.marks[0].clone();
        mark.color = [255, 0, 0, 128];
        DocumentAction::UpdateAnnotation { index: 0, mark }
            .apply(&mut d)
            .unwrap();
        d.undo();
        d.undo();
        assert_eq!(d.marks[0].points[0], (80., 80.));
        d.undo();
        assert_eq!(d.marks[0].style.radius, 128.);
        assert_eq!(d.base.dimensions(), (200, 200));
        // Repeated downsizing can legitimately produce sub-toolbar widths.
        DocumentAction::Resize {
            scale: 0.1,
            smart: false,
        }
        .apply(&mut d)
        .unwrap();
        DocumentAction::Resize {
            scale: 0.1,
            smart: false,
        }
        .apply(&mut d)
        .unwrap();
        DocumentAction::MoveAnnotation {
            index: 0,
            delta: (0.1, 0.1),
            remember: true,
        }
        .apply(&mut d)
        .unwrap();
        let mut far = d.marks[0].clone();
        far.points = vec![(20000., 10.), (21000., 20.)];
        DocumentAction::UpdateAnnotation {
            index: 0,
            mark: far,
        }
        .apply(&mut d)
        .unwrap();
        let base = d.base.clone();
        let marks = d.marks.clone();
        let undo = d.undo.len();
        assert!(
            DocumentAction::Resize {
                scale: 4.,
                smart: false
            }
            .apply(&mut d)
            .is_err()
        );
        assert!(std::sync::Arc::ptr_eq(&base, &d.base));
        assert_eq!(d.marks, marks);
        assert_eq!(d.undo.len(), undo);
    }
    #[test]
    fn animated_export_without_backdrop_keeps_source_alpha_and_history() {
        let source = image::RgbaImage::from_pixel(25, 19, image::Rgba([40, 150, 180, 128]));
        let mut d = Document::new(source.clone());
        DocumentAction::SetImageAnimation {
            animation: crate::animation::ImageAnimation {
                effect: crate::animation::Entrance::Pop,
                ..Default::default()
            },
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.export_at(0.5), source);
        assert!(d.export_at(0.).pixels().all(|p| p[3] == 0));
        d.undo();
        assert!(!d.image_animation.enabled());
        d.redo();
        assert!(d.image_animation.enabled());
        assert_eq!(d.render_snapshot().image_animation, d.image_animation);
    }
    #[test]
    fn group_edits_validate_every_member_before_changing_document_or_history() {
        let mut document = Document::new(image::RgbaImage::new(100, 100));
        let mark = Mark {
            tool: Tool::Rectangle,
            points: vec![(10., 10.), (20., 20.)],
            color: [255; 4],
            width: 2.,
            text: String::new(),
            curve: None,
            style: Default::default(),
        };
        document.commit(mark.clone());
        document.commit(mark.clone());
        let original = document.marks.clone();
        let undo = document.undo.len();
        let mut moved = mark.clone();
        moved.translate(10., 10.);
        let mut invalid = mark.clone();
        invalid.points[0].0 = f32::INFINITY;
        for edit in [
            DocumentAction::EditAnnotations {
                updates: vec![(0, moved.clone()), (1, invalid)],
                additions: Vec::new(),
                deletions: Vec::new(),
                remember: true,
            },
            DocumentAction::EditAnnotations {
                updates: vec![(0, moved.clone())],
                additions: Vec::new(),
                deletions: vec![0],
                remember: true,
            },
            DocumentAction::EditAnnotations {
                updates: Vec::new(),
                additions: vec![mark; 499],
                deletions: Vec::new(),
                remember: true,
            },
        ] {
            assert!(edit.apply(&mut document).is_err());
            assert_eq!(document.marks, original);
            assert_eq!(document.undo.len(), undo);
        }
        DocumentAction::EditAnnotations {
            updates: vec![(0, moved.clone()), (1, moved)],
            additions: Vec::new(),
            deletions: Vec::new(),
            remember: true,
        }
        .apply(&mut document)
        .unwrap();
        assert_eq!(document.undo.len(), undo + 1);
        document.undo();
        assert_eq!(document.marks, original);
    }
    #[test]
    fn invalid_edits_preserve_document_and_history() {
        let mut document = Document::new(image::RgbaImage::new(100, 80));
        let mark = Mark {
            style: Default::default(),
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
