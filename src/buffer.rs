use crate::{
    action::{BufferAction, Action, Undoable},
    utils::{BufByteIdx, BufCharIdx, BufCol, BufPos, BufRow, BufRange},
};
use ropey::{Rope, RopeSlice};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    ops::Range,
    path::PathBuf,
};

#[derive(Clone, Copy)]
pub enum EditMode {
    Normal,
    Insert,
}

impl Default for EditMode {
    fn default() -> Self {
        EditMode::Normal
    }
}

#[derive(Default)]
pub struct Buffer {
    /// Rope represtation of the contents of this buffer
    pub text: Rope,
    /// Current index of the cursor within the rope
    pub idx: BufCharIdx,
    /// The column index the cursor will snap to when moving between lines
    pub saved_col: BufCol,
    /// The mode the buffer is currently in
    pub mode: EditMode,
    /// Whether the buffer has been edited since saving
    pub edited: bool,
    /// The path of the file being edited
    pub path: PathBuf,
    pub undo: Vec<BufferAction>,
    pub redo: Vec<BufferAction>,
}

impl Buffer {
    pub fn new(path: PathBuf) -> Self {
        let text = Rope::from_reader(BufReader::new(File::open(&path).unwrap())).unwrap();
        Self {
            text,
            edited: false,
            path,
            ..Default::default()
        }
    }

    /// Returns which row the cursor is on
    pub fn row(&self) -> BufRow {
        self.char_to_row(self.idx)
    }

    /// Returns which column the cursor is on
    pub fn col(&self) -> BufCol {
        self.char_to_col(self.idx)
    }

    /// Returns the column of the last character in a given row
    pub fn max_col(&self, row: BufRow) -> BufCol {
        self.text.line(*row).len_chars().saturating_sub(1).into()
    }

    pub fn insert(&mut self, i: BufCharIdx, string: &str) {
        self.text.insert(*i, string);
    }

    pub fn remove(&mut self, range: BufRange) {
        self.idx = range.start;
        let range: Range<usize> = range.into();
        self.text.remove(range);
    }

    pub fn cursor(&self) -> BufPos {
        BufPos::new(self.col(), self.row())
    }

    pub fn char_to_col(&self, character: BufCharIdx) -> BufCol {
        (*character - *self.row_to_char(self.char_to_row(character))).into()
    }

    pub fn char_to_row(&self, character: BufCharIdx) -> BufRow {
        self.text.char_to_line(*character).into()
    }

    pub fn char_to_pos(&self, character: BufCharIdx) -> BufPos {
        BufPos::new(self.char_to_col(character), self.char_to_row(character))
    }

    pub fn row_to_char(&self, row: BufRow) -> BufCharIdx {
        self.text.line_to_char(*row).into()
    }

    pub fn byte_to_char(&self, byte: BufByteIdx) -> BufCharIdx {
        self.text.byte_to_char(*byte).into()
    }

    pub fn row_to_byte(&self, row: BufRow) -> BufByteIdx {
        self.text.line_to_byte(*row).into()
    }

    pub fn slice(&self, range: BufRange) -> RopeSlice<'_> {
        let range: Range<usize> = range.into();
        self.text.slice(range)
    }

    pub fn save_col(&mut self) {
        self.saved_col = self.col();
    }

    #[allow(unused)]
    /// Saves the current state of the buffer to the file
    pub fn write(&mut self) {
        self.text
            .write_to(BufWriter::new(
                File::create(&self.path)
                    .expect("Unable to create new/read existing file at buffer path"),
            ))
            .unwrap();
        self.edited = false;
    }

    pub fn apply(&mut self, action: BufferAction) -> Result<(), &'static str>{
        if let BufferAction::Delete(_) | BufferAction::Insert(_) | BufferAction::InsertAt(_, _) = action {
            self.redo.clear();
            self.undo.push(action.inverse(self));
        }
        action.apply(self)
    }
}
