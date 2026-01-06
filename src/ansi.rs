//! ANSI Escape Sequence Parser
//!
//! This module provides an incremental parser for ANSI escape sequences,
//! converting byte streams into terminal events.

#![allow(dead_code)]

/// Events produced by the ANSI parser
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiEvent {
    /// Print a character at the current cursor position
    Print(char),
    /// Execute a C0 control code
    Execute(u8),
    /// CSI (Control Sequence Introducer) sequence
    Csi(CsiSequence),
    /// OSC (Operating System Command) sequence
    Osc(OscSequence),
    /// ESC sequence (non-CSI)
    Esc(EscSequence),
    /// DCS (Device Control String)
    Dcs(Vec<u8>),
    /// APC (Application Program Command)
    Apc(Vec<u8>),
}

/// CSI sequence with parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsiSequence {
    /// Parameters (semicolon-separated numbers)
    pub params: Vec<u16>,
    /// Intermediate bytes (between parameters and final byte)
    pub intermediates: Vec<u8>,
    /// Final byte (determines the command)
    pub final_byte: u8,
    /// Private marker (? or > etc.)
    pub private_marker: Option<u8>,
}

impl CsiSequence {
    /// Get parameter at index, with default value
    pub fn param(&self, index: usize, default: u16) -> u16 {
        self.params.get(index).copied().unwrap_or(default)
    }

    /// Check if this is a cursor movement command
    pub fn is_cursor_movement(&self) -> bool {
        matches!(
            self.final_byte,
            b'A' | b'B' | b'C' | b'D' | b'E' | b'F' | b'G' | b'H' | b'f'
        )
    }

    /// Check if this is an erase command
    pub fn is_erase(&self) -> bool {
        matches!(self.final_byte, b'J' | b'K')
    }

    /// Check if this is an SGR (Select Graphic Rendition) command
    pub fn is_sgr(&self) -> bool {
        self.final_byte == b'm'
    }
}

/// OSC (Operating System Command) sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OscSequence {
    /// Command number
    pub command: u16,
    /// Data payload
    pub data: String,
}

/// Simple ESC sequences
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscSequence {
    /// ESC 7 - Save cursor
    SaveCursor,
    /// ESC 8 - Restore cursor
    RestoreCursor,
    /// ESC D - Index (move cursor down, scroll if needed)
    Index,
    /// ESC M - Reverse Index (move cursor up, scroll if needed)
    ReverseIndex,
    /// ESC E - Next Line
    NextLine,
    /// ESC c - Reset
    Reset,
    /// ESC = - Application Keypad
    ApplicationKeypad,
    /// ESC > - Normal Keypad
    NormalKeypad,
    /// ESC ( - Designate G0 Character Set
    DesignateG0(u8),
    /// ESC ) - Designate G1 Character Set
    DesignateG1(u8),
    /// ESC # 8 - DEC Screen Alignment Test
    DecAlignmentTest,
    /// Unknown sequence
    Unknown(Vec<u8>),
}

/// Parser state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    /// Ground state - processing normal characters
    Ground,
    /// Escape state - received ESC
    Escape,
    /// Escape intermediate - received ESC followed by intermediate byte
    EscapeIntermediate,
    /// CSI entry - received ESC [
    CsiEntry,
    /// CSI parameter - collecting parameters
    CsiParam,
    /// CSI intermediate - collecting intermediate bytes
    CsiIntermediate,
    /// CSI ignore - ignoring malformed sequence
    CsiIgnore,
    /// OSC string - collecting OSC data
    OscString,
    /// DCS entry
    DcsEntry,
    /// DCS passthrough
    DcsPassthrough,
    /// APC string
    ApcString,
    /// UTF-8 continuation
    Utf8,
}

/// Incremental ANSI parser
#[derive(Debug)]
pub struct AnsiParser {
    /// Current state
    state: ParserState,
    /// CSI parameters being collected
    csi_params: Vec<u16>,
    /// Current CSI parameter being built
    csi_current_param: u16,
    /// CSI intermediate bytes
    csi_intermediates: Vec<u8>,
    /// CSI private marker
    csi_private_marker: Option<u8>,
    /// OSC command number
    osc_command: u16,
    /// OSC data being collected
    osc_data: Vec<u8>,
    /// DCS data being collected
    dcs_data: Vec<u8>,
    /// APC data being collected
    apc_data: Vec<u8>,
    /// ESC sequence bytes
    esc_bytes: Vec<u8>,
    /// UTF-8 buffer
    utf8_buffer: Vec<u8>,
    /// Expected UTF-8 bytes remaining
    utf8_remaining: u8,
}

impl AnsiParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            csi_params: Vec::new(),
            csi_current_param: 0,
            csi_intermediates: Vec::new(),
            csi_private_marker: None,
            osc_command: 0,
            osc_data: Vec::new(),
            dcs_data: Vec::new(),
            apc_data: Vec::new(),
            esc_bytes: Vec::new(),
            utf8_buffer: Vec::new(),
            utf8_remaining: 0,
        }
    }

    /// Reset the parser to ground state
    pub fn reset(&mut self) {
        self.state = ParserState::Ground;
        self.clear_csi();
        self.clear_osc();
        self.clear_dcs();
        self.clear_apc();
        self.clear_esc();
        self.clear_utf8();
    }

    fn clear_csi(&mut self) {
        self.csi_params.clear();
        self.csi_current_param = 0;
        self.csi_intermediates.clear();
        self.csi_private_marker = None;
    }

    fn clear_osc(&mut self) {
        self.osc_command = 0;
        self.osc_data.clear();
    }

    fn clear_dcs(&mut self) {
        self.dcs_data.clear();
    }

    fn clear_apc(&mut self) {
        self.apc_data.clear();
    }

    fn clear_esc(&mut self) {
        self.esc_bytes.clear();
    }

    fn clear_utf8(&mut self) {
        self.utf8_buffer.clear();
        self.utf8_remaining = 0;
    }

    /// Parse a single byte, returning any events produced
    pub fn parse_byte(&mut self, byte: u8) -> Option<AnsiEvent> {
        match self.state {
            ParserState::Ground => self.ground(byte),
            ParserState::Escape => self.escape(byte),
            ParserState::EscapeIntermediate => self.escape_intermediate(byte),
            ParserState::CsiEntry => self.csi_entry(byte),
            ParserState::CsiParam => self.csi_param(byte),
            ParserState::CsiIntermediate => self.csi_intermediate(byte),
            ParserState::CsiIgnore => self.csi_ignore(byte),
            ParserState::OscString => self.osc_string(byte),
            ParserState::DcsEntry => self.dcs_entry(byte),
            ParserState::DcsPassthrough => self.dcs_passthrough(byte),
            ParserState::ApcString => self.apc_string(byte),
            ParserState::Utf8 => self.utf8(byte),
        }
    }

    /// Parse multiple bytes, returning all events produced
    pub fn parse(&mut self, data: &[u8]) -> Vec<AnsiEvent> {
        let mut events = Vec::new();
        for &byte in data {
            if let Some(event) = self.parse_byte(byte) {
                events.push(event);
            }
        }
        events
    }

    fn ground(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            // C0 control codes
            0x00..=0x1a | 0x1c..=0x1f => {
                if byte == 0x1b {
                    // ESC - start escape sequence
                    self.state = ParserState::Escape;
                    None
                } else {
                    Some(AnsiEvent::Execute(byte))
                }
            }
            0x1b => {
                // ESC
                self.state = ParserState::Escape;
                None
            }
            // DEL - ignore
            0x7f => None,
            // UTF-8 start bytes
            0xc0..=0xdf => {
                self.utf8_buffer.push(byte);
                self.utf8_remaining = 1;
                self.state = ParserState::Utf8;
                None
            }
            0xe0..=0xef => {
                self.utf8_buffer.push(byte);
                self.utf8_remaining = 2;
                self.state = ParserState::Utf8;
                None
            }
            0xf0..=0xf7 => {
                self.utf8_buffer.push(byte);
                self.utf8_remaining = 3;
                self.state = ParserState::Utf8;
                None
            }
            // Printable ASCII
            0x20..=0x7e => Some(AnsiEvent::Print(byte as char)),
            // High bytes (C1, continuation bytes treated as Latin-1)
            _ => Some(AnsiEvent::Print(byte as char)),
        }
    }

    fn utf8(&mut self, byte: u8) -> Option<AnsiEvent> {
        // Check for valid continuation byte
        if (byte & 0xc0) != 0x80 {
            // Invalid UTF-8, emit replacement character and reprocess byte
            self.clear_utf8();
            self.state = ParserState::Ground;
            // Process this byte in ground state
            return self.ground(byte);
        }

        self.utf8_buffer.push(byte);
        self.utf8_remaining -= 1;

        if self.utf8_remaining == 0 {
            // Complete UTF-8 sequence
            self.state = ParserState::Ground;
            let s = String::from_utf8_lossy(&self.utf8_buffer);
            let ch = s.chars().next().unwrap_or('\u{FFFD}');
            self.clear_utf8();
            Some(AnsiEvent::Print(ch))
        } else {
            None
        }
    }

    fn escape(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            // CSI
            b'[' => {
                self.state = ParserState::CsiEntry;
                self.clear_csi();
                None
            }
            // OSC
            b']' => {
                self.state = ParserState::OscString;
                self.clear_osc();
                None
            }
            // DCS
            b'P' => {
                self.state = ParserState::DcsEntry;
                self.clear_dcs();
                None
            }
            // APC
            b'_' => {
                self.state = ParserState::ApcString;
                self.clear_apc();
                None
            }
            // Simple ESC sequences
            b'7' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::SaveCursor))
            }
            b'8' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::RestoreCursor))
            }
            b'D' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::Index))
            }
            b'M' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::ReverseIndex))
            }
            b'E' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::NextLine))
            }
            b'c' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::Reset))
            }
            b'=' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::ApplicationKeypad))
            }
            b'>' => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::NormalKeypad))
            }
            // Intermediate bytes
            b'(' | b')' | b'#' => {
                self.esc_bytes.push(byte);
                self.state = ParserState::EscapeIntermediate;
                None
            }
            // Cancel
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                None
            }
            // ESC in escape state - start new escape
            0x1b => None,
            _ => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Esc(EscSequence::Unknown(vec![byte])))
            }
        }
    }

    fn escape_intermediate(&mut self, byte: u8) -> Option<AnsiEvent> {
        self.state = ParserState::Ground;

        if self.esc_bytes.first() == Some(&b'(') {
            return Some(AnsiEvent::Esc(EscSequence::DesignateG0(byte)));
        }
        if self.esc_bytes.first() == Some(&b')') {
            return Some(AnsiEvent::Esc(EscSequence::DesignateG1(byte)));
        }
        if self.esc_bytes.first() == Some(&b'#') && byte == b'8' {
            return Some(AnsiEvent::Esc(EscSequence::DecAlignmentTest));
        }

        self.esc_bytes.push(byte);
        Some(AnsiEvent::Esc(EscSequence::Unknown(std::mem::take(
            &mut self.esc_bytes,
        ))))
    }

    fn csi_entry(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            // Private marker
            b'?' | b'>' | b'<' | b'=' => {
                self.csi_private_marker = Some(byte);
                self.state = ParserState::CsiParam;
                None
            }
            // Parameter bytes
            b'0'..=b'9' => {
                self.csi_current_param = (byte - b'0') as u16;
                self.state = ParserState::CsiParam;
                None
            }
            b';' => {
                self.csi_params.push(0);
                self.state = ParserState::CsiParam;
                None
            }
            // Intermediate bytes
            b' '..=b'/' => {
                self.csi_intermediates.push(byte);
                self.state = ParserState::CsiIntermediate;
                None
            }
            // Final bytes
            0x40..=0x7e => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Csi(CsiSequence {
                    params: std::mem::take(&mut self.csi_params),
                    intermediates: std::mem::take(&mut self.csi_intermediates),
                    final_byte: byte,
                    private_marker: self.csi_private_marker.take(),
                }))
            }
            // Cancel
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                None
            }
            // ESC - start new escape
            0x1b => {
                self.state = ParserState::Escape;
                None
            }
            _ => {
                self.state = ParserState::CsiIgnore;
                None
            }
        }
    }

    fn csi_param(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            b'0'..=b'9' => {
                self.csi_current_param = self
                    .csi_current_param
                    .saturating_mul(10)
                    .saturating_add((byte - b'0') as u16);
                None
            }
            b';' => {
                self.csi_params.push(self.csi_current_param);
                self.csi_current_param = 0;
                None
            }
            b':' => {
                // Sub-parameter separator (used in SGR)
                self.csi_params.push(self.csi_current_param);
                self.csi_current_param = 0;
                None
            }
            // Intermediate bytes
            b' '..=b'/' => {
                self.csi_params.push(self.csi_current_param);
                self.csi_current_param = 0;
                self.csi_intermediates.push(byte);
                self.state = ParserState::CsiIntermediate;
                None
            }
            // Final bytes
            0x40..=0x7e => {
                self.csi_params.push(self.csi_current_param);
                self.state = ParserState::Ground;
                Some(AnsiEvent::Csi(CsiSequence {
                    params: std::mem::take(&mut self.csi_params),
                    intermediates: std::mem::take(&mut self.csi_intermediates),
                    final_byte: byte,
                    private_marker: self.csi_private_marker.take(),
                }))
            }
            // Cancel
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_csi();
                None
            }
            // ESC - start new escape
            0x1b => {
                self.state = ParserState::Escape;
                self.clear_csi();
                None
            }
            _ => {
                self.state = ParserState::CsiIgnore;
                None
            }
        }
    }

    fn csi_intermediate(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            // More intermediate bytes
            b' '..=b'/' => {
                self.csi_intermediates.push(byte);
                None
            }
            // Final bytes
            0x40..=0x7e => {
                self.state = ParserState::Ground;
                Some(AnsiEvent::Csi(CsiSequence {
                    params: std::mem::take(&mut self.csi_params),
                    intermediates: std::mem::take(&mut self.csi_intermediates),
                    final_byte: byte,
                    private_marker: self.csi_private_marker.take(),
                }))
            }
            // Cancel
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_csi();
                None
            }
            // ESC - start new escape
            0x1b => {
                self.state = ParserState::Escape;
                self.clear_csi();
                None
            }
            _ => {
                self.state = ParserState::CsiIgnore;
                None
            }
        }
    }

    fn csi_ignore(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            // Final bytes end the sequence
            0x40..=0x7e => {
                self.state = ParserState::Ground;
                self.clear_csi();
                None
            }
            // Cancel
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_csi();
                None
            }
            // ESC - start new escape
            0x1b => {
                self.state = ParserState::Escape;
                self.clear_csi();
                None
            }
            _ => None,
        }
    }

    fn osc_string(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            // ST (String Terminator) - ESC backslash handled elsewhere
            0x07 | 0x9c => {
                self.state = ParserState::Ground;
                let data = String::from_utf8_lossy(&self.osc_data).into_owned();
                let command = self.osc_command;
                self.clear_osc();
                Some(AnsiEvent::Osc(OscSequence { command, data }))
            }
            // ESC - could be start of ST
            0x1b => {
                // Peek ahead would be nice, but for now just treat as terminator
                self.state = ParserState::Escape;
                let data = String::from_utf8_lossy(&self.osc_data).into_owned();
                let command = self.osc_command;
                self.clear_osc();
                Some(AnsiEvent::Osc(OscSequence { command, data }))
            }
            // Digit - part of command number or separator
            b'0'..=b'9' if self.osc_data.is_empty() => {
                self.osc_command = self.osc_command * 10 + (byte - b'0') as u16;
                None
            }
            b';' if self.osc_data.is_empty() => {
                // Separator between command and data
                None
            }
            // Cancel
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_osc();
                None
            }
            _ => {
                self.osc_data.push(byte);
                None
            }
        }
    }

    fn dcs_entry(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            0x07 | 0x9c => {
                self.state = ParserState::Ground;
                let data = std::mem::take(&mut self.dcs_data);
                Some(AnsiEvent::Dcs(data))
            }
            0x1b => {
                self.state = ParserState::Escape;
                let data = std::mem::take(&mut self.dcs_data);
                Some(AnsiEvent::Dcs(data))
            }
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_dcs();
                None
            }
            _ => {
                self.dcs_data.push(byte);
                self.state = ParserState::DcsPassthrough;
                None
            }
        }
    }

    fn dcs_passthrough(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            0x07 | 0x9c => {
                self.state = ParserState::Ground;
                let data = std::mem::take(&mut self.dcs_data);
                Some(AnsiEvent::Dcs(data))
            }
            0x1b => {
                self.state = ParserState::Escape;
                let data = std::mem::take(&mut self.dcs_data);
                Some(AnsiEvent::Dcs(data))
            }
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_dcs();
                None
            }
            _ => {
                self.dcs_data.push(byte);
                None
            }
        }
    }

    fn apc_string(&mut self, byte: u8) -> Option<AnsiEvent> {
        match byte {
            0x07 | 0x9c => {
                self.state = ParserState::Ground;
                let data = std::mem::take(&mut self.apc_data);
                Some(AnsiEvent::Apc(data))
            }
            0x1b => {
                self.state = ParserState::Escape;
                let data = std::mem::take(&mut self.apc_data);
                Some(AnsiEvent::Apc(data))
            }
            0x18 | 0x1a => {
                self.state = ParserState::Ground;
                self.clear_apc();
                None
            }
            _ => {
                self.apc_data.push(byte);
                None
            }
        }
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_print_characters() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"hello");

        assert_eq!(events.len(), 5);
        assert_eq!(events[0], AnsiEvent::Print('h'));
        assert_eq!(events[1], AnsiEvent::Print('e'));
        assert_eq!(events[2], AnsiEvent::Print('l'));
        assert_eq!(events[3], AnsiEvent::Print('l'));
        assert_eq!(events[4], AnsiEvent::Print('o'));
    }

    #[test]
    fn parse_cursor_up() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b[5A"); // Cursor up 5

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Csi(csi) => {
                assert_eq!(csi.final_byte, b'A');
                assert_eq!(csi.param(0, 1), 5);
                assert!(csi.is_cursor_movement());
            }
            _ => panic!("Expected CSI"),
        }
    }

    #[test]
    fn parse_cursor_position() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b[10;20H"); // Move to row 10, col 20

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Csi(csi) => {
                assert_eq!(csi.final_byte, b'H');
                assert_eq!(csi.param(0, 1), 10);
                assert_eq!(csi.param(1, 1), 20);
            }
            _ => panic!("Expected CSI"),
        }
    }

    #[test]
    fn parse_clear_screen() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b[2J"); // Clear entire screen

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Csi(csi) => {
                assert_eq!(csi.final_byte, b'J');
                assert_eq!(csi.param(0, 0), 2);
                assert!(csi.is_erase());
            }
            _ => panic!("Expected CSI"),
        }
    }

    #[test]
    fn parse_clear_line() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b[K"); // Clear to end of line

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Csi(csi) => {
                assert_eq!(csi.final_byte, b'K');
                assert_eq!(csi.param(0, 0), 0);
                assert!(csi.is_erase());
            }
            _ => panic!("Expected CSI"),
        }
    }

    #[test]
    fn parse_sgr() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b[1;31m"); // Bold red

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Csi(csi) => {
                assert_eq!(csi.final_byte, b'm');
                assert_eq!(csi.params, vec![1, 31]);
                assert!(csi.is_sgr());
            }
            _ => panic!("Expected CSI"),
        }
    }

    #[test]
    fn parse_private_mode() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b[?25h"); // Show cursor

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Csi(csi) => {
                assert_eq!(csi.final_byte, b'h');
                assert_eq!(csi.param(0, 0), 25);
                assert_eq!(csi.private_marker, Some(b'?'));
            }
            _ => panic!("Expected CSI"),
        }
    }

    #[test]
    fn parse_osc_title() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b]0;My Title\x07"); // Set window title

        assert_eq!(events.len(), 1);
        match &events[0] {
            AnsiEvent::Osc(osc) => {
                assert_eq!(osc.command, 0);
                assert_eq!(osc.data, "My Title");
            }
            _ => panic!("Expected OSC"),
        }
    }

    #[test]
    fn parse_execute_codes() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\r\n\t");

        assert_eq!(events.len(), 3);
        assert_eq!(events[0], AnsiEvent::Execute(b'\r'));
        assert_eq!(events[1], AnsiEvent::Execute(b'\n'));
        assert_eq!(events[2], AnsiEvent::Execute(b'\t'));
    }

    #[test]
    fn parse_esc_save_restore_cursor() {
        let mut parser = AnsiParser::new();
        let events = parser.parse(b"\x1b7\x1b8");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], AnsiEvent::Esc(EscSequence::SaveCursor));
        assert_eq!(events[1], AnsiEvent::Esc(EscSequence::RestoreCursor));
    }

    #[test]
    fn malformed_sequence_recovery() {
        let mut parser = AnsiParser::new();

        // Test 1: Malformed CSI that gets cancelled by CAN (0x18)
        let events1 = parser.parse(b"\x1b[\xfe\x18hello");

        // CAN cancels the malformed sequence, then "hello" should be printed
        let print_chars1: String = events1
            .iter()
            .filter_map(|e| match e {
                AnsiEvent::Print(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(print_chars1, "hello");

        // Test 2: Verify parser doesn't crash on malformed sequences
        parser.reset();
        let events2 = parser.parse(b"\x1b[\xfe\xff\x00\x01@world");

        // Should eventually recover and print something
        let print_chars2: String = events2
            .iter()
            .filter_map(|e| match e {
                AnsiEvent::Print(c) => Some(*c),
                _ => None,
            })
            .collect();
        // @ (0x40) ends the ignored sequence as it's a final byte
        // Then "world" is printed
        assert_eq!(print_chars2, "world");

        // Test 3: Multiple malformed sequences don't corrupt state
        parser.reset();
        let events3 = parser.parse(b"\x1b[\xff@\x1b[\xfe@done");
        let print_chars3: String = events3
            .iter()
            .filter_map(|e| match e {
                AnsiEvent::Print(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(print_chars3, "done");
    }

    #[test]
    fn parse_utf8() {
        let mut parser = AnsiParser::new();
        let events = parser.parse("héllo 世界".as_bytes());

        let chars: String = events
            .iter()
            .filter_map(|e| match e {
                AnsiEvent::Print(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(chars, "héllo 世界");
    }

    #[test]
    fn incremental_parsing() {
        let mut parser = AnsiParser::new();

        // Parse escape sequence in parts
        let events1 = parser.parse(b"\x1b");
        assert!(events1.is_empty());

        let events2 = parser.parse(b"[");
        assert!(events2.is_empty());

        let events3 = parser.parse(b"5");
        assert!(events3.is_empty());

        let events4 = parser.parse(b"A");
        assert_eq!(events4.len(), 1);
        assert!(matches!(&events4[0], AnsiEvent::Csi(csi) if csi.final_byte == b'A'));
    }
}
