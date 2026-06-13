pub mod graphemes;
pub mod movement;
pub mod tree;

mod diff;

use ropey::{Rope, RopeSlice};
use std::{cmp, ops::Range};

pub use self::{
    diff::{CompoundDiff, DeleteOperation, OpaqueDiff},
    graphemes::{ByteIndex, CharIndex, LineIndex, RopeExt, RopeGraphemes},
    movement::Direction,
};

/// Rectangle selection state: (line range) x (visual column range).
///
/// `line_range` is inclusive on both ends (start..=end).
/// `column_range` is half-open: `[left, right)` in visual columns.
/// Width-0 rectangle: `left == right` (valid selection, no-op for copy/cut).
/// Anchor is the fixed corner; the cursor is the moving diagonal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RectangleSelection {
    pub anchor_line: LineIndex,
    pub anchor_column: usize,
    pub line_start: LineIndex,
    pub line_end: LineIndex,
    pub column_left: usize,
    pub column_right: usize,
}

impl RectangleSelection {
    pub fn new(anchor_line: LineIndex, anchor_column: usize) -> Self {
        Self {
            anchor_line,
            anchor_column,
            line_start: anchor_line,
            line_end: anchor_line,
            column_left: anchor_column,
            column_right: anchor_column,
        }
    }

    /// Update the diagonal (cursor position) and re-normalize.
    pub fn update_diagonal(&mut self, cursor_line: LineIndex, cursor_column: usize) {
        self.line_start = self.anchor_line.min(cursor_line);
        self.line_end = self.anchor_line.max(cursor_line);
        self.column_left = self.anchor_column.min(cursor_column);
        self.column_right = self.anchor_column.max(cursor_column);
    }

    /// Whether this rectangle has zero width (column_left == column_right).
    pub fn is_zero_width(&self) -> bool {
        self.column_left == self.column_right
    }
}

trait RopeCursorExt {
    fn cursor_to_line(&self, cursor: &Cursor) -> usize;

    #[allow(dead_code)]
    fn slice_cursor(&self, cursor: &Cursor) -> RopeSlice;
}

impl RopeCursorExt for Rope {
    fn cursor_to_line(&self, cursor: &Cursor) -> usize {
        self.char_to_line(cursor.range.start)
    }

    fn slice_cursor(&self, cursor: &Cursor) -> RopeSlice {
        self.slice(cursor.range.start..cursor.range.end)
    }
}

/// A `Cursor` represents a user cursor associated with a text buffer.
///
/// `Cursor`s consist of a location in a `Rope` and optionally a selection and
/// desired visual offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Cursor {
    /// The cursor position represented as the index of the gap between two adjacent
    /// characters inside a `Rope`.
    ///
    /// For a rope of length len, the valid range is 0..=length. The position is
    /// aligned to extended grapheme clusters and will never index a gap inside
    /// a grapheme.
    range: Range<CharIndex>,
    /// The start of a selection if in select mode, ending at `range.start` or
    /// `range.end`, depending on direction. Aligned to extended grapheme
    /// clusters.
    selection: Option<CharIndex>,
    visual_horizontal_offset: Option<usize>,
    /// Rectangle selection state. Exclusive with `selection` (D3):
    /// when `rectangle_selection` is Some, `selection` must be None.
    rectangle_selection: Option<RectangleSelection>,
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            range: 0..0,
            selection: None,
            visual_horizontal_offset: None,
            rectangle_selection: None,
        }
    }

    pub fn with_range(range: Range<CharIndex>) -> Self {
        Self {
            range,
            ..Self::new()
        }
    }

    #[cfg(test)]
    pub fn end_of_buffer(text: &Rope) -> Self {
        Self {
            range: text.prev_grapheme_boundary(text.len_chars())..text.len_chars(),
            visual_horizontal_offset: None,
            selection: None,
            rectangle_selection: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    pub fn range(&self) -> Range<CharIndex> {
        self.range.clone()
    }

    pub fn selection(&self) -> Range<CharIndex> {
        match self.selection {
            Some(selection) if selection > self.range.start => self.range.start..selection,
            Some(selection) if selection < self.range.start => selection..self.range.start,
            _ => self.range.clone(),
        }
    }

    pub fn column_offset(&self, tab_width: usize, text: &Rope) -> usize {
        let char_line_start = text.line_to_char(text.cursor_to_line(self));
        graphemes::width(tab_width, &text.slice(char_line_start..self.range.start))
    }

    pub fn reconcile(&mut self, new_text: &Rope, diff: &OpaqueDiff) {
        let OpaqueDiff {
            char_index,
            old_char_length,
            new_char_length,
            ..
        } = *diff;

        let modified_range = char_index..cmp::max(old_char_length, new_char_length);

        // The edit starts after the end of the cursor, nothing to do
        if modified_range.start >= self.range.end {
            return;
        }

        // The edit ends before the start of the cursor
        if modified_range.end <= self.range.start {
            let (start, end) = (self.range.start, self.range.end);
            if old_char_length > new_char_length {
                let length_change = old_char_length - new_char_length;
                self.range = start.saturating_sub(length_change)..end.saturating_sub(length_change);
            } else {
                let length_change = new_char_length - old_char_length;
                self.range = start + length_change..end + length_change;
            };
        }

        // Otherwise, the change overlaps with the cursor
        let grapheme_start =
            new_text.prev_grapheme_boundary(cmp::min(self.range.end, new_text.len_chars()));
        let grapheme_end = new_text.next_grapheme_boundary(grapheme_start);
        self.range = grapheme_start..grapheme_end
    }

    /// Reconcile cursor position against a compound diff (multiple sub-diffs).
    /// Applies each sub-diff's effect in order. The sub-diffs should be
    /// provided in the order they were applied to the text (forward order).
    pub fn reconcile_compound(&mut self, new_text: &Rope, compound: &CompoundDiff) {
        for diff in &compound.0 {
            if !diff.is_empty() {
                self.reconcile(new_text, diff);
            }
        }
    }

    pub fn begin_selection(&mut self) {
        self.rectangle_selection = None;
        self.selection = Some(self.range.start)
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn select_all(&mut self, text: &Rope) {
        movement::move_to_start_of_buffer(text, self);
        self.selection = Some(text.len_chars());
        self.rectangle_selection = None;
    }

    // Rectangle selection mode

    /// Enter rectangle selection mode. Current cursor position becomes the
    /// anchor. Clears any existing linear selection (D3: exclusive).
    pub fn begin_rectangle_selection(&mut self, text: &Rope, tab_width: usize) {
        self.selection = None;
        let line = text.char_to_line(self.range.start);
        let column = self.column_offset(tab_width, text);
        self.rectangle_selection = Some(RectangleSelection::new(line, column));
    }

    /// Update rectangle diagonal from current cursor position.
    /// Should be called after each cursor movement in rectangle mode.
    pub fn update_rectangle_diagonal(&mut self, text: &Rope, tab_width: usize) {
        let cursor_line = text.char_to_line(self.range.start);
        let cursor_column = self.column_offset(tab_width, text);
        if let Some(ref mut rect) = self.rectangle_selection {
            rect.update_diagonal(cursor_line, cursor_column);
        }
    }

    /// Exit rectangle selection mode without modifying the buffer.
    /// Cursor position is unchanged (C-g cancel).
    pub fn clear_rectangle_selection(&mut self) {
        self.rectangle_selection = None;
    }

    /// Get the current rectangle selection, if any.
    pub fn rectangle_selection(&self) -> Option<&RectangleSelection> {
        self.rectangle_selection.as_ref()
    }

    /// Whether rectangle mode is active.
    pub fn is_rectangle_mode(&self) -> bool {
        self.rectangle_selection.is_some()
    }

    // Editing

    pub fn insert_char(&mut self, text: &mut Rope, character: char) -> OpaqueDiff {
        self.clear_selection();
        self.clear_rectangle_selection();
        text.insert_char(self.range.start, character);
        OpaqueDiff::new(
            text.char_to_byte(self.range.start),
            0,
            character.len_utf8(),
            self.range.start,
            0,
            1,
        )
    }

    pub fn insert_chars(
        &mut self,
        text: &mut Rope,
        characters: impl IntoIterator<Item = char>,
    ) -> OpaqueDiff {
        self.clear_selection();
        self.clear_rectangle_selection();
        let mut num_bytes = 0;
        let mut num_chars = 0;
        characters
            .into_iter()
            .enumerate()
            .for_each(|(offset, character)| {
                text.insert_char(self.range.start + offset, character);
                num_bytes += character.len_utf8();
                num_chars += 1;
            });
        OpaqueDiff::new(
            text.char_to_byte(self.range.start),
            0,
            num_bytes,
            self.range.start,
            0,
            num_chars,
        )
    }

    pub fn delete_forward(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 || text.len_chars() == self.range.start {
            return DeleteOperation::empty();
        }

        let byte_range = text.char_to_byte(self.range.start)..text.char_to_byte(self.range.end);
        let diff = OpaqueDiff::new(
            byte_range.start,
            byte_range.end - byte_range.start,
            0,
            self.range.start,
            self.range.end - self.range.start,
            0,
        );
        text.remove(self.range.clone());

        let grapheme_start = self.range.start;
        let grapheme_end = text.next_grapheme_boundary(self.range.start);
        let deleted = text.slice(grapheme_start..grapheme_end).into();

        *self = Cursor::with_range(grapheme_start..grapheme_end);

        DeleteOperation { diff, deleted }
    }

    pub fn delete_backward(&mut self, text: &mut Rope) -> DeleteOperation {
        if self.range.start > 0 {
            movement::move_horizontally(text, self, Direction::Backward, 1);
            self.delete_forward(text)
        } else {
            DeleteOperation::empty()
        }
    }

    pub fn delete_line(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 {
            return DeleteOperation::empty();
        }

        // Delete line
        let line_index = text.char_to_line(self.range.start);
        let delete_range_start = text.line_to_char(line_index);
        let delete_range_end = text.line_to_char(line_index + 1);
        let deleted = text.slice(delete_range_start..delete_range_end).into();
        let diff = OpaqueDiff::new(
            text.char_to_byte(delete_range_start),
            text.char_to_byte(delete_range_end) - text.char_to_byte(delete_range_start),
            0,
            delete_range_start,
            delete_range_end - delete_range_start,
            0,
        );
        text.remove(delete_range_start..delete_range_end);

        // Update cursor position
        let grapheme_start =
            text.line_to_char(cmp::min(line_index, text.len_lines().saturating_sub(2)));
        let grapheme_end = text.next_grapheme_boundary(grapheme_start);

        *self = Cursor::with_range(grapheme_start..grapheme_end);

        DeleteOperation { diff, deleted }
    }

    pub fn delete_selection(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 {
            return DeleteOperation::empty();
        }

        // Delete selection
        let selection = self.selection();
        let deleted = text.slice(selection.start..selection.end).into();
        let diff = OpaqueDiff::new(
            text.char_to_byte(selection.start),
            text.char_to_byte(selection.end) - text.char_to_byte(selection.start),
            0,
            selection.start,
            selection.end - selection.start,
            0,
        );
        text.remove(selection.start..selection.end);

        // Update cursor position
        let grapheme_start = cmp::min(
            self.range.start,
            text.prev_grapheme_boundary(text.len_chars()),
        );
        let grapheme_end = text.next_grapheme_boundary(grapheme_start);

        *self = Cursor::with_range(grapheme_start..grapheme_end);

        DeleteOperation { diff, deleted }
    }

    pub fn sync(&mut self, current_text: &Rope, new_text: &Rope) {
        let current_line = current_text.char_to_line(self.range.start);
        let current_line_offset = self.range.start - current_text.line_to_char(current_line);

        let new_line = cmp::min(current_line, new_text.len_lines().saturating_sub(1));
        let new_line_offset = cmp::min(
            current_line_offset,
            new_text.line(new_line).len_chars().saturating_sub(1),
        );
        let grapheme_end =
            new_text.next_grapheme_boundary(new_text.line_to_char(new_line) + new_line_offset);
        let grapheme_start = new_text.prev_grapheme_boundary(grapheme_end);

        *self = Cursor::with_range(grapheme_start..grapheme_end);
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;

    use super::*;

    fn text_with_cursor(text: impl Into<Rope>) -> (Rope, Cursor) {
        let text = text.into();
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Backward, 1);
        (text, cursor)
    }

    #[test]
    fn sync_with_empty() {
        let current_text = Rope::from("Buy a milk goat\nAt the market\n");
        let new_text = Rope::from("");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&current_text, &mut cursor, Direction::Forward, 4);
        cursor.sync(&current_text, &new_text);
        assert_eq!(Cursor::new(), cursor);
    }

    // Delete forward
    #[test]
    fn delete_forward_at_the_end() {
        let (mut text, mut cursor) = text_with_cursor(TEXT);
        let expected = text.clone();
        movement::move_to_end_of_buffer(&text, &mut cursor);
        cursor.delete_forward(&mut text);
        assert_eq!(expected, text);
    }

    #[test]
    fn delete_forward_empty_text() {
        let (mut text, mut cursor) = text_with_cursor("");
        cursor.delete_forward(&mut text);
        assert_eq!(cursor, Cursor::new());
    }

    #[test]
    fn delete_forward_at_the_begining() {
        let (mut text, mut cursor) = text_with_cursor("// Hello world!\n\n");
        let expected = Rope::from("Hello world!\n\n");
        cursor.delete_forward(&mut text);
        cursor.delete_forward(&mut text);
        cursor.delete_forward(&mut text);
        assert_eq!(expected, text);
    }

    // Delete backward
    #[test]
    fn delete_backward_at_the_end() {
        let (mut text, mut cursor) = text_with_cursor("// Hello world!\n");
        movement::move_to_end_of_buffer(&text, &mut cursor);
        cursor.delete_backward(&mut text);
        assert_eq!(Rope::from("// Hello world!"), text);
        cursor.delete_backward(&mut text);
        assert_eq!(Rope::from("// Hello world"), text);
    }

    #[test]
    fn delete_backward_empty_text() {
        let (mut text, mut cursor) = text_with_cursor("");
        cursor.delete_backward(&mut text);
        assert_eq!(cursor, Cursor::new());
    }

    #[test]
    fn delete_backward_at_the_begining() {
        let (mut text, mut cursor) = text_with_cursor("// Hello world!\n");
        let expected = text.clone();
        cursor.delete_backward(&mut text);
        assert_eq!(expected, text);
    }

    const TEXT: &str = r#"
Basic Latin
    ! " # $ % & ' ( ) *+,-./012ABCDEFGHI` a m  t u v z { | } ~
CJK
    豈 更 車 Ⅷ
"#;

    // --- Selection regression tests (safety net for C-3) ---

    #[test]
    fn begin_selection_sets_anchor() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_selection();
        assert_eq!(cursor.selection, Some(3));
    }

    #[test]
    fn clear_selection_removes_anchor() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_selection();
        assert!(cursor.selection.is_some());
        cursor.clear_selection();
        assert!(cursor.selection.is_none());
    }

    #[test]
    fn selection_normalized_forward() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        // anchor at 3, cursor moves to 7 => selection = 3..7
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_selection();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 4);
        let sel = cursor.selection();
        assert_eq!(sel, 3..7);
    }

    #[test]
    fn selection_normalized_backward() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        // anchor at 7, cursor moves back to 3 => selection = 3..7
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 7);
        cursor.begin_selection();
        movement::move_horizontally(&text, &mut cursor, Direction::Backward, 4);
        let sel = cursor.selection();
        assert_eq!(sel, 3..7);
    }

    #[test]
    fn selection_no_anchor_returns_cursor_range() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 5);
        assert!(cursor.selection.is_none());
        let sel = cursor.selection();
        // Should return cursor's own range
        assert_eq!(sel, cursor.range);
    }

    #[test]
    fn select_all_selects_entire_buffer() {
        let text = Rope::from("Hello\nWorld");
        let mut cursor = Cursor::new();
        cursor.select_all(&text);
        let sel = cursor.selection();
        assert_eq!(sel.start, 0);
        assert_eq!(sel.end, text.len_chars());
    }

    // --- delete_selection regression tests (safety net for C-3) ---

    #[test]
    fn delete_selection_removes_selected_range() {
        let mut text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_selection();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 4);
        let operation = cursor.delete_selection(&mut text);
        assert_eq!(&text.to_string(), "Helorld");
        assert_eq!(operation.diff.char_index, 3);
        assert_eq!(operation.diff.old_char_length, 4);
    }

    #[test]
    fn delete_selection_resets_cursor() {
        let mut text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_selection();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 4);
        cursor.delete_selection(&mut text);
        // After delete_selection, cursor is created from min(range.start, text end)
        // range.start was 7 (diagonal), new text len = 7, so cursor at 6
        assert_eq!(cursor.range.start, 6);
        // Selection should be cleared (new cursor has no selection)
        assert!(cursor.selection.is_none());
    }

    #[test]
    fn delete_selection_backward_direction() {
        let mut text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        // anchor at 7, move back to 3 => selection = 3..7
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 7);
        cursor.begin_selection();
        movement::move_horizontally(&text, &mut cursor, Direction::Backward, 4);
        // cursor.range.start is now 3
        let operation = cursor.delete_selection(&mut text);
        assert_eq!(&text.to_string(), "Helorld");
        // cursor.range.start was 3, min(3, prev_boundary(7)) = 3
        assert_eq!(cursor.range.start, 3);
        assert!(cursor.selection.is_none());
    }

    #[test]
    fn delete_selection_on_empty_text() {
        let mut text = Rope::from("");
        let mut cursor = Cursor::new();
        let operation = cursor.delete_selection(&mut text);
        assert!(operation.diff.is_empty());
    }

    // --- Rectangle selection tests (spec: rectangle-selection) ---

    #[test]
    fn rectangle_enter_anchor_fixed() {
        let text = Rope::from("Hello\nWorld\n");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_rectangle_selection(&text, 4);
        // Anchor should be at (line 0, column 3)
        let rect = cursor.rectangle_selection().unwrap();
        assert_eq!(rect.anchor_line, 0);
        assert_eq!(rect.anchor_column, 3);
        assert_eq!(rect.line_start, 0);
        assert_eq!(rect.line_end, 0);
        assert_eq!(rect.column_left, 3);
        assert_eq!(rect.column_right, 3);
        assert!(rect.is_zero_width());
    }

    #[test]
    fn rectangle_diagonal_updates_normalized() {
        let text = Rope::from("Hello\nWorld\nFooBar\n");
        let mut cursor = Cursor::new();
        // Start at column 2, line 0
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 2);
        cursor.begin_rectangle_selection(&text, 4);
        // Move right 3 columns
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.update_rectangle_diagonal(&text, 4);
        let rect = cursor.rectangle_selection().unwrap();
        // Anchor at (0, 2), cursor now at column 5 => normalized: cols 2..5
        assert_eq!(rect.line_start, 0);
        assert_eq!(rect.line_end, 0);
        assert_eq!(rect.column_left, 2);
        assert_eq!(rect.column_right, 5);
    }

    #[test]
    fn rectangle_backward_direction_normalized() {
        let text = Rope::from("Hello\nWorld\nFooBar\n");
        let mut cursor = Cursor::new();
        // Start at column 5, line 0
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 5);
        cursor.begin_rectangle_selection(&text, 4);
        // Move left 3 columns
        movement::move_horizontally(&text, &mut cursor, Direction::Backward, 3);
        cursor.update_rectangle_diagonal(&text, 4);
        let rect = cursor.rectangle_selection().unwrap();
        // Anchor at (0, 5), cursor now at column 2 => normalized: cols 2..5
        assert_eq!(rect.column_left, 2);
        assert_eq!(rect.column_right, 5);
    }

    #[test]
    fn rectangle_cancel_nondestructive() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        let cursor_before = cursor.clone();
        cursor.begin_rectangle_selection(&text, 4);
        assert!(cursor.is_rectangle_mode());
        cursor.clear_rectangle_selection();
        assert!(!cursor.is_rectangle_mode());
        // Cursor position unchanged
        assert_eq!(cursor.range, cursor_before.range);
    }

    #[test]
    fn rectangle_exclusive_with_normal_selection() {
        let text = Rope::from("Hello world");
        let mut cursor = Cursor::new();
        // Start normal selection
        cursor.begin_selection();
        assert!(cursor.selection.is_some());
        assert!(!cursor.is_rectangle_mode());
        // Enter rectangle mode clears normal selection
        cursor.begin_rectangle_selection(&text, 4);
        assert!(cursor.selection.is_none());
        assert!(cursor.is_rectangle_mode());
        // Enter normal selection clears rectangle mode
        cursor.begin_selection();
        assert!(cursor.selection.is_some());
        assert!(!cursor.is_rectangle_mode());
    }

    #[test]
    fn rectangle_mode_buffer_unchanged() {
        let text = Rope::from("Hello\nWorld\n");
        let mut cursor = Cursor::new();
        movement::move_horizontally(&text, &mut cursor, Direction::Forward, 3);
        cursor.begin_rectangle_selection(&text, 4);
        movement::move_vertically(&text, &mut cursor, 4, Direction::Forward, 1);
        cursor.update_rectangle_diagonal(&text, 4);
        // Buffer should be unchanged (no edits happened)
        assert_eq!(&text.to_string(), "Hello\nWorld\n");
    }
}
