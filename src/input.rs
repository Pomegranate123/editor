use crate::{
    action::Action,
    buffer::EditMode,
    utils::{Movement, Selection},
};
use crossterm::event::{KeyCode, KeyEvent};

pub struct InputHandler;

impl InputHandler {
    pub fn parse_insert(key: KeyEvent) -> Option<Action> {
        Some(match key.code {
            KeyCode::Esc => Action::SetMode(EditMode::Normal),
            KeyCode::Char(c) => Action::Insert(String::from(c)),
            KeyCode::Tab => Action::Insert(String::from("\t")),
            KeyCode::Enter => Action::Insert(String::from("\n")),
            KeyCode::Up => Action::Move(Movement::Up(1)),
            KeyCode::Down => Action::Move(Movement::Down(1)),
            KeyCode::Left => Action::Move(Movement::Left(1)),
            KeyCode::Right => Action::Move(Movement::Right(1)),
            KeyCode::Home => Action::Move(Movement::Home),
            KeyCode::End => Action::Move(Movement::End),
            KeyCode::PageUp => Action::Move(Movement::Up(25)),
            KeyCode::PageDown => Action::Move(Movement::Down(25)),
            KeyCode::Backspace => Action::Delete(Selection::UpTo(Movement::Left(1))),
            KeyCode::Delete => Action::Delete(Selection::UpTo(Movement::Right(1))),
            _ => return None,
        })
    }

    pub fn parse_normal(key: KeyEvent) -> Option<Action> {
        Some(match key.code {
            KeyCode::Up => Action::Move(Movement::Up(1)),
            KeyCode::Down => Action::Move(Movement::Down(1)),
            KeyCode::Left => Action::Move(Movement::Left(1)),
            KeyCode::Right => Action::Move(Movement::Right(1)),
            KeyCode::Home => Action::Move(Movement::Home),
            KeyCode::End => Action::Move(Movement::End),
            KeyCode::PageUp => Action::Move(Movement::Up(25)),
            KeyCode::PageDown => Action::Move(Movement::Down(25)),
            KeyCode::Char('i') => Action::SetMode(EditMode::Insert),
            KeyCode::Char('d') => Action::Delete(Selection::Lines(1)),
            KeyCode::Char('u') => Action::Undo,
            KeyCode::Char('U') => Action::Redo,
            KeyCode::Delete => Action::Delete(Selection::UpTo(Movement::Right(1))),
            _ => return None,
        })
    }
}
