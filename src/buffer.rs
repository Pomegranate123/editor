use crate::{
    command::{Command, Movement, Selection},
    config::{self, Config},
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
    path::PathBuf,
    process,
};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

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
    /// The path of the file being edited
    path: PathBuf,
    ///
    config: Config,
    /// The mode the buffer is currently being edited in
    mode: EditMode,
    /// The current command buffer
    command: String,
    /// The contents of the bottom statusline
    status: String,
    /// Whether the buffer has been edited since saving
    edited: bool,
    /// Position the buffer is scrolled to
    offset_y: usize,
    offset_x: usize,
    /// Position of cursor in buffer
    idx: usize,
    /// Saved column index for easier traversal
    saved_col: usize,
    /// The width the buffer gets to render
    width: usize,
    /// The height the buffer gets to render
    height: usize,
    /// The amount of columns reserved for line numbers
    pub line_nr_cols: usize,
    /// The size of tab characters
    tab_size: usize,
    hl_conf: Option<HighlightConfiguration>,
    hl: Highlighter,
    hl_cache: Option<Vec<HighlightEvent>>,
}

impl Buffer {
    pub fn new(path: PathBuf, config: Config) -> Self {
        let (width, height) = terminal::size().unwrap();
        let content = Rope::from_reader(BufReader::new(File::open(&path).unwrap())).unwrap();
        let line_nr_cols = content.len_lines().to_string().len() + 1;
        let hl_conf = config::get_hl_conf(&path).map(|mut conf| {
            conf.configure(&config.hl_types);
            conf
        });

        Buffer {
            content,
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
        }
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

    fn row(&self) -> usize {
        self.content.char_to_line(self.idx)
    }

    fn col(&self) -> usize {
        self.idx - self.content.line_to_char(self.row())
    }

    pub fn update_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn set_mode<W: Write>(&mut self, w: &mut W, mode: EditMode) -> Result<()> {
        match mode {
            EditMode::Insert => queue!(w, SetCursorShape(CursorShape::Line))?,
            EditMode::Normal => queue!(w, SetCursorShape(CursorShape::Block))?,
            EditMode::Command => self.status = String::from(":"),
        }
        self.mode = mode;
        Ok(())
    }

    fn bounds(&self, sel: Selection) -> Range<usize> {
        match sel {
            Selection::Lines(amount) => {
                let start = self.content.line_to_char(self.row());
                let dest = usize::min(self.row() + amount, self.content.len_lines());
                let end = self.content.line_to_char(dest);
                start..end
            }
            Selection::UpTo(mov) => self.idx..self.get_destination(mov),
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

    fn execute<W: Write>(&mut self, w: &mut W, command: Command) -> Result<()> {
        match command {
            Command::Undo => (), //buffer.undo(),
            Command::Redo => (), //buffer.redo(),
            Command::Move(dir) => self.move_cursor(w, dir)?,
            Command::Delete(sel) => self.remove(w, self.bounds(sel))?,
            Command::Yank(_sel) => (), //buffer.copy(sel),
            Command::Paste => (),      //buffer.paste(plc),
            Command::CreateNewLine => self.insert(w, self.idx, "\n")?,
            Command::SetMode(mode) => self.set_mode(w, mode)?,
        }
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

    fn draw_line_nrs<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.line_nr_cols = self.content.len_lines().to_string().len() + 1;
        queue!(w, SavePosition, Hide, MoveTo(0, 0))?;
        for line_nr in 0..(self.height - 2) {
            queue!(
                w,
                Print(format!(
                    "{: >width$} ",
                    (line_nr as i64 - (self.row() - self.offset_y) as i64).abs(),
                    width = self.line_nr_cols - 1
                )),
                MoveToNextLine(1)
            )?;
        }
        queue!(w, RestorePosition, Show)?;
        Ok(())
    }

    fn draw_status_bar<W: Write>(&mut self, w: &mut W) -> Result<()> {
        queue!(
            w,
            SavePosition,
            Hide,
            MoveTo(0, self.height as u16 - 2),
            Print(format!(
                "{: <width$} {: >3}:{: <3}",
                self.path.to_str().unwrap(),
                self.row(),
                self.col(),
                width = self.width - 9
            )),
            MoveTo(0, self.height as u16 - 1),
            Print(&self.status),
            RestorePosition,
            Show
        )
    }

    pub fn draw_all<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.draw_line_nrs(w)?;
        self.draw_status_bar(w)?;
        self.draw(w, self.content.line_to_char(self.offset_y))?;
        Ok(())
    }

    pub fn draw<W: Write>(&mut self, w: &mut W, begin: usize) -> Result<()> {
        self.draw_status_bar(w)?;

        let row = self.content.char_to_line(begin);
        let col = begin - self.content.line_to_char(row);

        queue!(
            w,
            SavePosition,
            Hide,
            MoveTo(
                (col - self.offset_x + self.line_nr_cols) as u16,
                (row - self.offset_y) as u16
            ),
            Clear(ClearType::UntilNewLine)
        )?;

        if self.hl_cache.is_none() {
            self.update_highlights();
        }

        let last_line = self.offset_y + self.height - 3;
        let rendered = begin..(self.content.line_to_char(last_line) + self.max_col(last_line));
        for event in self.hl_cache.as_ref().unwrap() {
            match event {
                HighlightEvent::Source { start, end } => {
                    let mut first = usize::max(*start, rendered.start);
                    let last = usize::min(*end, rendered.end);
                    if first > last {
                        first = last;
                    };
                    let mut lines = self
                        .content
                        .slice(self.content.byte_to_char(first)..self.content.byte_to_char(last))
                        .lines();
                    if let Some(line) = lines.next() {
                        queue!(w, Print(line))?;
                        for line in lines {
                            queue!(
                                w,
                                MoveToColumn(self.line_nr_cols as u16 + 1),
                                Clear(ClearType::UntilNewLine),
                                Print(line)
                            )?;
                        }
                    }
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

    pub fn move_cursor<W: Write>(&mut self, w: &mut W, movement_type: Movement) -> Result<()> {
        self.idx = self.get_destination(movement_type);
        match movement_type {
            Movement::Left(_) | Movement::Right(_) | Movement::Home | Movement::End => {
                self.saved_col = self.col();
            }
            _ => (),
        }
        self.update_cursor(w)
    }

    fn max_col(&self, line: usize) -> usize {
        self.content.line(line).len_chars().saturating_sub(1)
    }

    fn get_destination(&self, movement_type: Movement) -> usize {
        match movement_type {
            Movement::Up(amount) => {
                let y = self.row().saturating_sub(amount);
                let x = usize::min(self.max_col(y), self.saved_col);
                self.content.line_to_char(y) + x
            }
            Movement::Down(amount) => {
                let y = usize::min(
                    self.row() + amount,
                    self.content.len_lines().saturating_sub(1),
                );
                let x = usize::min(self.max_col(y), self.saved_col);
                self.content.line_to_char(y) + x
            }
            Movement::Left(amount) => usize::max(
                self.idx.saturating_sub(amount),
                self.content.line_to_char(self.row()),
            ),
            Movement::Right(amount) => usize::min(
                self.idx + amount,
                self.content.line_to_char(self.row()) + self.max_col(self.row()),
            ),
            Movement::Home => self.content.line_to_char(self.row()),
            Movement::End => self.content.line_to_char(self.row()) + self.max_col(self.row()),
            Movement::FirstChar => {
                unimplemented!()
            }
            Movement::Top => {
                let y = self.offset_x + 3;
                let x = usize::min(self.max_col(y), self.saved_col);
                self.content.line_to_char(y) + x
            }
            Movement::Bottom => {
                let y = self.offset_x + self.height - 3;
                let x = usize::min(self.max_col(y), self.saved_col);
                self.content.line_to_char(y) + x
            }
            Movement::NextWord(_amount) => {
                unimplemented!()
            }
            Movement::PrevWord(_amount) => {
                unimplemented!()
            }
        }
    }

    fn update_cursor<W: Write>(&mut self, w: &mut W) -> Result<()> {
        match self.mode {
            EditMode::Command => {
                queue!(w, MoveTo(self.status.len() as u16, self.height as u16 - 1))?
            }
            _ => {
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
                    //queue!(w, ScrollDown((self.offset_y - scroll_y) as u16))?;
                    self.offset_y = scroll_y;
                    //self.draw(w, self.idx)?;
                    self.draw_all(w)?;
                }
                // Scroll down if cursor is below bounds
                if self.row().saturating_sub(self.offset_y) + 3 + 2 > self.height {
                    let scroll_y = (self.row() + 3 + 2).saturating_sub(self.height);
                    //queue!(w, ScrollUp((scroll_y - self.offset_y) as u16))?;
                    self.offset_y = scroll_y;
                    //self.draw(w, self.idx)?;
                    self.draw_all(w)?;
                }
                queue!(
                    w,
                    MoveTo(
                        (self.col() - self.offset_x + self.line_nr_cols) as u16,
                        (self.row() - self.offset_y) as u16
                    )
                )?;
            }
        }
        w.flush()?;
        Ok(())
    }

    fn insert<W: Write>(&mut self, w: &mut W, i: usize, text: &str) -> Result<()> {
        self.content.insert(i, text);
        self.update_highlights();
        self.draw(w, self.content.line_to_char(self.row()))
        //self.draw_all(w)
    }

    fn remove<W: Write>(&mut self, w: &mut W, range: Range<usize>) -> Result<()> {
        self.idx = range.start;
        self.content.remove(range);
        self.update_cursor(w)?;
        self.update_highlights();
        self.draw(w, self.content.line_to_char(self.row()))
        //self.draw_all(w)
    }

    pub fn handle_keyevent<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        match self.mode {
            EditMode::Normal => {
                match key_event.code {
                    KeyCode::Esc => self.command.clear(),
                    KeyCode::Char(c) => self.command.push(c),
                    KeyCode::Up => self.command.push('k'),
                    KeyCode::Down => self.command.push('j'),
                    KeyCode::Left => self.command.push('h'),
                    KeyCode::Right => self.command.push('l'),
                    KeyCode::Home => self.command.push('0'),
                    KeyCode::End => self.command.push('$'),
                    KeyCode::Delete => self.command.push('x'),
                    _ => (),
                }
                self.status = self.command.clone();
                if let Some(commands) = Command::parse(&self.status) {
                    self.command.clear();
                    for com in commands {
                        self.execute(w, com)?;
                    }
                }
            }
            EditMode::Insert => match key_event.code {
                KeyCode::Char(c) => {
                    let mut tmp = [0u8; 4];
                    self.insert(w, self.idx, c.encode_utf8(&mut tmp))?;
                    self.move_cursor(w, Movement::Right(1))?;
                }
                KeyCode::Esc => {
                    self.set_mode(w, EditMode::Normal)?;
                    self.move_cursor(w, Movement::Left(1))?;
                }
                KeyCode::Up => self.move_cursor(w, Movement::Up(1))?,
                KeyCode::Down => self.move_cursor(w, Movement::Down(1))?,
                KeyCode::Left => self.move_cursor(w, Movement::Left(1))?,
                KeyCode::Right => self.move_cursor(w, Movement::Right(1))?,
                KeyCode::Home => self.move_cursor(w, Movement::Home)?,
                KeyCode::End => self.move_cursor(w, Movement::End)?,
                KeyCode::PageUp => self.move_cursor(w, Movement::Up(self.height / 2))?,
                KeyCode::PageDown => self.move_cursor(w, Movement::Down(self.height / 2))?,
                KeyCode::Backspace => self.remove(w, self.idx.saturating_sub(1)..self.idx)?,
                KeyCode::Delete => self.remove(w, self.idx..self.idx + 1)?,
                KeyCode::Tab => self.insert(w, self.idx, "\t")?,
                KeyCode::Enter => {
                    self.insert(w, self.idx, "\n")?;
                    self.move_cursor(w, Movement::Down(1))?;
                    self.move_cursor(w, Movement::Home)?;
                }
                _ => (),
            },
            EditMode::Command => match key_event.code {
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
            },
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
