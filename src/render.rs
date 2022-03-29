use crate::{
    action::Move,
    buffer::{Buffer, EditMode},
    config::{self, Config},
    utils::{BufCharIdx, BufRow, Movement, TermCol, TermRow},
    rect::Rect,
};
use crossterm::{
    cursor::{
        CursorShape, Hide, MoveTo, MoveToColumn, MoveToNextLine, RestorePosition, SavePosition,
        SetCursorShape, Show,
    },
    event::{DisableMouseCapture, KeyCode, KeyEvent},
    execute, queue,
    style::{Print, ResetColor, SetAttributes, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnableLineWrap, LeaveAlternateScreen},
    Result,
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    ops::Range,
    path::PathBuf,
    process,
};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use undo::Action;

/// Renders the a buffer in a view
pub struct BufferRenderer {
    /// The text contained by the buffer
    pub buf: Buffer,
    /// The space the buffer gets to render
    pub rect: Rect,
    /// The path of the file being edited
    path: PathBuf,
    /// Configuration for this buffer
    config: Config,
    /// The current command buffer
    command: String,
    /// The contents of the bottom statusline
    status: String,
    /// Whether the buffer has been edited since saving
    edited: bool,
    /// The amount of columns reserved for line numbers
    line_nrs_width: TermCol,
    /// The amount of lines reserved for the status bar
    status_bar_height: TermRow,
    /// The size of tab characters
    tab_size: u16,
    hl_conf: Option<HighlightConfiguration>,
    hl: Highlighter,
    hl_cache: Option<Vec<HighlightEvent>>,
}

impl BufferRenderer {
    pub fn new(path: PathBuf, config: Config) -> Self {
        let buf = Buffer::new(&path);

        let (width, height) = terminal::size().unwrap();
        let line_nrs_width = TermCol(buf.text.len_lines().to_string().len() as u16 + 1);
        let status_bar_height = TermRow(2);

        let hl_conf = config::get_hl_conf(&path).map(|mut conf| {
            conf.configure(&config.hl_types);
            conf
        });

        BufferRenderer {
            buf,
            rect: Rect::new(
                TermCol(width) - line_nrs_width,
                TermRow(height) - status_bar_height,
                line_nrs_width,
                0.into(),
            ),
            path,
            config,
            line_nrs_width,
            status_bar_height,
            tab_size: 4,
            hl_conf,
            command: String::new(),
            status: String::new(),
            edited: false,
            hl: Highlighter::new(),
            hl_cache: None,
        }
    }

    fn update_highlights(&mut self) {
        match &self.hl_conf {
            None => (),
            Some(hl_conf) => {
                self.hl_cache = Some(
                    self.hl
                        .highlight(
                            hl_conf,
                            &self.buf.text.bytes().collect::<Vec<u8>>(),
                            None,
                            |_| None,
                        )
                        .unwrap()
                        .map(|event| event.unwrap())
                        .collect(),
                );
            }
        }
    }

    pub fn update_size(&mut self, width: u16, height: u16) {
        self.rect
            .resize(TermCol(width) - self.line_nrs_width, TermRow(height) - self.status_bar_height);
    }

    pub fn set_mode<W: Write>(&mut self, w: &mut W, mode: EditMode) -> Result<()> {
        match mode {
            EditMode::Insert => queue!(w, SetCursorShape(CursorShape::Line))?,
            EditMode::Normal => queue!(w, SetCursorShape(CursorShape::Block))?,
            EditMode::Command => self.status = String::from(":"),
        }
        self.buf.mode = mode;
        Ok(())
    }

    /// Saves the current state of the buffer to the file
    fn save(&mut self) -> Result<()> {
        self.buf
            .text
            .write_to(BufWriter::new(File::create(&self.path)?))?;
        self.status = format!(
            "\"{}\" {}L, {}C written",
            self.path.to_str().unwrap(),
            self.buf.text.len_lines(),
            self.buf.text.len_chars(),
        );
        self.edited = false;
        Ok(())
    }

    /// Cleans up and quits the application
    fn quit<W: Write>(&mut self, w: &mut W) -> Result<()> {
        execute!(
            w,
            DisableMouseCapture,
            LeaveAlternateScreen,
            RestorePosition,
            EnableLineWrap,
        )?;
        terminal::disable_raw_mode()?;
        process::exit(0);
    }

    fn cursor_y(&self) -> TermRow {
        self.rect.terminal_y(self.buf.row())
    }

    fn cursor_x(&self) -> TermCol {
        self.rect.terminal_x(self.buf.col())
    }

    fn draw_line_nrs<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.line_nrs_width = TermCol(self.buf.text.len_lines().to_string().len() as u16 + 1);
        self.rect.offset.x = self.line_nrs_width;
        queue!(w, SavePosition, Hide, MoveTo(0, 0))?;
        for line_nr in 0..*self.rect.height {
            let nr = (line_nr as i64 - (*self.cursor_y()) as i64).abs() as usize;
            let (style, nr) = if nr == 0 {
                (self.config.line_nr_active, *self.buf.row() + 1)
            } else {
                (self.config.line_nr_column, nr)
            };
            queue!(
                w,
                SetBackgroundColor(style.background_color.unwrap()),
                SetForegroundColor(style.foreground_color.unwrap()),
                SetAttributes(style.attributes),
                Print(format!(
                    "{: >width$} ",
                    nr,
                    width = *self.line_nrs_width as usize - 1
                )),
                MoveToNextLine(1)
            )?;
        }
        queue!(w, RestorePosition, Show)?;
        Ok(())
    }

    fn draw_status_bar<W: Write>(&mut self, w: &mut W) -> Result<()> {
        let path = self.path.to_str().unwrap();
        let (row, col) = (self.buf.row(), self.buf.col());
        let status = &self.status;
        queue!(
            w,
            SavePosition,
            Hide,
            MoveTo(0, *self.rect.height),
            Print(format!(
                "{: <width$} {: >3}:{: <3}",
                path,
                *row,
                *col,
                width = *self.rect.width as usize - 9
            )),
            MoveToNextLine(1),
            Print(status),
            RestorePosition,
            Show
        )
    }

    pub fn draw_all<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.draw_line_nrs(w)?;
        self.draw_status_bar(w)?;
        self.draw(w, self.rect.top())?;
        Ok(())
    }

    /// Draws the buffer in the given view starting from the line at index `begin`.
    pub fn draw<W: Write>(&mut self, w: &mut W, first_line: BufRow) -> Result<()> {
        let last_line: BufRow = self.rect.bottom().min(self.buf.text.len_lines()).into();

        queue!(
            w,
            SavePosition,
            Hide,
            MoveTo(*self.cursor_x(), *self.rect.terminal_y(first_line)),
            Clear(ClearType::UntilNewLine)
        )?;

        if self.hl_conf.is_none() {
            let rendered_chars =
                self.buf.line_to_char(first_line)..self.buf.line_to_char(last_line);
            self.draw_char_range(w, rendered_chars)?;
            return Ok(());
        }

        if self.hl_cache.is_none() {
            self.update_highlights();
        }
        let rendered_bytes = self.buf.line_to_byte(first_line)..self.buf.line_to_byte(last_line);

        for event in self.hl_cache.as_ref().unwrap() {
            match event {
                HighlightEvent::Source { start, end } => {
                    if *start > *rendered_bytes.end || *end <= *rendered_bytes.start {
                        continue;
                    }
                    let first = self
                        .buf
                        .byte_to_char(usize::max(*start, *rendered_bytes.start).into());
                    let last = self
                        .buf
                        .byte_to_char(usize::min(*end, *rendered_bytes.end).into());
                    self.draw_char_range(w, first..last)?;
                }
                HighlightEvent::HighlightStart(s) => {
                    let style = self.config.hl_styles[s.0];
                    if let Some(fg) = style.foreground_color {
                        queue!(w, SetForegroundColor(fg))?
                    };
                    if let Some(bg) = style.background_color {
                        queue!(w, SetBackgroundColor(bg))?
                    };
                    queue!(w, SetAttributes(style.attributes))?;
                }
                HighlightEvent::HighlightEnd => queue!(w, ResetColor)?,
            }
        }

        queue!(w, RestorePosition, Show)?;
        Ok(())
    }

    fn draw_char_range<W: Write>(&self, w: &mut W, range: Range<BufCharIdx>) -> Result<()> {
        let mut lines = self.buf.slice(range).lines();
        if let Some(line) = lines.next() {
            queue!(w, Print(line))?;
            for line in lines {
                queue!(
                    w,
                    MoveToColumn(*self.line_nrs_width + 1),
                    Clear(ClearType::UntilNewLine),
                    Print(line)
                )?;
            }
        }
        Ok(())
    }

    fn update_cursor<W: Write>(&mut self, w: &mut W) -> Result<()> {
        match self.buf.mode {
            EditMode::Command => {
                queue!(w, MoveTo(self.status.len() as u16, *self.rect.height + 1))
            }
            _ => {
                let cursor = self.buf.cursor();
                self.rect.scroll_to_cursor(cursor);
                let pos = self.rect.terminal_pos(cursor);
                queue!(w, MoveTo(*pos.x, *pos.y))?;
                self.draw_line_nrs(w)
            }
        }
    }

    pub fn insert<W: Write>(&mut self, w: &mut W, i: BufCharIdx, text: &str) -> Result<()> {
        self.buf.insert(i, text);
        self.update_cursor(w)?;
        self.update_highlights();
        self.draw(w, self.buf.row())
    }

    pub fn remove<W: Write>(&mut self, w: &mut W, range: Range<BufCharIdx>) -> Result<()> {
        self.buf.remove(range);
        self.update_cursor(w)?;
        self.update_highlights();
        self.draw(w, self.buf.row())
    }

    fn move_cursor(&mut self, movement: Movement) -> Result<()> {
        Move::new(movement).apply(&mut self.buf).unwrap();
        Ok(())
    }

    pub fn handle_keyevent<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        match self.buf.mode {
            EditMode::Normal => {
                match key_event.code {
                    KeyCode::Char('q') => self.quit(w)?,
                    _ => (),
                //     KeyCode::Esc => self.command.clear(),
                //     KeyCode::Char(c) => self.command.push(c),
                //     KeyCode::Up => self.command.push('k'),
                //     KeyCode::Down => self.command.push('j'),
                //     KeyCode::Left => self.command.push('h'),
                //     KeyCode::Right => self.command.push('l'),
                //     KeyCode::Home => self.command.push('0'),
                //     KeyCode::End => self.command.push('$'),
                //     KeyCode::Delete => self.command.push('x'),
                //     _ => (),
                // }
                // self.status = self.command.clone();
                // if let Some(commands) = Command::parse(&self.status) {
                //     self.command.clear();
                //     for com in commands {
                //         self.execute(com)?;
                //     }
                }
            }
            EditMode::Insert => match key_event.code {
                KeyCode::Char(c) => {
                    let mut tmp = [0u8; 4];
                    self.insert(w, self.buf.idx, c.encode_utf8(&mut tmp))?;
                    self.move_cursor(Movement::Right(1))?;
                }
                KeyCode::Esc => {
                    self.set_mode(w, EditMode::Normal)?;
                    self.move_cursor(Movement::Left(1))?;
                }
                KeyCode::Up => self.move_cursor(Movement::Up(1))?,
                KeyCode::Down => self.move_cursor(Movement::Down(1))?,
                KeyCode::Left => self.move_cursor(Movement::Left(1))?,
                KeyCode::Right => self.move_cursor(Movement::Right(1))?,
                KeyCode::Home => self.move_cursor(Movement::Home)?,
                KeyCode::End => self.move_cursor(Movement::End)?,
                KeyCode::PageUp => {
                    self.move_cursor(Movement::Up(*self.rect.height as usize / 2))?
                }
                KeyCode::PageDown => {
                    self.move_cursor(Movement::Down(*self.rect.height as usize / 2))?
                }
                KeyCode::Backspace => {
                    self.remove(w, self.buf.idx.saturating_sub(1).into()..self.buf.idx)?
                }
                KeyCode::Delete => self.remove(w, self.buf.idx..(self.buf.idx + 1.into()))?,
                KeyCode::Tab => self.insert(w, self.buf.idx, "\t")?,
                KeyCode::Enter => {
                    self.insert(w, self.buf.idx, "\n")?;
                    self.move_cursor(Movement::Down(1))?;
                    self.move_cursor(Movement::Home)?;
                }
                _ => (),
            },
            EditMode::Command => {
                match key_event.code {
                    KeyCode::Char(c) => self.status.push(c),
                    KeyCode::Backspace => {
                        self.status.pop();
                        if self.status.is_empty() {
                            self.set_mode(w, EditMode::Normal)?;
                        }
                    }
                    KeyCode::Esc => self.set_mode(w, EditMode::Normal)?,
                    KeyCode::Enter => {
                        self.set_mode(w, EditMode::Normal)?;
                        match self.status.as_str() {
                            ":w" => self.save()?,
                            ":q" => {
                                if !self.edited {
                                    self.quit(w)?
                                } else {
                                    self.status = String::from("Error: No write since last change. To quit without saving, use ':q!'")
                                }
                            }
                            ":q!" => self.quit(w)?,
                            ":wq" | ":x" => {
                                self.save()?;
                                self.quit(w)?;
                            }
                            "r" => self.draw_all(w)?,
                            _ => self.status = format!("Error: invalid command ({})", self.status),
                        }
                    }
                    _ => (),
                }
                self.draw_status_bar(w)?;
            }
        }
        w.flush()?;
        Ok(())
    }
}
