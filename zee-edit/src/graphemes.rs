use ropey::{Rope, RopeSlice, iter::Chunks, str_utils};
use std::ops::Range;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use unicode_width::UnicodeWidthStr;

pub type ByteIndex = usize;
pub type CharIndex = usize;
pub type LineIndex = usize;

pub fn width(tab_width: usize, slice: &RopeSlice) -> usize {
    rope_slice_as_str(slice, |text| {
        if text == "\t" {
            tab_width
        } else if text == "\n" || text == "\r\n" || text == "\r" {
            0
        } else {
            text.chars().filter(|character| *character == '\t').count() * tab_width
                + UnicodeWidthStr::width(text)
        }
    })
}

pub fn rope_slice_as_str<T>(slice: &RopeSlice, closure: impl FnOnce(&str) -> T) -> T {
    if let Some(text) = slice.as_str() {
        closure(text)
    } else {
        let text = slice.chars().collect::<String>();
        closure(text.as_str())
    }
}

pub struct RopeGrapheme<'a> {
    pub slice: RopeSlice<'a>,
    pub byte_start: usize,
    pub byte_end: usize,
}

impl<'a> std::ops::Deref for RopeGrapheme<'a> {
    type Target = RopeSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.slice
    }
}

/// An iterator over the graphemes of a RopeSlice.
pub struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: Chunks<'a>,
    chunk: &'a str,
    chunk_byte_start: usize,
    previous_chunk: &'a str,
    previous_chunk_byte_start: usize,
    pub cursor: GraphemeCursor,
}

impl<'a> RopeGraphemes<'a> {
    pub fn new<'b>(slice: &RopeSlice<'b>) -> RopeGraphemes<'b> {
        let mut chunks = slice.chunks();
        let chunk = chunks.next().unwrap_or("");
        RopeGraphemes {
            text: *slice,
            chunks,
            chunk,
            chunk_byte_start: 0,
            previous_chunk: "",
            previous_chunk_byte_start: 0,
            cursor: GraphemeCursor::new(0, slice.len_bytes(), true),
        }
    }
}

impl<'a> Iterator for RopeGraphemes<'a> {
    type Item = RopeGrapheme<'a>;

    fn next(&mut self) -> Option<RopeGrapheme<'a>> {
        let byte_start = self.cursor.cur_cursor();
        let byte_end;
        loop {
            match self.cursor.next_boundary(self.chunk, self.chunk_byte_start) {
                Ok(None) => {
                    return None;
                }
                Ok(Some(n)) => {
                    byte_end = n;
                    break;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    self.previous_chunk = self.chunk;
                    self.previous_chunk_byte_start = self.chunk_byte_start;
                    self.chunk_byte_start += self.chunk.len();
                    self.chunk = self.chunks.next().unwrap_or("");
                }
                Err(GraphemeIncomplete::PreContext(context_length)) => {
                    assert!(context_length <= self.previous_chunk.len());
                    self.cursor
                        .provide_context(self.previous_chunk, self.previous_chunk_byte_start);
                }
                Err(error) => {
                    panic!(
                        "unexpectedly encountered `{:?}` while iterating over grapheme clusters",
                        error
                    );
                }
            }
        }

        let slice = if byte_start < self.chunk_byte_start {
            let char_start = self.text.byte_to_char(byte_start);
            let char_end = self.text.byte_to_char(byte_end);
            self.text.slice(char_start..char_end)
        } else {
            let chunk_byte_start = byte_start - self.chunk_byte_start;
            let chunk_byte_end = byte_end - self.chunk_byte_start;
            self.chunk[chunk_byte_start..chunk_byte_end].into()
        };
        Some(RopeGrapheme {
            slice,
            byte_start,
            byte_end,
        })
    }
}

/// Ragged line policy: what to do when a line is shorter than the rectangle's
/// left column. This is the single branch point isolated per D5.
/// Currently "empty" (Emacs-style: no padding). To switch to whitespace
/// padding, change this enum variant and the match arm in
/// `visual_column_range_to_char_range`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaggedLinePolicy {
    /// Lines shorter than `left` yield an empty char range (no padding).
    Empty,
    // Future: Padding variant can be added here without touching callers.
}

/// Result of mapping a visual column range to a char range on a single line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnMapping {
    /// Char range in the line (relative to line start char index).
    pub char_start: CharIndex,
    /// Char end (exclusive). May equal char_start for empty/short lines.
    pub char_end: CharIndex,
    /// Whether the line was shorter than the requested left column.
    pub line_shorter_than_left: bool,
}

impl ColumnMapping {
    pub fn char_range(&self) -> Range<CharIndex> {
        self.char_start..self.char_end
    }

    pub fn is_empty(&self) -> bool {
        self.char_start == self.char_end
    }
}

/// Map a visual column range `[left, right)` on a given line to a char range.
///
/// This is the shared extraction rule for highlight/copy/cut (C-4).
/// Uses `RaggedLinePolicy` for lines shorter than `left` (D5/C-5).
///
/// `line_char_start` is the char index of the start of the line in the Rope.
/// `line_slice` is the line content (without trailing newline).
/// Returns a `ColumnMapping` with the char range relative to the line.
pub fn visual_column_range_to_char_range(
    tab_width: usize,
    line_slice: &RopeSlice,
    line_char_start: CharIndex,
    left: usize,
    right: usize,
    policy: RaggedLinePolicy,
) -> ColumnMapping {
    if left >= right {
        // Zero-width rectangle: empty range at `left`
        // Find the char index corresponding to visual column `left`
        let (char_at_left, _) = visual_column_to_char_index(tab_width, line_slice, left);
        return ColumnMapping {
            char_start: line_char_start + char_at_left,
            char_end: line_char_start + char_at_left,
            line_shorter_than_left: false,
        };
    }

    let (char_at_left, visual_at_left) = visual_column_to_char_index(tab_width, line_slice, left);
    let char_at_right = visual_column_to_char_end(tab_width, line_slice, right);

    // Check if line is shorter than left
    let line_visual_end = width(tab_width, line_slice);
    if line_visual_end <= left {
        return match policy {
            RaggedLinePolicy::Empty => ColumnMapping {
                char_start: line_char_start + char_at_left,
                char_end: line_char_start + char_at_left,
                line_shorter_than_left: true,
            },
        };
    }

    // If the left visual column falls in the middle of a wide grapheme,
    // include that grapheme (it overlaps the range).
    let actual_char_start = if visual_at_left < left {
        // The grapheme at char_at_left started before `left` visual column
        // but overlaps it - include it
        char_at_left
    } else {
        char_at_left
    };

    ColumnMapping {
        char_start: line_char_start + actual_char_start,
        char_end: line_char_start + char_at_right,
        line_shorter_than_left: false,
    }
}

/// Find the char index (relative to line start) and the actual visual column
/// at the start of the grapheme that spans the given visual column.
///
/// Returns (char_offset_in_line, actual_visual_column_of_grapheme_start).
pub fn visual_column_to_char_index_pub(
    tab_width: usize,
    line_slice: &RopeSlice,
    target_visual_col: usize,
) -> (CharIndex, usize) {
    visual_column_to_char_index(tab_width, line_slice, target_visual_col)
}

/// Find the char index (relative to line start) and the actual visual column
/// at the start of the grapheme that spans the given visual column.
///
/// Returns (char_offset_in_line, actual_visual_column_of_grapheme_start).
fn visual_column_to_char_index(
    tab_width: usize,
    line_slice: &RopeSlice,
    target_visual_col: usize,
) -> (CharIndex, usize) {
    let mut visual_x = 0;
    let mut char_offset = 0;

    for grapheme in RopeGraphemes::new(line_slice) {
        let grapheme_width = width(tab_width, &grapheme);
        if visual_x >= target_visual_col {
            return (char_offset, visual_x);
        }
        // If this grapheme spans the target column (starts before, ends at or after)
        if visual_x + grapheme_width > target_visual_col {
            return (char_offset, visual_x);
        }
        char_offset += grapheme.len_chars();
        visual_x += grapheme_width;
    }

    // Target column is at or beyond EOL
    (char_offset, visual_x)
}

/// Find the char index (relative to line start) of the first char that starts
/// at or after the given visual column. This is used for the "right" (end)
/// boundary of the column range.
fn visual_column_to_char_end(
    tab_width: usize,
    line_slice: &RopeSlice,
    target_visual_col: usize,
) -> CharIndex {
    let mut visual_x = 0;
    let mut char_offset = 0;

    for grapheme in RopeGraphemes::new(line_slice) {
        let grapheme_width = width(tab_width, &grapheme);
        // If this grapheme ends at or after the target column,
        // the char after this grapheme is the end boundary
        if visual_x + grapheme_width >= target_visual_col {
            return char_offset + grapheme.len_chars();
        }
        char_offset += grapheme.len_chars();
        visual_x += grapheme_width;
    }

    char_offset
}

pub fn strip_trailing_whitespace(mut text: Rope) -> Rope {
    // Pretty inefficient (t)

    let mut trailing_empty_line = true;
    for line_index in (0..text.len_lines()).rev() {
        let start = text.line_to_char(line_index);
        let end = if line_index + 1 < text.len_lines() {
            text.line_to_char(line_index + 1)
        } else {
            text.len_chars()
        };
        if start == end {
            continue;
        }

        let mut cursor = end - 1;
        while cursor > start {
            cursor -= 1;
            let character = text.char(cursor);
            if character.is_whitespace() {
                text.remove(cursor..=cursor);
            } else {
                trailing_empty_line = false;
                break;
            }
        }
        if trailing_empty_line && cursor == start {
            text.remove(start..text.len_chars());
        }
    }

    if text.len_chars() > 1 && text.char(text.len_chars() - 1) != '\n' {
        text.insert_char(text.len_chars(), '\n');
    }

    text
}

pub trait RopeExt {
    /// Finds the previous grapheme boundary before the given char position
    fn prev_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex;

    /// Finds the next grapheme boundary after the given char position
    fn next_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex;

    /// Finds the nth previous grapheme boundary before the given char position
    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> usize {
        self.prev_grapheme_boundary_n(char_index, 1)
    }

    /// Finds the nth next grapheme boundary after the given char position
    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        self.next_grapheme_boundary_n(char_index, 1)
    }
}

impl RopeExt for Rope {
    /// Finds the previous grapheme boundary before the given char position
    fn prev_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex {
        prev_grapheme_boundary_n(self.slice(..), char_index, n)
    }

    /// Finds the next grapheme boundary after the given char position
    fn next_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex {
        next_grapheme_boundary_n(self.slice(..), char_index, n)
    }
}

/// Finds the previous grapheme boundary before the given char position.
fn prev_grapheme_boundary_n(slice: RopeSlice, char_index: CharIndex, n: usize) -> CharIndex {
    // Bounds check
    debug_assert!(char_index <= slice.len_chars());

    // We work with bytes for this, so convert.
    let mut byte_index = slice.char_to_byte(char_index);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_index, mut chunk_char_index, _) =
        slice.chunk_at_byte(byte_index);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_index, slice.len_bytes(), true);

    // Find the previous grapheme cluster boundary.
    for _ in 0..n {
        loop {
            match gc.prev_boundary(chunk, chunk_byte_index) {
                Ok(None) => return 0,
                Ok(Some(boundry_offset)) => {
                    byte_index = boundry_offset;
                    break;
                }
                Err(GraphemeIncomplete::PrevChunk) => {
                    let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_index - 1);
                    chunk = a;
                    chunk_byte_index = b;
                    chunk_char_index = c;
                }
                Err(GraphemeIncomplete::PreContext(offset)) => {
                    let ctx_chunk = slice.chunk_at_byte(offset - 1).0;
                    gc.provide_context(ctx_chunk, offset - ctx_chunk.len());
                }
                _ => unreachable!(),
            }
        }
    }

    let tmp = str_utils::byte_to_char_idx(chunk, byte_index - chunk_byte_index);
    chunk_char_index + tmp
}

/// Finds the next grapheme boundary after the given char position.
fn next_grapheme_boundary_n(slice: RopeSlice, char_index: CharIndex, n: usize) -> CharIndex {
    debug_assert!(char_index <= slice.len_chars());

    // We work with bytes for this, so convert.
    let mut byte_index = slice.char_to_byte(char_index);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_index, mut chunk_char_index, _) =
        slice.chunk_at_byte(byte_index);

    // Set up the grapheme cursor.
    let mut cursor = GraphemeCursor::new(byte_index, slice.len_bytes(), true);

    // Find the next grapheme cluster boundary.
    for _ in 0..n {
        loop {
            match cursor.next_boundary(chunk, chunk_byte_index) {
                Ok(None) => return slice.len_chars(),
                Ok(Some(boundry_offset)) => {
                    byte_index = boundry_offset;
                    break;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    chunk_byte_index += chunk.len();
                    let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_index);
                    chunk = a;
                    chunk_char_index = c;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                    cursor.provide_context(ctx_chunk, n - ctx_chunk.len());
                }
                _ => unreachable!(),
            }
        }
    }

    let tmp = str_utils::byte_to_char_idx(chunk, byte_index - chunk_byte_index);
    chunk_char_index + tmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn prev_grapheme_1() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let grapheme_start = text.prev_grapheme_boundary(text.len_chars() - 1);
        assert_eq!(0, grapheme_start);
    }

    #[test]
    fn end_grapheme_1() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let grapheme_end = text.next_grapheme_boundary(0);
        assert_eq!(text.len_chars(), grapheme_end);
    }

    const MULTI_CHAR_EMOJI: &str = r#"👨‍👨‍👧‍👧"#;

    // --- width() regression for unicode-width 0.2 ---
    // unicode-width 0.2 reports `\n` as width 1 (0.1 reported 0). The textarea
    // renderer relies on line-break graphemes being width 0, so width() must
    // keep treating them as 0 regardless of the unicode-width version.

    #[test]
    fn width_newline_is_zero() {
        for line_break in ["\n", "\r\n", "\r"] {
            let text = Rope::from(line_break);
            assert_eq!(width(4, &text.slice(..)), 0, "line break {line_break:?}");
        }
    }

    #[test]
    fn width_tab_and_text_unaffected() {
        let tab = Rope::from("\t");
        assert_eq!(width(4, &tab.slice(..)), 4);

        let ascii = Rope::from("hello");
        assert_eq!(width(4, &ascii.slice(..)), 5);

        // CJK stays width 2 across unicode-width versions.
        let cjk = Rope::from("漢");
        assert_eq!(width(4, &cjk.slice(..)), 2);
    }

    #[test]
    fn width_line_grapheme_sum_ignores_trailing_newline() {
        // Mirrors the textarea path: it sums width() per grapheme. The trailing
        // `\n` grapheme must contribute 0 so end-of-line rendering and cursor
        // placement do not shift under unicode-width 0.2.
        let text = Rope::from("abc\n");
        let line = text.slice(text.line_to_char(0)..text.line_to_char(1));
        let total: usize = RopeGraphemes::new(&line)
            .map(|grapheme| width(4, &grapheme))
            .sum();
        assert_eq!(total, 3);
    }

    // --- Column mapping tests (spec: rectangle-column-mapping) ---

    fn line_slice(text: &Rope, line_index: usize) -> RopeSlice {
        let line = text.line(line_index);
        // Strip trailing newline for mapping purposes
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            text.slice(text.line_to_char(line_index)..text.line_to_char(line_index) + len - 1)
        } else {
            text.slice(text.line_to_char(line_index)..text.line_to_char(line_index) + len)
        }
    }

    #[test]
    fn column_mapping_ascii() {
        let text = Rope::from("Hello world\n");
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            2,
            5,
            RaggedLinePolicy::Empty,
        );
        assert_eq!(mapping.char_range(), 2..5);
        assert!(!mapping.line_shorter_than_left);
    }

    #[test]
    fn column_mapping_ascii_full_line() {
        let text = Rope::from("Hello world\n");
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            0,
            11,
            RaggedLinePolicy::Empty,
        );
        assert_eq!(mapping.char_range(), 0..11);
    }

    #[test]
    fn column_mapping_cjk() {
        // CJK char takes width 2
        let text = Rope::from("AB漢D\n");
        // Visual columns: A=0-1, B=1-2, 漢=2-4, D=4-5
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        // Range [2, 5) covers 漢 and D
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            2,
            5,
            RaggedLinePolicy::Empty,
        );
        // 漢 starts at char 2, D ends at char 4
        assert_eq!(
            mapping.char_range(),
            line_char_start + 2..line_char_start + 4
        );
    }

    #[test]
    fn column_mapping_tab() {
        // Tab takes tab_width columns
        let text = Rope::from("A\tB\n");
        // Visual with tab_width=4: A=0-1, tab=1-5, B=5-6
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        // Range [1, 5) covers the tab
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            1,
            5,
            RaggedLinePolicy::Empty,
        );
        // Tab is at char 1, char after tab is 2
        assert_eq!(
            mapping.char_range(),
            line_char_start + 1..line_char_start + 2
        );
    }

    #[test]
    fn column_mapping_short_line_before_left() {
        // Line "Hi" (visual width 2), left=5, right=8
        let text = Rope::from("Hi\nlonger line\n");
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            5,
            8,
            RaggedLinePolicy::Empty,
        );
        // Line is shorter than left => empty range
        assert!(mapping.is_empty());
        assert!(mapping.line_shorter_than_left);
    }

    #[test]
    fn column_mapping_line_ends_mid_range() {
        // Line "Hello" (visual width 5), left=3, right=8
        let text = Rope::from("Hello\n");
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            3,
            8,
            RaggedLinePolicy::Empty,
        );
        // Should return [3, 5) — only the existing chars, no padding
        assert_eq!(
            mapping.char_range(),
            line_char_start + 3..line_char_start + 5
        );
        assert!(!mapping.line_shorter_than_left);
    }

    #[test]
    fn column_mapping_zero_width() {
        let text = Rope::from("Hello\n");
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            3,
            3,
            RaggedLinePolicy::Empty,
        );
        assert!(mapping.is_empty());
    }

    #[test]
    fn column_mapping_cjk_partial_overlap() {
        // "漢字" — both CJK, each width 2
        // Visual: 漢=0-2, 字=2-4
        let text = Rope::from("漢字\n");
        let slice = line_slice(&text, 0);
        let line_char_start = text.line_to_char(0);
        // Range [1, 3) overlaps both 漢 and 字
        let mapping = visual_column_range_to_char_range(
            4,
            &slice,
            line_char_start,
            1,
            3,
            RaggedLinePolicy::Empty,
        );
        // Both graphemes overlap the range, so chars 0..2
        assert_eq!(
            mapping.char_range(),
            line_char_start + 0..line_char_start + 2
        );
    }

    #[test]
    fn column_mapping_three_consumers_share_same_range() {
        // C-4: highlight, copy, cut must use identical char ranges.
        // All three call the same visual_column_range_to_char_range,
        // so we just verify it returns the same result for the same input.
        let text = Rope::from("Hello\nWorld\nShort\n");
        let tab_width = 4;

        for line_idx in 0..3 {
            let slice = line_slice(&text, line_idx);
            let line_char_start = text.line_to_char(line_idx);
            for left in 0..8 {
                for right in left..8 {
                    let mapping1 = visual_column_range_to_char_range(
                        tab_width,
                        &slice,
                        line_char_start,
                        left,
                        right,
                        RaggedLinePolicy::Empty,
                    );
                    let mapping2 = visual_column_range_to_char_range(
                        tab_width,
                        &slice,
                        line_char_start,
                        left,
                        right,
                        RaggedLinePolicy::Empty,
                    );
                    assert_eq!(
                        mapping1, mapping2,
                        "line={} left={} right={}",
                        line_idx, left, right
                    );
                }
            }
        }
    }
}
