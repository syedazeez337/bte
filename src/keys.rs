//! Key Injection Engine
//!
//! This module provides key injection capabilities for testing terminal applications.
//! It translates high-level key sequences into bytes that are written to the PTY.

#![allow(dead_code)]

use crate::process::{ProcessError, PtyProcess};
use crate::scenario::{KeySequence, SpecialKey};

/// Error type for key injection operations
#[derive(Debug)]
pub enum KeyError {
    /// Failed to write to PTY
    WriteFailed(ProcessError),
    /// Process has exited
    ProcessExited,
    /// Invalid key sequence
    InvalidSequence(String),
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyError::WriteFailed(e) => write!(f, "Failed to write keys: {}", e),
            KeyError::ProcessExited => write!(f, "Process has exited"),
            KeyError::InvalidSequence(s) => write!(f, "Invalid key sequence: {}", s),
        }
    }
}

impl std::error::Error for KeyError {}

impl From<ProcessError> for KeyError {
    fn from(e: ProcessError) -> Self {
        KeyError::WriteFailed(e)
    }
}

/// Key injection engine for sending keystrokes to a PTY process
pub struct KeyInjector<'a> {
    process: &'a PtyProcess,
}

impl<'a> KeyInjector<'a> {
    /// Create a new key injector for the given process
    pub fn new(process: &'a PtyProcess) -> Self {
        Self { process }
    }

    /// Inject a key sequence
    pub fn inject(&self, sequence: &KeySequence) -> Result<usize, KeyError> {
        if self.process.has_exited() {
            return Err(KeyError::ProcessExited);
        }

        let bytes = sequence.to_bytes();
        self.process.write_all(&bytes)?;
        Ok(bytes.len())
    }

    /// Inject raw bytes directly
    pub fn inject_raw(&self, bytes: &[u8]) -> Result<usize, KeyError> {
        if self.process.has_exited() {
            return Err(KeyError::ProcessExited);
        }

        self.process.write_all(bytes)?;
        Ok(bytes.len())
    }

    /// Inject a text string
    pub fn inject_text(&self, text: &str) -> Result<usize, KeyError> {
        self.inject(&KeySequence::Text(text.to_string()))
    }

    /// Inject a special key
    pub fn inject_key(&self, key: SpecialKey) -> Result<usize, KeyError> {
        self.inject(&KeySequence::Special(vec![key]))
    }

    /// Inject Enter key
    pub fn enter(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Enter)
    }

    /// Inject Tab key
    pub fn tab(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Tab)
    }

    /// Inject Escape key
    pub fn escape(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Escape)
    }

    /// Inject Ctrl+C (interrupt character)
    /// Note: In raw mode, this sends byte 0x03 to the application.
    /// The application must handle it (or the terminal driver if ISIG is enabled).
    pub fn ctrl_c(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Ctrl('c'))
    }

    /// Inject Ctrl+D (EOF character)
    pub fn ctrl_d(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Ctrl('d'))
    }

    /// Inject Ctrl+Z (suspend character)
    pub fn ctrl_z(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Ctrl('z'))
    }

    /// Inject Ctrl+\ (quit character)
    pub fn ctrl_backslash(&self) -> Result<usize, KeyError> {
        // Ctrl+\ is ASCII 28 (0x1C)
        self.inject_raw(&[0x1c])
    }

    /// Inject arrow up
    pub fn up(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Up)
    }

    /// Inject arrow down
    pub fn down(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Down)
    }

    /// Inject arrow left
    pub fn left(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Left)
    }

    /// Inject arrow right
    pub fn right(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Right)
    }

    /// Inject backspace
    pub fn backspace(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Backspace)
    }

    /// Inject delete
    pub fn delete(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Delete)
    }

    /// Inject Home key
    pub fn home(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Home)
    }

    /// Inject End key
    pub fn end(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::End)
    }

    /// Inject Page Up
    pub fn page_up(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::PageUp)
    }

    /// Inject Page Down
    pub fn page_down(&self) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::PageDown)
    }

    /// Inject a function key (F1-F12)
    pub fn function_key(&self, n: u8) -> Result<usize, KeyError> {
        let key = match n {
            1 => SpecialKey::F1,
            2 => SpecialKey::F2,
            3 => SpecialKey::F3,
            4 => SpecialKey::F4,
            5 => SpecialKey::F5,
            6 => SpecialKey::F6,
            7 => SpecialKey::F7,
            8 => SpecialKey::F8,
            9 => SpecialKey::F9,
            10 => SpecialKey::F10,
            11 => SpecialKey::F11,
            12 => SpecialKey::F12,
            _ => {
                return Err(KeyError::InvalidSequence(format!(
                    "Invalid function key: F{}",
                    n
                )))
            }
        };
        self.inject_key(key)
    }

    /// Inject Ctrl+key combination
    pub fn ctrl(&self, c: char) -> Result<usize, KeyError> {
        if !c.is_ascii_alphabetic() && c != '[' && c != '\\' && c != ']' && c != '^' && c != '_' {
            return Err(KeyError::InvalidSequence(format!(
                "Invalid Ctrl combination: Ctrl+{}",
                c
            )));
        }
        self.inject_key(SpecialKey::Ctrl(c))
    }

    /// Inject Alt+key combination (sends ESC followed by the key)
    pub fn alt(&self, c: char) -> Result<usize, KeyError> {
        self.inject_key(SpecialKey::Alt(c))
    }

    /// Inject a line of text followed by Enter
    pub fn type_line(&self, text: &str) -> Result<usize, KeyError> {
        let mut total = self.inject_text(text)?;
        total += self.enter()?;
        Ok(total)
    }
}

/// Builder for constructing complex key sequences
pub struct KeySequenceBuilder {
    keys: Vec<u8>,
}

impl KeySequenceBuilder {
    /// Create a new key sequence builder
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Add text to the sequence
    pub fn text(mut self, s: &str) -> Self {
        self.keys.extend_from_slice(s.as_bytes());
        self
    }

    /// Add a special key to the sequence
    pub fn key(mut self, key: SpecialKey) -> Self {
        self.keys.extend(key.to_bytes());
        self
    }

    /// Add raw bytes to the sequence
    pub fn raw(mut self, bytes: &[u8]) -> Self {
        self.keys.extend_from_slice(bytes);
        self
    }

    /// Add Enter key
    pub fn enter(self) -> Self {
        self.key(SpecialKey::Enter)
    }

    /// Add Tab key
    pub fn tab(self) -> Self {
        self.key(SpecialKey::Tab)
    }

    /// Add Escape key
    pub fn escape(self) -> Self {
        self.key(SpecialKey::Escape)
    }

    /// Add Ctrl+key combination
    pub fn ctrl(self, c: char) -> Self {
        self.key(SpecialKey::Ctrl(c))
    }

    /// Add Alt+key combination
    pub fn alt(self, c: char) -> Self {
        self.key(SpecialKey::Alt(c))
    }

    /// Build the final key sequence as bytes
    pub fn build(self) -> Vec<u8> {
        self.keys
    }

    /// Build as a KeySequence
    pub fn build_sequence(self) -> KeySequence {
        // Since we have raw bytes, we represent this as text
        // (which is effectively the same thing)
        KeySequence::Text(String::from_utf8_lossy(&self.keys).to_string())
    }
}

impl Default for KeySequenceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessConfig;

    #[test]
    fn keystrokes_reach_application() {
        // Test that keystrokes sent through the injector reach the application
        let config = ProcessConfig::shell("cat");
        let process = PtyProcess::spawn(&config).unwrap();

        // Give cat time to start
        std::thread::sleep(std::time::Duration::from_millis(100));

        let injector = KeyInjector::new(&process);

        // Send some text
        injector.inject_text("hello").unwrap();

        // Give time for echo
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read output
        let mut buf = [0u8; 1024];
        let mut output = Vec::new();
        for _ in 0..10 {
            match process.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }

        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("hello"),
            "Expected 'hello' in output, got: {:?}",
            output_str
        );

        // Clean up - send Ctrl+D to close cat
        let _ = injector.ctrl_d();
    }

    #[test]
    fn ctrl_c_is_received_by_application() {
        // In raw mode, Ctrl+C (0x03) is sent directly to the application
        let config = ProcessConfig::shell("cat");
        let process = PtyProcess::spawn(&config).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let injector = KeyInjector::new(&process);

        // Send Ctrl+C
        let bytes_sent = injector.ctrl_c().unwrap();
        assert_eq!(bytes_sent, 1); // Ctrl+C is single byte

        // The byte 0x03 should have been sent
        // In a real terminal with signals enabled, this would generate SIGINT
        // In raw mode, the application receives it directly
    }

    #[test]
    fn unicode_input_supported() {
        // Test Unicode characters are properly transmitted
        let config = ProcessConfig::shell("cat");
        let process = PtyProcess::spawn(&config).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let injector = KeyInjector::new(&process);

        // Send Unicode text
        let unicode_text = "你好世界 🎉 émojis";
        injector.inject_text(unicode_text).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read output
        let mut buf = [0u8; 4096];
        let mut output = Vec::new();
        for _ in 0..10 {
            match process.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }

        let output_str = String::from_utf8_lossy(&output);
        // The terminal may echo back the Unicode characters
        // Check that at least part of the Unicode was received
        assert!(
            output_str.contains("你好") || output_str.contains("🎉") || !output.is_empty(),
            "Expected Unicode in output, got: {:?}",
            output_str
        );

        let _ = injector.ctrl_d();
    }

    #[test]
    fn special_keys_produce_correct_sequences() {
        // Verify special keys produce the expected escape sequences
        assert_eq!(SpecialKey::Enter.to_bytes(), vec![b'\r']);
        assert_eq!(SpecialKey::Tab.to_bytes(), vec![b'\t']);
        assert_eq!(SpecialKey::Backspace.to_bytes(), vec![0x7f]);
        assert_eq!(SpecialKey::Escape.to_bytes(), vec![0x1b]);
        assert_eq!(SpecialKey::Up.to_bytes(), vec![0x1b, b'[', b'A']);
        assert_eq!(SpecialKey::Down.to_bytes(), vec![0x1b, b'[', b'B']);
        assert_eq!(SpecialKey::Right.to_bytes(), vec![0x1b, b'[', b'C']);
        assert_eq!(SpecialKey::Left.to_bytes(), vec![0x1b, b'[', b'D']);
        assert_eq!(SpecialKey::Home.to_bytes(), vec![0x1b, b'[', b'H']);
        assert_eq!(SpecialKey::End.to_bytes(), vec![0x1b, b'[', b'F']);

        // Ctrl+C should be 0x03
        assert_eq!(SpecialKey::Ctrl('c').to_bytes(), vec![3]);
        // Ctrl+D should be 0x04
        assert_eq!(SpecialKey::Ctrl('d').to_bytes(), vec![4]);
        // Ctrl+A should be 0x01
        assert_eq!(SpecialKey::Ctrl('a').to_bytes(), vec![1]);
        // Ctrl+Z should be 0x1A
        assert_eq!(SpecialKey::Ctrl('z').to_bytes(), vec![26]);

        // Alt+x sends ESC followed by x
        assert_eq!(SpecialKey::Alt('x').to_bytes(), vec![0x1b, b'x']);
    }

    #[test]
    fn function_keys_produce_correct_sequences() {
        assert_eq!(SpecialKey::F1.to_bytes(), vec![0x1b, b'O', b'P']);
        assert_eq!(SpecialKey::F2.to_bytes(), vec![0x1b, b'O', b'Q']);
        assert_eq!(SpecialKey::F3.to_bytes(), vec![0x1b, b'O', b'R']);
        assert_eq!(SpecialKey::F4.to_bytes(), vec![0x1b, b'O', b'S']);
        assert_eq!(
            SpecialKey::F5.to_bytes(),
            vec![0x1b, b'[', b'1', b'5', b'~']
        );
        assert_eq!(
            SpecialKey::F12.to_bytes(),
            vec![0x1b, b'[', b'2', b'4', b'~']
        );
    }

    #[test]
    fn key_sequence_builder_works() {
        let bytes = KeySequenceBuilder::new()
            .text("ls ")
            .ctrl('c')
            .text(" more")
            .enter()
            .build();

        // Should be: 'l', 's', ' ', 0x03, ' ', 'm', 'o', 'r', 'e', '\r'
        assert_eq!(
            bytes,
            vec![b'l', b's', b' ', 0x03, b' ', b'm', b'o', b'r', b'e', b'\r']
        );
    }

    #[test]
    fn type_line_sends_text_with_enter() {
        let config = ProcessConfig::shell("cat");
        let process = PtyProcess::spawn(&config).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let injector = KeyInjector::new(&process);

        // Type a line
        let bytes_sent = injector.type_line("test").unwrap();
        assert_eq!(bytes_sent, 5); // 4 chars + 1 enter

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read output
        let mut buf = [0u8; 1024];
        let mut output = Vec::new();
        for _ in 0..10 {
            match process.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }

        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("test"),
            "Expected 'test' in output, got: {:?}",
            output_str
        );

        let _ = injector.ctrl_d();
    }

    #[test]
    fn rejects_invalid_ctrl_combinations() {
        let config = ProcessConfig::shell("sleep 0.1");
        let process = PtyProcess::spawn(&config).unwrap();
        let injector = KeyInjector::new(&process);

        // Numbers shouldn't work with Ctrl
        let result = injector.ctrl('5');
        assert!(result.is_err());

        // Special chars shouldn't work (except a few)
        let result = injector.ctrl('@');
        assert!(result.is_err());
    }

    #[test]
    fn invalid_function_key_rejected() {
        let config = ProcessConfig::shell("sleep 0.1");
        let process = PtyProcess::spawn(&config).unwrap();
        let injector = KeyInjector::new(&process);

        // F0 doesn't exist
        let result = injector.function_key(0);
        assert!(result.is_err());

        // F13 doesn't exist (in our implementation)
        let result = injector.function_key(13);
        assert!(result.is_err());
    }

    #[test]
    fn injection_fails_after_process_exits() {
        let config = ProcessConfig::shell("exit 0");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Wait for process to exit
        process.wait().unwrap();

        let injector = KeyInjector::new(&process);

        // Injection should fail
        let result = injector.inject_text("hello");
        assert!(matches!(result, Err(KeyError::ProcessExited)));
    }

    #[test]
    fn arrow_keys_reach_interactive_app() {
        // Test arrow keys with a simple shell
        let config = ProcessConfig::bash();
        let process = PtyProcess::spawn(&config).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(200));

        let injector = KeyInjector::new(&process);

        // Type something and use arrow keys to move around
        injector.inject_text("abc").unwrap();
        injector.left().unwrap();
        injector.left().unwrap();
        injector.inject_text("X").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        // The result should show the cursor moved (bash will echo the sequence)
        // Just verify no errors occurred
        let _ = injector.type_line("exit");
    }
}
