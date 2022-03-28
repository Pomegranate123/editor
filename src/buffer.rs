use crate::{
    action::Move,
    config::{self, Config},
    utils::{Movement, Pos},
    view::Rect,
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
    io::{BufReader, BufWriter, Write},
    ops::Range,
    path::{PathBuf, Path},
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

pub struct Content {
    pub text: Rope,
    pub idx: usize,
    pub saved_col: usize,
    pub mode: EditMode,
}

impl Content {
    pub fn new(path: &Path) -> Self {
        let text = Rope::from_reader(BufReader::new(File::open(path).unwrap())).unwrap();
        Self {
            text, idx: 0, saved_col: 0, mode: EditMode::Normal
        }
    }

    /// Returns which row the cursor is on
    pub fn row(&self) -> usize {
        self.text.char_to_line(self.idx)
    }

    /// Returns which column the cursor is on
    pub fn col(&self) -> usize {
        self.idx - self.text.line_to_char(self.row())
    }

    pub fn max_col(&self, line: usize) -> usize {
        self.text.line(line).len_chars().saturating_sub(1)
    }

    pub fn insert(&mut self, i: usize, string: &str) {
        self.text.insert(i, string);
    }

    pub fn remove(&mut self, range: Range<usize>) {
        self.idx = range.start;
        self.text.remove(range);
    }

    pub fn cursor(&self) -> Pos {
        Pos::new(self.col(), self.row())
    }
}

/// Renders the contents of a file in a view
pub struct Buffer {
    /// The text contained by the buffer
    pub buf: Content,

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
    line_nr_cols: usize,
    /// The size of tab characters
    tab_size: usize,
    hl_conf: Option<HighlightConfiguration>,
    hl: Highlighter,
    hl_cache: Option<Vec<HighlightEvent>>,
}

impl Buffer {
    pub fn new(path: PathBuf, config: Config) -> Self {
        let (width, height) = terminal::size().unwrap();
        let buf = Content::new(&path);
        let line_nr_cols = buf.text.len_lines().to_string().len() + 1;
        let hl_conf = config::get_hl_conf(&path).map(|mut conf| {
            conf.configure(&config.hl_types);
            conf
        });

        Buffer {
            buf,
            rect: Rect::new(width as usize, height as usize, line_nr_cols, 0),
            path,
            config,
            line_nr_cols,
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
            None => return,
            Some(hl_conf) => {
                self.hl_cache = Some(
                    self.hl
                        .highlight(
                            &hl_conf,
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

    pub fn update_size(&mut self, width: usize, height: usize) {
        self.rect.resize(width, height);
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
        self.buf.text
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

    fn cursor_y(&self) -> usize {
        self.rect.terminal_y(self.buf.row())
    }

    fn cursor_x(&self) -> usize {
        self.rect.terminal_x(self.buf.col())
    }

    fn len_lines(&self) -> usize {
        self.buf.text.len_lines()
    }

    fn draw_line_nrs<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.line_nr_cols = self.buf.text.len_lines().to_string().len() + 1;
        self.rect.scroll.x = self.line_nr_cols;
        queue!(w, SavePosition, Hide, MoveTo(0, 0))?;
        for line_nr in 0..(self.rect.height - 2) {
            let nr = (line_nr as i64 - (self.cursor_y()) as i64).abs() as usize;
            let (style, nr) = if nr == 0 {
                (self.config.line_nr_active, self.buf.row() + 1)
            } else {
                (self.config.line_nr_column, nr)
            };
            queue!(
                w,
                SetBackgroundColor(style.background_color.unwrap()),
                SetForegroundColor(style.foreground_color.unwrap()),
                SetAttributes(style.attributes),
                Print(format!("{: >width$} ", nr, width = self.line_nr_cols - 1)),
                MoveToNextLine(1)
            )?;
        }
        queue!(w, RestorePosition, Show)?;
        Ok(())
    }

    fn draw_status_bar<W: Write>(&mut self, w: &mut W) -> Result<()> {
        let status_bar_y = self.rect.height as u16 - 2;
        let path = self.path.to_str().unwrap();
        let (row, col) = (self.buf.row(), self.buf.col());
        let line_info_x = self.rect.width - 9;
        let status = &self.status;
        queue!(
            w,
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

    pub fn draw_all<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.draw_line_nrs(w)?;
        self.draw_status_bar(w)?;
        self.draw(w, self.rect.scroll.y)?;
        Ok(())
    }

    /// Draws the buffer in the given view starting from the line at index `begin`.
    pub fn draw<W: Write>(&mut self, w: &mut W, begin: usize) -> Result<()> {
        //let row = self.buf.text.char_to_line(begin) - self.rect.scroll.y;
        //let col = begin - self.buf.text.line_to_char(row) - self.rect.scroll.x + self.line_nr_cols;

        queue!(
            w,
            SavePosition,
            Hide,
            MoveTo(0, self.rect.terminal_y(begin) as u16),
            Clear(ClearType::UntilNewLine)
        )?;

        if self.hl_conf.is_none() {

        }

        if self.hl_cache.is_none() {
            self.update_highlights();
        }

        let last_line = usize::min(
            self.rect.bottom() - 3,
            self.buf.text.lines().count(),
        );
        let rendered = self.buf.text.line_to_char(begin)..(self.buf.text.line_to_char(last_line) + self.buf.max_col(last_line));
        for event in self.hl_cache.as_ref().unwrap() {
            match event {
                HighlightEvent::Source { start, end } => {
                    //TODO: Potential bug: `rendered` is char indices, while `start` & `end` are byte indices.
                    if *start > rendered.end || *end <= rendered.start {
                        continue;
                    }
                    let first = self.buf.text.byte_to_char(usize::max(*start, rendered.start));
                    let last = self.buf.text.byte_to_char(usize::min(*end, rendered.end));
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

    fn draw_char_range<W: Write>(&self, w: &mut W, range: Range<usize>) -> Result<()> {
        let mut lines = self
            .buf.text
            .slice(range)
            .lines();
        if let Some(line) = lines.next() {
            queue!(w, Print(line))?;
            for line in lines {
                let line_start = self.line_nr_cols + 1;
                queue!(
                    w,
                    MoveToColumn(line_start as u16),
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
                let (x, y) = (self.status.len(), self.rect.height - 1);
                queue!(w, MoveTo(x as u16, y as u16))?
            }
            _ => {
                self.rect.scroll_to_cursor(self.buf.cursor());
                let col = self.buf.col() - self.rect.scroll.x + self.line_nr_cols;
                let row = self.buf.row() - self.rect.scroll.y;
                queue!(w, MoveTo(col as u16, row as u16))
                self.draw_line_nrs(w)?;
            }
        }
    }

    pub fn insert<W: Write>(&mut self, w: &mut W, i: usize, text: &str) -> Result<()> {
        self.buf.insert(i, text);
        self.update_cursor(w)?;
        self.update_highlights();
        self.draw(w, self.buf.text.line_to_char(self.buf.row()))
    }

    pub fn remove<W: Write>(&mut self, w: &mut W, range: Range<usize>) -> Result<()> {
        self.buf.remove(range);
        self.update_cursor(w)?;
        self.update_highlights();
        self.draw(w, self.buf.text.line_to_char(self.buf.row()))
    }

    fn move_cursor(&mut self, movement: Movement) -> Result<()> {
        Move::new(movement).apply(&mut self.buf).unwrap();
        Ok(())
    }

    pub fn handle_keyevent<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        match self.buf.mode {
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
                KeyCode::PageUp => self.move_cursor(Movement::Up(self.rect.height / 2))?,
                KeyCode::PageDown => self.move_cursor(Movement::Down(self.rect.height / 2))?,
                KeyCode::Backspace => self.remove(w, self.buf.idx.saturating_sub(1)..self.buf.idx)?,
                KeyCode::Delete => self.remove(w, self.buf.idx..self.buf.idx + 1)?,
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

#[cfg(test)]
mod test {
    use super::{Buffer, Content};
    use ropey::Rope;

    fn get_buffer() -> Buffer {
        let buf = Content::new(
            "fn test(x: usize) -> Pos {
    let y = usize::min(x, 3);
    Pos { x, y }
}
"
            .to_string(),
        );
        Buffer {
            buf,
            width: 100,
            height: 50,
            ..Buffer::default()
        }
    }

    #[test]
    fn test_insert_newline() {
        let mut buf = Content::new("test\nhallo\n".to_string());
        assert_eq!(
            Content {
                raw: "test\nhallo\n".to_string(),
                lines: vec![0, 5, 11]
            },
            buf
        );
        buf.insert(Pos::new(5, 0), "\n");
        assert_eq!(
            Content {
                raw: "test\n\nhallo\n".to_string(),
                lines: vec![0, 5, 6, 12]
            },
            buf
        );
    }

    #[test]
    fn test_insert_newline2() {
        let mut buf = Content::new("test\nhallo\n".to_string());
        assert_eq!(
            Content {
                raw: "test\nhallo\n".to_string(),
                lines: vec![0, 5, 11]
            },
            buf
        );
        buf.insert(Pos::new(4, 0), "\n");
        assert_eq!(
            Content {
                raw: "test\n\nhallo\n".to_string(),
                lines: vec![0, 5, 6, 12]
            },
            buf
        );
    }

    #[test]
    fn test_ropey() {
        let rope = Rope::from_str("test\nhallo\n");
        assert_eq!("test\nhallo\n", rope)
    }
}
