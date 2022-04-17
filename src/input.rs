use crate::{
    action::{BufferAction, RenderAction, Command},
    buffer::EditMode,
    utils::{Movement, Selection},
};
use crossterm::event::{KeyCode, KeyEvent};

pub struct InputHandler;

impl InputHandler {
    pub fn parse_insert(key: KeyEvent) -> Option<Command> {
        Some(Command::new(match key.code {
            KeyCode::Esc => BufferAction::SetMode(EditMode::Normal),
            KeyCode::Char(c) => BufferAction::Insert(String::from(c)),
            KeyCode::Tab => BufferAction::Insert(String::from("\t")),
            KeyCode::Enter => BufferAction::Insert(String::from("\n")),
            KeyCode::Up => BufferAction::Move(Movement::Up(1)),
            KeyCode::Down => BufferAction::Move(Movement::Down(1)),
            KeyCode::Left => BufferAction::Move(Movement::Left(1)),
            KeyCode::Right => BufferAction::Move(Movement::Right(1)),
            KeyCode::Home => BufferAction::Move(Movement::Home),
            KeyCode::End => BufferAction::Move(Movement::End),
            KeyCode::PageUp => BufferAction::Move(Movement::Up(25)),
            KeyCode::PageDown => BufferAction::Move(Movement::Down(25)),
            KeyCode::Backspace => BufferAction::Delete(Selection::UpTo(Movement::Left(1))),
            KeyCode::Delete => BufferAction::Delete(Selection::UpTo(Movement::Right(1))),
            _ => return None,
        }, match key.code {
            _ => RenderAction::DrawAll,
            // KeyCode::Esc => RenderAction::UpdateCursor,
            // KeyCode::Char(_) | KeyCode::Tab | KeyCode::Enter | KeyCode::Backspace | KeyCode::Delete => RenderAction::DrawFromCursor,
            // KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown => RenderAction::UpdateCursor,
            // _ => RenderAction::Nothing
        }))
    }

    pub fn parse_normal(key: KeyEvent) -> Option<Command> {
        Some(Command::new(match key.code {
            KeyCode::Up => BufferAction::Move(Movement::Up(1)),
            KeyCode::Down => BufferAction::Move(Movement::Down(1)),
            KeyCode::Left => BufferAction::Move(Movement::Left(1)),
            KeyCode::Right => BufferAction::Move(Movement::Right(1)),
            KeyCode::Home => BufferAction::Move(Movement::Home),
            KeyCode::End => BufferAction::Move(Movement::End),
            KeyCode::PageUp => BufferAction::Move(Movement::Up(25)),
            KeyCode::PageDown => BufferAction::Move(Movement::Down(25)),
            KeyCode::Char('i') => BufferAction::SetMode(EditMode::Insert),
            KeyCode::Char('d') => BufferAction::Delete(Selection::Lines(1)),
            KeyCode::Char('u') => BufferAction::Undo,
            KeyCode::Char('U') => BufferAction::Redo,
            KeyCode::Delete => BufferAction::Delete(Selection::UpTo(Movement::Right(1))),
            _ => return None,
        }, match key.code {
            _ => RenderAction::DrawAll
            // KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown => RenderAction::UpdateCursor,
            // KeyCode::Char('i') => RenderAction::UpdateCursor,
            // KeyCode::Char('d') => RenderAction::DrawFromCursor,
            // KeyCode::Char('u') | KeyCode::Char('U') => RenderAction::DrawAll,
            // _ => RenderAction::Nothing
        }))
    }
}
