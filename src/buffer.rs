use crate::command::{Command, Movement, Selection};
use crossterm::{
    cursor::{
        CursorShape, Hide, MoveLeft, MoveTo, MoveToNextLine, RestorePosition, SavePosition,
        SetCursorShape, Show,
    },
    event::{DisableMouseCapture, KeyCode, KeyEvent},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnableLineWrap, LeaveAlternateScreen},
    Result,
};
use std::{fs, io::Write, path::PathBuf, process};

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

struct Bounds {
    pub left: Pos,
    pub right: Pos,
}

impl Bounds {
    pub fn new(pos1: Pos, pos2: Pos) -> Self {
        if pos1 < pos2 {
            Self {
                left: pos1,
                right: pos2,
            }
        } else {
            Self {
                left: pos2,
                right: pos1,
            }
        }
    }

    pub fn from_delimiters(pos1: Pos, pos2: Pos, inclusive: bool) -> Self {
        let mut bounds = Self::new(pos1, pos2);
        match inclusive {
            true => bounds.right.x += 1,
            false => bounds.left.x += 1,
        }
        bounds
    }
}

#[derive(Default)]
pub struct Buffer {
    /// The text contained by the buffer
    content: Vec<String>,
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
    /// Saved column index for easier traversal
    saved_col: usize,
    /// Position the buffer is scrolled to
    offset: Pos,
    /// Position of cursor in buffer
    pos: Pos,
    /// The width the buffer gets to render
    width: usize,
    /// The height the buffer gets to render
    height: usize,
    /// The amount of columns reserved for line numbers
    pub line_nr_cols: usize,
    /// The size of tab characters
    tab_size: usize,
}

impl Buffer {
    pub fn new(path: PathBuf) -> Self {
        let (width, height) = terminal::size().unwrap();
        let content: Vec<String> = fs::read_to_string(&path)
            .unwrap_or(String::from("\n"))
            .lines()
            .map(String::from)
            .collect();
        let line_nr_cols = content.len().to_string().len() + 1;

        Buffer {
            content,
            path,
            width: width as usize,
            height: height as usize,
            line_nr_cols,
            tab_size: 4,
            ..Buffer::default()
        }
    }

    pub fn update_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    fn set_mode<W: Write>(&mut self, w: &mut W, mode: EditMode) -> Result<()> {
        match mode {
            EditMode::Insert => queue!(w, SetCursorShape(CursorShape::Line))?,
            EditMode::Normal => queue!(w, SetCursorShape(CursorShape::Block))?,
            EditMode::Command => self.status = String::from(":"),
        }
        self.mode = mode;
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
                self.try_execute_normal_mode_command(w)?;
            }
            EditMode::Insert => match key_event.code {
                KeyCode::Char(c) => self.insert(c),
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
                    self.delete(self.bounds(Selection::UpTo(Movement::Left(1))).unwrap())
                }
                KeyCode::Delete => {
                    self.delete(self.bounds(Selection::UpTo(Movement::Right(1))).unwrap())
                }
                KeyCode::Tab => self.insert_tab(),
                KeyCode::Enter => {
                    self.new_line();
                    self.move_cursor(Movement::Down(1));
                    self.move_cursor(Movement::Home);
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

    fn try_execute_normal_mode_command<W: Write>(&mut self, w: &mut W) -> Result<()> {
        if let Some(commands) = Command::parse(&self.status) {
            self.command.clear();
            for com in commands {
                match com {
                    Command::Undo => (), //self.undo(),
                    Command::Redo => (), //self.redo(),
                    Command::Move(dir) => self.move_cursor(dir),
                    Command::Delete(sel) => {
                        if let Some(bounds) = self.bounds(sel) {
                            self.delete(bounds)
                        }
                    }
                    Command::Yank(_sel) => (), //self.yank(sel),
                    Command::Paste => (),      //self.paste(plc),
                    Command::CreateNewLine => self.new_line(),
                    Command::SetMode(mode) => self.set_mode(w, mode)?,
                }
            }
        }
        Ok(())
    }

    fn bounds(&self, sel: Selection) -> Option<Bounds> {
        Some(match sel {
            Selection::Lines(amount) => {
                Bounds::new(Pos::new(0, self.pos.y), Pos::new(0, self.pos.y + amount))
            }
            Selection::UpTo(mov) => Bounds::new(self.pos, self.get_destination(mov)),
            Selection::Between {
                first,
                last,
                inclusive,
            } => match (self.rfind(first), self.find(last)) {
                (Some(pos1), Some(pos2)) => Bounds::from_delimiters(pos1, pos2, inclusive),
                _ => return None,
            },
            Selection::Word { inclusive: _ } => return None,
            Selection::Paragraph { inclusive: _ } => return None,
        })
    }

    /// Finds position of rightmost character that matches `c` on the left side of the cursor
    fn rfind(&self, c: char) -> Option<Pos> {
        let (left, _) = self.content[self.pos.y].split_at(self.pos.x);
        if let Some(x) = left.rfind(c) {
            return Some(Pos::new(x, self.pos.y));
        }
        for y in (0..self.pos.y).rev() {
            if let Some(x) = self.content[y].rfind(c) {
                return Some(Pos::new(x, y));
            }
        }
        None
    }

    /// Finds position of leftmost character that matches `c` on the right side of the cursor
    fn find(&self, c: char) -> Option<Pos> {
        let (left, right) = self.content[self.pos.y].split_at(self.pos.x);
        if let Some(x) = right.find(c) {
            return Some(Pos::new(left.len() + x, self.pos.y));
        }
        for y in self.pos.y..self.content.len() {
            if let Some(x) = self.content[y].find(c) {
                return Some(Pos::new(x, y));
            }
        }
        None
    }

    /// Saves the current state of the buffer to the file
    fn save(&mut self) -> Result<()> {
        fs::write(&self.path, self.content.join("\n").as_bytes())?;
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
        process::exit(1);
    }

    pub fn draw<W: Write>(&self, w: &mut W) -> Result<()> {
        queue!(w, SavePosition, Hide, MoveTo(0, 0), Clear(ClearType::All))?;
        for (i, line) in self
            .content
            .iter()
            .enumerate()
            .skip(self.offset.y)
            .take(self.height.saturating_sub(2))
        {
            queue!(
                w,
                Print(format!(
                    "{: >width$} ",
                    (i as i64 - self.pos.y as i64).abs(),
                    width = self.line_nr_cols - 1
                ))
            )?;
            if line.len() > self.offset.x {
                queue!(
                    w,
                    Print(
                        &line[self.offset.x
                            ..usize::min(
                                line.len(),
                                self.offset.x + self.width - self.line_nr_cols
                            )]
                    )
                )?;
            } else if line.len() > 0 {
                queue!(w, MoveLeft(1), Print("<"))?;
            }
            queue!(w, MoveToNextLine(1))?;
        }
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
    fn update_line_nr_cols(&mut self) {
        self.line_nr_cols = self.content.len().to_string().len() + 1;
    }

    fn new_line(&mut self) {
        self.edited = true;
        let new_line = self.content[self.pos.y].split_off(self.pos.x);
        self.content.insert(self.pos.y + 1, new_line);
        self.update_line_nr_cols();
    }

    fn insert(&mut self, c: char) {
        self.edited = true;
        self.content
            .get_mut(self.pos.y)
            .expect("self.pos.y out of range for buffer")
            .insert(self.pos.x, c);
        self.pos.x += 1;
    }

    fn insert_tab(&mut self) {
        self.edited = true;
        let spaces = 4 - self.pos.x % self.tab_size;
        self.content
            .get_mut(self.pos.y)
            .expect("self.pos.y out of range for buffer")
            .insert_str(self.pos.x, &" ".repeat(spaces));
        self.pos.x += spaces;
    }

    fn delete(&mut self, b: Bounds) {
        self.edited = true;
        let left = String::from(self.content[b.left.y].split_at(b.left.x).0);
        let right = String::from(self.content[b.right.y].split_at(b.right.x).1);
        self.content[b.right.y] = left.to_string() + &right;
        self.content.drain(b.left.y..b.right.y);
        self.pos = b.left;
    }

    fn max_col(&self, y: usize) -> usize {
        self.content[y].len()
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
            Movement::FirstChar => Pos::new(
                self.content[self.pos.y]
                    .find(|c| !char::is_whitespace(c))
                    .unwrap_or(0),
                self.pos.y,
            ),
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

    fn move_cursor(&mut self, movement_type: Movement) {
        self.pos = self.get_destination(movement_type);
        match movement_type {
            Movement::Left(_) | Movement::Right(_) | Movement::Home | Movement::End => {
                self.saved_col = self.pos.x
            }
            _ => (),
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
}

#[cfg(test)]
mod test {
    use super::{Buffer, Pos};

    fn get_buffer() -> Buffer {
        let content: Vec<String> = "fn test(x: usize) -> Pos {
    let y = usize::min(x, 3);
    Pos { x, y }
}
"
        .lines()
        .map(String::from)
        .collect();
        Buffer {
            content,
            width: 100,
            height: 50,
            ..Buffer::default()
        }
    }

    #[test]
    fn find_left_bracket() {
        let mut buffer = get_buffer();
        buffer.pos = Pos::new(10, 0);
        assert_eq!(Some(Pos::new(7, 0)), buffer.rfind('('));
        buffer.pos = Pos::new(11, 0);
        assert_eq!(Some(Pos::new(7, 0)), buffer.rfind('('));
        buffer.pos = Pos::new(12, 0);
        assert_eq!(Some(Pos::new(7, 0)), buffer.rfind('('));
        buffer.pos = Pos::new(13, 0);
        assert_eq!(Some(Pos::new(7, 0)), buffer.rfind('('));
    }

    #[test]
    fn find_right_bracket() {
        let mut buffer = get_buffer();
        buffer.pos = Pos::new(10, 0);
        assert_eq!(Some(Pos::new(16, 0)), buffer.find(')'));
        buffer.pos = Pos::new(11, 0);
        assert_eq!(Some(Pos::new(16, 0)), buffer.find(')'));
        buffer.pos = Pos::new(12, 0);
        assert_eq!(Some(Pos::new(16, 0)), buffer.find(')'));
        buffer.pos = Pos::new(13, 0);
        assert_eq!(Some(Pos::new(16, 0)), buffer.find(')'));
    }

    #[test]
    fn skip_other_bracket_pairs() {
        let mut buffer = get_buffer();
        buffer.pos = Pos::new(0, 1);
        assert_eq!(Some(Pos::new(36, 0)), buffer.rfind('{'));
        assert_eq!(Some(Pos::new(0, 3)), buffer.find('}'));
    }
}
