use crossterm::{
    cursor::{
        CursorShape, Hide, MoveLeft, MoveTo, MoveToNextLine, RestorePosition, SavePosition,
        SetCursorShape, Show,
    },
    event::{KeyCode, KeyEvent, DisableMouseCapture},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType,
            LeaveAlternateScreen,
            EnableLineWrap
    },
    Result,
};
use std::{fs, 
    path::PathBuf,
    io::Write, process};
use crate::command::{Command, Movement};

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


#[derive(Default)]
pub struct Buffer {
    /// The text contained by the buffer
    pub content: Vec<String>,
    /// The path of the file being edited
    path: PathBuf,
    /// The mode the buffer is currently being edited in
    mode: EditMode,
    /// The contents of the bottom statusline
    status: String,
    /// Whether the buffer has been edited since saving
    pub edited: bool,
    /// The amount of lines scrolled down
    row_offset: usize,
    /// The amount of column scrolled to the side
    col_offset: usize,
    /// Saved column index for easier traversal
    saved_col: usize,
    /// Row index in `content` vector
    row: usize,
    /// Column index in row
    col: usize,
    /// The width the buffer gets to render
    pub width: usize,
    /// The height the buffer gets to render
    pub height: usize,
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

    fn set_mode<W: Write>(&mut self, w: &mut W, mode: EditMode) -> Result<()> {
        match mode {
            EditMode::Insert => queue!(w, SetCursorShape(CursorShape::Line))?,
            EditMode::Normal => queue!(w, SetCursorShape(CursorShape::Block))?,
            EditMode::Command => (),
        }
        self.mode = mode;
        Ok(())
    }

    pub fn handle_keyevent<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        match self.mode {
            EditMode::Normal => self.handle_keyevent_normal(w, key_event)?,
            EditMode::Insert => self.handle_keyevent_insert(w, key_event)?,
            EditMode::Command => self.handle_keyevent_command(w, key_event)?,
        }
        self.update_cursor(w)?;
        Ok(())
    }

    pub fn handle_keyevent_normal<W: Write>(
        &mut self,
        w: &mut W,
        key_event: KeyEvent,
    ) -> Result<()> {
        match key_event.code {
            KeyCode::Char('i') => self.set_mode(w, EditMode::Insert)?,
            KeyCode::Char('I') => {
                self.set_mode(w, EditMode::Insert)?;
                self.move_cursor(Movement::FirstChar);
            }
            KeyCode::Char('a') => {
                self.set_mode(w, EditMode::Insert)?;
                self.move_cursor(Movement::Right(1));
            }
            KeyCode::Char('A') => {
                self.set_mode(w, EditMode::Insert)?;
                self.move_cursor(Movement::End);
            }
            KeyCode::Char('x') => self.delete(0),
            KeyCode::Char('o') => {
                self.set_mode(w, EditMode::Insert)?;
                self.move_cursor(Movement::End);
                self.new_line();
                self.move_cursor(Movement::Down(1));
                self.move_cursor(Movement::Home);
            }
            KeyCode::Char('O') => {
                self.set_mode(w, EditMode::Insert)?;
                self.move_cursor(Movement::Home);
                self.new_line();
            }
            KeyCode::Esc => self.status.clear(),
            KeyCode::Char(':') => {
                self.set_mode(w, EditMode::Command)?;
                self.status = String::from(":");
            }
            KeyCode::Char(c) => self.status.push(c),
            KeyCode::Up => self.status.push('k'),
            KeyCode::Down => self.status.push('j'),
            KeyCode::Left => self.status.push('h'),
            KeyCode::Right => self.status.push('l'),
            KeyCode::Home => self.status.push('0'),
            KeyCode::End => self.status.push('$'),
            KeyCode::Delete => self.status.push('x'),
            _ => (),
        }
        Ok(())
    }

    fn handle_keyevent_insert<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
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
            KeyCode::Char(c) => self.insert(c),
            KeyCode::Backspace => self.delete(-1),
            KeyCode::Delete => self.delete(0),
            KeyCode::Tab => self.insert_tab(),
            KeyCode::Enter => {
                self.new_line();
                self.move_cursor(Movement::Down(1));
                self.move_cursor(Movement::Home);
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_keyevent_command<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char(c) => self.status.push(c),
            KeyCode::Backspace => { self.status.pop(); }
            KeyCode::Esc => {
                self.set_mode(w, EditMode::Normal)?;
                queue!(w, RestorePosition)?;
            }
            KeyCode::Enter => {
                self.set_mode(w, EditMode::Normal)?;
                queue!(w, RestorePosition)?;
                self.execute_command(w)?;
            }
            _ => (),
        }
        Ok(())
    }

    fn execute_command<W: Write>(&mut self, w: &mut W) -> Result<()> {
        let (command, argument) = self.status.as_str().split_at(1);
        match command {
            ":" => match argument {
                "w" => self.save()?,
                "q" => if !self.edited {
                        self.quit(w)?
                    } else {
                        self.status = String::from("Error: No write since last change. To quit without saving, use ':q!'")
                    }
                "q!" => self.quit(w)?,
                "wq" | "x" => {
                    self.save()?;
                    self.quit(w)?;
                }
                _ => self.status = format!("Error: invalid argument ({}) for command ({})", argument, command),
            }
            _ => {
                for com in Command::parse(&self.status) {
                    match com {
                        
                    }
                }
            }
        }
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        fs::write(&self.path, self.content.join("\n").as_bytes())?;
        self.status = format!("\"{}\" {}L written", self.path.to_str().unwrap(), self.content.len());
        self.edited = false;
        Ok(())
    }

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
            .skip(self.row_offset)
            .take(self.height.saturating_sub(2))
        {
            queue!(
                w,
                Print(format!(
                    "{: >width$} ",
                    (i as i64 - self.row as i64).abs(),
                    width = self.line_nr_cols - 1
                ))
            )?;
            if line.len() > self.col_offset {
                queue!(
                    w,
                    Print(
                        &line[self.col_offset
                            ..usize::min(
                                line.len(),
                                self.col_offset + self.width - self.line_nr_cols
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
            Print(format!("{: <width$} {: >3}:{: <3}", self.path.to_str().unwrap(), self.row, self.col, width=self.width - 9)),
            MoveTo(0, self.height as u16 - 1),
            Print(&self.status)
        )?;
        queue!(w, RestorePosition, Show)?;
        w.flush()?;
        Ok(())
    }

    fn update_line_nr_cols(&mut self) {
        self.line_nr_cols = self.content.len().to_string().len() + 1;
    }

    fn new_line(&mut self) {
        self.edited = true;
        let new_line = self.content[self.row].split_off(self.col);
        self.content.insert(self.row + 1, new_line);
        self.update_line_nr_cols();
    }

    fn insert(&mut self, c: char) {
        self.edited = true;
        self.content
            .get_mut(self.row)
            .expect("self.row out of range for buffer")
            .insert(self.col, c);
        self.col += 1;
    }

    fn insert_tab(&mut self) {
        self.edited = true;
        let spaces = 4 - self.col % self.tab_size;
        self.content
            .get_mut(self.row)
            .expect("self.row out of range for buffer")
            .insert_str(self.col, &" ".repeat(spaces));
        self.col += spaces;
    }

    fn delete(&mut self, offset: isize) {
        self.edited = true;
        if self.col == 0 && offset == -1 {
            if self.row == 0 {
                return;
            }
            self.row -= 1;
            self.move_cursor(Movement::End);
            let row = self.content[self.row + 1].clone();
            self.content[self.row].push_str(row.as_str());
            self.content.remove(self.row + 1);
            self.update_line_nr_cols();
        } else {
            let removed = (self.col as isize + offset) as usize;
            self.content
                .get_mut(self.row)
                .expect("self.row out of range for buffer")
                .remove(removed);
            self.col = removed;
        }
        self.col = usize::min(self.max_col(), self.col);
    }

    // Scroll left if cursor is on left side of bounds
    fn clamp_cursor_left(&mut self, pad: usize) {
        if self.col.saturating_sub(self.col_offset) < pad {
            self.col_offset = self.col.saturating_sub(pad);
        }
    }

    // Scroll right if cursor is on right side of bounds
    fn clamp_cursor_right(&mut self, pad: usize) {
        if self.col.saturating_sub(self.col_offset) + self.line_nr_cols + pad + 1 > self.width {
            self.col_offset = (self.col + self.line_nr_cols + pad + 1).saturating_sub(self.width);
        }
    }

    // Scroll up if cursor is above bounds
    fn clamp_cursor_top(&mut self, pad: usize) {
        if self.row.saturating_sub(self.row_offset) < pad {
            self.row_offset = self.row.saturating_sub(pad);
        }
    }

    // Scroll down if cursor is below bounds
    fn clamp_cursor_bottom(&mut self, pad: usize) {
        if self.row.saturating_sub(self.row_offset) + pad + 2 > self.height {
            self.row_offset = (self.row + pad + 2).saturating_sub(self.height);
        }
    }

    fn restore_saved_col(&mut self) {
        self.col = usize::min(
            self.saved_col,
            self.content[self.row].len().saturating_sub(1),
        );
    }

    fn max_col(&self) -> usize {
        match self.mode {
            EditMode::Normal => self.content[self.row].len().saturating_sub(1),
            EditMode::Insert => self.content[self.row].len(),
            EditMode::Command => self.status.len(),
        }
    }

    fn move_cursor(&mut self, movement_type: Movement) {
        match movement_type {
            Movement::Up(amount) => {
                self.row = self.row.saturating_sub(amount);
                self.restore_saved_col();
                self.clamp_cursor_top(3);
                self.clamp_cursor_left(5);
                self.clamp_cursor_right(5);
            }
            Movement::Down(amount) => {
                self.row = usize::min(self.row + amount, self.content.len().saturating_sub(1));
                self.restore_saved_col();
                self.clamp_cursor_bottom(3);
                self.clamp_cursor_left(5);
                self.clamp_cursor_right(5);
            }
            Movement::Left(amount) => {
                self.col = self.col.saturating_sub(amount);
                self.saved_col = self.col;
                self.clamp_cursor_left(5);
            }
            Movement::Right(amount) => {
                self.col = usize::min(self.col + amount, self.max_col());
                self.saved_col = self.col;
                self.clamp_cursor_right(5);
            }
            Movement::Home => {
                self.col = 0;
                self.saved_col = 0;
                // No clamping needed, because we already know the offset will be 0.
                self.col_offset = 0;
            }
            Movement::End => {
                self.col = self.max_col();
                self.saved_col = self.col;
                self.clamp_cursor_right(5);
            }
            Movement::FirstChar => {
                let index = self.content[self.row]
                    .find(|c| !char::is_whitespace(c))
                    .unwrap_or(0);
                self.col = index;
                self.clamp_cursor_left(5);
            }
            Movement::Top => {
                self.row = self.col_offset + 3;
                self.restore_saved_col();
            }
            Movement::Bottom => {
                self.row = self.col_offset + self.height - 3;
                self.restore_saved_col();
            }
            Movement::NextWord(_amount) => {
                unimplemented!();
            }
            Movement::PrevWord(_amount) => {
                unimplemented!();
            }

        }
    }

    fn update_cursor<W: Write>(&mut self, w: &mut W) -> Result<()> {
        match self.mode {
            EditMode::Command => {
                queue!(w, MoveTo(self.status.len() as u16, self.height as u16 - 1))?
            }
            _ => queue!(
                w,
                MoveTo(
                    (self.col - self.col_offset + self.line_nr_cols) as u16,
                    (self.row - self.row_offset) as u16
                )
            )?,
        }
        Ok(())
    }
}
