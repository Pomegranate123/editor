use crate::{
    buffer::Buffer,
    rect::Rect,
    utils::{TermCol, TermRow, TermPos, BufRange},
};
use crossterm::{
    cursor::{
        CursorShape, Hide, MoveTo, RestorePosition, SavePosition,
        SetCursorShape, Show,
    },
    queue,
    style::{ContentStyle, Print, ResetColor, SetAttributes, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType, ScrollUp, ScrollDown},
    Result,
};
use std::{
    io::{self, Write, Stdout},
    fmt::Display,
};

pub struct Renderer(Stdout);

impl Renderer {
    pub fn new() -> Self {
        Self(io::stdout())
    }

    pub fn set_style(&mut self, style: &ContentStyle) -> Result<()> {
        if let Some(fg) = style.foreground_color {
            queue!(self.0, SetForegroundColor(fg))?
        };
        if let Some(bg) = style.background_color {
            queue!(self.0, SetBackgroundColor(bg))?
        };
        queue!(self.0, SetAttributes(style.attributes))
    }

    pub fn reset_style(&mut self) -> Result<()> {
        queue!(self.0, ResetColor)
    }

    pub fn save_cursor(&mut self) -> Result<()> {
        queue!(self.0, SavePosition, Hide)
    }

    pub fn restore_cursor(&mut self) -> Result<()> {
        queue!(self.0, RestorePosition, Show)
    }

    pub fn set_cursor_shape(&mut self, shape: CursorShape) -> Result<()> {
        queue!(self.0, SetCursorShape(shape))
    }

    pub fn scroll_down(&mut self, amount: u16) -> Result<()> {
        queue!(self.0, ScrollDown(amount))
    }

    pub fn scroll_up(&mut self, amount: u16) -> Result<()> {
        queue!(self.0, ScrollUp(amount))
    }

    pub fn move_to(&mut self, x: impl Into<TermCol>, y: impl Into<TermRow>) -> Result<()> {
        queue!(self.0, MoveTo(*x.into(), *y.into()))
    }

    pub fn print(&mut self, content: impl Display) -> Result<()> {
        queue!(self.0, Print(content))
    }

    pub fn clear(&mut self, cleartype: ClearType) -> Result<()> {
        queue!(self.0, Clear(cleartype))
    }

    pub fn print_range(&mut self, rect: &Rect, buf: &Buffer, range: BufRange) -> Result<()> {
        let mut start = rect.terminal_pos(buf.char_to_pos(range.start));
        let lines = buf.slice(range.into()).lines();
        for line in lines {
            self.move_to(start.x, start.y)?;
            self.clear(ClearType::UntilNewLine)?;
            self.print(line)?;
            start = TermPos::new(rect.offset.x, *start.y + 1);
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

