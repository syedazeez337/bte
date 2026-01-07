//! Terminal Screen Grid Model
//!
//! This module provides a 2D grid model for terminal memory,
//! with scrollback buffer, cursor tracking, and dirty line management.

#![allow(dead_code)]

use crate::ansi::{AnsiEvent, AnsiParser, CsiSequence, EscSequence};
use std::collections::{HashSet, VecDeque};
use std::hash::{Hash, Hasher};

/// Simple FNV-1a hasher for deterministic hashing
#[derive(Clone)]
pub struct FnvHasher {
    state: u64,
}

impl FnvHasher {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    pub fn new() -> Self {
        Self {
            state: Self::FNV_OFFSET,
        }
    }

    pub fn finish(&self) -> u64 {
        self.state
    }
}

impl Default for FnvHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for FnvHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= byte as u64;
            self.state = self.state.wrapping_mul(Self::FNV_PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

/// A single cell in the terminal grid
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cell {
    /// The character in this cell
    pub ch: char,
    /// Cell attributes (foreground color, background color, flags)
    pub attrs: CellAttrs,
}

impl Cell {
    /// Create a new empty cell
    pub fn new() -> Self {
        Self {
            ch: ' ',
            attrs: CellAttrs::default(),
        }
    }

    /// Create a cell with a character
    pub fn with_char(ch: char) -> Self {
        Self {
            ch,
            attrs: CellAttrs::default(),
        }
    }

    /// Check if this cell is empty (space with default attributes)
    pub fn is_empty(&self) -> bool {
        self.ch == ' ' && self.attrs == CellAttrs::default()
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::new()
    }
}

/// Cell attributes (colors and style flags)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct CellAttrs {
    /// Foreground color (0-255, or -1 for default)
    pub fg: i16,
    /// Background color (0-255, or -1 for default)
    pub bg: i16,
    /// Style flags
    pub flags: AttrFlags,
}

impl CellAttrs {
    /// Create default attributes
    pub fn new() -> Self {
        Self {
            fg: -1,
            bg: -1,
            flags: AttrFlags::empty(),
        }
    }
}

bitflags::bitflags! {
    /// Style flags for cell attributes
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
    pub struct AttrFlags: u16 {
        /// Bold text
        const BOLD = 1 << 0;
        /// Dim/faint text
        const DIM = 1 << 1;
        /// Italic text
        const ITALIC = 1 << 2;
        /// Underlined text
        const UNDERLINE = 1 << 3;
        /// Blinking text
        const BLINK = 1 << 4;
        /// Inverse video (swap fg/bg)
        const INVERSE = 1 << 5;
        /// Hidden/invisible text
        const HIDDEN = 1 << 6;
        /// Strikethrough text
        const STRIKETHROUGH = 1 << 7;
    }
}

/// Cursor position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Row (0-indexed from top)
    pub row: usize,
    /// Column (0-indexed from left)
    pub col: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { row: 0, col: 0 }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// A single row in the grid
#[derive(Debug, Clone)]
pub struct Row {
    /// Cells in this row
    cells: Vec<Cell>,
}

impl Row {
    /// Create a new row with the given width
    pub fn new(width: usize) -> Self {
        Self {
            cells: vec![Cell::new(); width],
        }
    }

    /// Get the width of this row
    pub fn width(&self) -> usize {
        self.cells.len()
    }

    /// Resize the row
    pub fn resize(&mut self, width: usize) {
        self.cells.resize(width, Cell::new());
    }

    /// Get a cell at a column
    pub fn get(&self, col: usize) -> Option<&Cell> {
        self.cells.get(col)
    }

    /// Get a mutable cell at a column
    pub fn get_mut(&mut self, col: usize) -> Option<&mut Cell> {
        self.cells.get_mut(col)
    }

    /// Clear the row
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::new();
        }
    }

    /// Clear from a column to the end
    pub fn clear_from(&mut self, col: usize) {
        for cell in self.cells.iter_mut().skip(col) {
            *cell = Cell::new();
        }
    }

    /// Clear from the beginning to a column (inclusive)
    pub fn clear_to(&mut self, col: usize) {
        for cell in self.cells.iter_mut().take(col + 1) {
            *cell = Cell::new();
        }
    }

    /// Check if the row is empty
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|c| c.is_empty())
    }
}

/// Terminal screen with grid and scrollback
pub struct Screen {
    /// Current visible grid
    grid: Vec<Row>,
    /// Scrollback buffer (VecDeque for O(1) push/pop at both ends)
    scrollback: VecDeque<Row>,
    /// Maximum scrollback lines
    max_scrollback: usize,
    /// Screen dimensions
    cols: usize,
    rows: usize,
    /// Cursor position
    cursor: Cursor,
    /// Saved cursor position
    saved_cursor: Option<Cursor>,
    /// Current cell attributes for new characters
    current_attrs: CellAttrs,
    /// Whether we're in alternate screen mode
    alternate_screen: bool,
    /// Saved primary screen (when in alternate mode)
    saved_primary: Option<(Vec<Row>, VecDeque<Row>, Cursor)>,
    /// Scroll region (top, bottom)
    scroll_region: (usize, usize),
    /// ANSI parser
    parser: AnsiParser,
    /// Lines that have been modified since last render
    dirty_lines: HashSet<usize>,
    /// Whether dirty tracking is enabled
    dirty_tracking_enabled: bool,
}

impl Screen {
    /// Create a new screen with the given dimensions
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = (0..rows).map(|_| Row::new(cols)).collect();
        Self {
            grid,
            scrollback: VecDeque::new(),
            max_scrollback: 10000,
            cols,
            rows,
            cursor: Cursor::new(),
            saved_cursor: None,
            current_attrs: CellAttrs::new(),
            alternate_screen: false,
            saved_primary: None,
            scroll_region: (0, rows.saturating_sub(1)),
            parser: AnsiParser::new(),
            dirty_lines: HashSet::new(),
            dirty_tracking_enabled: false,
        }
    }

    /// Get screen dimensions
    #[must_use]
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// Get cursor position
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Get a cell at a position
    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.grid.get(row)?.get(col)
    }

    /// Get scrollback length
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Set maximum scrollback lines
    pub fn set_max_scrollback(&mut self, max: usize) {
        self.max_scrollback = max;
        // Trim scrollback if needed (O(1) pop_front with VecDeque)
        while self.scrollback.len() > self.max_scrollback {
            self.scrollback.pop_front();
        }
    }

    /// Resize the screen
    pub fn resize(&mut self, cols: usize, rows: usize) {
        // Resize existing rows
        for row in &mut self.grid {
            row.resize(cols);
        }
        for row in &mut self.scrollback {
            row.resize(cols);
        }

        // Add or remove rows
        while self.grid.len() < rows {
            self.grid.push(Row::new(cols));
        }
        while self.grid.len() > rows {
            // Move removed rows to scrollback
            let removed = self.grid.remove(0);
            if !removed.is_empty() {
                self.scrollback.push_back(removed);
                if self.scrollback.len() > self.max_scrollback {
                    self.scrollback.pop_front(); // O(1) with VecDeque
                }
            }
        }

        self.cols = cols;
        self.rows = rows;

        // Update scroll region
        self.scroll_region = (0, rows.saturating_sub(1));

        // Clamp cursor
        self.clamp_cursor();
    }

    /// Clamp cursor to valid bounds
    fn clamp_cursor(&mut self) {
        self.cursor.row = self.cursor.row.min(self.rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1));
    }

    /// Process raw bytes
    pub fn process(&mut self, data: &[u8]) {
        let events = self.parser.parse(data);
        for event in events {
            self.handle_event(event);
        }
    }

    /// Handle a parsed ANSI event
    fn handle_event(&mut self, event: AnsiEvent) {
        match event {
            AnsiEvent::Print(ch) => self.print_char(ch),
            AnsiEvent::Execute(code) => self.execute(code),
            AnsiEvent::Csi(csi) => self.handle_csi(csi),
            AnsiEvent::Esc(esc) => self.handle_esc(esc),
            AnsiEvent::Osc(_) => {} // Ignore OSC for now
            AnsiEvent::Dcs(_) => {} // Ignore DCS for now
            AnsiEvent::Apc(_) => {} // Ignore APC for now
        }
    }

    /// Print a character at the cursor position
    fn print_char(&mut self, ch: char) {
        if self.cursor.col >= self.cols {
            // Wrap to next line
            self.cursor.col = 0;
            self.cursor.row += 1;
            if self.cursor.row > self.scroll_region.1 {
                self.scroll_up(1);
                self.cursor.row = self.scroll_region.1;
            }
        }

        if let Some(row) = self.grid.get_mut(self.cursor.row) {
            if let Some(cell) = row.get_mut(self.cursor.col) {
                cell.ch = ch;
                cell.attrs = self.current_attrs;
            }
        }

        // Mark current line as dirty
        self.mark_dirty(self.cursor.row);

        self.cursor.col += 1;
    }

    /// Execute a C0 control code
    fn execute(&mut self, code: u8) {
        match code {
            // BEL - Bell
            0x07 => {}
            // BS - Backspace
            0x08 => {
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                    self.mark_dirty(self.cursor.row);
                }
            }
            // HT - Horizontal Tab
            0x09 => {
                let next_tab = ((self.cursor.col / 8) + 1) * 8;
                self.cursor.col = next_tab.min(self.cols.saturating_sub(1));
                self.mark_dirty(self.cursor.row);
            }
            // LF, VT, FF - Line Feed (and variants)
            0x0a..=0x0c => {
                self.cursor.row += 1;
                if self.cursor.row > self.scroll_region.1 {
                    self.scroll_up(1);
                    self.cursor.row = self.scroll_region.1;
                }
                self.mark_dirty(self.cursor.row);
            }
            // CR - Carriage Return
            0x0d => {
                self.cursor.col = 0;
                self.mark_dirty(self.cursor.row);
            }
            _ => {}
        }
    }

    /// Handle CSI sequence
    fn handle_csi(&mut self, csi: CsiSequence) {
        match csi.final_byte {
            // CUU - Cursor Up
            b'A' => {
                let n = csi.param(0, 1) as usize;
                self.cursor.row = self.cursor.row.saturating_sub(n);
                if self.cursor.row < self.scroll_region.0 {
                    self.cursor.row = self.scroll_region.0;
                }
                self.mark_dirty(self.cursor.row);
            }
            // CUD - Cursor Down
            b'B' => {
                let n = csi.param(0, 1) as usize;
                self.cursor.row = (self.cursor.row + n).min(self.scroll_region.1);
                self.mark_dirty(self.cursor.row);
            }
            // CUF - Cursor Forward
            b'C' => {
                let n = csi.param(0, 1) as usize;
                self.cursor.col = (self.cursor.col + n).min(self.cols.saturating_sub(1));
                self.mark_dirty(self.cursor.row);
            }
            // CUB - Cursor Back
            b'D' => {
                let n = csi.param(0, 1) as usize;
                self.cursor.col = self.cursor.col.saturating_sub(n);
                self.mark_dirty(self.cursor.row);
            }
            // CNL - Cursor Next Line
            b'E' => {
                let n = csi.param(0, 1) as usize;
                self.cursor.row = (self.cursor.row + n).min(self.scroll_region.1);
                self.cursor.col = 0;
                self.mark_dirty(self.cursor.row);
            }
            // CPL - Cursor Previous Line
            b'F' => {
                let n = csi.param(0, 1) as usize;
                self.cursor.row = self.cursor.row.saturating_sub(n);
                if self.cursor.row < self.scroll_region.0 {
                    self.cursor.row = self.scroll_region.0;
                }
                self.cursor.col = 0;
                self.mark_dirty(self.cursor.row);
            }
            // CHA - Cursor Horizontal Absolute
            b'G' => {
                let col = csi.param(0, 1) as usize;
                self.cursor.col = col.saturating_sub(1).min(self.cols.saturating_sub(1));
                self.mark_dirty(self.cursor.row);
            }
            // CUP/HVP - Cursor Position
            b'H' | b'f' => {
                let row = csi.param(0, 1) as usize;
                let col = csi.param(1, 1) as usize;
                self.cursor.row = row.saturating_sub(1).min(self.rows.saturating_sub(1));
                self.cursor.col = col.saturating_sub(1).min(self.cols.saturating_sub(1));
                self.mark_dirty(self.cursor.row);
            }
            // ED - Erase in Display
            b'J' => {
                let mode = csi.param(0, 0);
                match mode {
                    0 => self.erase_below(),
                    1 => self.erase_above(),
                    2 | 3 => self.erase_all(),
                    _ => {}
                }
            }
            // EL - Erase in Line
            b'K' => {
                let mode = csi.param(0, 0);
                match mode {
                    0 => self.erase_line_right(),
                    1 => self.erase_line_left(),
                    2 => self.erase_line(),
                    _ => {}
                }
            }
            // IL - Insert Lines
            b'L' => {
                let n = csi.param(0, 1) as usize;
                self.insert_lines(n);
            }
            // DL - Delete Lines
            b'M' => {
                let n = csi.param(0, 1) as usize;
                self.delete_lines(n);
            }
            // DCH - Delete Characters
            b'P' => {
                let n = csi.param(0, 1) as usize;
                self.delete_chars(n);
            }
            // SU - Scroll Up
            b'S' => {
                let n = csi.param(0, 1) as usize;
                self.scroll_up(n);
            }
            // SD - Scroll Down
            b'T' => {
                let n = csi.param(0, 1) as usize;
                self.scroll_down(n);
            }
            // ICH - Insert Characters
            b'@' => {
                let n = csi.param(0, 1) as usize;
                self.insert_chars(n);
            }
            // SGR - Select Graphic Rendition
            b'm' => {
                self.handle_sgr(&csi.params);
            }
            // DECSTBM - Set Top and Bottom Margins
            b'r' => {
                let top = csi.param(0, 1) as usize;
                let bottom = csi.param(1, self.rows as u16) as usize;
                let top = top.saturating_sub(1).min(self.rows.saturating_sub(1));
                let bottom = bottom.saturating_sub(1).min(self.rows.saturating_sub(1));
                if top < bottom {
                    self.scroll_region = (top, bottom);
                }
                self.cursor.row = 0;
                self.cursor.col = 0;
            }
            // Private modes
            b'h' | b'l' if csi.private_marker == Some(b'?') => {
                let set = csi.final_byte == b'h';
                for &param in &csi.params {
                    self.handle_private_mode(param, set);
                }
            }
            _ => {}
        }
    }

    /// Handle SGR (Select Graphic Rendition) parameters
    fn handle_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            self.current_attrs = CellAttrs::new();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.current_attrs = CellAttrs::new(),
                1 => self.current_attrs.flags |= AttrFlags::BOLD,
                2 => self.current_attrs.flags |= AttrFlags::DIM,
                3 => self.current_attrs.flags |= AttrFlags::ITALIC,
                4 => self.current_attrs.flags |= AttrFlags::UNDERLINE,
                5 => self.current_attrs.flags |= AttrFlags::BLINK,
                7 => self.current_attrs.flags |= AttrFlags::INVERSE,
                8 => self.current_attrs.flags |= AttrFlags::HIDDEN,
                9 => self.current_attrs.flags |= AttrFlags::STRIKETHROUGH,
                21 => self.current_attrs.flags.remove(AttrFlags::BOLD),
                22 => {
                    self.current_attrs.flags.remove(AttrFlags::BOLD);
                    self.current_attrs.flags.remove(AttrFlags::DIM);
                }
                23 => self.current_attrs.flags.remove(AttrFlags::ITALIC),
                24 => self.current_attrs.flags.remove(AttrFlags::UNDERLINE),
                25 => self.current_attrs.flags.remove(AttrFlags::BLINK),
                27 => self.current_attrs.flags.remove(AttrFlags::INVERSE),
                28 => self.current_attrs.flags.remove(AttrFlags::HIDDEN),
                29 => self.current_attrs.flags.remove(AttrFlags::STRIKETHROUGH),
                // Foreground colors
                30..=37 => self.current_attrs.fg = (params[i] - 30) as i16,
                38 => {
                    // Extended foreground color
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        self.current_attrs.fg = params[i + 2] as i16;
                        i += 2;
                    }
                }
                39 => self.current_attrs.fg = -1, // Default foreground
                // Background colors
                40..=47 => self.current_attrs.bg = (params[i] - 40) as i16,
                48 => {
                    // Extended background color
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        self.current_attrs.bg = params[i + 2] as i16;
                        i += 2;
                    }
                }
                49 => self.current_attrs.bg = -1, // Default background
                // Bright foreground colors
                90..=97 => self.current_attrs.fg = (params[i] - 90 + 8) as i16,
                // Bright background colors
                100..=107 => self.current_attrs.bg = (params[i] - 100 + 8) as i16,
                _ => {}
            }
            i += 1;
        }
    }

    /// Handle private modes (DEC modes)
    fn handle_private_mode(&mut self, mode: u16, set: bool) {
        match mode {
            // DECCKM - Cursor Keys Mode
            1 => {}
            // DECCOLM - 80/132 Column Mode
            3 => {}
            // DECOM - Origin Mode
            6 => {}
            // DECAWM - Auto Wrap Mode
            7 => {}
            // DECTCEM - Text Cursor Enable Mode
            25 => {}
            // Alternate screen buffer
            1047 | 1049 => {
                if set && !self.alternate_screen {
                    // Save primary screen and switch to alternate
                    self.saved_primary =
                        Some((self.grid.clone(), self.scrollback.clone(), self.cursor));
                    self.scrollback.clear();
                    self.clear_all();
                    self.alternate_screen = true;
                } else if !set && self.alternate_screen {
                    // Restore primary screen
                    if let Some((grid, scrollback, cursor)) = self.saved_primary.take() {
                        self.grid = grid;
                        self.scrollback = scrollback;
                        self.cursor = cursor;
                    }
                    self.alternate_screen = false;
                }
            }
            // Save cursor for alternate screen
            1048 => {
                if set {
                    self.saved_cursor = Some(self.cursor);
                } else if let Some(cursor) = self.saved_cursor.take() {
                    self.cursor = cursor;
                    self.clamp_cursor();
                }
            }
            _ => {}
        }
    }

    /// Handle ESC sequence
    fn handle_esc(&mut self, esc: EscSequence) {
        match esc {
            EscSequence::SaveCursor => {
                self.saved_cursor = Some(self.cursor);
            }
            EscSequence::RestoreCursor => {
                if let Some(cursor) = self.saved_cursor.take() {
                    self.cursor = cursor;
                    self.clamp_cursor();
                }
            }
            EscSequence::Index => {
                self.cursor.row += 1;
                if self.cursor.row > self.scroll_region.1 {
                    self.scroll_up(1);
                    self.cursor.row = self.scroll_region.1;
                }
            }
            EscSequence::ReverseIndex => {
                if self.cursor.row == self.scroll_region.0 {
                    self.scroll_down(1);
                } else {
                    self.cursor.row = self.cursor.row.saturating_sub(1);
                }
            }
            EscSequence::NextLine => {
                self.cursor.col = 0;
                self.cursor.row += 1;
                if self.cursor.row > self.scroll_region.1 {
                    self.scroll_up(1);
                    self.cursor.row = self.scroll_region.1;
                }
            }
            EscSequence::Reset => {
                self.reset();
            }
            _ => {}
        }
    }

    /// Erase from cursor to end of screen
    fn erase_below(&mut self) {
        self.erase_line_right();
        for row in self.grid.iter_mut().skip(self.cursor.row + 1) {
            row.clear();
        }
        // Mark all affected rows as dirty
        if self.dirty_tracking_enabled {
            for i in (self.cursor.row + 1)..self.rows {
                self.dirty_lines.insert(i);
            }
        }
    }

    /// Erase from beginning of screen to cursor
    fn erase_above(&mut self) {
        self.erase_line_left();
        for row in self.grid.iter_mut().take(self.cursor.row) {
            row.clear();
        }
    }

    /// Erase entire screen
    fn erase_all(&mut self) {
        for row in &mut self.grid {
            row.clear();
        }
        if self.dirty_tracking_enabled {
            for i in 0..self.rows {
                self.dirty_lines.insert(i);
            }
        }
    }

    /// Clear entire screen and reset cursor
    fn clear_all(&mut self) {
        self.erase_all();
        self.cursor = Cursor::new();
    }

    /// Erase from cursor to end of line
    fn erase_line_right(&mut self) {
        if let Some(row) = self.grid.get_mut(self.cursor.row) {
            row.clear_from(self.cursor.col);
            self.mark_dirty(self.cursor.row);
        }
    }

    /// Erase from beginning of line to cursor
    fn erase_line_left(&mut self) {
        if let Some(row) = self.grid.get_mut(self.cursor.row) {
            row.clear_to(self.cursor.col);
            self.mark_dirty(self.cursor.row);
        }
    }

    /// Erase entire line
    fn erase_line(&mut self) {
        if let Some(row) = self.grid.get_mut(self.cursor.row) {
            row.clear();
            self.mark_dirty(self.cursor.row);
        }
    }

    /// Scroll up by n lines
    fn scroll_up(&mut self, n: usize) {
        let (top, bottom) = self.scroll_region;
        for _ in 0..n {
            if top < self.grid.len() && top <= bottom {
                let removed = self.grid.remove(top);
                // Add to scrollback if we're scrolling the entire screen
                if top == 0 && !self.alternate_screen && !removed.is_empty() {
                    self.scrollback.push_back(removed);
                    if self.scrollback.len() > self.max_scrollback {
                        self.scrollback.pop_front(); // O(1) with VecDeque
                    }
                }
                if bottom < self.grid.len() {
                    self.grid.insert(bottom, Row::new(self.cols));
                } else {
                    self.grid.push(Row::new(self.cols));
                }
                // Mark all rows in the scroll region as dirty
                if self.dirty_tracking_enabled {
                    for i in top..=bottom.min(self.rows.saturating_sub(1)) {
                        self.dirty_lines.insert(i);
                    }
                }
            }
        }
    }

    /// Scroll down by n lines
    fn scroll_down(&mut self, n: usize) {
        let (top, bottom) = self.scroll_region;
        for _ in 0..n {
            if bottom < self.grid.len() && top <= bottom {
                self.grid.remove(bottom);
                self.grid.insert(top, Row::new(self.cols));
                // Mark all rows in the scroll region as dirty
                if self.dirty_tracking_enabled {
                    for i in top..=bottom.min(self.rows.saturating_sub(1)) {
                        self.dirty_lines.insert(i);
                    }
                }
            }
        }
    }

    /// Insert n blank lines at cursor
    fn insert_lines(&mut self, n: usize) {
        let (_, bottom) = self.scroll_region;
        for _ in 0..n {
            if self.cursor.row <= bottom && bottom < self.grid.len() {
                self.grid.remove(bottom);
                self.grid.insert(self.cursor.row, Row::new(self.cols));
            }
        }
        // Mark affected rows as dirty
        if self.dirty_tracking_enabled {
            for i in self.cursor.row..=bottom.min(self.rows.saturating_sub(1)) {
                self.dirty_lines.insert(i);
            }
        }
    }

    /// Delete n lines at cursor
    fn delete_lines(&mut self, n: usize) {
        let (_, bottom) = self.scroll_region;
        for _ in 0..n {
            if self.cursor.row < self.grid.len() && self.cursor.row <= bottom {
                self.grid.remove(self.cursor.row);
                if bottom < self.grid.len() {
                    self.grid.insert(bottom, Row::new(self.cols));
                } else {
                    self.grid.push(Row::new(self.cols));
                }
            }
        }
        // Mark affected rows as dirty
        if self.dirty_tracking_enabled {
            for i in self.cursor.row..=bottom.min(self.rows.saturating_sub(1)) {
                self.dirty_lines.insert(i);
            }
        }
    }

    /// Insert n blank characters at cursor
    fn insert_chars(&mut self, n: usize) {
        if let Some(row) = self.grid.get_mut(self.cursor.row) {
            for _ in 0..n {
                if self.cursor.col < row.width() {
                    row.cells.pop();
                    row.cells.insert(self.cursor.col, Cell::new());
                }
            }
            self.mark_dirty(self.cursor.row);
        }
    }

    /// Delete n characters at cursor
    fn delete_chars(&mut self, n: usize) {
        if let Some(row) = self.grid.get_mut(self.cursor.row) {
            for _ in 0..n {
                if self.cursor.col < row.width() {
                    row.cells.remove(self.cursor.col);
                    row.cells.push(Cell::new());
                }
            }
            self.mark_dirty(self.cursor.row);
        }
    }

    /// Mark a line as dirty
    fn mark_dirty(&mut self, row: usize) {
        if self.dirty_tracking_enabled {
            self.dirty_lines.insert(row);
        }
    }

    /// Get and clear dirty lines
    pub fn take_dirty_lines(&mut self) -> HashSet<usize> {
        std::mem::take(&mut self.dirty_lines)
    }

    /// Enable or disable dirty line tracking
    pub fn set_dirty_tracking(&mut self, enabled: bool) {
        self.dirty_tracking_enabled = enabled;
        if !enabled {
            self.dirty_lines.clear();
        }
    }

    /// Check if dirty line tracking is enabled
    pub fn is_dirty_tracking_enabled(&self) -> bool {
        self.dirty_tracking_enabled
    }

    /// Clear all dirty lines
    pub fn clear_dirty_lines(&mut self) {
        self.dirty_lines.clear();
    }

    /// Get number of dirty lines
    pub fn dirty_line_count(&self) -> usize {
        self.dirty_lines.len()
    }

    /// Reset screen to initial state
    pub fn reset(&mut self) {
        self.grid = (0..self.rows).map(|_| Row::new(self.cols)).collect();
        self.scrollback.clear();
        self.cursor = Cursor::new();
        self.saved_cursor = None;
        self.current_attrs = CellAttrs::new();
        self.alternate_screen = false;
        self.saved_primary = None;
        self.scroll_region = (0, self.rows.saturating_sub(1));
        self.parser.reset();
    }

    /// Get the content of a row as a string
    pub fn row_text(&self, row: usize) -> String {
        self.grid
            .get(row)
            .map(|r| r.cells.iter().map(|c| c.ch).collect())
            .unwrap_or_default()
    }

    /// Get all visible text
    pub fn text(&self) -> String {
        self.grid
            .iter()
            .map(|r| r.cells.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Compute a stable hash of the visual terminal state.
    ///
    /// This hash includes:
    /// - All visible cell contents and attributes
    /// - Cursor position
    /// - Screen dimensions
    ///
    /// This hash excludes non-deterministic metadata like:
    /// - Scrollback buffer contents
    /// - Parser state
    /// - Saved cursor position
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        let mut hasher = FnvHasher::new();

        // Hash dimensions
        self.cols.hash(&mut hasher);
        self.rows.hash(&mut hasher);

        // Hash cursor position
        self.cursor.row.hash(&mut hasher);
        self.cursor.col.hash(&mut hasher);

        // Hash all visible cells
        for row in &self.grid {
            for cell in &row.cells {
                cell.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Compute a hash that includes only the visible text (no attributes).
    ///
    /// Useful for comparing screen content ignoring styling.
    #[must_use]
    pub fn text_hash(&self) -> u64 {
        let mut hasher = FnvHasher::new();

        self.cols.hash(&mut hasher);
        self.rows.hash(&mut hasher);
        self.cursor.row.hash(&mut hasher);
        self.cursor.col.hash(&mut hasher);

        for row in &self.grid {
            for cell in &row.cells {
                cell.ch.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Check if two screens have the same visual state
    pub fn visual_equals(&self, other: &Screen) -> bool {
        if self.cols != other.cols || self.rows != other.rows {
            return false;
        }
        if self.cursor != other.cursor {
            return false;
        }

        for (row1, row2) in self.grid.iter().zip(other.grid.iter()) {
            for (cell1, cell2) in row1.cells.iter().zip(row2.cells.iter()) {
                if cell1 != cell2 {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_print() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello");

        assert_eq!(screen.cursor().col, 5);
        assert_eq!(screen.cursor().row, 0);
        assert!(screen.row_text(0).starts_with("Hello"));
    }

    #[test]
    fn cursor_movement() {
        let mut screen = Screen::new(80, 24);

        // Move to position (10, 5) - 1-indexed in ANSI
        screen.process(b"\x1b[5;10H");
        assert_eq!(screen.cursor().row, 4);
        assert_eq!(screen.cursor().col, 9);

        // Move up 2
        screen.process(b"\x1b[2A");
        assert_eq!(screen.cursor().row, 2);

        // Move right 5
        screen.process(b"\x1b[5C");
        assert_eq!(screen.cursor().col, 14);
    }

    #[test]
    fn cursor_never_out_of_bounds() {
        let mut screen = Screen::new(80, 24);

        // Try to move way out of bounds
        screen.process(b"\x1b[999;999H");
        assert!(screen.cursor().row < 24);
        assert!(screen.cursor().col < 80);

        // Try to move negative
        screen.process(b"\x1b[1;1H\x1b[999A\x1b[999D");
        assert_eq!(screen.cursor().row, 0);
        assert_eq!(screen.cursor().col, 0);
    }

    #[test]
    fn clear_screen() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        screen.process(b"\x1b[2J"); // Clear all

        assert!(screen.row_text(0).trim().is_empty());
    }

    #[test]
    fn clear_line() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        screen.process(b"\x1b[5G"); // Move to column 5
        screen.process(b"\x1b[K"); // Clear to end of line

        let text = screen.row_text(0);
        assert!(text.starts_with("Hell"));
        assert!(!text.contains("World"));
    }

    #[test]
    fn scrollback_grows_deterministically() {
        let mut screen = Screen::new(80, 5);
        screen.set_max_scrollback(100);

        // Generate enough output to scroll
        for i in 0..10 {
            screen.process(format!("Line {}\n", i).as_bytes());
        }

        // Check scrollback has expected content
        assert!(screen.scrollback_len() > 0);
        assert!(screen.scrollback_len() <= 100);
    }

    #[test]
    fn resize_preserves_state() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        // Move to next line
        screen.process(b"\x1b[2;1H");
        screen.process(b"Line 2");
        screen.process(b"\x1b[3;1H");
        screen.process(b"Line 3");

        // Initial state check
        assert!(screen.row_text(0).starts_with("Hello"), "Initial row 0");
        assert!(screen.row_text(1).starts_with("Line 2"), "Initial row 1");

        // Resize to still have enough rows
        screen.resize(40, 24);
        assert!(
            screen.row_text(0).starts_with("Hello"),
            "After width resize row 0"
        );

        // Resize larger
        screen.resize(100, 30);
        assert!(
            screen.row_text(0).starts_with("Hello"),
            "After resize larger row 0"
        );
    }

    #[test]
    fn line_wrapping() {
        let mut screen = Screen::new(10, 5);
        screen.process(b"Hello World!");

        // "Hello Worl" should be on line 0, "d!" on line 1
        assert!(screen.row_text(0).contains("Hello"));
        assert!(screen.cursor().row >= 1 || screen.cursor().col > 0);
    }

    #[test]
    fn sgr_attributes() {
        let mut screen = Screen::new(80, 24);

        // Set bold and red foreground
        screen.process(b"\x1b[1;31mHello\x1b[0m");

        let cell = screen.get_cell(0, 0).unwrap();
        assert!(cell.attrs.flags.contains(AttrFlags::BOLD));
        assert_eq!(cell.attrs.fg, 1); // Red is color 1
    }

    #[test]
    fn scroll_region() {
        let mut screen = Screen::new(80, 10);

        // Set scroll region to lines 3-7 (1-indexed in ANSI)
        screen.process(b"\x1b[3;7r");
        assert_eq!(screen.scroll_region, (2, 6));
    }

    #[test]
    fn alternate_screen() {
        let mut screen = Screen::new(80, 24);

        // Write some content in primary screen
        screen.process(b"Primary Content");
        assert!(screen.row_text(0).contains("Primary"));

        // Switch to alternate screen
        screen.process(b"\x1b[?1049h");
        assert!(screen.alternate_screen);

        // Write in alternate screen
        screen.process(b"Alternate Content");
        assert!(screen.row_text(0).contains("Alternate"));

        // Switch back to primary
        screen.process(b"\x1b[?1049l");
        assert!(!screen.alternate_screen);

        // Primary content should be restored
        assert!(
            screen.row_text(0).contains("Primary"),
            "Expected 'Primary' after switching back, got: {:?}",
            screen.row_text(0)
        );
    }

    #[test]
    fn attributes_reset() {
        let mut screen = Screen::new(80, 24);

        // Set various attributes
        screen.process(b"\x1b[1;4;31mStyled\x1b[0mNormal");

        // First cell should have attributes
        let styled = screen.get_cell(0, 0).unwrap();
        assert!(styled.attrs.flags.contains(AttrFlags::BOLD));
        assert!(styled.attrs.flags.contains(AttrFlags::UNDERLINE));
        assert_eq!(styled.attrs.fg, 1);

        // "Normal" starts at column 6
        let normal = screen.get_cell(0, 6).unwrap();
        assert!(!normal.attrs.flags.contains(AttrFlags::BOLD));
        assert!(!normal.attrs.flags.contains(AttrFlags::UNDERLINE));
        assert_eq!(normal.attrs.fg, -1); // Default
    }

    #[test]
    fn same_state_same_hash() {
        let mut screen1 = Screen::new(80, 24);
        let mut screen2 = Screen::new(80, 24);

        // Same operations on both screens
        screen1.process(b"Hello World");
        screen2.process(b"Hello World");

        assert_eq!(screen1.state_hash(), screen2.state_hash());
        assert!(screen1.visual_equals(&screen2));
    }

    #[test]
    fn different_content_different_hash() {
        let mut screen1 = Screen::new(80, 24);
        let mut screen2 = Screen::new(80, 24);

        screen1.process(b"Hello World");
        screen2.process(b"Goodbye World");

        assert_ne!(screen1.state_hash(), screen2.state_hash());
        assert!(!screen1.visual_equals(&screen2));
    }

    #[test]
    fn different_cursor_different_hash() {
        let mut screen1 = Screen::new(80, 24);
        let mut screen2 = Screen::new(80, 24);

        screen1.process(b"Hello");
        screen2.process(b"Hello");
        screen2.process(b"\x1b[1;1H"); // Move cursor to start

        assert_ne!(screen1.state_hash(), screen2.state_hash());
    }

    #[test]
    fn different_attributes_different_hash() {
        let mut screen1 = Screen::new(80, 24);
        let mut screen2 = Screen::new(80, 24);

        screen1.process(b"Hello");
        screen2.process(b"\x1b[1mHello\x1b[0m"); // Bold

        // Full hash should differ (includes attributes)
        assert_ne!(screen1.state_hash(), screen2.state_hash());

        // Text hash should be the same (excludes attributes)
        assert_eq!(screen1.text_hash(), screen2.text_hash());
    }

    #[test]
    fn hash_stable_across_operations() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Test Content");
        let hash1 = screen.state_hash();

        // Hash should be the same when called again
        let hash2 = screen.state_hash();
        assert_eq!(hash1, hash2);

        // Hash should be the same after re-creating same state
        let mut screen2 = Screen::new(80, 24);
        screen2.process(b"Test Content");
        assert_eq!(hash1, screen2.state_hash());
    }

    #[test]
    fn dirty_tracking_basic() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Initially no dirty lines
        assert!(screen.take_dirty_lines().is_empty());

        // Print something - should mark line 0 as dirty
        screen.process(b"Hello");
        let dirty = screen.take_dirty_lines();
        assert!(dirty.contains(&0), "Expected line 0 to be dirty");

        // Clear and check again
        assert!(screen.take_dirty_lines().is_empty());
    }

    #[test]
    fn dirty_tracking_cursor_movement() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Move cursor - should mark line as dirty
        screen.process(b"\x1b[5;10H");
        let dirty = screen.take_dirty_lines();
        assert!(
            dirty.contains(&4),
            "Expected line 4 to be dirty after cursor move"
        );
    }

    #[test]
    fn dirty_tracking_erase() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Erase screen - all lines should be dirty
        screen.process(b"\x1b[2J");
        let dirty = screen.take_dirty_lines();
        for i in 0..24 {
            assert!(
                dirty.contains(&i),
                "Expected line {} to be dirty after erase",
                i
            );
        }
    }

    #[test]
    fn dirty_tracking_line_erase() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        screen.process(b"Hello World");
        screen.process(b"\x1b[5G"); // Move to column 5
        screen.process(b"\x1b[K"); // Clear to end of line

        let dirty = screen.take_dirty_lines();
        assert!(
            dirty.contains(&0),
            "Expected line 0 to be dirty after line erase"
        );
    }

    #[test]
    fn dirty_tracking_disabled() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(false);

        screen.process(b"Hello");
        let dirty = screen.take_dirty_lines();
        assert!(dirty.is_empty(), "No dirty lines when tracking disabled");
    }

    #[test]
    fn dirty_tracking_scroll() {
        let mut screen = Screen::new(80, 5);
        screen.set_dirty_tracking(true);

        // Fill screen and trigger scroll
        for i in 0..10 {
            screen.process(format!("Line {}\n", i).as_bytes());
        }

        let dirty = screen.take_dirty_lines();
        // At least some lines should be dirty after scrolling
        assert!(!dirty.is_empty(), "Expected dirty lines after scrolling");
    }

    #[test]
    fn dirty_tracking_insert_delete_lines() {
        let mut screen = Screen::new(80, 10);
        screen.set_dirty_tracking(true);

        screen.process(b"\x1b[3;8r"); // Set scroll region
        screen.process(b"\x1b[3H"); // Move to line 3

        // Insert lines
        screen.process(b"\x1b[2L");
        let dirty = screen.take_dirty_lines();
        assert!(!dirty.is_empty(), "Expected dirty lines after insert");

        // Delete lines
        screen.process(b"\x1b[1M");
        let dirty = screen.take_dirty_lines();
        assert!(!dirty.is_empty(), "Expected dirty lines after delete");
    }

    #[test]
    fn dirty_tracking_insert_delete_chars() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        screen.process(b"Hello World");
        screen.process(b"\x1b[1;6H"); // Move to row 1, column 6

        // Insert characters
        screen.process(b"\x1b[2@"); // Insert 2 chars
        let dirty = screen.take_dirty_lines();
        assert!(
            dirty.contains(&0),
            "Expected line 0 dirty after insert chars, got: {:?}",
            dirty
        );

        // Delete characters - cursor at column 6, delete 2 chars
        screen.process(b"\x1b[2P"); // Delete 2 chars
        let dirty = screen.take_dirty_lines();
        // After delete, line should still be dirty because content changed
        assert!(
            dirty.contains(&0),
            "Expected line 0 dirty after delete chars, got: {:?}",
            dirty
        );
    }

    #[test]
    fn dirty_tracking_alternate_screen() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Write in primary
        screen.process(b"Primary");
        let dirty1 = screen.take_dirty_lines();

        // Switch to alternate
        screen.process(b"\x1b[?1049h");
        let dirty2 = screen.take_dirty_lines();

        // Write in alternate
        screen.process(b"Alternate");
        let dirty3 = screen.take_dirty_lines();

        // All should have dirty lines
        assert!(!dirty1.is_empty());
        assert!(!dirty2.is_empty());
        assert!(!dirty3.is_empty());
    }

    #[test]
    fn dirty_line_count() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        assert_eq!(screen.dirty_line_count(), 0);

        screen.process(b"Line 1\nLine 2\nLine 3");
        screen.take_dirty_lines(); // Clear dirty lines

        // After scrolling, more lines should be dirty
        for _ in 0..30 {
            screen.process(b"More text\n");
        }

        assert!(screen.dirty_line_count() > 0);
    }

    // =========================================================================
    // Stress Tests for Dirty Line Tracking
    // =========================================================================

    #[test]
    fn stress_dirty_tracking_large_screen() {
        let mut screen = Screen::new(200, 100);
        screen.set_dirty_tracking(true);

        // Fill the screen
        for row in 0..100 {
            let line = format!("Line {} with some content to fill the screen", row);
            screen.process(line.as_bytes());
            screen.process(b"\n");
        }

        let dirty = screen.take_dirty_lines();
        // All rows should be dirty
        assert_eq!(dirty.len(), 100, "Expected all 100 rows to be dirty");
    }

    #[test]
    fn stress_dirty_tracking_heavy_scrolling() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Heavy scrolling - 1000 lines
        for i in 0..1000 {
            screen.process(format!("Line {}\n", i).as_bytes());
        }

        let dirty = screen.take_dirty_lines();
        // Should have dirty lines from scrolling
        assert!(
            dirty.len() >= 20,
            "Expected significant dirty lines from scrolling"
        );

        // Scrollback should have accumulated
        assert!(
            screen.scrollback_len() > 900,
            "Expected scrollback to accumulate"
        );
    }

    #[test]
    fn stress_dirty_tracking_cursor_storm() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Cursor movement storm - move cursor around wildly
        for i in 0..1000 {
            let row = (i % 24) as u16 + 1;
            let col = (i % 80) as u16 + 1;
            screen.process(format!("\x1b[{};{}H", row, col).as_bytes());
        }

        let dirty = screen.take_dirty_lines();
        // All rows should be affected by cursor movement
        assert_eq!(dirty.len(), 24, "Expected all rows to be marked dirty");
    }

    #[test]
    fn stress_dirty_tracking_repeated_clears() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Repeated clear screen operations
        for _ in 0..100 {
            screen.process(&b"Some content here\n".repeat(5));
            screen.process(b"\x1b[2J"); // Clear screen
            screen.take_dirty_lines(); // Clear dirty tracking
        }

        // Each clear should mark all lines dirty
        for _ in 0..5 {
            screen.process(b"Test\n");
            let dirty = screen.take_dirty_lines();
            assert!(!dirty.is_empty(), "Expected dirty lines after content");
        }
    }

    #[test]
    fn stress_dirty_tracking_insert_delete_lines() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);
        screen.process(b"\x1b[3;20r"); // Set scroll region

        // Heavy insert/delete line operations
        for i in 0..500 {
            screen.process(b"\x1b[5H"); // Move to line 5
            screen.process(b"\x1b[3L"); // Insert 3 lines
            screen.take_dirty_lines();

            screen.process(b"\x1b[10H"); // Move to line 10
            screen.process(b"\x1b[2M"); // Delete 2 lines
            let dirty = screen.take_dirty_lines();
            assert!(
                !dirty.is_empty(),
                "Expected dirty lines from insert/delete at iteration {}",
                i
            );
        }
    }

    #[test]
    fn stress_dirty_tracking_erase_operations() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Various erase operations
        for _ in 0..200 {
            screen.process(&b"Hello World Test Data\n".repeat(3));

            // Erase below
            screen.process(b"\x1b[10;1H");
            screen.process(b"\x1b[J");
            let dirty1 = screen.take_dirty_lines();

            // Erase line
            screen.process(b"\x1b[5;1H");
            screen.process(b"\x1b[2K");
            let dirty2 = screen.take_dirty_lines();

            assert!(!dirty1.is_empty() || !dirty2.is_empty());
        }
    }

    #[test]
    fn stress_dirty_tracking_sgr_rainbow() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // SGR attribute changes - each line with different attributes
        for i in 0..100 {
            // Bold, colors, underline, etc.
            let attr = format!("\x1b[{}m", (i % 10) + 1);
            screen.process(attr.as_bytes());
            screen.process(format!("Styled line {}\n", i).as_bytes());
        }

        let dirty = screen.take_dirty_lines();
        assert!(
            dirty.len() >= 20,
            "Expected many dirty lines from SGR changes"
        );
    }

    #[test]
    fn stress_dirty_tracking_alternate_screen_toggle() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Toggle alternate screen repeatedly
        for i in 0..100 {
            screen.process(b"\x1b[?1049h"); // Enter alternate screen
            screen.process(&b"Alternate content\n".repeat(5));
            let dirty1 = screen.take_dirty_lines();

            screen.process(b"\x1b[?1049l"); // Exit alternate screen
            screen.process(&b"Primary content\n".repeat(5));
            let dirty2 = screen.take_dirty_lines();

            assert!(
                !dirty1.is_empty(),
                "Expected dirty lines in alternate screen at {}",
                i
            );
            assert!(
                !dirty2.is_empty(),
                "Expected dirty lines back in primary at {}",
                i
            );
        }
    }

    #[test]
    fn stress_dirty_tracking_enabled_disabled() {
        let mut screen = Screen::new(80, 24);

        // Without tracking
        screen.process(&b"Hello\n".repeat(100));
        assert_eq!(screen.dirty_line_count(), 0);

        // Enable tracking
        screen.set_dirty_tracking(true);
        screen.process(&b"New content\n".repeat(50));
        assert!(screen.dirty_line_count() > 0);

        // Disable tracking
        screen.set_dirty_tracking(false);
        screen.process(&b"More content\n".repeat(50));
        assert_eq!(screen.dirty_line_count(), 0);

        // Re-enable - should start fresh
        screen.set_dirty_tracking(true);
        screen.process(&b"Final content\n".repeat(10));
        assert!(
            screen.dirty_line_count() > 0,
            "Expected dirty lines after re-enabling"
        );
    }

    #[test]
    fn stress_dirty_tracking_concurrent_modifications() {
        let mut screen = Screen::new(80, 50);
        screen.set_dirty_tracking(true);

        // Simulate concurrent modifications at different locations
        for i in 0..1000 {
            let row = (i * 7) % 50;
            let col = (i * 13) % 80;

            // Move cursor and modify
            screen.process(format!("\x1b[{};{}H", row + 1, col + 1).as_bytes());
            screen.process(b"X");

            // Every 10 operations, check dirty state
            if i % 10 == 9 {
                let dirty = screen.take_dirty_lines();
                assert!(!dirty.is_empty(), "Expected dirty lines at iteration {}", i);
            }
        }
    }

    #[test]
    fn stress_dirty_tracking_hash_performance() {
        let mut screen = Screen::new(80, 24);
        screen.set_dirty_tracking(true);

        // Fill screen
        screen.process(&b"Test data for hashing\n".repeat(20));
        let dirty = screen.take_dirty_lines();

        // Hash performance with dirty tracking
        for _ in 0..100 {
            let _ = screen.state_hash();
            let _ = screen.text_hash();
        }

        assert!(!dirty.is_empty());
    }

    // ==================== EDGE CASE TESTS ====================

    #[test]
    fn zero_size_screen() {
        let screen = Screen::new(0, 0);
        assert_eq!(screen.size(), (0, 0));
        // Should not panic
        let _ = screen.state_hash();
        let _ = screen.text_hash();
        let _ = screen.text();
    }

    #[test]
    fn very_long_line_wrapping() {
        let mut screen = Screen::new(10, 5);
        let long_line = "A".repeat(100);
        screen.process(long_line.as_bytes());
        // Should handle long lines without panicking
        // Note: In terminal semantics, cursor can be at position `cols` temporarily
        // (the "wrap position") before the next character triggers a wrap.
        // This is valid behavior - the cursor is at the position where the NEXT
        // character will go, which may be past the last column.
        assert!(
            screen.cursor().col <= 10,
            "Cursor col {} should be <= 10 (cols=10)",
            screen.cursor().col
        );
        assert!(
            screen.cursor().row < 5,
            "Cursor row {} should be < 5 (rows=5)",
            screen.cursor().row
        );
    }

    #[test]
    fn scrollback_trim_efficiency() {
        // Test that scrollback trimming works correctly with VecDeque
        let mut screen = Screen::new(80, 5);
        screen.set_max_scrollback(10);

        // Generate many lines to test scrollback trimming
        for i in 0..100 {
            screen.process(format!("Line {}\n", i).as_bytes());
        }

        // Scrollback should be capped at max
        assert!(
            screen.scrollback_len() <= 10,
            "Scrollback should be <= 10, got {}",
            screen.scrollback_len()
        );
    }

    #[test]
    fn resize_shrink_with_content() {
        let mut screen = Screen::new(80, 24);
        for i in 0..30 {
            screen.process(format!("Line {}\n", i).as_bytes());
        }
        screen.resize(40, 10);
        // Cursor should be within bounds
        assert!(screen.cursor().row < 10);
        assert!(screen.cursor().col < 40);
    }

    #[test]
    fn resize_to_one_cell() {
        let mut screen = Screen::new(80, 24);
        screen.process(b"Hello World");
        screen.resize(1, 1);
        assert_eq!(screen.size(), (1, 1));
        // Should not panic
        let _ = screen.state_hash();
    }

    #[test]
    fn binary_garbage_input() {
        let mut screen = Screen::new(80, 24);
        let garbage: Vec<u8> = (0..255).collect();
        // Should not panic
        screen.process(&garbage);
        screen.process(&garbage);
        screen.process(&garbage);
        let _ = screen.text();
        let _ = screen.state_hash();
    }

    #[test]
    fn invalid_cursor_position_clamped() {
        let mut screen = Screen::new(80, 24);
        // Try to move cursor to absurd position
        screen.process(b"\x1b[99999;99999H");
        // Should be clamped to valid bounds
        assert!(screen.cursor().row < 24);
        assert!(screen.cursor().col < 80);
    }

    #[test]
    fn negative_cursor_movement_clamped() {
        let mut screen = Screen::new(80, 24);
        // Move to 0,0 then try to go negative
        screen.process(b"\x1b[1;1H"); // Move to row 1, col 1 (0-indexed: 0, 0)
        screen.process(b"\x1b[99999A"); // Up way too much
        screen.process(b"\x1b[99999D"); // Left way too much
                                        // Should be clamped at 0,0
        assert_eq!(screen.cursor().row, 0);
        assert_eq!(screen.cursor().col, 0);
    }

    #[test]
    fn set_max_scrollback_trims_existing() {
        let mut screen = Screen::new(80, 5);
        screen.set_max_scrollback(100);

        // Generate scrollback
        for i in 0..50 {
            screen.process(format!("Line {}\n", i).as_bytes());
        }
        let initial_scrollback = screen.scrollback_len();
        assert!(initial_scrollback > 0);

        // Reduce max scrollback - should trim
        screen.set_max_scrollback(5);
        assert!(
            screen.scrollback_len() <= 5,
            "Scrollback should be trimmed to 5, got {}",
            screen.scrollback_len()
        );
    }

    #[test]
    fn alternate_screen_no_scrollback() {
        // In alternate screen mode, scrollback should not grow
        let mut screen = Screen::new(80, 5);
        screen.set_max_scrollback(100);

        // Switch to alternate screen
        screen.process(b"\x1b[?1049h");
        assert!(screen.alternate_screen);

        // Generate lots of output that would normally create scrollback
        for i in 0..50 {
            screen.process(format!("Alt line {}\n", i).as_bytes());
        }

        // Scrollback should be empty in alternate mode
        assert_eq!(
            screen.scrollback_len(),
            0,
            "Scrollback should be 0 in alternate screen mode"
        );

        // Switch back to primary
        screen.process(b"\x1b[?1049l");
        assert!(!screen.alternate_screen);

        // Scrollback should still be 0 (we were in alternate mode)
        assert_eq!(screen.scrollback_len(), 0);

        // Now generate output in primary mode
        for i in 0..50 {
            screen.process(format!("Primary line {}\n", i).as_bytes());
        }

        // NOW scrollback should have grown
        assert!(
            screen.scrollback_len() > 0,
            "Scrollback should grow in primary mode"
        );
    }

    #[test]
    fn screen_dimension_validation() {
        // Test that screen dimensions are stored correctly
        let screen = Screen::new(80, 24);
        assert_eq!(screen.size(), (80, 24));

        let screen = Screen::new(1, 1);
        assert_eq!(screen.size(), (1, 1));

        // Large dimensions
        let screen = Screen::new(1000, 500);
        assert_eq!(screen.size(), (1000, 500));
    }
}
