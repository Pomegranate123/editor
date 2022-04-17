use crate::{
    window::Window,
    buffer::{Buffer, EditMode},
    utils::{BufCharIdx, BufCol, Movement, Selection},
};

pub struct Command {
    pub buffer_action: BufferAction,
    pub render_action: RenderAction,
}

impl Command {
    pub fn new(buffer_action: BufferAction, render_action: RenderAction) -> Self {
        Self { buffer_action, render_action }
    }
}

pub trait Action {
    type Target;
    type Error;

    fn apply(self, target: &mut Self::Target) -> Result<(), Self::Error>;
}

pub trait Undoable : Action {
    fn inverse(&self, target: &Self::Target) -> Self;
}

#[allow(unused)]
pub enum BufferAction {
    Undo,
    Redo,
    MoveTo(BufCharIdx, BufCol),
    Move(Movement),
    Delete(Selection),
    InsertAt(BufCharIdx, String),
    Insert(String),
    Yank(Selection),
    SetMode(EditMode),
    Nothing,
}

impl Action for BufferAction {
    type Target = Buffer;
    type Error = &'static str;

    fn apply(self, buf: &mut Buffer) -> Result<(), &'static str> {
        match self {
            BufferAction::Undo => {
                match buf.undo.pop() {
                    Some(action) => {
                        buf.redo.push(action.inverse(buf));
                        action.apply(buf)
                    }
                    None => Err("Nothing to undo"),
                }
            }
            BufferAction::Redo => {
                match buf.redo.pop() {
                    Some(action) => {
                        buf.undo.push(action.inverse(buf));
                        action.apply(buf)
                    }
                    None => Err("Nothing to redo"),
                }
            }
            BufferAction::MoveTo(idx, saved_col) => {
                buf.idx = idx;
                buf.saved_col = saved_col;
                Ok(())
            }
            BufferAction::Move(movement) => {
                buf.idx = movement.dest(buf);
                if movement.is_horizontal() {
                    buf.save_col();
                }
                Ok(())
            }
            BufferAction::Delete(selection) => {
                buf.remove(selection.bounds(buf));
                Ok(())
            }
            BufferAction::InsertAt(idx, text) => {
                buf.insert(idx, &text);
                buf.idx = buf.idx + text.chars().count().into();
                Ok(())
            }
            BufferAction::Insert(text) => {
                buf.insert(buf.idx, &text);
                buf.idx = buf.idx + text.chars().count().into();
                Ok(())
            }
            BufferAction::Yank(selection) => {
                cli_clipboard::set_contents(buf.slice(selection.bounds(buf)).to_string())
                    .expect("Error setting system clipboard");
                Ok(())
            }
            BufferAction::SetMode(mode) => {
                buf.mode = mode;
                Ok(())
            }
            BufferAction::Nothing => Ok(())
        }
    }
}

impl Undoable for BufferAction {
    fn inverse(&self, buf: &Self::Target) -> Self {
        match self {
            BufferAction::Undo => BufferAction::Nothing,
            BufferAction::Redo => BufferAction::Nothing,
            BufferAction::MoveTo(_, _) | BufferAction::Move(_) => BufferAction::MoveTo(buf.idx, buf.saved_col),
            BufferAction::Delete(selection) => {
                let bounds = selection.bounds(&buf);
                BufferAction::InsertAt(bounds.start, buf.slice(bounds).to_string())
            }
            BufferAction::InsertAt(idx, text) => {
                BufferAction::Delete(Selection::Bounds(*idx, *idx + text.chars().count().into()))
            }
            BufferAction::Insert(text) => BufferAction::Delete(Selection::Bounds(
                buf.idx,
                buf.idx + text.chars().count().into(),
            )),
            BufferAction::Yank(_) => BufferAction::Nothing,
            BufferAction::SetMode(_) => BufferAction::SetMode(buf.mode),
            BufferAction::Nothing => BufferAction::Nothing,
        }
    }
}

pub enum RenderAction {
    DrawAll,
    DrawFromCursor,
    UpdateCursor,
    Nothing,
}

impl Action for RenderAction {
    type Target = Window;
    type Error = crossterm::ErrorKind;

    fn apply(self, renderer: &mut Self::Target) -> Result<(), Self::Error> {
        match self {
            RenderAction::DrawAll => {
                renderer.draw_all()?;
                renderer.update_cursor()
            }
            RenderAction::DrawFromCursor => renderer.draw(renderer.buf.row()),
            RenderAction::UpdateCursor => renderer.update_cursor(),
            _ => Ok(())
        }
    }
}
