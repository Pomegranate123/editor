use crate::utils::{BufCol, BufPos, BufRow, TermCol, TermPos, TermRow};

const MARGIN_LEFT: usize = 5;
const MARGIN_RIGHT: usize = 5;
const MARGIN_TOP: usize = 3;
const MARGIN_BOTTOM: usize = 3;

pub struct Rect {
    pub width: TermCol,
    pub height: TermRow,
    pub offset: TermPos,
    pub scroll: BufPos,
}

impl Rect {
    pub fn new(width: TermCol, height: TermRow, x: TermCol, y: TermRow) -> Self {
        Self {
            width,
            height,
            offset: TermPos::new(x, y),
            scroll: BufPos::default(),
        }
    }

    pub fn resize(&mut self, width: TermCol, height: TermRow) {
        self.width = width;
        self.height = height;
    }

    pub fn scroll_to_cursor(&mut self, cursor: BufPos) {
        // Scroll left if cursor is on left side of bounds
        if cursor.x.saturating_sub(*self.scroll.x) < MARGIN_LEFT {
            self.scroll.x = cursor.x.saturating_sub(MARGIN_LEFT).into();
        }
        // Scroll right if cursor is on right side of bounds
        if cursor.x.saturating_sub(*self.scroll.x) + MARGIN_RIGHT > *self.width as usize {
            self.scroll.x = (*cursor.x + MARGIN_RIGHT)
                .saturating_sub(*self.width as usize)
                .into();
        }
        // Scroll up if cursor is above bounds
        if cursor.y.saturating_sub(*self.scroll.y) < MARGIN_TOP {
            self.scroll.y = cursor.y.saturating_sub(MARGIN_TOP).into();
        }
        // Scroll down if cursor is below bounds (+2 is for status bar height)
        if cursor.y.saturating_sub(*self.scroll.y) + MARGIN_BOTTOM > *self.height as usize {
            self.scroll.y = (*cursor.y + MARGIN_BOTTOM)
                .saturating_sub(*self.height as usize)
                .into();
        }
    }

    #[allow(unused)]
    /// Returns the leftmost column of the currently visible area
    pub fn left(&self) -> BufCol {
        self.scroll.x
    }

    #[allow(unused)]
    /// Returns the rightmost column of the currently visible area
    pub fn right(&self) -> BufCol {
        self.scroll.x + self.width.as_bufcol()
    }

    /// Returns the top row of the currently visible area
    pub fn top(&self) -> BufRow {
        self.scroll.y
    }

    /// Returns the bottom row of the currently visible area
    pub fn bottom(&self) -> BufRow {
        self.scroll.y + self.height.as_bufrow()
    }

    pub fn terminal_x(&self, x: BufCol) -> TermCol {
        (x - self.scroll.x).as_termcol() + self.offset.x
    }

    pub fn terminal_y(&self, y: BufRow) -> TermRow {
        (y - self.scroll.y).as_termrow() + self.offset.y
    }

    pub fn terminal_pos(&self, pos: BufPos) -> TermPos {
        TermPos::new(self.terminal_x(pos.x), self.terminal_y(pos.y))
    }
}
