use crate::buffer::Buffer;
use derive_more::{Add, Deref, From, Sub};
use std::ops::Range;

#[derive(Clone, Copy, Default, From, Deref, Add, Sub)]
pub struct BufCharIdx(pub usize);

#[derive(Clone, Copy, Default, From, Deref, Add, Sub)]
pub struct BufByteIdx(pub usize);

#[derive(Clone, Copy, Default, From)]
pub struct BufRange {
    pub start: BufCharIdx,
    pub end: BufCharIdx
}

impl BufRange {
    pub fn new(start: BufCharIdx, end: BufCharIdx) -> Self {
        if *end < *start {
            Self { start: end, end: start }
        } else {
            Self { start, end }
        }
    }
}

impl From<BufRange> for Range<usize> {
    fn from(range: BufRange) -> Self {
        *range.start..*range.end
    }
}

impl From<Range<BufCharIdx>> for BufRange {
    fn from(range: Range<BufCharIdx>) -> Self {
        Self::new(range.start, range.end)
    }
}

#[derive(Clone, Copy, Default, From, Deref, Add, Sub)]
pub struct BufCol(pub usize);

impl BufCol {
    pub fn as_termcol(self) -> TermCol {
        TermCol(self.0 as u16)
    }
}

#[derive(Clone, Copy, Default, From, Deref, Add, Sub)]
pub struct BufRow(pub usize);

impl BufRow {
    pub fn as_termrow(self) -> TermRow {
        TermRow(self.0 as u16)
    }
}

#[derive(Clone, Copy, Default)]
pub struct BufPos {
    pub x: BufCol,
    pub y: BufRow,
}

impl BufPos {
    pub fn new(x: BufCol, y: BufRow) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Default, From, Deref, Add, Sub)]
pub struct TermCol(pub u16);

impl TermCol {
    pub fn as_bufcol(self) -> BufCol {
        BufCol(self.0 as usize)
    }
}

#[derive(Clone, Copy, Default, From, Deref, Add, Sub)]
pub struct TermRow(pub u16);

impl TermRow {
    pub fn as_bufrow(self) -> BufRow {
        BufRow(self.0 as usize)
    }
}

#[derive(Clone, Copy, Default)]
pub struct TermPos {
    pub x: TermCol,
    pub y: TermRow,
}

impl TermPos {
    pub fn new(x: TermCol, y: TermRow) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy)]
pub enum Movement {
    Up(usize),
    Down(usize),
    Left(usize),
    Right(usize),
    Home,
    End,
    Top,
    Bottom,
    FirstChar,
    NextWord(usize),
    PrevWord(usize),
}

impl Movement {
    pub fn is_horizontal(&self) -> bool {
        match self {
            Movement::Up(_) => false,
            Movement::Down(_) => false,
            Movement::Top => false,
            Movement::Bottom => false,
            _ => true,
        }
    }

    pub fn dest(&self, buf: &Buffer) -> BufCharIdx {
        match &self {
            Movement::Up(amount) => {
                let y = buf.row().saturating_sub(*amount).into();
                let x = usize::min(*buf.max_col(y), *buf.saved_col).into();
                buf.line_to_char(y) + x
            }
            Movement::Down(amount) => {
                let y =
                    usize::min(*buf.row() + amount, buf.text.len_lines().saturating_sub(1)).into();
                let x = usize::min(*buf.max_col(y), *buf.saved_col).into();
                buf.line_to_char(y) + x
            }
            Movement::Left(amount) => usize::max(
                buf.idx.saturating_sub(*amount),
                *buf.line_to_char(buf.row()),
            )
            .into(),
            Movement::Right(amount) => usize::min(
                *buf.idx + amount,
                *buf.line_to_char(buf.row()) + *buf.max_col(buf.row()),
            )
            .into(),
            Movement::Home => buf.line_to_char(buf.row()),
            Movement::End => buf.line_to_char(buf.row() + BufRow(1)) - BufCharIdx(1),
            Movement::FirstChar => {
                unimplemented!()
            }
            Movement::Top => BufCharIdx(0),
            Movement::Bottom => buf.text.len_chars().into(),
            Movement::NextWord(_amount) => {
                unimplemented!()
            }
            Movement::PrevWord(_amount) => {
                unimplemented!()
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Selection {
    Bounds(BufCharIdx, BufCharIdx),
    Lines(usize),
    UpTo(Movement),
    Between {
        first: char,
        last: char,
        inclusive: bool,
    },
    //    Surrounders(char, char),
    Word {
        inclusive: bool,
    },
    Paragraph {
        inclusive: bool,
    },
}

impl Selection {
    pub fn bounds(&self, buf: &Buffer) -> BufRange {
        match self {
            Selection::Bounds(start, end) => *start..*end,
            Selection::Lines(amount) => {
                let start = buf.line_to_char(buf.row());
                let dest = usize::min(*buf.row() + amount, buf.text.len_lines()).into();
                let end = buf.line_to_char(dest);
                start..end
            }
            Selection::UpTo(mov) => buf.idx..mov.dest(buf),
            Selection::Between {
                // TODO: implement
                first: _,
                last: _,
                inclusive: _,
            } => BufCharIdx(0)..BufCharIdx(0),
            Selection::Word { inclusive: _ } => BufCharIdx(0)..BufCharIdx(0), // TODO: implement
            Selection::Paragraph { inclusive: _ } => BufCharIdx(0)..BufCharIdx(0), // TODO: implement
        }.into()
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::Lines(0)
    }
}
