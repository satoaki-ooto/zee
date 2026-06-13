use euclid::default::Vector2D;
use ropey::Rope;
use smallvec::SmallVec;
use std::ops::{Deref, DerefMut};

use crate::{movement, CompoundDiff, Cursor, OpaqueDiff};

#[derive(Debug, Clone)]
pub struct Revision {
    text: Rope,
    cursor: Cursor,
    pub parent: Option<Reference>,
    pub children: SmallVec<[Reference; 1]>,
    pub redo_index: usize,
}

impl Revision {
    fn root(text: Rope) -> Self {
        let mut cursor = Cursor::new();
        movement::move_to_start_of_buffer(&text, &mut cursor);
        Self {
            text,
            cursor,
            parent: None,
            children: SmallVec::new(),
            redo_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub index: usize,
    diff: CompoundDiff,
}

#[derive(Debug, Clone)]
pub struct EditTree {
    pub revisions: Vec<Revision>,
    pub head_index: usize,
    staged: Rope,
    has_staged_changes: bool,
}

impl EditTree {
    pub fn new(text: Rope) -> Self {
        Self {
            revisions: vec![Revision::root(text.clone())],
            head_index: 0,
            staged: text,
            has_staged_changes: false,
        }
    }

    pub fn next_child(&mut self) {
        let current_revision = &mut self.revisions[self.head_index];
        if current_revision.redo_index < current_revision.children.len().saturating_sub(1) {
            current_revision.redo_index += 1;
        }
    }

    pub fn previous_child(&mut self) {
        let current_revision = &mut self.revisions[self.head_index];
        if current_revision.redo_index > 0 {
            current_revision.redo_index -= 1;
        }
    }

    /// Create a revision from a single OpaqueDiff (backward compat convenience).
    pub fn create_revision(&mut self, diff: OpaqueDiff, cursor: Cursor) {
        self.create_compound_revision(CompoundDiff::single(diff), cursor)
    }

    /// Create a revision from a compound diff (multiple sub-diffs).
    pub fn create_compound_revision(&mut self, diff: CompoundDiff, cursor: Cursor) {
        let parent_to_child_diff = diff;
        let child_to_parent_diff = parent_to_child_diff.reverse();
        let new_revision_index = self.revisions.len();

        self.revisions.push(Revision {
            text: self.staged.clone(),
            cursor,
            parent: Some(Reference {
                index: self.head_index,
                diff: child_to_parent_diff,
            }),
            children: SmallVec::new(),
            redo_index: 0,
        });
        {
            let head = &mut self.revisions[self.head_index];
            head.children.push(Reference {
                index: new_revision_index,
                diff: parent_to_child_diff,
            });
            head.redo_index = head.children.len() - 1;
        }
        self.head_index = new_revision_index;
        self.has_staged_changes = false;
    }

    pub fn undo(&mut self) -> Option<(CompoundDiff, Cursor)> {
        if let Some(Reference {
            ref diff,
            index: previous_index,
        }) = self.revisions[self.head_index].parent
        {
            let previous_revision = &self.revisions[previous_index];
            self.staged = previous_revision.text.clone();
            self.head_index = previous_index;

            self.has_staged_changes = false;
            Some((diff.clone(), previous_revision.cursor.clone()))
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<(CompoundDiff, Cursor)> {
        let Self {
            revisions,
            head_index,
            staged,
            has_staged_changes,
            ..
        } = self;
        let Revision {
            ref children,
            redo_index,
            ..
        } = revisions[*head_index];
        children
            .get(redo_index)
            .map(|Reference { ref diff, index }| {
                let Revision {
                    ref cursor,
                    ref text,
                    ..
                } = revisions[*index];
                *staged = text.clone();
                *has_staged_changes = false;
                *head_index = *index;
                (diff.clone(), cursor.clone())
            })
    }

    pub fn staged(&self) -> &Rope {
        self.deref()
    }

    pub fn staged_mut(&mut self) -> &mut Rope {
        self.deref_mut()
    }
}

impl Deref for EditTree {
    type Target = Rope;

    fn deref(&self) -> &Rope {
        &self.staged
    }
}

impl DerefMut for EditTree {
    fn deref_mut(&mut self) -> &mut Rope {
        self.has_staged_changes = true;
        &mut self.staged
    }
}

pub struct FormattedRevision {
    pub transform: Vector2D<isize>,
    pub current_branch: bool,
}

pub fn format_revision(
    revisions: &[Revision],
    formatted: &mut [FormattedRevision],
    index: usize,
    transform: Vector2D<isize>,
    current_branch: bool,
) -> isize {
    {
        let formatted_revision = &mut formatted[index];
        formatted_revision.transform = transform;
        formatted_revision.current_branch = current_branch;
    }

    let revision = &revisions[index];
    let mut subtree_width = 0;
    for (child_index, child) in revision.children.iter().enumerate() {
        if child_index > 0 {
            subtree_width += 8;
        }
        subtree_width += format_revision(
            revisions,
            formatted,
            child.index,
            transform + Vector2D::new(subtree_width, 2),
            current_branch && (child_index == revision.redo_index),
        );
    }
    subtree_width
}

pub fn format_tree(tree: &EditTree) -> Vec<FormattedRevision> {
    let mut formatted = Vec::with_capacity(tree.revisions.len());
    formatted.resize_with(tree.revisions.len(), || FormattedRevision {
        transform: Vector2D::zero(),
        current_branch: true,
    });
    format_revision(&tree.revisions, &mut formatted, 0, Vector2D::zero(), true);
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_with_revisions_and_no_undo() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars();
        tree.insert(position, " so...\n");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars();
        tree.insert(position, "dunno.");

        assert_eq!("The flowers are... so...\ndunno.", &tree.to_string());
    }

    #[test]
    fn undo_at_root_has_no_effect() {
        let mut tree = EditTree::new("The flowers are violet.\n".into());
        assert_eq!("The flowers are violet.\n", &tree.to_string());
        assert_eq!(None, tree.undo());
        assert_eq!("The flowers are violet.\n", &tree.to_string());
    }

    #[test]
    fn insert_and_undo() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars();
        tree.insert(position, " so...\n");
        let position = tree.len_chars();
        tree.insert(position, "dunno.");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        assert_eq!("The flowers are... so...\ndunno.", &tree.to_string());
        tree.undo();
        assert_eq!("The flowers are...", &tree.to_string());

        let position = tree.len_chars();
        tree.insert(position, " violet.");
        assert_eq!("The flowers are... violet.", &tree.to_string());
    }

    #[test]
    fn undo_redo_idempotent() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars();
        tree.insert(position, " so...\n");
        let position = tree.len_chars();
        tree.insert(position, "dunno.");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        assert_eq!("The flowers are... so...\ndunno.", &tree.to_string());
        tree.undo();
        assert_eq!("The flowers are...", &tree.to_string());
        tree.redo();
        assert_eq!("The flowers are... so...\ndunno.", &tree.to_string());
        tree.undo();
        assert_eq!("The flowers are...", &tree.to_string());
        tree.undo();
        assert_eq!("", &tree.to_string());
    }

    #[test]
    fn render_undo_tree() {}

    // --- Undo/Redo regression tests (C-3 baseline) ---

    #[test]
    fn single_edit_undo_restores_exact_text() {
        let mut tree = EditTree::new(Rope::from("abc"));
        tree.insert(3, "def");
        tree.create_revision(
            OpaqueDiff::new(3, 0, 3, 3, 0, 3),
            Cursor::end_of_buffer(&tree),
        );
        assert_eq!("abcdef", &tree.to_string());
        let (compound, cursor) = tree.undo().unwrap();
        assert_eq!("abc", &tree.to_string());
        let diff = &compound.0[0];
        assert_eq!(diff.char_index, 3);
        assert_eq!(diff.old_char_length, 3); // reverse: old=new of forward
    }

    #[test]
    fn single_edit_redo_restores_exact_text() {
        let mut tree = EditTree::new(Rope::from("abc"));
        tree.insert(3, "def");
        tree.create_revision(
            OpaqueDiff::new(3, 0, 3, 3, 0, 3),
            Cursor::end_of_buffer(&tree),
        );
        tree.undo();
        let (compound, _) = tree.redo().unwrap();
        assert_eq!("abcdef", &tree.to_string());
        let diff = &compound.0[0];
        assert_eq!(diff.char_index, 3);
        assert_eq!(diff.new_char_length, 3);
    }

    #[test]
    fn undo_returns_correct_cursor() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "Hello");
        let cursor_before = Cursor::end_of_buffer(&tree);
        tree.create_revision(OpaqueDiff::empty(), cursor_before.clone());
        // Insert more text
        tree.insert(5, " world");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));
        let (_, undo_cursor) = tree.undo().unwrap();
        assert_eq!(undo_cursor, cursor_before);
    }

    #[test]
    fn redo_at_leaf_returns_none() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "Hello");
        tree.create_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));
        assert_eq!(None, tree.redo());
    }

    // --- Compound diff tests (C-1, C-2, C-3) ---

    /// Helper: apply a compound diff to a Rope by removing char ranges in
    /// descending order (as rectangle cut would do).
    fn apply_compound_deletion(text: &mut Rope, compound: &CompoundDiff) {
        // Sub-diffs are already in descending char_index order
        for diff in &compound.0 {
            if !diff.is_empty() && diff.old_char_length > 0 {
                text.remove(diff.char_index..diff.char_index + diff.old_char_length);
            }
        }
    }

    #[test]
    fn compound_diff_single_is_degenerate_case() {
        // C-3: single OpaqueDiff in a CompoundDiff works the same
        let mut tree = EditTree::new(Rope::from("Hello world"));
        tree.insert(0, "X");
        let diff = OpaqueDiff::new(0, 0, 1, 0, 0, 1);
        tree.create_compound_revision(CompoundDiff::single(diff), Cursor::end_of_buffer(&tree));
        assert_eq!("XHello world", &tree.to_string());
        let (compound, _) = tree.undo().unwrap();
        assert_eq!("Hello world", &tree.to_string());
        assert_eq!(compound.0.len(), 1);
    }

    #[test]
    fn compound_diff_multi_line_undo_is_atomic() {
        // C-1: N-line discontinuous deletion as 1 revision, undo 1 click restores all
        let text = Rope::from("ABCDE\nFGHIJ\nKLMNO\n");
        let mut tree = EditTree::new(text);

        // Simulate rectangle cut of columns 1..4 from lines 0,1,2
        // Line 0: "ABCDE\n" -> remove chars 1..4 => "AE\n"
        // Line 1: "FGHIJ\n" -> remove chars 7..10 => "FJ\n"
        // Line 2: "KLMNO\n" -> remove chars 13..16 => "KO\n"
        // Diffs in descending char_index order:
        let diff2 = OpaqueDiff::new(13, 3, 0, 13, 3, 0); // line 2
        let diff1 = OpaqueDiff::new(7, 3, 0, 7, 3, 0);   // line 1
        let diff0 = OpaqueDiff::new(1, 3, 0, 1, 3, 0);    // line 0

        let compound = CompoundDiff(vec![diff2, diff1, diff0]);

        // Apply to staged
        apply_compound_deletion(tree.staged_mut(), &compound);

        tree.create_compound_revision(compound, Cursor::with_range(1..2));

        assert_eq!("AE\nFJ\nKO\n", &tree.to_string());

        // Undo 1 click should restore all lines
        let (undo_compound, _) = tree.undo().unwrap();
        assert_eq!("ABCDE\nFGHIJ\nKLMNO\n", &tree.to_string());

        // Redo 1 click should re-apply all lines
        let (redo_compound, _) = tree.redo().unwrap();
        assert_eq!("AE\nFJ\nKO\n", &tree.to_string());
    }

    #[test]
    fn compound_diff_outside_ranges_unchanged() {
        // C-2: outside [0,left), [right,EOL], and non-selected lines unchanged
        let text = Rope::from("ABCDE\nFGHIJ\nKLMNO\nPQRST\n");
        let mut tree = EditTree::new(text);

        // Rectangle: lines 1..2 (FGHIJ, KLMNO), columns 1..4
        // Line 1: "FGHIJ\n" chars: F(6) G(7) H(8) I(9) J(10) \n(11)
        //   Remove [7, 10) = "GHI" => "FJ\n"
        // Line 2: "KLMNO\n" chars: K(12) L(13) M(14) N(15) O(16) \n(17)
        //   Remove [13, 16) = "LMN" => "KO\n"
        // Lines 0 and 3 unchanged
        let diff1 = OpaqueDiff::new(13, 3, 0, 13, 3, 0); // line 2 first (higher char_index)
        let diff0 = OpaqueDiff::new(7, 3, 0, 7, 3, 0);    // line 1

        let compound = CompoundDiff(vec![diff1, diff0]);
        apply_compound_deletion(tree.staged_mut(), &compound);
        tree.create_compound_revision(compound, Cursor::with_range(7..8));

        assert_eq!("ABCDE\nFJ\nKO\nPQRST\n", &tree.to_string());

        // Verify lines 0 and 3 are unchanged
        assert_eq!("ABCDE\n", &tree.line(0).to_string());
        assert_eq!("PQRST\n", &tree.line(3).to_string());
    }

    #[test]
    fn normal_edit_undo_granularity_unchanged_after_compound() {
        // C-3: normal single-edit undo granularity is unchanged
        let mut tree = EditTree::new(Rope::from("Hello"));
        // Normal edit: insert 'X' at position 0
        tree.insert(0, "X");
        tree.create_revision(OpaqueDiff::new(0, 0, 1, 0, 0, 1), Cursor::with_range(1..2));
        assert_eq!("XHello", &tree.to_string());

        // Now do a compound edit: remove "el" (chars 2..4) from "XHello"
        let compound = CompoundDiff(vec![
            OpaqueDiff::new(2, 2, 0, 2, 2, 0),
        ]);
        apply_compound_deletion(tree.staged_mut(), &compound);
        tree.create_compound_revision(compound, Cursor::with_range(2..3));
        assert_eq!("XHlo", &tree.to_string());

        // Undo the compound: should go back to "XHello"
        tree.undo();
        assert_eq!("XHello", &tree.to_string());

        // Undo the single edit: should go back to "Hello"
        tree.undo();
        assert_eq!("Hello", &tree.to_string());
    }
}
