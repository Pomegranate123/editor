use crate::{command::{Command, Movement, Selection}, highlight};
use crossterm::{
    cursor::{
        CursorShape, Hide, MoveTo, MoveToNextLine, RestorePosition, SavePosition,
        SetCursorShape, Show,
    },
    event::{DisableMouseCapture, KeyCode, KeyEvent},
    execute, queue,
    style::{Print, PrintStyledContent, ContentStyle, SetBackgroundColor, SetForegroundColor, SetAttributes, Color, Attributes, Attribute, ResetColor},
    terminal::{self, Clear, ClearType, EnableLineWrap, LeaveAlternateScreen},
    Result,
};
use tree_sitter_highlight::{Highlighter, HighlightConfiguration, HighlightEvent};
use std::{fs, io::Write, path::PathBuf, process};

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

pub struct Bounds {
    pub left: usize,
    pub right: usize,
}

impl Bounds {
    pub fn new(i: usize, j: usize) -> Self {
        if i < j {
            Self {
                left: i,
                right: j,
            }
        } else {
            Self {
                left: j,
                right: i,
            }
        }
    }

    pub fn from_delimiters(i: usize, j: usize, inclusive: bool) -> Self {
        let mut bounds = Self::new(i, j);
        match inclusive {
            true => bounds.right += 1,
            false => bounds.left += 1,
        }
        bounds
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct Content {
    raw: String,
    lines: Vec<usize>,
}

impl Content {
    pub fn new(raw: String) -> Self {
        let lines = std::iter::once(0).chain(raw.match_indices("\n").map(|(i, _)| i + 1)).collect();
        eprintln!("{}", raw);
        Self { raw, lines }
    }

    // Returns line length including \n char
    pub fn line_len(&self, y: usize) -> usize {
        self.lines[y + 1] - self.lines[y]
    }

    pub fn len(&self) -> usize {
        self.lines.len() - 1
    }

    // test\nhallo\n,  vec![0, 5, 11]
    // test\n\nhallo\n
    pub fn insert(&mut self, pos: Pos, text: &str) {
        let index = self.index_from_pos(pos).unwrap();
        self.raw.insert_str(index, text);
        self.lines = std::iter::once(0).chain(self.raw.match_indices("\n").map(|(i, _)| i + 1)).collect();
    }

    pub fn delete(&mut self, b: Bounds) {
        self.raw.replace_range(b.left..b.right, "");
        self.lines = std::iter::once(0).chain(self.raw.match_indices("\n").map(|(i, _)| i + 1)).collect();
    }
    
    fn index_from_pos(&self, pos: Pos) -> Option<usize> {
        //TODO: Check if pos.x is within bounds
        self.lines.get(pos.y).map(|index| { index + pos.x })
    }

    fn pos_from_index(&self, index: usize) -> Option<Pos> {
        if index >= self.raw.len() { None }
        else {
            let y = self.lines.iter().position(|&i| i > index).unwrap() - 1;
            let x = index - self.lines[y];
            Some(Pos::new(x, y))
        }
    }

    fn find(&self, _c: char) -> Option<usize> {
        unimplemented!()
    }

    fn rfind(&self, _c: char) -> Option<usize> {
        unimplemented!()
    }

}

impl Default for Content {
    fn default() -> Self {
        Self { raw: String::from("\n"), lines: vec![0, 1] }
    }
}

pub struct Buffer {
    /// The text contained by the buffer
    pub content: Content,
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
    pub pos: Pos,
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
        let content = Content::new(fs::read_to_string(&path).unwrap_or(String::from("\n")));
        let line_nr_cols = content.len().to_string().len() + 1;
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
            pos: Pos::new(0, 0),
            saved_col: 0,
        }
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

    pub fn bounds(&self, sel: Selection) -> Option<Bounds> {
        Some(match sel {
            Selection::Lines(amount) => {
                Bounds::new(self.content.index_from_pos(Pos::new(0, self.pos.y)).unwrap(), self.content.index_from_pos(Pos::new(0, self.pos.y + amount)).unwrap())
            }
            Selection::UpTo(mov) => Bounds::new(self.content.index_from_pos(self.pos).unwrap(), self.content.index_from_pos(self.get_destination(mov)).unwrap()),
            Selection::Between {
                first,
                last,
                inclusive,
            } => match (self.content.rfind(first), self.content.find(last)) {
                (Some(pos1), Some(pos2)) => Bounds::from_delimiters(pos1, pos2, inclusive),
                _ => return None,
            },
            Selection::Word { inclusive: _ } => return None,
            Selection::Paragraph { inclusive: _ } => return None,
        })
    }

    /// Saves the current state of the buffer to the file
    fn save(&mut self) -> Result<()> {
        fs::write(&self.path, self.content.raw.as_bytes())?;
        self.status = format!(
            "\"{}\" {}L written",
            self.path.to_str().unwrap(),
            self.content.len()
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

        let mut hl = Highlighter::new();
        queue!(w, SavePosition, Hide, MoveTo(0, 0), Clear(ClearType::All))?;

        queue!(w, Print(format!(
            "{: >width$} ",
            self.pos.y,
            width = self.line_nr_cols - 1
        )))?;
        let mut line_nr = 1;
        let mut chars = self.content.raw.chars();

        match &self.hl_conf {
            None => {
                for c in chars {
                    match c {
                        '\t' => queue!(w, Print("   "))?,
                        '\n' => {
                            queue!(w, MoveToNextLine(1),
                                Print(format!(
                                    "{: >width$} ",
                                    (line_nr as i64 - self.pos.y as i64).abs(),
                                    width = self.line_nr_cols - 1
                                ))
                            )?;
                            line_nr += 1;
                        }
                        _ => queue!(w, Print(c))?,
                    }
                }
            }
            Some(hl_conf) => {
                let highlights = hl.highlight(
                    hl_conf,
                    self.content.raw.as_bytes(),
                    None,
                    |_| None
                ).unwrap();

                for event in highlights {
                    match event.unwrap() {
                        HighlightEvent::Source {start, end} => {
                            for _ in start..end {
                                match chars.next().unwrap() {
                                    '\t' => queue!(w, Print("   "))?,
                                    '\n' => {
                                        queue!(w, MoveToNextLine(1),
                                            Print(format!(
                                                "{: >width$} ",
                                                (line_nr as i64 - self.pos.y as i64).abs(),
                                                width = self.line_nr_cols - 1
                                            ))
                                        )?;
                                        line_nr += 1;
                                    }
                                    c => queue!(w, Print(c))?,
                                }
                            }
                        },
                        HighlightEvent::HighlightStart(s) => {
                            let style = hl_styles[s.0];
                            queue!(w, SetBackgroundColor(style.background_color.unwrap()))?;
                            queue!(w, SetForegroundColor(style.foreground_color.unwrap()))?;
                            queue!(w, SetAttributes(style.attributes))?;
                        },
                        HighlightEvent::HighlightEnd => {
                            queue!(w, ResetColor)?;
                        },
                    }
                }
            }
        }

        //let visible = self.content.lines[self.offset.y]..self.content.lines[usize::min(self.offset.y + self.height, self.content.len())];
        
        queue!(
            w,
            MoveTo(0, self.height as u16 - 2),
            Print(format!(
                "{: <width$} {: >3}:{: <3}",
                self.path.to_str().unwrap(),
                self.pos.y,
                self.pos.x,
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
        self.line_nr_cols = self.content.len().to_string().len() + 1;
    }

    pub fn move_cursor(&mut self, movement_type: Movement) {
        self.pos = self.get_destination(movement_type);
        match movement_type {
            Movement::Left(_) | Movement::Right(_) | Movement::Home | Movement::End => {
                self.saved_col = self.pos.x
            }
            _ => (),
        }
    }

    fn max_col(&self, y: usize) -> usize {
        self.content.line_len(y) - 1
    }

    fn get_destination(&self, movement_type: Movement) -> Pos {
        match movement_type {
            Movement::Up(amount) => {
                let y = self.pos.y.saturating_sub(amount);
                let x = usize::min(self.max_col(y), self.saved_col);
                Pos::new(x, y)
            }
            Movement::Down(amount) => {
                let y = usize::min(self.pos.y + amount, self.content.len().saturating_sub(1));
                let x = usize::min(self.max_col(y), self.saved_col);
                Pos::new(x, y)
            }
            Movement::Left(amount) => {
                let x = self.pos.x.saturating_sub(amount);
                Pos::new(x, self.pos.y)
            }
            Movement::Right(amount) => {
                let x = usize::min(self.pos.x + amount, self.max_col(self.pos.y));
                Pos::new(x, self.pos.y)
            }
            Movement::Home => Pos::new(0, self.pos.y),
            Movement::End => Pos::new(self.max_col(self.pos.y), self.pos.y),
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
                Pos::new(x, y)
            }
            Movement::Bottom => {
                let y = self.offset.x + self.height - 3;
                let x = usize::min(self.max_col(y), self.saved_col);
                Pos::new(x, y)
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
                if self.pos.x.saturating_sub(self.offset.x) < 5 {
                    self.offset.x = self.pos.x.saturating_sub(5);
                }
                // Scroll right if cursor is on right side of bounds
                if self.pos.x.saturating_sub(self.offset.x) + self.line_nr_cols + 5 + 1 > self.width {
                    self.offset.x = (self.pos.x + self.line_nr_cols + 5 + 1).saturating_sub(self.width);
                }
                // Scroll up if cursor is above bounds
                if self.pos.y.saturating_sub(self.offset.y) < 3 {
                    self.offset.y = self.pos.y.saturating_sub(3);
                }
                // Scroll down if cursor is below bounds
                if self.pos.y.saturating_sub(self.offset.y) + 3 + 2 > self.height {
                    self.offset.y = (self.pos.y + 3 + 2).saturating_sub(self.height);
                }
                queue!(
                    w,
                    MoveTo(
                        (self.pos.x - self.offset.x + self.line_nr_cols) as u16,
                        (self.pos.y - self.offset.y) as u16
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
                        com.execute(w, self)?;
                    }
                }
            }
            EditMode::Insert => match key_event.code {
                KeyCode::Char(c) => {
                    let mut tmp = [0u8; 4];
                    self.content.insert(self.pos, c.encode_utf8(&mut tmp));
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
                    let right = self.content.index_from_pos(self.pos).unwrap();
                    let left = right.saturating_sub(1);
                    self.content.delete(Bounds::new(left, right));
                    self.pos = self.content.pos_from_index(left).unwrap();
                }
                KeyCode::Delete => {
                    let index = self.content.index_from_pos(self.pos).unwrap();
                    self.content.delete(Bounds::new(index, index + 1));
                }
                KeyCode::Tab => self.content.insert(self.pos, "\t"),
                KeyCode::Enter => {
                    self.content.insert(self.pos, "\n");
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
}
