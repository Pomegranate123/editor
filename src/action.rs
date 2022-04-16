use crate::{
    buffer::{Buffer, EditMode},
    utils::{BufCharIdx, BufCol, Movement, Selection},
};

pub enum Action {
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

impl Action {
    pub fn inverse(&self, buf: &Buffer) -> Action {
        match self {
            Action::Undo => Action::Nothing,
            Action::Redo => Action::Nothing,
            Action::MoveTo(_, _) | Action::Move(_) => Action::MoveTo(buf.idx, buf.saved_col),
            Action::Delete(selection) => {
                let bounds = selection.bounds(&buf);
                Action::InsertAt(bounds.start, buf.slice(bounds).to_string())
            }
            Action::InsertAt(idx, text) => {
                Action::Delete(Selection::Bounds(*idx, *idx + text.chars().count().into()))
            }
            Action::Insert(text) => Action::Delete(Selection::Bounds(
                buf.idx,
                buf.idx + text.chars().count().into(),
            )),
            Action::Yank(_) => Action::Nothing,
            Action::SetMode(_) => Action::SetMode(buf.mode),
            Action::Nothing => Action::Nothing,
        }
    }

    pub fn apply(self, buf: &mut Buffer) -> Result<(), &'static str> {
        match self {
            Action::Undo => {
                match buf.undo.pop() {
                    Some(action) => {
                        buf.redo.push(action.inverse(buf));
                        action.apply(buf)
                    }
                    None => Err("Nothing to undo"),
                }
            }
            Action::Redo => {
                match buf.redo.pop() {
                    Some(action) => {
                        buf.undo.push(action.inverse(buf));
                        action.apply(buf)
                    }
                    None => Err("Nothing to redo"),
                }
            }
            Action::MoveTo(idx, saved_col) => {
                buf.idx = idx;
                buf.saved_col = saved_col;
                Ok(())
            }
            Action::Move(movement) => {
                buf.idx = movement.dest(buf);
                if movement.is_horizontal() {
                    buf.save_col();
                }
                Ok(())
            }
            Action::Delete(selection) => {
                buf.remove(selection.bounds(buf));
                Ok(())
            }
            Action::InsertAt(idx, text) => {
                buf.insert(idx, &text);
                buf.idx = buf.idx + text.chars().count().into();
                Ok(())
            }
            Action::Insert(text) => {
                buf.insert(buf.idx, &text);
                buf.idx = buf.idx + text.chars().count().into();
                Ok(())
            }
            Action::Yank(selection) => {
                cli_clipboard::set_contents(buf.slice(selection.bounds(buf)).to_string())
                    .expect("Error setting system clipboard");
                Ok(())
            }
            Action::SetMode(mode) => {
                buf.mode = mode;
                Ok(())
            }
            Action::Nothing => Ok(())
        }
    }
}
