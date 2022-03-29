use crate::buffer::{Buffer, EditMode};
use crate::utils::{BufCharIdx, BufCol, Movement, Selection};
use undo::Action;

pub struct Move {
    pub movement: Movement,
    origin: BufCharIdx,
    saved_col: Option<BufCol>,
}

impl Move {
    pub fn new(movement: Movement) -> Self {
        Self {
            movement,
            origin: 0.into(),
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
        buf.idx = dest;
        if let Movement::Left(_) | Movement::Right(_) | Movement::Home | Movement::End =
            self.movement
        {
            self.saved_col = Some(buf.saved_col);
            buf.saved_col = buf.col();
        }
        Ok(())
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.idx = self.origin;
        if let Some(col) = self.saved_col {
            buf.saved_col = col
        };
        Ok(())
    }
}

#[derive(Default)]
pub struct Delete {
    selection: Selection,
    origin: BufCharIdx,
    left_bound: BufCharIdx,
    removed: String,
}

impl Delete {
    pub fn new(selection: Selection) -> Self {
        Self {
            selection,
            ..Default::default()
        }
    }
}

impl Action for Delete {
    type Target = Buffer;
    type Output = ();
    type Error = crossterm::ErrorKind;

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        let bounds = self.selection.bounds(buf);
        self.left_bound = bounds.start;
        self.removed = buf.slice(bounds.clone()).to_string();
        buf.remove(bounds);
        Ok(())
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.insert(self.left_bound, &self.removed);
        buf.idx = self.origin;
        Ok(())
    }
}

#[derive(Default)]
pub struct Insert {
    text: String,
    origin: BufCharIdx,
}

impl Insert {
    pub fn new(text: String) -> Self {
        Self {
            text,
            ..Default::default()
        }
    }

    pub fn from_clipboard() -> Self {
        Self {
            text: cli_clipboard::get_contents().expect("Error getting system clipboard"),
            ..Default::default()
        }
    }
}

impl Action for Insert {
    type Target = Buffer;
    type Output = ();
    type Error = ();

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        self.origin = buf.idx;
        buf.insert(buf.idx, &self.text);
        Ok(())
    }

    fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        buf.remove(self.origin..(self.origin + self.text.len().into()));
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
    type Error = ();

    fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        let bounds = self.selection.bounds(buf);
        let text = buf.slice(bounds);
        cli_clipboard::set_contents(text.to_string()).expect("Error setting system clipboard");
        Ok(())
    }

    fn undo(&mut self, _buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
        Ok(())
    }
}

pub struct SetMode {
    mode: EditMode,
    prev_mode: EditMode,
}

impl SetMode {
    pub fn new(mode: EditMode) -> Self {
        Self {
            mode,
            prev_mode: EditMode::default(),
        }
    }
}

impl Action for SetMode {
    type Target = Buffer;
    type Output = ();
    type Error = ();

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
//     type Error = ();
//
//     fn apply(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
//         Ok(())
//     }
//
//     fn undo(&mut self, buf: &mut Self::Target) -> Result<Self::Output, Self::Error> {
//         Ok(())
//     }
// }
