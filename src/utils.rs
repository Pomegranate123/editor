use crate::buffer::Buffer;
use std::ops::Range;

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

#[derive(Clone, Copy)]
pub enum Selection {
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

impl Movement {
    pub fn dest(&self, buf: &Buffer) -> usize {
        match &self {
            Movement::Up(amount) => {
                let y = buf.row().saturating_sub(*amount);
                let x = usize::min(buf.max_col(y), buf.saved_col);
                buf.content.line_to_char(y) + x
            }
            Movement::Down(amount) => {
                let y = usize::min(
                    buf.row() + amount,
                    buf.content.len_lines().saturating_sub(1),
                );
                let x = usize::min(buf.max_col(y), buf.saved_col);
                buf.content.line_to_char(y) + x
            }
            Movement::Left(amount) => usize::max(
                buf.idx.saturating_sub(*amount),
                buf.content.line_to_char(buf.row()),
            ),
            Movement::Right(amount) => usize::min(
                buf.idx + amount,
                buf.content.line_to_char(buf.row()) + buf.max_col(buf.row()),
            ),
            Movement::Home => buf.content.line_to_char(buf.row()),
            Movement::End => buf.content.line_to_char(buf.row()) + buf.max_col(buf.row()),
            Movement::FirstChar => {
                unimplemented!()
            }
            Movement::Top => {
                let y = buf.offset_x + 3;
                let x = usize::min(buf.max_col(y), buf.saved_col);
                buf.content.line_to_char(y) + x
            }
            Movement::Bottom => {
                let y = buf.offset_x + buf.height - 3;
                let x = usize::min(buf.max_col(y), buf.saved_col);
                buf.content.line_to_char(y) + x
            }
            Movement::NextWord(_amount) => {
                unimplemented!()
            }
            Movement::PrevWord(_amount) => {
                unimplemented!()
            }
        }
    }
}

impl Selection {
    pub fn bounds(&self, buf: &Buffer) -> Range<usize> {
        match self {
            Selection::Lines(amount) => {
                let start = buf.content.line_to_char(buf.row());
                let dest = usize::min(buf.row() + amount, buf.content.len_lines());
                let end = buf.content.line_to_char(dest);
                start..end
            }
            Selection::UpTo(mov) => buf.idx..mov.dest(&buf),
            Selection::Between {
                // TODO: implement
                first: _,
                last: _,
                inclusive: _,
            } => 0..0,
            Selection::Word { inclusive: _ } => 0..0, // TODO: implement
            Selection::Paragraph { inclusive: _ } => 0..0, // TODO: implement
        }
    }
}
