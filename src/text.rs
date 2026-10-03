//! Unicode text buffer, selection, history, and inline editing geometry.
use crate::document::Mark;
use gpui::{Bounds, Pixels, Point, ShapedLine, px};
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
    pub(crate) fn selection_reversed(&self) -> bool {
        self.state.reversed
    }
    pub(crate) fn select_range(&mut self, range: Range<usize>) {
        self.state.selection = range;
        self.state.reversed = false;
    }
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
    pub(crate) fn utf16(&self, byte: usize) -> usize {
        self.text()[..byte].encode_utf16().count()
    }
    pub(crate) fn utf16_to_byte_range(&self, range: Range<usize>) -> Range<usize> {
        self.byte(range.start)..self.byte(range.end)
    }
    pub(crate) fn byte_to_utf16_range(&self, range: Range<usize>) -> Range<usize> {
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
