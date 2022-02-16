use crate::buffer::EditMode;
use lazy_static::lazy_static;
use regex::Regex;

#[derive(Clone, Copy)]
pub enum Movement {
    Up(usize),
    Down(usize),
    Left(usize),
    Right(usize),
    Home,
    End,
    Top,
    Bottom,
    FirstChar,
    NextWord(usize),
    PrevWord(usize),
}

#[derive(Clone, Copy)]
pub enum Selection {
    Lines(usize),
    UpTo(Movement),
    Between {
        first: char,
        last: char,
        inclusive: bool,
    },
    //    Surrounders(char, char),
    Word {
        inclusive: bool,
    },
    Paragraph {
        inclusive: bool,
    },
}

pub enum Command {
    Undo,
    Redo,
    Move(Movement),
    Delete(Selection),
    Yank(Selection),
    Paste,
    CreateNewLine,
    SetMode(EditMode),
}

impl Command {
    pub fn parse(command: &str) -> Option<Vec<Self>> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r#"^([AaCDPphIiJjKklOoUuYXx0$:])$|^(?:([dcy])([1-9][0-9]*)?([dcyw$0]|[ais][wp"'\{\}\[\]\(\)]))$"#).unwrap();
        }
        let captures = match RE.captures(command) {
            Some(cap) => cap,
            None => return None,
        };
        let amount = captures
            .get(3)
            .map(|cap| {
                cap.as_str()
                    .parse::<usize>()
                    .expect("Unable to parse count in command regex")
            })
            .unwrap_or(1);
        match captures.get(1) {
            // Matches if command has length 1
            Some(cap) => Command::parse_simple(cap.as_str()),
            None => captures.get(4).and_then(|cap| {
                let c = captures.get(2).unwrap().as_str(); // Every command should have a main command unless it has length 1
                Command::parse_selection(c, cap.as_str(), amount).and_then(|selection| {
                    Some(match c {
                        "d" => vec![Self::Delete(selection)],
                        "c" => vec![Self::Delete(selection), Self::SetMode(EditMode::Insert)],
                        "y" => vec![Self::Yank(selection)],
                        _ => return None,
                    })
                })
            }),
        }
    }

    fn parse_simple(command: &str) -> Option<Vec<Self>> {
        Some(match command {
            "A" => vec![Self::SetMode(EditMode::Insert), Self::Move(Movement::End)],
            "a" => vec![
                Self::SetMode(EditMode::Insert),
                Self::Move(Movement::Right(1)),
            ],
            "b" => vec![Self::Move(Movement::PrevWord(1))],
            "C" => vec![
                Self::Delete(Selection::UpTo(Movement::End)),
                Self::SetMode(EditMode::Insert),
            ],
            "D" => vec![Self::Delete(Selection::UpTo(Movement::End))],
            "P" => vec![
                Self::Move(Movement::Up(1)),
                Self::Move(Movement::End),
                Self::Paste,
            ],
            "p" => vec![Self::Paste],
            "h" => vec![Self::Move(Movement::Left(1))],
            "I" => vec![
                Self::SetMode(EditMode::Insert),
                Self::Move(Movement::FirstChar),
            ],
            "i" => vec![Self::SetMode(EditMode::Insert)],
            "J" => vec![Self::Move(Movement::Bottom)],
            "j" => vec![Self::Move(Movement::Down(1))],
            "K" => vec![Self::Move(Movement::Top)],
            "k" => vec![Self::Move(Movement::Up(1))],
            "l" => vec![Self::Move(Movement::Right(1))],
            "O" => vec![
                Self::SetMode(EditMode::Insert),
                Self::Move(Movement::Up(1)),
                Self::Move(Movement::End),
                Self::CreateNewLine,
                Self::Move(Movement::Down(1)),
            ],
            "o" => vec![
                Self::SetMode(EditMode::Insert),
                Self::Move(Movement::End),
                Self::CreateNewLine,
                Self::Move(Movement::Down(1)),
            ],
            "w" => vec![Self::Move(Movement::NextWord(1))],
            "U" => vec![Self::Redo],
            "u" => vec![Self::Undo],
            "Y" => vec![Self::Yank(Selection::Lines(1))],
            "X" => vec![Self::Delete(Selection::UpTo(Movement::Left(1)))],
            "x" => vec![Self::Delete(Selection::UpTo(Movement::Right(1)))],
            "0" => vec![Self::Move(Movement::Home)],
            "$" => vec![Self::Move(Movement::End)],
            ":" => vec![Self::SetMode(EditMode::Command)],
            _ => return None,
        })
    }

    fn parse_selection(c: &str, selection: &str, amount: usize) -> Option<Selection> {
        Some(match selection {
            "w" => Selection::UpTo(Movement::NextWord(1)),
            "iw" => Selection::Word { inclusive: false },
            "aw" => Selection::Word { inclusive: true },
            "ip" => Selection::Paragraph { inclusive: false },
            "ap" => Selection::Paragraph { inclusive: true },
            "$" => Selection::UpTo(Movement::End),
            "0" => Selection::UpTo(Movement::Home),
            "i\"" => Selection::Between {
                first: '\"',
                last: '\"',
                inclusive: false,
            },
            "i\'" => Selection::Between {
                first: '\'',
                last: '\'',
                inclusive: false,
            },
            "i[" | "i]" => Selection::Between {
                first: '[',
                last: ']',
                inclusive: false,
            },
            "i(" | "i)" => Selection::Between {
                first: '(',
                last: ')',
                inclusive: false,
            },
            "i{" | "i}" => Selection::Between {
                first: '{',
                last: '}',
                inclusive: false,
            },
            "a\"" => Selection::Between {
                first: '\"',
                last: '\"',
                inclusive: true,
            },
            "a\'" => Selection::Between {
                first: '\'',
                last: '\'',
                inclusive: true,
            },
            "a[" | "a]" => Selection::Between {
                first: '[',
                last: ']',
                inclusive: true,
            },
            "a(" | "a)" => Selection::Between {
                first: '(',
                last: ')',
                inclusive: true,
            },
            "a{" | "a}" => Selection::Between {
                first: '{',
                last: '}',
                inclusive: true,
            },
            //"s\"" => Selection::Surrounders('\"', '\"'),
            //"s\'" => Selection::Surrounders('\'', '\''),
            //"s[" | "s]" => Selection::Surrounders('[', ']'),
            //"s(" | "s)" => Selection::Surrounders('(', ')'),
            //"s{" | "s}" => Selection::Surrounders('{', '}'),
            _ if selection == c => Selection::Lines(amount),
            _ => return None,
        })
    }
}
