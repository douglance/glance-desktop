//! Native macOS text input on the GPUI canvas, including Unicode and IME ranges.
use crate::{Editor, Layout, document::Mark};
use gpui::{prelude::*, *};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Default)]
struct State {
    text: String,
    selection: Range<usize>,
    reversed: bool,
}
#[derive(Default)]
pub struct Buffer {
    state: State,
    pub marked: Option<Range<usize>>,
    undo: Vec<State>,
    redo: Vec<State>,
}
impl Buffer {
    pub fn text(&self) -> &str {
        &self.state.text
    }
    pub fn selection(&self) -> Range<usize> {
        self.state.selection.clone()
    }
    pub fn cursor(&self) -> usize {
        if self.state.reversed {
            self.state.selection.start
        } else {
            self.state.selection.end
        }
    }
    fn byte(&self, utf16: usize) -> usize {
        let mut count = 0;
        for (byte, c) in self.text().char_indices() {
            if count >= utf16 {
                return byte;
            }
            count += c.len_utf16();
        }
        self.text().len()
    }
    fn utf16(&self, byte: usize) -> usize {
        self.text()[..byte].encode_utf16().count()
    }
    fn utf16_to_byte_range(&self, range: Range<usize>) -> Range<usize> {
        self.byte(range.start)..self.byte(range.end)
    }
    fn byte_to_utf16_range(&self, range: Range<usize>) -> Range<usize> {
        self.utf16(range.start)..self.utf16(range.end)
    }
    pub fn move_to(&mut self, byte: usize, select: bool) {
        let byte = byte.min(self.text().len());
        if select {
            let anchor = if self.state.reversed {
                self.state.selection.end
            } else {
                self.state.selection.start
            };
            self.state.selection = anchor.min(byte)..anchor.max(byte);
            self.state.reversed = byte < anchor;
        } else {
            self.state.selection = byte..byte;
            self.state.reversed = false;
        }
    }
    pub fn move_cursor(&mut self, right: bool, select: bool) {
        let cursor = self.cursor();
        let byte = if !select && !self.state.selection.is_empty() {
            if right {
                self.state.selection.end
            } else {
                self.state.selection.start
            }
        } else if right {
            self.text()
                .grapheme_indices(true)
                .find_map(|(i, _)| (i > cursor).then_some(i))
                .unwrap_or(self.text().len())
        } else {
            self.text()
                .grapheme_indices(true)
                .rev()
                .find_map(|(i, _)| (i < cursor).then_some(i))
                .unwrap_or(0)
        };
        self.move_to(byte, select);
    }
    pub fn select_all(&mut self) {
        self.state.selection = 0..self.text().len();
        self.state.reversed = false;
    }
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> usize {
        let range = range
            .map(|r| self.utf16_to_byte_range(r))
            .or(self.marked.clone())
            .unwrap_or(self.selection());
        self.undo.push(self.state.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        // Screenshot labels are single-line; native paste/composition uses the same normalization.
        let text = text.replace(['\n', '\r'], " ");
        self.state.text.replace_range(range.clone(), &text);
        let cursor = range.start + text.len();
        self.move_to(cursor, false);
        self.marked = None;
        range.start
    }
    pub fn delete(&mut self, forward: bool) {
        if self.selection().is_empty() {
            self.move_cursor(forward, true);
        }
        if !self.selection().is_empty() {
            self.replace(None, "");
        }
    }
    pub fn history(&mut self, redo: bool) {
        let state = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(state) = state {
            let old = std::mem::replace(&mut self.state, state);
            if redo {
                self.undo.push(old);
            } else {
                self.redo.push(old);
            }
            self.marked = None;
        }
    }
}
pub struct Edit {
    pub mark: Mark,
    pub buffer: Buffer,
    pub line: Option<ShapedLine>,
    pub bounds: Option<Bounds<Pixels>>,
    pub scroll: Pixels,
    pub selecting: bool,
    pub caret_on: bool,
}
impl Edit {
    pub fn new(mark: Mark) -> Self {
        Self {
            mark,
            buffer: Buffer::default(),
            line: None,
            bounds: None,
            scroll: px(0.),
            selecting: false,
            caret_on: true,
        }
    }
    pub fn replace_text(&mut self, text: &str) {
        self.buffer.replace(None, text);
    }
    pub fn index(&self, p: Point<Pixels>) -> usize {
        match (&self.line, self.bounds) {
            (Some(line), Some(bounds)) => {
                line.closest_index_for_x(p.x - bounds.left() + self.scroll)
            }
            _ => 0,
        }
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let edit = self.text_edit.as_ref()?;
        let range = edit.buffer.utf16_to_byte_range(range);
        *actual = Some(edit.buffer.byte_to_utf16_range(range.clone()));
        Some(edit.buffer.text()[range].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let buffer = &self.text_edit.as_ref()?.buffer;
        Some(UTF16Selection {
            range: buffer.byte_to_utf16_range(buffer.selection()),
            reversed: buffer.state.reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let buffer = &self.text_edit.as_ref()?.buffer;
        buffer
            .marked
            .clone()
            .map(|range| buffer.byte_to_utf16_range(range))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = &mut self.text_edit {
            edit.buffer.marked = None;
            cx.notify();
        }
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = &mut self.text_edit {
            edit.buffer.replace(range, text);
            edit.caret_on = true;
            cx.notify();
        }
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = &mut self.text_edit {
            let start = edit.buffer.replace(range, text);
            let end = edit.buffer.cursor();
            if start != end {
                edit.buffer.marked = Some(start..end);
            }
            if let Some(selected) = selected {
                let prefix = edit.buffer.utf16(start);
                let range = edit
                    .buffer
                    .utf16_to_byte_range(prefix + selected.start..prefix + selected.end);
                edit.buffer.state.selection = range;
                edit.buffer.state.reversed = false;
            }
            edit.caret_on = true;
            cx.notify();
        }
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let edit = self.text_edit.as_ref()?;
        let bounds = edit.bounds?;
        let line = edit.line.as_ref()?;
        let range = edit.buffer.utf16_to_byte_range(range);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(range.start) - edit.scroll,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(range.end) - edit.scroll,
                bounds.bottom(),
            ),
        ))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let edit = self.text_edit.as_ref()?;
        if !edit.bounds?.contains(&p) {
            return None;
        }
        Some(edit.buffer.utf16(edit.index(p)))
    }
}

pub fn paint(
    entity: &Entity<Editor>,
    layout: Layout,
    image_bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(edit) = entity.read(cx).text_edit.as_ref() else {
        return;
    };
    let font_size = px((edit.mark.width * 7.).max(1.) * layout.scale);
    let text = edit.buffer.text();
    let empty = text.is_empty();
    let content: SharedString = if empty {
        "Type here…".into()
    } else {
        text.to_string().into()
    };
    let color = if empty {
        rgb(0x858995)
    } else {
        rgb((edit.mark.color[0] as u32) << 16
            | (edit.mark.color[1] as u32) << 8
            | edit.mark.color[2] as u32)
    };
    let run = TextRun {
        len: content.len(),
        font: font("Arial"),
        color: color.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(content, font_size, &[run], None);
    let p = edit.mark.points[0];
    let origin = point(
        px(layout.x + p.0 * layout.scale),
        px(layout.y + p.1 * layout.scale),
    );
    let width = (line.width + px(16.))
        .max(px(180.))
        .min((image_bounds.right() - origin.x).max(px(16.)));
    let bounds = Bounds::new(origin, size(width, font_size * 1.25));
    let cursor = line.x_for_index(edit.buffer.cursor());
    let scroll = (cursor - width + px(12.)).max(px(0.));
    let paint_origin = origin - point(scroll, px(0.));
    let selection = edit.buffer.selection();
    let caret = edit.caret_on;
    let marked = edit.buffer.marked.clone();
    let background = if edit.mark.color[..3].iter().all(|c| *c > 200) {
        rgb(0x232936)
    } else {
        rgb(0xffffff)
    };
    window.paint_quad(quad(
        bounds.dilate(px(5.)),
        px(3.),
        background,
        px(1.),
        rgb(0xf35d45),
        Default::default(),
    ));
    let focus = entity.read(cx).focus.clone();
    window.handle_input(&focus, ElementInputHandler::new(bounds, entity.clone()), cx);
    window.with_content_mask(
        Some(ContentMask {
            bounds: bounds.intersect(&image_bounds),
        }),
        |window| {
            if !selection.is_empty() {
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(paint_origin.x + line.x_for_index(selection.start), origin.y),
                        point(
                            paint_origin.x + line.x_for_index(selection.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x4c8dff45),
                ));
            }
            let _ = line.paint(paint_origin, font_size, window, cx);
            if let Some(marked) = marked {
                window.paint_quad(fill(
                    Bounds::new(
                        point(
                            paint_origin.x + line.x_for_index(marked.start),
                            bounds.bottom() - px(2.),
                        ),
                        size(
                            line.x_for_index(marked.end) - line.x_for_index(marked.start),
                            px(1.),
                        ),
                    ),
                    color,
                ));
            }
            if selection.is_empty() && caret {
                window.paint_quad(fill(
                    Bounds::new(
                        point(paint_origin.x + cursor, origin.y + px(2.)),
                        size(px(1.5), font_size * 1.15),
                    ),
                    rgb(0xf35d45),
                ));
            }
        },
    );
    entity.update(cx, |editor, _| {
        if let Some(edit) = &mut editor.text_edit {
            edit.bounds = Some(bounds);
            edit.line = Some(line);
            edit.scroll = scroll;
        }
    });
}
#[cfg(test)]
mod tests {
    use super::Buffer;
    #[test]
    fn unicode_ranges_and_grapheme_deletion() {
        let mut buffer = Buffer::default();
        buffer.replace(None, "A👩🏽‍💻e\u{301}中");
        assert_eq!(
            buffer.byte_to_utf16_range(buffer.utf16_to_byte_range(1..8)),
            1..8
        );
        buffer.delete(false);
        assert_eq!(buffer.text(), "A👩🏽‍💻e\u{301}");
        buffer.delete(false);
        assert_eq!(buffer.text(), "A👩🏽‍💻");
        buffer.delete(false);
        assert_eq!(buffer.text(), "A");
        buffer.history(false);
        assert_eq!(buffer.text(), "A👩🏽‍💻");
    }
    #[test]
    fn selections_replace_and_redo_without_screenshot_history() {
        let mut buffer = Buffer::default();
        buffer.replace(None, "hello");
        buffer.move_to(1, false);
        buffer.move_to(4, true);
        buffer.replace(None, "i");
        assert_eq!(buffer.text(), "hio");
        buffer.history(false);
        assert_eq!(buffer.text(), "hello");
        buffer.history(true);
        assert_eq!(buffer.text(), "hio");
        buffer.select_all();
        buffer.replace(None, "paste\nline");
        assert_eq!(buffer.text(), "paste line");
    }
}
