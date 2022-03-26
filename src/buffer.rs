use crate::{
    command::Move,
    config::{self, Config},
    utils::{Movement, Selection},
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
use ropey::Rope;
use std::{
    fs::File,
    io::{BufReader, BufWriter, Stdout, Write},
    ops::Range,
    path::PathBuf,
    process,
};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use undo::Action;

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

pub struct Buffer {
    /// The text contained by the buffer
    pub content: Rope,
    /// The writer that terminal commands are sent to
    pub w: Stdout,
    /// The path of the file being edited
    path: PathBuf,
    /// Configuration for this buffer
    config: Config,
    /// The mode the buffer is currently being edited in
    pub mode: EditMode,
    /// The current command buffer
    command: String,
    /// The contents of the bottom statusline
    status: String,
    /// Whether the buffer has been edited since saving
    edited: bool,
    /// Position the buffer is scrolled to
    pub offset_y: usize,
    pub offset_x: usize,
    /// Position of cursor in buffer
    pub idx: usize,
    /// Saved column index for easier traversal
    pub saved_col: usize,
    /// The width the buffer gets to render
    pub width: usize,
    /// The height the buffer gets to render
    pub height: usize,
    /// The amount of columns reserved for line numbers
    line_nr_cols: usize,
    /// The size of tab characters
    tab_size: usize,
    hl_conf: Option<HighlightConfiguration>,
    hl: Highlighter,
    hl_cache: Option<Vec<HighlightEvent>>,
}

impl Buffer {
    pub fn new(w: Stdout, path: PathBuf, config: Config) -> Self {
        let (width, height) = terminal::size().unwrap();
        let content = Rope::from_reader(BufReader::new(File::open(&path).unwrap())).unwrap();
        let line_nr_cols = content.len_lines().to_string().len() + 1;
        let hl_conf = config::get_hl_conf(&path).map(|mut conf| {
            conf.configure(&config.hl_types);
            conf
        });

        let mut buffer = Buffer {
            content,
            w,
            path,
            config,
            width: width as usize,
            height: height as usize,
            line_nr_cols,
            tab_size: 4,
            hl_conf,
            command: String::new(),
            status: String::new(),
            edited: false,
            mode: EditMode::Normal,
            offset_y: 0,
            offset_x: 0,
            idx: 0,
            saved_col: 0,
            hl: Highlighter::new(),
            hl_cache: None,
        };

        buffer.update_cursor().expect("Unable to update cursor in buffer constructor");
        buffer
    }

    fn update_highlights(&mut self) {
        self.hl_cache = Some(
            self.hl
                .highlight(
                    self.hl_conf.as_ref().unwrap(),
                    &self.content.bytes().collect::<Vec<u8>>(),
                    None,
                    |_| None,
                )
                .unwrap()
                .map(|event| event.unwrap())
                .collect(),
        );
    }

    /// Returns which row the cursor is on
    pub fn row(&self) -> usize {
        self.content.char_to_line(self.idx)
    }

    /// Returns which column the cursor is on
    pub fn col(&self) -> usize {
        self.idx - self.content.line_to_char(self.row())
    }

    pub fn update_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn set_mode(&mut self, mode: EditMode) -> Result<()> {
        match mode {
            EditMode::Insert => queue!(self.w, SetCursorShape(CursorShape::Line))?,
            EditMode::Normal => queue!(self.w, SetCursorShape(CursorShape::Block))?,
            EditMode::Command => self.status = String::from(":"),
        }
        self.mode = mode;
        Ok(())
    }

    /// Saves the current state of the buffer to the file
    fn save(&mut self) -> Result<()> {
        self.content
            .write_to(BufWriter::new(File::create(&self.path)?))?;
        self.status = format!(
            "\"{}\" {}L, {}C written",
            self.path.to_str().unwrap(),
            self.content.len_lines(),
            self.content.len_chars(),
        );
        self.edited = false;
        Ok(())
    }

    /// Cleans up and quits the application
    fn quit(&mut self) -> Result<()> {
        execute!(
            self.w,
            DisableMouseCapture,
            LeaveAlternateScreen,
            RestorePosition,
            EnableLineWrap,
        )?;
        terminal::disable_raw_mode()?;
        process::exit(0);
    }

    fn draw_line_nrs(&mut self) -> Result<()> {
        self.line_nr_cols = self.content.len_lines().to_string().len() + 1;
        queue!(self.w, SavePosition, Hide, MoveTo(0, 0))?;
        for line_nr in 0..(self.height - 2) {
            let mut nr = (line_nr as i64 - (self.row() - self.offset_y) as i64).abs();
            let style = if nr == 0 {
                self.config.line_nr_active
            } else {
                self.config.line_nr_column
            };
            if nr == 0 {
                nr = self.row() as i64 + 1
            };
            let width = self.line_nr_cols - 1;
            queue!(
                self.w,
                SetBackgroundColor(style.background_color.unwrap()),
                SetForegroundColor(style.foreground_color.unwrap()),
                SetAttributes(style.attributes),
                Print(format!("{: >width$} ", nr, width = width)),
                MoveToNextLine(1)
            )?;
        }
        queue!(self.w, RestorePosition, Show)?;
        Ok(())
    }

    fn draw_status_bar(&mut self) -> Result<()> {
        let status_bar_y = self.height as u16 - 2;
        let path = self.path.to_str().unwrap();
        let (row, col) = (self.row(), self.col());
        let line_info_x = self.width - 9;
        let status = &self.status;
        queue!(
            self.w,
            SavePosition,
            Hide,
            MoveTo(0, status_bar_y),
            Print(format!(
                "{: <width$} {: >3}:{: <3}",
                path,
                row,
                col,
                width = line_info_x
            )),
            MoveTo(0, status_bar_y + 1),
            Print(status),
            RestorePosition,
            Show
        )
    }

    pub fn draw_all(&mut self) -> Result<()> {
        self.draw_line_nrs()?;
        self.draw_status_bar()?;
        self.draw(self.content.line_to_char(self.offset_y))?;
        Ok(())
    }

    pub fn draw(&mut self, begin: usize) -> Result<()> {
        let row = self.content.char_to_line(begin) - self.offset_y;
        let col = begin - self.content.line_to_char(row) - self.offset_x + self.line_nr_cols;

        queue!(
            self.w,
            SavePosition,
            Hide,
            MoveTo(col as u16, row as u16),
            Clear(ClearType::UntilNewLine)
        )?;

        if self.hl_cache.is_none() {
            self.update_highlights();
        }

        let last_line = usize::min(
            self.offset_y + self.height - 3,
            self.content.lines().count(),
        );
        let rendered = begin..(self.content.line_to_char(last_line) + self.max_col(last_line));
        for event in self.hl_cache.as_ref().unwrap() {
            match event {
                HighlightEvent::Source { start, end } => {
                    let first = usize::max(*start, rendered.start);
                    let last = usize::min(*end, rendered.end);
                    if first >= last {
                        continue;
                    };
                    let mut lines = self
                        .content
                        .slice(self.content.byte_to_char(first)..self.content.byte_to_char(last))
                        .lines();
                    if let Some(line) = lines.next() {
                        queue!(self.w, Print(line))?;
                        for line in lines {
                            let line_start = self.line_nr_cols + 1;
                            queue!(
                                self.w,
                                MoveToColumn(line_start as u16),
                                Clear(ClearType::UntilNewLine),
                                Print(line)
                            )?;
                        }
                    }
                }
                HighlightEvent::HighlightStart(s) => {
                    let style = self.config.hl_styles[s.0];
                    if let Some(fg) = style.foreground_color {
                        queue!(self.w, SetForegroundColor(fg))?
                    };
                    if let Some(bg) = style.background_color {
                        queue!(self.w, SetBackgroundColor(bg))?
                    };
                    queue!(self.w, SetAttributes(style.attributes))?;
                }
                HighlightEvent::HighlightEnd => queue!(self.w, ResetColor)?,
            }
        }

        queue!(self.w, RestorePosition, Show)?;
        Ok(())
    }

    pub fn set_cursor(&mut self, idx: usize) -> Result<()> {
        self.idx = idx;
        match self.mode {
            EditMode::Command => {
                let (x, y) = (self.status.len(), self.height - 1);
                queue!(self.w, MoveTo(x as u16, y as u16))?
            }
            _ => self.update_cursor()?,
        }
        self.draw_line_nrs()?;
        self.w.flush()?;
        Ok(())
    }

    fn update_cursor(&mut self) -> Result<()> {
        // Scroll left if cursor is on left side of bounds
        if self.col().saturating_sub(self.offset_x) < 5 {
            self.offset_x = self.col().saturating_sub(5);
        }
        // Scroll right if cursor is on right side of bounds
        if self.col().saturating_sub(self.offset_x) + self.line_nr_cols + 5 + 1 > self.width
        {
            self.offset_x =
                (self.col() + self.line_nr_cols + 5 + 1).saturating_sub(self.width);
        }
        // Scroll up if cursor is above bounds
        if self.row().saturating_sub(self.offset_y) < 3 {
            let scroll_y = self.row().saturating_sub(3);
            self.offset_y = scroll_y;
            self.draw_all()?;
        }
        // Scroll down if cursor is below bounds
        if self.row().saturating_sub(self.offset_y) + 3 + 2 > self.height {
            let scroll_y = (self.row() + 3 + 2).saturating_sub(self.height);
            self.offset_y = scroll_y;
            self.draw_all()?;
        }
        let col = self.col() - self.offset_x + self.line_nr_cols;
        let row = self.row() - self.offset_y;
        queue!(self.w, MoveTo(col as u16, row as u16))
    }

    pub fn max_col(&self, line: usize) -> usize {
        self.content.line(line).len_chars().saturating_sub(1)
    }

    pub fn insert(&mut self, i: usize, text: &str) -> Result<()> {
        self.content.insert(i, text);
        self.update_highlights();
        self.draw(self.content.line_to_char(self.row()))
    }

    pub fn remove(&mut self, range: Range<usize>) -> Result<()> {
        self.set_cursor(range.start)?;
        self.content.remove(range);
        self.update_highlights();
        self.draw(self.content.line_to_char(self.row()))
    }

    fn move_cursor(&mut self, movement: Movement) -> Result<()> {
        Move::new(movement).apply(self).unwrap();
        Ok(())
    }

    pub fn handle_keyevent(&mut self, key_event: KeyEvent) -> Result<()> {
        match self.mode {
            EditMode::Normal => {
                // match key_event.code {
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
                //}
            }
            EditMode::Insert => match key_event.code {
                KeyCode::Char(c) => {
                    let mut tmp = [0u8; 4];
                    self.insert(self.idx, c.encode_utf8(&mut tmp))?;
                    self.move_cursor(Movement::Right(1))?;
                }
                KeyCode::Esc => {
                    self.set_mode(EditMode::Normal)?;
                    self.move_cursor(Movement::Left(1))?;
                }
                KeyCode::Up => self.move_cursor(Movement::Up(1))?,
                KeyCode::Down => self.move_cursor(Movement::Down(1))?,
                KeyCode::Left => self.move_cursor(Movement::Left(1))?,
                KeyCode::Right => self.move_cursor(Movement::Right(1))?,
                KeyCode::Home => self.move_cursor(Movement::Home)?,
                KeyCode::End => self.move_cursor(Movement::End)?,
                KeyCode::PageUp => self.move_cursor(Movement::Up(self.height / 2))?,
                KeyCode::PageDown => self.move_cursor(Movement::Down(self.height / 2))?,
                KeyCode::Backspace => self.remove(self.idx.saturating_sub(1)..self.idx)?,
                KeyCode::Delete => self.remove(self.idx..self.idx + 1)?,
                KeyCode::Tab => self.insert(self.idx, "\t")?,
                KeyCode::Enter => {
                    self.insert(self.idx, "\n")?;
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
                            self.set_mode(EditMode::Normal)?;
                        }
                    }
                    KeyCode::Esc => self.set_mode(EditMode::Normal)?,
                    KeyCode::Enter => {
                        self.set_mode(EditMode::Normal)?;
                        match self.status.as_str() {
                            ":w" => self.save()?,
                            ":q" => {
                                if !self.edited {
                                    self.quit()?
                                } else {
                                    self.status = String::from("Error: No write since last change. To quit without saving, use ':q!'")
                                }
                            }
                            ":q!" => self.quit()?,
                            ":wq" | ":x" => {
                                self.save()?;
                                self.quit()?;
                            }
                            "r" => self.draw_all()?,
                            _ => self.status = format!("Error: invalid command ({})", self.status),
                        }
                    }
                    _ => (),
                }
                self.draw_status_bar()?;
            }
        }
        self.w.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Buffer, Content};
    use ropey::Rope;

    fn get_buffer() -> Buffer {
        let content = Content::new(
            "fn test(x: usize) -> Pos {
    let y = usize::min(x, 3);
    Pos { x, y }
}
"
            .to_string(),
        );
        Buffer {
            content,
            width: 100,
            height: 50,
            ..Buffer::default()
        }
    }

    #[test]
    fn test_insert_newline() {
        let mut content = Content::new("test\nhallo\n".to_string());
        assert_eq!(
            Content {
                raw: "test\nhallo\n".to_string(),
                lines: vec![0, 5, 11]
            },
            content
        );
        content.insert(Pos::new(5, 0), "\n");
        assert_eq!(
            Content {
                raw: "test\n\nhallo\n".to_string(),
                lines: vec![0, 5, 6, 12]
            },
            content
        );
    }

    #[test]
    fn test_insert_newline2() {
        let mut content = Content::new("test\nhallo\n".to_string());
        assert_eq!(
            Content {
                raw: "test\nhallo\n".to_string(),
                lines: vec![0, 5, 11]
            },
            content
        );
        content.insert(Pos::new(4, 0), "\n");
        assert_eq!(
            Content {
                raw: "test\n\nhallo\n".to_string(),
                lines: vec![0, 5, 6, 12]
            },
            content
        );
    }

    #[test]
    fn test_ropey() {
        let rope = Rope::from_str("test\nhallo\n");
        assert_eq!("test\nhallo\n", rope)
    }
}
