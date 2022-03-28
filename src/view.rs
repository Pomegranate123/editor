use crate::utils::Pos;

const MARGIN_LEFT: usize = 5;
const MARGIN_RIGHT: usize = 5;
const MARGIN_TOP: usize = 3;
const MARGIN_BOTTOM: usize = 3;

pub struct Rect {
    pub width: usize,
    pub height: usize,
    pub offset: Pos,
    pub scroll: Pos,
}

impl Rect {
    pub fn new(width: usize, height: usize, x: usize, y: usize) -> Self {
        Self { width, height, offset: Pos::new(x, y), scroll: Pos::default() }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn scroll_to_cursor(&mut self, cursor: Pos) {
        // Scroll left if cursor is on left side of bounds
        if cursor.x.saturating_sub(self.scroll.x) < MARGIN_LEFT {
            self.scroll.x = cursor.x.saturating_sub(MARGIN_LEFT);
        }
        // Scroll right if cursor is on right side of bounds
        if cursor.x.saturating_sub(self.scroll.x) + MARGIN_RIGHT + 1 > self.width
        {
            self.scroll.x =
                (cursor.x + MARGIN_RIGHT + 1).saturating_sub(self.width);
        }
        // Scroll up if cursor is above bounds
        if cursor.y.saturating_sub(self.scroll.y) < MARGIN_TOP {
            let scroll_y = cursor.y.saturating_sub(MARGIN_TOP);
            self.scroll.y = scroll_y;
        }
        // Scroll down if cursor is below bounds (+2 is for status bar height)
        if cursor.y.saturating_sub(self.scroll.y) + MARGIN_BOTTOM + 2 > self.height {
            let scroll_y = (cursor.y + MARGIN_BOTTOM + 2).saturating_sub(self.height);
            self.scroll.y = scroll_y;
        }
    }

    pub fn left(&self) -> usize {
        self.scroll.x
    }

    pub fn right(&self) -> usize {
        self.scroll.x + self.width
    }

    pub fn top(&self) -> usize {
        self.scroll.y
    }

    pub fn bottom(&self) -> usize {
        self.scroll.y + self.height
    }

    pub fn terminal_x(&self, x: usize) -> usize {
        x + self.offset.x - self.scroll.x
    }

    pub fn terminal_y(&self, y: usize) -> usize {
        y + self.offset.y - self.scroll.y
    }

    pub fn terminal_pos(&self, pos: &Pos) -> Pos {
        Pos::new(self.terminal_x(pos.x), self.terminal_y(pos.y))
    }
}
