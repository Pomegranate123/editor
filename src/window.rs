use crate::{
    action::Action,
    buffer::{Buffer, EditMode},
    config::Config,
    highlight::{Highlighter, language},
    input::InputHandler,
    rect::Rect,
    render::Renderer,
    utils::{BufRow, TermCol, TermRow, BufRange},
};
use crossterm::{
    cursor::{
        CursorShape,
    },
    event::KeyEvent,
    terminal::{self, ClearType},
    Result,
};
use std::{
    path::PathBuf,
};
use tree_sitter_highlight::HighlightEvent;

pub struct Window {
    /// The buffer displayed by the window
    pub buf: Buffer,
    /// The renderer used to draw stuff onto the terminal
    renderer: Renderer,
    /// The space the window gets to render
    pub rect: Rect,
    /// Configuration for this window
    config: Config,
    hl: Highlighter,
}

impl Window {
    pub fn new(path: PathBuf, config: Config) -> Self {
        let hl = Highlighter::new(language::detect(&path), config.hl.clone());
        let buf = Buffer::new(path);
        let (width, height) = terminal::size().unwrap();
        let line_nrs_width = buf.text.len_lines().to_string().len() as u16 + 1;

        Window {
            buf,
            renderer: Renderer::new(),
            rect: Rect::new(
                width - line_nrs_width,
                height,
                line_nrs_width,
                0,
            ),
            config,
            hl,
        }
    }

    pub fn update_size(&mut self, width: u16, height: u16) {
        self.rect.resize(
            TermCol(width) - self.rect.offset.x,
            TermRow(height),
        );
    }

    fn draw_line_nrs(&mut self) -> Result<()> {
        self.rect.offset.x = TermCol(self.buf.text.len_lines().to_string().len() as u16 + 1);
        self.renderer.save_cursor()?;
        for line_nr in 0..*self.rect.height {
            self.renderer.move_to(0, line_nr)?;
            let nr = (line_nr as i64 - (*self.rect.terminal_y(self.buf.row())) as i64).abs() as usize;
            let (style, nr) = if nr == 0 {
                (self.config.line_nr_active, *self.buf.row() + 1)
            } else {
                (self.config.line_nr_column, nr)
            };
            self.renderer.set_style(&style)?;
            self.renderer.print(format!("{: >width$} ", nr, width = *self.rect.offset.x as usize - 1))?;
        }
        self.renderer.restore_cursor()?;
        Ok(())
    }

    pub fn draw_all(&mut self) -> Result<()> {
        self.update_cursor()?;
        self.draw(self.rect.top())?;
        Ok(())
    }

    /// Draws the buffer in the given view starting from the line at index `begin`.
    pub fn draw(&mut self, first_line: BufRow) -> Result<()> {
        let last_line: BufRow = (self.rect.bottom() - 1.into()).min(self.buf.text.len_lines()).into();
    
        self.renderer.save_cursor()?;
        self.renderer.move_to(self.rect.terminal_x(0.into()), self.rect.terminal_y(first_line))?;
        self.renderer.clear(ClearType::UntilNewLine)?;
    
        let rendered_bytes = self.buf.row_to_byte(first_line)..self.buf.row_to_byte(last_line);
        if !self.hl.has_hl() {
            self.hl.update_hl(&self.buf);
        }
        for event in self.hl.get_hl() {
            match event {
                HighlightEvent::Source { start, end } => {
                    if *start > *rendered_bytes.end || *end <= *rendered_bytes.start {
                        continue;
                    }
                    let first = self.buf.byte_to_char(usize::max(*start, *rendered_bytes.start).into());
                    let last = self.buf.byte_to_char(usize::min(*end, *rendered_bytes.end).into());
                    self.renderer.print_range(&self.rect, &self.buf, BufRange::new(first, last))?;
                }
                HighlightEvent::HighlightStart(s) => self.renderer.set_style(self.hl.get_style(&s))?,
                HighlightEvent::HighlightEnd => self.renderer.reset_style()?,
            }
        }
        self.renderer.move_to(10, self.rect.terminal_y(self.rect.bottom()))?;
        self.renderer.print(format!("{}:{}", *self.buf.row(), *self.buf.col()))?;
    
        self.renderer.restore_cursor()?;
        Ok(())
    }

    pub fn update_cursor(&mut self) -> Result<()> {
        match self.buf.mode {
            EditMode::Normal => self.renderer.set_cursor_shape(CursorShape::Block)?,
            EditMode::Insert => self.renderer.set_cursor_shape(CursorShape::Line)?,
        }
        let cursor = self.buf.cursor();
        let dy = self.rect.scroll_to_cursor(cursor);
        if dy < 0 {
            self.renderer.scroll_down(dy.abs() as u16)?;
        } else if dy > 0 {
            self.renderer.scroll_up(dy.abs() as u16)?;
        }
        let pos = self.rect.terminal_pos(cursor);
        self.renderer.move_to(pos.x, pos.y)?;
        self.draw_line_nrs()
    }

    pub fn handle_keyevent(&mut self, key_event: KeyEvent) -> Result<()> {
        match self.buf.mode {
            EditMode::Normal => {
                match InputHandler::parse_normal(key_event) {
                    Some(command) => {
                        self.buf.apply(command.buffer_action).unwrap_or(());
                        command.render_action.apply(self)?;
                    }
                    None => (),
                }
            }
            EditMode::Insert => {
                match InputHandler::parse_insert(key_event) {
                    Some(command) => {
                        self.buf.apply(command.buffer_action).unwrap_or(());
                        command.render_action.apply(self)?;
                    }
                    None => (),
                }
            }
        }
        self.renderer.flush()?;
        Ok(())
    }
}
