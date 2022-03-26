use crate::buffer::Buffer;
use crate::buffer::EditMode;
use crate::utils::{Movement, Selection};
use undo::Action;
use cli_clipboard;

pub struct Move {
    pub movement: Movement,
    origin: usize,
    saved_col: Option<usize>,
}

impl Move {
    pub fn new(movement: Movement) -> Self {
        Self {
            movement,
            origin: 0,
            saved_col: None,
        }
    }
}

impl Action for Move {
    type Target = Buffer;
    type Output = ();
    type Error = crossterm::ErrorKind;

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        self.origin = buf.idx;
        let dest = self.movement.dest(buf);
        buf.set_cursor(dest)?;
        if let Movement::Left(_) | Movement::Right(_) | Movement::Home | Movement::End =
            self.movement
        {
            self.saved_col = Some(buf.saved_col);
            buf.saved_col = buf.col();
        }
        Ok(())
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.set_cursor(self.origin)?;
        if let Some(col) = self.saved_col {
            buf.saved_col = col
        };
        Ok(())
    }
}

pub struct Delete {
    selection: Selection,
    origin: usize,
    left_bound: usize,
    removed: String,
}

impl Delete {
    pub fn new(selection: Selection) -> Self {
        Self {
            selection,
            origin: 0,
            left_bound: 0,
            removed: String::new(),
        }
    }
}

impl Action for Delete {
    type Target = Buffer;
    type Output = ();
    type Error = crossterm::ErrorKind;

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        let bounds = self.selection.bounds(&buf);
        self.left_bound = bounds.start;
        self.removed = buf.content.slice(bounds.clone()).to_string();
        buf.remove(bounds)
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.insert(self.left_bound, &self.removed)?;
        buf.set_cursor(self.origin)
    }
}

pub struct Insert {
    text: String,
    origin: usize,
}

impl Insert {
    pub fn new(text: String) -> Self {
        Self { text, origin: 0 }
    }

    pub fn from_clipboard() -> Self {
        Self { text: cli_clipboard::get_contents().expect("Error getting system clipboard"), origin: 0 }
    }
}

impl Action for Insert {
    type Target = Buffer;
    type Output = ();
    type Error = crossterm::ErrorKind;

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        self.origin = buf.idx;
        buf.insert(buf.idx, &self.text)?;
        Ok(())
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.remove(self.origin..(self.origin + self.text.len()))?;
        Ok(())
    }
}
pub struct Yank {
    selection: Selection,   
}

impl Yank {
    pub fn new(selection: Selection) -> Self {
        Self { selection }
    }
}

impl Action for Yank {
    type Target = Buffer;
    type Output = ();
    type Error = crossterm::ErrorKind;

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        let bounds = self.selection.bounds(buf);
        let text = buf.content.slice(bounds);
        cli_clipboard::set_contents(text.to_string()).expect("Error setting system clipboard");
        Ok(())
    }

    fn undo(&mut self, _buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        Ok(())
    }
}

pub struct SetMode {
    mode: EditMode,
    prev_mode: EditMode
}

impl SetMode {
    pub fn new(mode: EditMode) -> Self {
        Self { mode, prev_mode: EditMode::Normal }
    }
}

impl Action for SetMode {
    type Target = Buffer;
    type Output = ();
    type Error = crossterm::ErrorKind;

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        self.prev_mode = buf.mode;
        buf.mode = self.mode;
        Ok(())
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.mode = self.prev_mode;
        Ok(())
    }
}

// pub struct Command {
//
// }
//
// impl Command {
//     pub fn new() -> Self {
//         Command { }
//     }
// }
//
// impl Action for Command {
//     type Target = Buffer;
//     type Output = ();
//     type Error = crossterm::ErrorKind;
//
//     fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
//         Ok(())
//     }
//
//     fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
//         Ok(())
//     }
// }


// impl Command {
//     pub fn parse(command: &str) -> Option<Vec<impl Command>> {
//         lazy_static! {
//             static ref RE: Regex = Regex::new(r#"^([AaCDPphIiJjKklOoUuYXx0$:])$|^(?:([dcy])([1-9][0-9]*)?([dcyw$0]|[ais][wp"'\{\}\[\]\(\)]))$"#).unwrap();
//         }
//         let captures = match RE.captures(command) {
//             Some(cap) => cap,
//             None => return None,
//         };
//         let amount = captures
//             .get(3)
//             .map(|cap| {
//                 cap.as_str()
//                     .parse::<usize>()
//                     .expect("Unable to parse count in command regex")
//             })
//             .unwrap_or(1);
//         match captures.get(1) {
//             // Matches if command has length 1
//             Some(cap) => Command::parse_simple(cap.as_str()),
//             None => captures.get(4).and_then(|cap| {
//                 let c = captures.get(2).unwrap().as_str(); // Every command should have a main command unless it has length 1
//                 Command::parse_selection(c, cap.as_str(), amount).and_then(|selection| {
//                     Some(match c {
//                         "d" => vec![Self::Delete(selection)],
//                         "c" => vec![Self::Delete(selection), Self::SetMode(EditMode::Insert)],
//                         "y" => vec![Self::Yank(selection)],
//                         _ => return None,
//                     })
//                 })
//             }),
//         }
//     }
//
//     fn parse_simple(command: &str) -> Option<Vec<Self>> {
//         Some(match command {
//             "A" => vec![Self::SetMode(EditMode::Insert), Self::Move(Movement::End)],
//             "a" => vec![
//                 Self::SetMode(EditMode::Insert),
//                 Self::Move(Movement::Right(1)),
//             ],
//             "b" => vec![Self::Move(Movement::PrevWord(1))],
//             "C" => vec![
//                 Self::Delete(Selection::UpTo(Movement::End)),
//                 Self::SetMode(EditMode::Insert),
//             ],
//             "D" => vec![Self::Delete(Selection::UpTo(Movement::End))],
//             "P" => vec![
//                 Self::Move(Movement::Up(1)),
//                 Self::Move(Movement::End),
//                 Self::Paste,
//             ],
//             "p" => vec![Self::Paste],
//             "h" => vec![Self::Move(Movement::Left(1))],
//             "I" => vec![
//                 Self::SetMode(EditMode::Insert),
//                 Self::Move(Movement::FirstChar),
//             ],
//             "i" => vec![Self::SetMode(EditMode::Insert)],
//             "J" => vec![Self::Move(Movement::Bottom)],
//             "j" => vec![Self::Move(Movement::Down(1))],
//             "K" => vec![Self::Move(Movement::Top)],
//             "k" => vec![Self::Move(Movement::Up(1))],
//             "l" => vec![Self::Move(Movement::Right(1))],
//             "O" => vec![
//                 Self::SetMode(EditMode::Insert),
//                 Self::Move(Movement::Up(1)),
//                 Self::Move(Movement::End),
//                 Self::CreateNewLine,
//                 Self::Move(Movement::Down(1)),
//             ],
//             "o" => vec![
//                 Self::SetMode(EditMode::Insert),
//                 Self::Move(Movement::End),
//                 Self::CreateNewLine,
//                 Self::Move(Movement::Down(1)),
//             ],
//             "w" => vec![Self::Move(Movement::NextWord(1))],
//             "U" => vec![Self::Redo],
//             "u" => vec![Self::Undo],
//             "Y" => vec![Self::Yank(Selection::Lines(1))],
//             "X" => vec![Self::Delete(Selection::UpTo(Movement::Left(1)))],
//             "x" => vec![Self::Delete(Selection::UpTo(Movement::Right(1)))],
//             "0" => vec![Self::Move(Movement::Home)],
//             "$" => vec![Self::Move(Movement::End)],
//             ":" => vec![Self::SetMode(EditMode::Command)],
//             _ => return None,
//         })
//     }
//
//     fn parse_selection(c: &str, selection: &str, amount: usize) -> Option<Selection> {
//         Some(match selection {
//             "w" => Selection::UpTo(Movement::NextWord(1)),
//             "iw" => Selection::Word { inclusive: false },
//             "aw" => Selection::Word { inclusive: true },
//             "ip" => Selection::Paragraph { inclusive: false },
//             "ap" => Selection::Paragraph { inclusive: true },
//             "$" => Selection::UpTo(Movement::End),
//             "0" => Selection::UpTo(Movement::Home),
//             "i\"" => Selection::Between {
//                 first: '\"',
//                 last: '\"',
//                 inclusive: false,
//             },
//             "i\'" => Selection::Between {
//                 first: '\'',
//                 last: '\'',
//                 inclusive: false,
//             },
//             "i[" | "i]" => Selection::Between {
//                 first: '[',
//                 last: ']',
//                 inclusive: false,
//             },
//             "i(" | "i)" => Selection::Between {
//                 first: '(',
//                 last: ')',
//                 inclusive: false,
//             },
//             "i{" | "i}" => Selection::Between {
//                 first: '{',
//                 last: '}',
//                 inclusive: false,
//             },
//             "a\"" => Selection::Between {
//                 first: '\"',
//                 last: '\"',
//                 inclusive: true,
//             },
//             "a\'" => Selection::Between {
//                 first: '\'',
//                 last: '\'',
//                 inclusive: true,
//             },
//             "a[" | "a]" => Selection::Between {
//                 first: '[',
//                 last: ']',
//                 inclusive: true,
//             },
//             "a(" | "a)" => Selection::Between {
//                 first: '(',
//                 last: ')',
//                 inclusive: true,
//             },
//             "a{" | "a}" => Selection::Between {
//                 first: '{',
//                 last: '}',
//                 inclusive: true,
//             },
//             //"s\"" => Selection::Surrounders('\"', '\"'),
//             //"s\'" => Selection::Surrounders('\'', '\''),
//             //"s[" | "s]" => Selection::Surrounders('[', ']'),
//             //"s(" | "s)" => Selection::Surrounders('(', ')'),
//             //"s{" | "s}" => Selection::Surrounders('{', '}'),
//             _ if selection == c => Selection::Lines(amount),
//             _ => return None,
//         })
//     }
// }
