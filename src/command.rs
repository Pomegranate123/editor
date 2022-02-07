use regex::Regex;
use lazy_static::lazy_static;
use crate::buffer::EditMode;

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

pub enum Selection {
    Lines(usize),
    UpTo(Movement),
    Between { first: char, last: char, inclusive: bool },
    Surrounders(char, char),
    Word { inclusive: bool },
    Paragraph { inclusive: bool },
}

pub enum Place {
    Before,
    After
}

pub enum Command {
    Move(Movement),
    Delete(Selection),
    Yank(Selection),
    Change(Selection),
    Paste(Place),
    CreateNewLine(Place),
    SetMode(EditMode),
}

impl Command {
    pub fn parse(command: &str) -> Vec<Self> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r#"^([AaCDPphIiJjKklOoYXx0$])$|^(?:([dcy])([1-9][0-9]*)?([dcyw$0]|[ais][wp\"\'\{\}\[\]\(\)]))$"#).unwrap();
        }
        let captures = RE.captures(command).unwrap();
        let amount = captures.get(3).map(|cap| cap.as_str().parse::<usize>().expect("Unable to parse count in command regex")).unwrap_or(1);
        match captures.get(1) { // Matches if command has length 1
            Some(cap) => Command::parse_simple(cap.as_str()),
            None => match captures.get(4) { // Matches the movement specifier at the end of a command
                Some(cap) => {
                    let c = captures.get(2).unwrap().as_str(); // Every command should have a main command unless it has length 1
                    let selection = Command::parse_selection(c, cap.as_str(), amount);
                    match c {
                        "d" => vec![Self::Delete(selection)],
                        "c" => vec![Self::Change(selection)],
                        "y" => vec![Self::Yank(selection)],
                        _ => panic!("Unable to parse main command '{}' in regex: '{}'", c, command),
                    }
                }
                None => panic!("Unable to parse movement specifier in regex: '{}'", command)
            }

        }
        
    }

    fn parse_simple(command: &str) -> Vec<Self> {
        match command {
            "A" => vec![Self::SetMode(EditMode::Insert), Self::Move(Movement::End)],
            "a" => vec![Self::SetMode(EditMode::Insert), Self::Move(Movement::Right(1))],
            "b" => vec![Self::Move(Movement::PrevWord(1))],
            "C" => vec![Self::Change(Selection::UpTo(Movement::End))],
            "D" => vec![Self::Delete(Selection::UpTo(Movement::End))],
            "P" => vec![Self::Paste(Place::Before)],
            "p" => vec![Self::Paste(Place::After)],
            "h" => vec![Self::Move(Movement::Left(1))],
            "I" => vec![Self::SetMode(EditMode::Insert), Self::Move(Movement::FirstChar)],
            "i" => vec![Self::SetMode(EditMode::Insert)],
            "J" => vec![Self::Move(Movement::Bottom)],
            "j" => vec![Self::Move(Movement::Down(1))],
            "K" => vec![Self::Move(Movement::Top)],
            "k" => vec![Self::Move(Movement::Up(1))],
            "l" => vec![Self::Move(Movement::Right(1))],
            "O" => vec![Self::SetMode(EditMode::Insert), Self::CreateNewLine(Place::Before)],
            "o" => vec![Self::SetMode(EditMode::Insert), Self::CreateNewLine(Place::After)],
            "w" => vec![Self::Move(Movement::NextWord(1))],
            "Y" => vec![Self::Yank(Selection::Lines(1))],
            "X" => vec![Self::Delete(Selection::UpTo(Movement::Left(1)))],
            "x" => vec![Self::Delete(Selection::UpTo(Movement::Right(1)))],
            "0" => vec![Self::Move(Movement::Home)],
            "$" => vec![Self::Move(Movement::End)],
            _ => panic!("Unable to parse simple command '{}' in regex", command),
        }
    }

    fn parse_selection(c: &str, selection: &str, amount: usize) -> Selection {
        match selection {
            "w" => Selection::UpTo(Movement::NextWord(1)),
            "iw" => Selection::Word { inclusive: false },
            "aw" => Selection::Word { inclusive: true },
            "ip" => Selection::Paragraph { inclusive: false },
            "ap" => Selection::Paragraph { inclusive: true },
            "$" => Selection::UpTo(Movement::End),
            "0" => Selection::UpTo(Movement::Home),
            "i\"" => Selection::Between { first: '\"', last: '\"', inclusive: false },
            "i\'" => Selection::Between { first: '\'', last: '\'', inclusive: false },
            "i[" | "i]" => Selection::Between { first: '[', last: ']', inclusive: false },
            "i(" | "i)" => Selection::Between { first: '(', last: ')', inclusive: false },
            "i{" | "i}" => Selection::Between { first: '{', last: '}', inclusive: false },
            "a\"" => Selection::Between { first: '\"', last: '\"', inclusive: true },
            "a\'" => Selection::Between { first: '\'', last: '\'', inclusive: true },
            "a[" | "a]" => Selection::Between { first: '[', last: ']', inclusive: true },
            "a(" | "a)" => Selection::Between { first: '(', last: ')', inclusive: true },
            "a{" | "a}" => Selection::Between { first: '{', last: '}', inclusive: true },
            "s\"" => Selection::Surrounders('\"', '\"'),
            "s\'" => Selection::Surrounders('\'', '\''),
            "s[" | "s]" => Selection::Surrounders('[', ']'),
            "s(" | "s)" => Selection::Surrounders('(', ')'),
            "s{" | "s}" => Selection::Surrounders('{', '}'),
            _ if selection == c => Selection::Lines(amount),
            _ => panic!("Unable to parse selection '{}' in regex", selection),
        }
    }
}
