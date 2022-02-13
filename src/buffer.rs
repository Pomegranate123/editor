use crate::{command::{Command, Movement, Selection}, highlight};
use crossterm::{
    cursor::{
        CursorShape, Hide, MoveTo, MoveToNextLine, RestorePosition, SavePosition,
        SetCursorShape, Show,
    },
    event::{DisableMouseCapture, KeyCode, KeyEvent},
    execute, queue,
    style::{Print, ResetColor},
    terminal::{self, Clear, ClearType, EnableLineWrap, LeaveAlternateScreen},
    Result,
};
use tree_sitter_highlight::{Highlighter, HighlightConfiguration, HighlightEvent};
use ropey::Rope;
use std::{fs::File, io::{Write, BufWriter, BufReader}, path::PathBuf, process, ops::Range};

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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Pos {
    pub y: usize,
    pub x: usize,
}

impl Pos {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl Default for Pos {
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

pub struct Buffer {
    /// The text contained by the buffer
    pub content: Rope,
    /// The path of the file being edited
    path: PathBuf,
    /// The mode the buffer is currently being edited in
    mode: EditMode,
    /// The current command buffer
    command: String,
    /// The contents of the bottom statusline
    status: String,
    /// Whether the buffer has been edited since saving
    edited: bool,
    /// Position the buffer is scrolled to
    offset: Pos,
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
}

impl Buffer {
    pub fn new(path: PathBuf) -> Self {
        let (width, height) = terminal::size().unwrap();
        let content = Rope::from_reader(BufReader::new(File::open(&path).unwrap())).unwrap();
        let line_nr_cols = content.len_lines().to_string().len() + 1;
        let hl_conf = highlight::get_hl_conf(&path);

        Buffer {
            content,
            path,
            width: width as usize,
            height: height as usize,
            line_nr_cols,
            tab_size: 4,
            hl_conf,
            command: String::new(),
            status: String::new(),
            edited: false,
            mode: EditMode::Normal,
            offset: Pos::new(0, 0),
            idx: 0,
            saved_col: 0,
        }
    }

    fn row(&self) -> usize {
        self.content.char_to_line(self.idx)
    }

    fn col(&self) -> usize {
        self.idx - self.content.line_to_char(self.row())
    }

    fn pos_to_index(&self, pos: Pos) -> usize {
        self.content.line_to_char(pos.y) + pos.x
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
                let end = self.content.line_to_char(dest) + self.max_col(dest);
                start..end
            }
            Selection::UpTo(mov) => {
                self.idx..self.get_destination(mov)
            }
            Selection::Between {
                first,
                last,
                inclusive,
            } => 0..0,
            Selection::Word { inclusive: _ } => 0..0,
            Selection::Paragraph { inclusive: _ } => 0..0,
        }
    }

    fn execute<W: Write>(&mut self, w: &mut W, command: Command) -> Result<()> {
        match command {
            Command::Undo => (), //buffer.undo(),
            Command::Redo => (), //buffer.redo(),
            Command::Move(dir) => self.move_cursor(dir),
            Command::Delete(sel) => self.content.remove(self.bounds(sel)),
            Command::Yank(_sel) => (), //buffer.copy(sel),
            Command::Paste => (),      //buffer.paste(plc),
            Command::CreateNewLine => {
                self.content.insert(self.idx, "\n");
                self.update_line_nr_cols();
            },
            Command::SetMode(mode) => self.set_mode(w, mode)?,
        }
        Ok(())
    }


    /// Saves the current state of the buffer to the file
    fn save(&mut self) -> Result<()> {
        self.content.write_to(BufWriter::new(File::create(&self.path)?));
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

    pub fn draw<W: Write>(&self, w: &mut W) -> Result<()> {
        let highlight_names = &[
            "attribute",
            "constant",
            "function.builtin",
            "function",
            "keyword",
            "operator",
            "property",
            "punctuation",
            "punctuation.bracket",
            "punctuation.delimiter",
            "string",
            "string.special",
            "tag",
            "type",
            "type.builtin",
            "variable",
            "variable.builtin",
            "variable.parameter",
        ];

        let mut hl = Highlighter::new();
        queue!(w, SavePosition, Hide, MoveTo(0, 0), Clear(ClearType::All))?;

        queue!(w, Print(format!(
            "{: >width$} ",
            self.row(),
            width = self.line_nr_cols - 1
        )))?;
        let mut line_nr = 1;


        let last_line = self.offset.y + self.height - 3;
        let rendered = self.content.line_to_char(self.offset.y)..(self.content.line_to_char(last_line) + self.max_col(last_line));
        let rendered_text = self.content.slice(rendered);
        let bytes = rendered_text.bytes().collect::<Vec<u8>>();
        for event in hl.highlight(
            self.hl_conf.as_ref().unwrap(),
            &bytes,
            None,
            |_| None).unwrap() {
            match event.unwrap() {
                HighlightEvent::Source {start, end} => {
                    let text = rendered_text.slice(self.content.byte_to_char(start)..self.content.byte_to_char(end));
                    for c in text.chars() {
                        match c {
                            '\t' => queue!(w, Print("   "))?,
                            '\n' => {
                                queue!(w, MoveToNextLine(1),
                                    Print(format!(
                                        "{: >width$} ",
                                        (line_nr as i64 - self.row() as i64).abs(),
                                        width = self.line_nr_cols - 1
                                    ))
                                )?;
                                line_nr += 1;
                            }
                            _ => queue!(w, Print(c))?,
                        }
                    }
                },
                HighlightEvent::HighlightStart(s) => {
                    //let style = hl_styles[s.0];
                    //queue!(w, SetBackgroundColor(style.background_color.unwrap()))?;
                    //queue!(w, SetForegroundColor(style.foreground_color.unwrap()))?;
                    //queue!(w, SetAttributes(style.attributes))?;
                },
                HighlightEvent::HighlightEnd => queue!(w, ResetColor)?,
                
            }
        }
        //let visible = self.content.lines[self.offset.y]..self.content.lines[usize::min(self.offset.y + self.height, self.content.len())];
        
        queue!(
            w,
            MoveTo(0, self.height as u16 - 2),
            Print(format!(
                "{: <width$} {: >3}:{: <3}",
                self.path.to_str().unwrap(),
                self.row(),
                self.col(),
                width = self.width - 9
            )),
            MoveTo(0, self.height as u16 - 1),
            Print(&self.status)
        )?;
        queue!(w, RestorePosition, Show)?;
        w.flush()?;
        Ok(())
    }

    /// Recalculate the necessary number of columns needed to display line numbers for the buffer
    pub fn update_line_nr_cols(&mut self) {
        self.line_nr_cols = self.content.len_lines().to_string().len() + 1;
    }

    pub fn move_cursor(&mut self, movement_type: Movement) {
        self.idx = self.get_destination(movement_type);
        match movement_type {
            Movement::Left(_) | Movement::Right(_) | Movement::Home | Movement::End => {
                self.saved_col = self.col();
            }
            _ => (),
        }
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
                let y = usize::min(self.row() + amount, self.content.len_lines().saturating_sub(1));
                let x = usize::min(self.max_col(y), self.saved_col);
                self.content.line_to_char(y) + x
            }
            Movement::Left(amount) => {
                usize::max(self.idx.saturating_sub(amount), self.content.line_to_char(self.row()))
            }
            Movement::Right(amount) => {
                usize::min(self.idx + amount, self.content.line_to_char(self.row()) + self.max_col(self.row()))
            }
            Movement::Home => self.content.line_to_char(self.row()),
            Movement::End => self.content.line_to_char(self.row()) + self.max_col(self.row()),
            Movement::FirstChar => {
                unimplemented!()
                // Pos::new(
                // self.content[self.pos.y]
                //     .find(|c| !char::is_whitespace(c))
                //     .unwrap_or(self.content.lines[self.pos.y]),
                // self.pos.y,
            }
            Movement::Top => {
                let y = self.offset.x + 3;
                let x = usize::min(self.max_col(y), self.saved_col);
                self.content.line_to_char(y) + x
            }
            Movement::Bottom => {
                let y = self.offset.x + self.height - 3;
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
                if self.col().saturating_sub(self.offset.x) < 5 {
                    self.offset.x = self.col().saturating_sub(5);
                }
                // Scroll right if cursor is on right side of bounds
                if self.col().saturating_sub(self.offset.x) + self.line_nr_cols + 5 + 1 > self.width {
                    self.offset.x = (self.col() + self.line_nr_cols + 5 + 1).saturating_sub(self.width);
                }
                // Scroll up if cursor is above bounds
                if self.row().saturating_sub(self.offset.y) < 3 {
                    self.offset.y = self.row().saturating_sub(3);
                }
                // Scroll down if cursor is below bounds
                if self.row().saturating_sub(self.offset.y) + 3 + 2 > self.height {
                    self.offset.y = (self.row() + 3 + 2).saturating_sub(self.height);
                }
                queue!(
                    w,
                    MoveTo(
                        (self.col() - self.offset.x + self.line_nr_cols) as u16,
                        (self.row() - self.offset.y) as u16
                    )
                )?;
            }
        }
        Ok(())
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
                    self.content.insert(self.idx, c.encode_utf8(&mut tmp));
                    self.move_cursor(Movement::Right(1));
                }
                KeyCode::Esc => {
                    self.set_mode(w, EditMode::Normal)?;
                    self.move_cursor(Movement::Left(1));
                }
                KeyCode::Up => self.move_cursor(Movement::Up(1)),
                KeyCode::Down => self.move_cursor(Movement::Down(1)),
                KeyCode::Left => self.move_cursor(Movement::Left(1)),
                KeyCode::Right => self.move_cursor(Movement::Right(1)),
                KeyCode::Home => self.move_cursor(Movement::Home),
                KeyCode::End => self.move_cursor(Movement::End),
                KeyCode::PageUp => self.move_cursor(Movement::Up(self.height / 2)),
                KeyCode::PageDown => self.move_cursor(Movement::Down(self.height / 2)),
                KeyCode::Backspace => {
                    self.content.remove(self.idx.saturating_sub(1)..self.idx);
                    self.idx = self.idx.saturating_sub(1);
                }
                KeyCode::Delete => {
                    self.content.remove(self.idx..self.idx + 1);
                }
                KeyCode::Tab => self.content.insert(self.idx, "\t"),
                KeyCode::Enter => {
                    self.content.insert(self.idx, "\n");
                    self.move_cursor(Movement::Down(1));
                    self.move_cursor(Movement::Home);
                    self.update_line_nr_cols();
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
                                self.status = String::from(
                                        "Error: No write since last change. To quit without saving, use ':q!'",
                                    )
                            }
                        }
                        ":q!" => self.quit(w)?,
                        ":wq" | ":x" => {
                            self.save()?;
                            self.quit(w)?;
                        }
                        _ => self.status = format!("Error: invalid command ({})", self.status),
                    }
                }
                _ => (),
            },
        }
        self.update_cursor(w)?;
        Ok(())
    }

}

#[cfg(test)]
mod test {
    use super::{Buffer, Pos, Content};
    use ropey::Rope;

    fn get_buffer() -> Buffer {
        let content = Content::new(
"fn test(x: usize) -> Pos {
    let y = usize::min(x, 3);
    Pos { x, y }
}
".to_string());
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
        assert_eq!(Content { raw: "test\nhallo\n".to_string(), lines: vec![0, 5, 11] }, content);
        content.insert(Pos::new(5, 0), "\n");
        assert_eq!(Content { raw: "test\n\nhallo\n".to_string(), lines: vec![0, 5, 6, 12] }, content);
    }

    #[test]
    fn test_insert_newline2() {
        let mut content = Content::new("test\nhallo\n".to_string());
        assert_eq!(Content { raw: "test\nhallo\n".to_string(), lines: vec![0, 5, 11] }, content);
        content.insert(Pos::new(4, 0), "\n");
        assert_eq!(Content { raw: "test\n\nhallo\n".to_string(), lines: vec![0, 5, 6, 12] }, content);
    }

    #[test]
    fn test_ropey() {
        let rope = Rope::from_str("test\nhallo\n");
        assert_eq!("test\nhallo\n", rope)
    }
}
