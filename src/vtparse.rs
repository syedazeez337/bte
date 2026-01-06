//! VTParser-based ANSI Escape Sequence Parser
//!
//! This module provides an incremental parser for ANSI escape sequences,
//! based on the DEC ANSI Parser state machine. It uses the Handler trait
//! pattern to process different types of sequences.
//!
//! Reference: https://vt100.net/emu/dec_ansi_parser

use std::fmt;

/// Parser state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    OscString,
    SosPmString,
    ApcString,
    Anywhere,
    Utf8Sequence,
}

/// Actions performed during state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Ignore,
    Print,
    Execute,
    Clear,
    Collect,
    Param,
    EscDispatch,
    CsiDispatch,
    Hook,
    Put,
    Unhook,
    OscStart,
    OscPut,
    OscEnd,
    Utf8,
    ApcStart,
    ApcPut,
    ApcEnd,
}

/// Maximum number of intermediate bytes
const MAX_INTERMEDIATES: usize = 2;
/// Maximum OSC string length
const MAX_OSC: usize = 64;
/// Maximum number of parameters
const MAX_PARAMS: usize = 256;

/// Represents a parameter to a CSI-based escape sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsiParam {
    Integer(i64),
    ColonList(Vec<Option<i64>>),
}

impl CsiParam {
    pub fn value(&self) -> i64 {
        match self {
            CsiParam::Integer(v) => *v,
            CsiParam::ColonList(v) => v.first().copied().flatten().unwrap_or(0),
        }
    }
}

/// Handler trait for processing parsed events
pub trait Handler {
    fn print(&mut self, ch: char);
    fn execute(&mut self, control: u8);
    fn hook(&mut self, params: &[CsiParam], intermediates: &[u8], ignored_excess: bool, byte: u8);
    fn put(&mut self, byte: u8);
    fn unhook(&mut self);
    fn esc_dispatch(
        &mut self,
        params: &[i64],
        intermediates: &[u8],
        ignored_excess: bool,
        byte: u8,
    );
    fn csi_dispatch(&mut self, params: &[CsiParam], ignored_excess: bool, byte: u8);
    fn osc_start(&mut self);
    fn osc_put(&mut self, byte: u8);
    fn osc_end(&mut self);
    fn apc_start(&mut self);
    fn apc_put(&mut self, byte: u8);
    fn apc_end(&mut self);
}

/// Default handler that ignores all events
impl<H: Handler> Handler for std::marker::PhantomData<H> {
    fn print(&mut self, _ch: char) {}
    fn execute(&mut self, _control: u8) {}
    fn hook(
        &mut self,
        _params: &[CsiParam],
        _intermediates: &[u8],
        _ignored_excess: bool,
        _byte: u8,
    ) {
    }
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn esc_dispatch(
        &mut self,
        _params: &[i64],
        _intermediates: &[u8],
        _ignored_excess: bool,
        _byte: u8,
    ) {
    }
    fn csi_dispatch(&mut self, _params: &[CsiParam], _ignored_excess: bool, _byte: u8) {}
    fn osc_start(&mut self) {}
    fn osc_put(&mut self, _byte: u8) {}
    fn osc_end(&mut self) {}
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _byte: u8) {}
    fn apc_end(&mut self) {}
}

/// Incremental ANSI parser based on DEC ANSI Parser
pub struct Parser<H: Handler> {
    handler: H,
    state: State,
    intermediates: [u8; MAX_INTERMEDIATES],
    num_intermediates: usize,
    ignored_excess_intermediates: bool,
    osc_buffer: Vec<u8>,
    osc_params: Vec<usize>,
    params: [CsiParam; MAX_PARAMS],
    num_params: usize,
    current_param: Option<i64>,
    params_truncated: bool,
    utf8_parser: Utf8Parser,
}

impl<H: Handler> Parser<H> {
    /// Create a new parser with the given handler
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            state: State::Ground,
            intermediates: [0; MAX_INTERMEDIATES],
            num_intermediates: 0,
            ignored_excess_intermediates: false,
            osc_buffer: Vec::with_capacity(256),
            osc_params: Vec::with_capacity(16),
            params: [const { CsiParam::Integer(0) }; MAX_PARAMS],
            num_params: 0,
            current_param: None,
            params_truncated: false,
            utf8_parser: Utf8Parser::new(),
        }
    }

    /// Get reference to handler
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Get mutable reference to handler
    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.handler
    }

    /// Consume the parser and return the handler
    pub fn into_handler(self) -> H {
        self.handler
    }

    /// Reset parser to ground state
    pub fn reset(&mut self) {
        self.state = State::Ground;
        self.num_intermediates = 0;
        self.ignored_excess_intermediates = false;
        self.osc_buffer.clear();
        self.osc_params.clear();
        self.num_params = 0;
        self.current_param = None;
        self.params_truncated = false;
    }

    /// Parse a single byte
    pub fn parse_byte(&mut self, byte: u8) {
        self.transition(byte);
    }

    /// Parse multiple bytes
    pub fn parse(&mut self, data: &[u8]) {
        for &byte in data {
            self.parse_byte(byte);
        }
    }

    fn transition(&mut self, byte: u8) {
        match self.state {
            State::Ground => self.state_ground(byte),
            State::Escape => self.state_escape(byte),
            State::EscapeIntermediate => self.state_escape_intermediate(byte),
            State::CsiEntry => self.state_csi_entry(byte),
            State::CsiParam => self.state_csi_param(byte),
            State::CsiIntermediate => self.state_csi_intermediate(byte),
            State::CsiIgnore => self.state_csi_ignore(byte),
            State::DcsEntry => self.state_dcs_entry(byte),
            State::DcsParam => self.state_dcs_param(byte),
            State::DcsIntermediate => self.state_dcs_intermediate(byte),
            State::DcsPassthrough => self.state_dcs_passthrough(byte),
            State::DcsIgnore => self.state_dcs_ignore(byte),
            State::OscString => self.state_osc_string(byte),
            State::SosPmString => self.state_sos_pm_string(byte),
            State::ApcString => self.state_apc_string(byte),
            State::Anywhere => self.state_anywhere(byte),
            State::Utf8Sequence => self.state_utf8_sequence(byte),
        }
    }

    fn state_ground(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
            }
            0x1b => {
                self.state = State::Escape;
            }
            0x20..=0x7e => {
                self.handler.print(byte as char);
            }
            0x7f => {
                // DEL - ignore
            }
            0x80..=0x9f => {
                // C1 controls
                self.handler.execute(byte);
            }
            _ => {
                // High bytes - treat as print
                self.handler.print(byte as char);
            }
        }
    }

    fn state_escape(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
                self.state = State::Ground;
            }
            0x1b => {
                // ESC in escape - stay in escape
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
                self.state = State::EscapeIntermediate;
            }
            0x5b => {
                // ESC [ - enter CSI mode
                self.clear_params();
                self.state = State::CsiEntry;
            }
            0x30..=0x3f => {
                self.handler.esc_dispatch(
                    &[],
                    &self.intermediates[..self.num_intermediates],
                    false,
                    byte,
                );
                self.state = State::Ground;
            }
            0x40..=0x5a | 0x5c..=0x5f => {
                let params: Vec<i64> = self
                    .osc_params
                    .windows(2)
                    .map(|w| {
                        let start = w[0];
                        let end = w[1];
                        self.osc_buffer[start..end]
                            .iter()
                            .fold(0i64, |acc, &b| acc * 10 + (b - b'0') as i64)
                    })
                    .collect();
                self.osc_buffer.clear();
                self.osc_params.clear();
                self.handler.esc_dispatch(
                    &params,
                    &self.intermediates[..self.num_intermediates],
                    false,
                    byte,
                );
                self.state = State::Ground;
            }
            0x60..=0x7e => {
                let params: Vec<i64> = self
                    .osc_params
                    .windows(2)
                    .map(|w| {
                        let start = w[0];
                        let end = w[1];
                        self.osc_buffer[start..end]
                            .iter()
                            .fold(0i64, |acc, &b| acc * 10 + (b - b'0') as i64)
                    })
                    .collect();
                self.osc_buffer.clear();
                self.osc_params.clear();
                self.handler.esc_dispatch(
                    &params,
                    &self.intermediates[..self.num_intermediates],
                    false,
                    byte,
                );
                self.state = State::Ground;
            }
            _ => {
                self.state = State::Ground;
            }
        }
    }

    fn state_escape_intermediate(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
                self.state = State::Ground;
            }
            0x1b => {
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
            }
            0x30..=0x7e => {
                let params: Vec<i64> = self
                    .osc_params
                    .windows(2)
                    .map(|w| {
                        let start = w[0];
                        let end = w[1];
                        self.osc_buffer[start..end]
                            .iter()
                            .fold(0i64, |acc, &b| acc * 10 + (b - b'0') as i64)
                    })
                    .collect();
                self.osc_buffer.clear();
                self.osc_params.clear();
                self.handler.esc_dispatch(
                    &params,
                    &self.intermediates[..self.num_intermediates],
                    self.ignored_excess_intermediates,
                    byte,
                );
                self.num_intermediates = 0;
                self.ignored_excess_intermediates = false;
                self.state = State::Ground;
            }
            _ => {
                self.num_intermediates = 0;
                self.ignored_excess_intermediates = false;
                self.state = State::Ground;
            }
        }
    }

    fn state_csi_entry(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
                self.state = State::CsiIntermediate;
            }
            0x30..=0x3f => {
                self.collect_param(byte);
                self.state = State::CsiParam;
            }
            0x40..=0x7e => {
                self.dispatch_csi(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            _ => {
                self.clear_params();
                self.state = State::CsiIgnore;
            }
        }
    }

    fn state_csi_param(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
                self.state = State::CsiIntermediate;
            }
            0x3b => {
                if let Some(param) = self.current_param.take() {
                    if self.num_params < MAX_PARAMS {
                        self.params[self.num_params] = CsiParam::Integer(param);
                        self.num_params += 1;
                    }
                }
                self.current_param = None;
            }
            0x30..=0x3a | 0x3c..=0x3f => {
                self.collect_param(byte);
            }
            0x40..=0x7e => {
                self.dispatch_csi(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            _ => {
                self.clear_params();
                self.state = State::CsiIgnore;
            }
        }
    }

    fn state_csi_intermediate(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
            }
            0x40..=0x7e => {
                self.dispatch_csi(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            _ => {
                self.clear_params();
                self.state = State::CsiIgnore;
            }
        }
    }

    fn state_csi_ignore(&mut self, byte: u8) {
        match byte {
            0x00..=0x1a | 0x1c..=0x1f => {
                self.handler.execute(byte);
                self.clear_params();
                self.state = State::Ground;
            }
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x40..=0x7e => {
                self.clear_params();
                self.state = State::Ground;
            }
            _ => {}
        }
    }

    fn state_dcs_entry(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
                self.state = State::DcsIntermediate;
            }
            0x30..=0x3f => {
                self.collect_param(byte);
                self.state = State::DcsParam;
            }
            0x40..=0x7e => {
                self.hook();
                self.clear_params();
                self.state = State::DcsPassthrough;
            }
            _ => {
                self.clear_params();
                self.state = State::DcsIgnore;
            }
        }
    }

    fn state_dcs_param(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
                self.state = State::DcsIntermediate;
            }
            0x30..=0x3f => {
                self.collect_param(byte);
            }
            0x40..=0x7e => {
                self.hook();
                self.clear_params();
                self.state = State::DcsPassthrough;
            }
            _ => {
                self.clear_params();
                self.state = State::DcsIgnore;
            }
        }
    }

    fn state_dcs_intermediate(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.clear_params();
                self.state = State::Escape;
            }
            0x20..=0x2f => {
                self.collect_intermediate(byte);
            }
            0x40..=0x7e => {
                self.hook();
                self.clear_params();
                self.state = State::DcsPassthrough;
            }
            _ => {
                self.clear_params();
                self.state = State::DcsIgnore;
            }
        }
    }

    fn state_dcs_passthrough(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.unhook();
                self.state = State::Escape;
            }
            0x07 => {
                self.unhook();
                self.state = State::Ground;
            }
            _ => {
                self.put(byte);
            }
        }
    }

    fn state_dcs_ignore(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.state = State::Escape;
            }
            0x07 => {
                self.state = State::Ground;
            }
            _ => {}
        }
    }

    fn state_osc_string(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.osc_end();
                self.state = State::Escape;
            }
            0x07 => {
                self.osc_end();
                self.state = State::Ground;
            }
            0x20..=0x7f => {
                if byte == b';' && self.osc_params.len() < MAX_OSC {
                    self.osc_params.push(self.osc_buffer.len());
                } else if byte < 0x20 || byte > 0x7f {
                    // Ignore control characters
                } else {
                    self.osc_buffer.push(byte);
                }
            }
            _ => {
                // Ignore
            }
        }
    }

    fn state_sos_pm_string(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.state = State::Escape;
            }
            0x07 => {
                self.state = State::Ground;
            }
            _ => {
                // Ignore
            }
        }
    }

    fn state_apc_string(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.apc_end();
                self.state = State::Escape;
            }
            0x07 => {
                self.apc_end();
                self.state = State::Ground;
            }
            _ => {
                self.apc_put(byte);
            }
        }
    }

    fn state_anywhere(&mut self, byte: u8) {
        // This state is used for special handling
        self.state_ground(byte);
    }

    fn state_utf8_sequence(&mut self, byte: u8) {
        // UTF-8 handling would go here
        self.state_ground(byte);
    }

    fn collect_intermediate(&mut self, byte: u8) {
        if self.num_intermediates < MAX_INTERMEDIATES {
            self.intermediates[self.num_intermediates] = byte;
            self.num_intermediates += 1;
        } else {
            self.ignored_excess_intermediates = true;
        }
    }

    fn collect_param(&mut self, byte: u8) {
        if self.num_params < MAX_PARAMS {
            if let Some(ref mut param) = self.current_param {
                if byte == b':' {
                    // Colon separator - handle sub-parameters
                } else if byte >= b'0' && byte <= b'9' {
                    *param = *param * 10 + (byte - b'0') as i64;
                }
            } else {
                if byte >= b'0' && byte <= b'9' {
                    self.current_param = Some((byte - b'0') as i64);
                    self.params[self.num_params] = CsiParam::Integer(self.current_param.unwrap());
                } else if byte == b':' {
                    self.current_param = Some(0);
                    self.params[self.num_params] = CsiParam::ColonList(vec![Some(0)]);
                }
            }
        } else {
            self.params_truncated = true;
        }
    }

    fn clear_params(&mut self) {
        self.num_params = 0;
        self.current_param = None;
        self.params_truncated = false;
        self.num_intermediates = 0;
        self.ignored_excess_intermediates = false;
    }

    fn dispatch_csi(&mut self, byte: u8) {
        // Finalize current parameter if any
        if let Some(param) = self.current_param.take() {
            if self.num_params < MAX_PARAMS {
                self.params[self.num_params] = CsiParam::Integer(param);
                self.num_params += 1;
            }
        }

        let params: &[CsiParam] = &self.params[..self.num_params];
        self.handler
            .csi_dispatch(params, self.params_truncated, byte);
    }

    fn hook(&mut self) {
        let params: Vec<CsiParam> = (0..self.num_params)
            .filter_map(|i| match self.params[i] {
                CsiParam::Integer(v) => Some(CsiParam::Integer(v)),
                CsiParam::ColonList(ref v) => Some(CsiParam::ColonList(v.clone())),
            })
            .collect();
        self.handler.hook(
            &params,
            &self.intermediates[..self.num_intermediates],
            self.ignored_excess_intermediates,
            0,
        );
    }

    fn put(&mut self, byte: u8) {
        self.handler.put(byte);
    }

    fn unhook(&mut self) {
        self.handler.unhook();
    }

    fn osc_end(&mut self) {
        // Parse OSC parameters and data
        let mut osc_params: Vec<&[u8]> = Vec::new();
        let mut last_idx = 0;
        for &idx in &self.osc_params {
            if idx > 0 {
                osc_params.push(&self.osc_buffer[last_idx..idx - 1]);
            }
            last_idx = idx;
        }
        if last_idx < self.osc_buffer.len() {
            osc_params.push(&self.osc_buffer[last_idx..]);
        }

        // Call handler
        self.handler.osc_end();
    }

    fn apc_start(&mut self) {
        self.handler.apc_start();
    }

    fn apc_put(&mut self, byte: u8) {
        self.handler.apc_put(byte);
    }

    fn apc_end(&mut self) {
        self.handler.apc_end();
    }
}

/// Simple UTF-8 parser for processing multi-byte sequences
#[derive(Debug, Clone, Default)]
pub struct Utf8Parser {
    buffer: Vec<u8>,
    expected: u8,
}

impl Utf8Parser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.expected = 0;
    }

    pub fn push(&mut self, byte: u8) -> Option<char> {
        if byte < 0x80 {
            // ASCII
            return Some(byte as char);
        }

        self.buffer.push(byte);

        // Determine expected continuation bytes
        if byte >= 0xc0 && byte < 0xe0 {
            self.expected = 1;
        } else if byte >= 0xe0 && byte < 0xf0 {
            self.expected = 2;
        } else if byte >= 0xf0 && byte < 0xf8 {
            self.expected = 3;
        } else {
            self.expected = 0;
        }

        // Check if we have all expected bytes
        if self.buffer.len() as u8 == self.expected + 1 {
            if let Ok(s) = std::str::from_utf8(&self.buffer) {
                if let Some(ch) = s.chars().next() {
                    self.buffer.clear();
                    self.expected = 0;
                    return Some(ch);
                }
            }
        }

        None
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Ground => write!(f, "Ground"),
            State::Escape => write!(f, "Escape"),
            State::EscapeIntermediate => write!(f, "EscapeIntermediate"),
            State::CsiEntry => write!(f, "CsiEntry"),
            State::CsiParam => write!(f, "CsiParam"),
            State::CsiIntermediate => write!(f, "CsiIntermediate"),
            State::CsiIgnore => write!(f, "CsiIgnore"),
            State::DcsEntry => write!(f, "DcsEntry"),
            State::DcsParam => write!(f, "DcsParam"),
            State::DcsIntermediate => write!(f, "DcsIntermediate"),
            State::DcsPassthrough => write!(f, "DcsPassthrough"),
            State::DcsIgnore => write!(f, "DcsIgnore"),
            State::OscString => write!(f, "OscString"),
            State::SosPmString => write!(f, "SosPmString"),
            State::ApcString => write!(f, "ApcString"),
            State::Anywhere => write!(f, "Anywhere"),
            State::Utf8Sequence => write!(f, "Utf8Sequence"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestHandler {
        prints: Vec<char>,
        executes: Vec<u8>,
        csi_dispatches: Vec<(Vec<CsiParam>, u8)>,
        esc_dispatches: Vec<(Vec<i64>, u8)>,
    }

    impl Handler for TestHandler {
        fn print(&mut self, ch: char) {
            self.prints.push(ch);
        }
        fn execute(&mut self, control: u8) {
            self.executes.push(control);
        }
        fn hook(
            &mut self,
            _params: &[CsiParam],
            _intermediates: &[u8],
            _ignored_excess: bool,
            _byte: u8,
        ) {
        }
        fn put(&mut self, _byte: u8) {}
        fn unhook(&mut self) {}
        fn esc_dispatch(
            &mut self,
            params: &[i64],
            _intermediates: &[u8],
            _ignored_excess: bool,
            byte: u8,
        ) {
            self.esc_dispatches.push((params.to_vec(), byte));
        }
        fn csi_dispatch(&mut self, params: &[CsiParam], _ignored_excess: bool, byte: u8) {
            self.csi_dispatches.push((params.to_vec(), byte));
        }
        fn osc_start(&mut self) {}
        fn osc_put(&mut self, _byte: u8) {}
        fn osc_end(&mut self) {}
        fn apc_start(&mut self) {}
        fn apc_put(&mut self, _byte: u8) {}
        fn apc_end(&mut self) {}
    }

    #[test]
    fn test_parse_print() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"hello");
        let handler = parser.into_handler();

        assert_eq!(handler.prints, vec!['h', 'e', 'l', 'l', 'o']);
    }

    #[test]
    fn test_parse_cursor_up() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\x1b[5A");
        let handler = parser.into_handler();

        assert_eq!(handler.csi_dispatches.len(), 1);
        let (params, final_byte) = &handler.csi_dispatches[0];
        assert_eq!(*final_byte, b'A');
        assert_eq!(params.len(), 1);
        match params[0] {
            CsiParam::Integer(v) => assert_eq!(v, 5),
            _ => panic!("Expected integer param"),
        }
    }

    #[test]
    fn test_parse_cursor_position() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\x1b[10;20H");
        let handler = parser.into_handler();

        assert_eq!(handler.csi_dispatches.len(), 1);
        let (params, final_byte) = &handler.csi_dispatches[0];
        assert_eq!(*final_byte, b'H');
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_parse_clear_screen() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\x1b[2J");
        let handler = parser.into_handler();

        assert_eq!(handler.csi_dispatches.len(), 1);
        let (params, final_byte) = &handler.csi_dispatches[0];
        assert_eq!(*final_byte, b'J');
    }

    #[test]
    fn test_parse_sgr() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\x1b[1;31m");
        let handler = parser.into_handler();

        assert_eq!(handler.csi_dispatches.len(), 1);
        let (params, final_byte) = &handler.csi_dispatches[0];
        assert_eq!(*final_byte, b'm');
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_parse_esc_save_restore_cursor() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\x1b7\x1b8");
        let handler = parser.into_handler();

        assert_eq!(handler.esc_dispatches.len(), 2);
        assert_eq!(handler.esc_dispatches[0], (vec![], b'7'));
        assert_eq!(handler.esc_dispatches[1], (vec![], b'8'));
    }

    #[test]
    fn test_parse_execute_codes() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\r\n\t");
        let handler = parser.into_handler();

        assert_eq!(handler.executes, vec![b'\r', b'\n', b'\t']);
    }

    #[test]
    fn test_incremental_parsing() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);

        parser.parse(b"\x1b");
        assert!(parser.into_handler().csi_dispatches.is_empty());

        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"[");
        assert!(parser.into_handler().csi_dispatches.is_empty());

        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"5");
        assert!(parser.into_handler().csi_dispatches.is_empty());

        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);
        parser.parse(b"\x1b[5A");
        assert_eq!(parser.into_handler().csi_dispatches.len(), 1);
    }

    #[test]
    fn test_state_transitions() {
        let handler = TestHandler::default();
        let mut parser = Parser::new(handler);

        assert_eq!(parser.state, State::Ground);

        parser.parse_byte(0x1b);
        assert_eq!(parser.state, State::Escape);

        parser.parse_byte(b'[');
        assert_eq!(parser.state, State::CsiEntry);

        parser.parse_byte(b'5');
        assert_eq!(parser.state, State::CsiParam);

        parser.parse_byte(b'A');
        assert_eq!(parser.state, State::Ground);
    }
}
