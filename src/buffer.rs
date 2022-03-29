use crate::utils::{BufByteIdx, BufCharIdx, BufCol, BufPos, BufRow};
use ropey::{Rope, RopeSlice};
use std::{fs::File, io::BufReader, ops::Range, path::Path};

#[derive(Clone, Copy)]
pub enum EditMode {
    Normal,
    Insert,
    Command,
}

impl Default for EditMode {
    fn default() -> Self {
        EditMode::Normal
    }
}

#[derive(Default)]
pub struct Buffer {
    pub text: Rope,
    pub idx: BufCharIdx,
    pub saved_col: BufCol,
    pub mode: EditMode,
}

impl Buffer {
    pub fn new(path: &Path) -> Self {
        let text = Rope::from_reader(BufReader::new(File::open(path).unwrap())).unwrap();
        Self {
            text,
            ..Default::default()
        }
    }

    /// Returns which row the cursor is on
    pub fn row(&self) -> BufRow {
        self.char_to_line(self.idx)
    }

    /// Returns which column the cursor is on
    pub fn col(&self) -> BufCol {
        (*self.idx - *self.line_to_char(self.row())).into()
    }

    /// Returns the column of the last character in a given line
    pub fn max_col(&self, line: BufRow) -> BufCol {
        self.text.line(*line).len_chars().saturating_sub(1).into()
    }

    pub fn insert(&mut self, i: BufCharIdx, string: &str) {
        self.text.insert(*i, string);
    }

    pub fn remove(&mut self, range: Range<BufCharIdx>) {
        self.idx = range.start;
        let range: Range<usize> = *range.start..*range.end;
        self.text.remove(range);
    }

    pub fn cursor(&self) -> BufPos {
        BufPos::new(self.col(), self.row())
    }

    pub fn char_to_line(&self, character: BufCharIdx) -> BufRow {
        self.text.char_to_line(*character).into()
    }

    pub fn line_to_char(&self, line: BufRow) -> BufCharIdx {
        self.text.line_to_char(*line).into()
    }

    pub fn byte_to_char(&self, byte: BufByteIdx) -> BufCharIdx {
        self.text.byte_to_char(*byte).into()
    }

    pub fn line_to_byte(&self, line: BufRow) -> BufByteIdx {
        self.text.line_to_byte(*line).into()
    }

    pub fn slice(&self, range: Range<BufCharIdx>) -> RopeSlice<'_> {
        let range = *range.start..*range.end;
        self.text.slice(range)
    }
}
