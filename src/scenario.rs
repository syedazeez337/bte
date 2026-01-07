//! Scenario Schema Definition
//!
//! This module provides a declarative format for defining interaction scenarios.
//! No imperative scripting is allowed - all interactions are declared as data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete test scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Scenario name
    pub name: String,

    /// Scenario description
    #[serde(default)]
    pub description: String,

    /// Command to run
    pub command: Command,

    /// Terminal configuration
    #[serde(default)]
    pub terminal: TerminalConfig,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Sequence of steps to execute
    pub steps: Vec<Step>,

    /// Invariants to check throughout execution
    #[serde(default)]
    pub invariants: Vec<InvariantRef>,

    /// Random seed for deterministic replay
    #[serde(default)]
    pub seed: Option<u64>,

    /// Timeout in milliseconds
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Command to execute
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Command {
    /// Simple command string
    Simple(String),
    /// Full command specification
    Full {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
    },
}

impl Command {
    pub fn program(&self) -> &str {
        match self {
            Command::Simple(_) => "/bin/sh",
            Command::Full { program, .. } => program,
        }
    }

    pub fn args(&self) -> Vec<String> {
        match self {
            Command::Simple(s) => vec!["sh".to_string(), "-c".to_string(), s.clone()],
            Command::Full { program, args, .. } => {
                let mut result = vec![program.clone()];
                result.extend(args.clone());
                result
            }
        }
    }
}

/// Terminal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Number of columns
    #[serde(default = "default_cols")]
    pub cols: u16,

    /// Number of rows
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: default_cols(),
            rows: default_rows(),
        }
    }
}

/// A single step in the scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum Step {
    /// Wait for output matching a pattern
    #[serde(rename = "wait_for")]
    WaitFor {
        /// Pattern to match (regex)
        pattern: String,
        /// Timeout in milliseconds
        #[serde(default)]
        timeout_ms: Option<u64>,
    },

    /// Wait for a specific number of logical ticks
    #[serde(rename = "wait_ticks")]
    WaitTicks {
        /// Number of ticks to wait
        ticks: u64,
    },

    /// Send keystrokes
    #[serde(rename = "send_keys")]
    SendKeys {
        /// Keys to send (can include escape sequences)
        keys: KeySequence,
    },

    /// Send a signal to the process
    #[serde(rename = "send_signal")]
    SendSignal {
        /// Signal name (SIGINT, SIGTERM, SIGKILL, SIGWINCH)
        signal: SignalName,
    },

    /// Resize the terminal
    #[serde(rename = "resize")]
    Resize {
        /// New column count
        cols: u16,
        /// New row count
        rows: u16,
    },

    /// Assert screen content matches pattern
    #[serde(rename = "assert_screen")]
    AssertScreen {
        /// Pattern to match
        pattern: String,
        /// Whether to match anywhere on screen (default) or exact position
        #[serde(default)]
        anywhere: bool,
        /// Row to check (0-indexed, if not matching anywhere)
        #[serde(default)]
        row: Option<usize>,
    },

    /// Assert cursor is at position
    #[serde(rename = "assert_cursor")]
    AssertCursor {
        /// Expected row (0-indexed)
        row: usize,
        /// Expected column (0-indexed)
        col: usize,
    },

    /// Capture a snapshot of the screen state
    #[serde(rename = "snapshot")]
    Snapshot {
        /// Name for this snapshot
        name: String,
    },

    /// Check an invariant
    #[serde(rename = "check_invariant")]
    CheckInvariant {
        /// Invariant to check
        invariant: InvariantRef,
    },
}

/// Key sequence to send
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeySequence {
    /// Plain text
    Text(String),
    /// Special keys
    Special(Vec<SpecialKey>),
}

/// Special key names
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    /// Ctrl + key
    Ctrl(char),
    /// Alt + key
    Alt(char),
}

impl SpecialKey {
    /// Convert to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SpecialKey::Enter => vec![b'\r'],
            SpecialKey::Tab => vec![b'\t'],
            SpecialKey::Backspace => vec![0x7f],
            SpecialKey::Escape => vec![0x1b],
            SpecialKey::Up => vec![0x1b, b'[', b'A'],
            SpecialKey::Down => vec![0x1b, b'[', b'B'],
            SpecialKey::Right => vec![0x1b, b'[', b'C'],
            SpecialKey::Left => vec![0x1b, b'[', b'D'],
            SpecialKey::Home => vec![0x1b, b'[', b'H'],
            SpecialKey::End => vec![0x1b, b'[', b'F'],
            SpecialKey::PageUp => vec![0x1b, b'[', b'5', b'~'],
            SpecialKey::PageDown => vec![0x1b, b'[', b'6', b'~'],
            SpecialKey::Insert => vec![0x1b, b'[', b'2', b'~'],
            SpecialKey::Delete => vec![0x1b, b'[', b'3', b'~'],
            SpecialKey::F1 => vec![0x1b, b'O', b'P'],
            SpecialKey::F2 => vec![0x1b, b'O', b'Q'],
            SpecialKey::F3 => vec![0x1b, b'O', b'R'],
            SpecialKey::F4 => vec![0x1b, b'O', b'S'],
            SpecialKey::F5 => vec![0x1b, b'[', b'1', b'5', b'~'],
            SpecialKey::F6 => vec![0x1b, b'[', b'1', b'7', b'~'],
            SpecialKey::F7 => vec![0x1b, b'[', b'1', b'8', b'~'],
            SpecialKey::F8 => vec![0x1b, b'[', b'1', b'9', b'~'],
            SpecialKey::F9 => vec![0x1b, b'[', b'2', b'0', b'~'],
            SpecialKey::F10 => vec![0x1b, b'[', b'2', b'1', b'~'],
            SpecialKey::F11 => vec![0x1b, b'[', b'2', b'3', b'~'],
            SpecialKey::F12 => vec![0x1b, b'[', b'2', b'4', b'~'],
            SpecialKey::Ctrl(c) => {
                let byte = (*c as u8).to_ascii_lowercase() - b'a' + 1;
                vec![byte]
            }
            SpecialKey::Alt(c) => {
                vec![0x1b, *c as u8]
            }
        }
    }
}

impl KeySequence {
    /// Convert to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            KeySequence::Text(s) => s.as_bytes().to_vec(),
            KeySequence::Special(keys) => keys.iter().flat_map(|k| k.to_bytes()).collect(),
        }
    }
}

/// Signal names
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SignalName {
    Sigint,
    Sigterm,
    Sigkill,
    Sigwinch,
    Sigstop,
    Sigcont,
}

impl SignalName {
    /// Convert to nix Signal
    #[allow(clippy::wrong_self_convention)]
    pub fn to_nix_signal(self) -> nix::sys::signal::Signal {
        use nix::sys::signal::Signal;
        match self {
            SignalName::Sigint => Signal::SIGINT,
            SignalName::Sigterm => Signal::SIGTERM,
            SignalName::Sigkill => Signal::SIGKILL,
            SignalName::Sigwinch => Signal::SIGWINCH,
            SignalName::Sigstop => Signal::SIGSTOP,
            SignalName::Sigcont => Signal::SIGCONT,
        }
    }
}

/// Reference to an invariant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InvariantRef {
    /// Cursor must stay within bounds
    #[serde(rename = "cursor_bounds")]
    CursorBounds,

    /// No deadlock detection
    #[serde(rename = "no_deadlock")]
    NoDeadlock {
        /// Timeout in milliseconds for detecting deadlock
        #[serde(default)]
        timeout_ms: Option<u64>,
    },

    /// Signal must be handled
    #[serde(rename = "signal_handled")]
    SignalHandled {
        /// Signal that must be handled
        signal: SignalName,
    },

    /// Screen content must match pattern
    #[serde(rename = "screen_contains")]
    ScreenContains {
        /// Pattern to match
        pattern: String,
    },

    /// Screen content must not match pattern
    #[serde(rename = "screen_not_contains")]
    ScreenNotContains {
        /// Pattern to not match
        pattern: String,
    },

    /// No output after process exits
    #[serde(rename = "no_output_after_exit")]
    NoOutputAfterExit,

    /// Process must terminate cleanly
    #[serde(rename = "process_terminated_cleanly")]
    ProcessTerminatedCleanly {
        /// Allowed signal numbers for clean exit
        #[serde(default)]
        allowed_signals: Vec<i32>,
    },

    /// Screen must be stable (not changing)
    #[serde(rename = "screen_stability")]
    ScreenStability {
        /// Minimum ticks of stability required
        #[serde(default = "default_stable_ticks")]
        min_ticks: u64,
    },

    /// Viewport must have valid dimensions
    #[serde(rename = "viewport_valid")]
    ViewportValid,

    /// Response time must be within limit
    #[serde(rename = "response_time")]
    ResponseTime {
        /// Maximum ticks before considered timeout
        max_ticks: u64,
    },

    /// Maximum redraw latency
    #[serde(rename = "max_latency")]
    MaxLatency {
        /// Maximum ticks for screen redraw
        max_ticks: u64,
    },

    /// Custom named invariant
    #[serde(rename = "custom")]
    Custom {
        /// Invariant name
        name: String,
    },
}

fn default_stable_ticks() -> u64 {
    10
}

/// Validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
    pub path: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for ValidationError {}

impl Scenario {
    /// Load a scenario from YAML
    pub fn _from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Load a scenario from JSON
    pub fn _from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to YAML
    pub fn _to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Serialize to JSON
    pub fn _to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Validate the scenario
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate name
        if self.name.is_empty() {
            errors.push(ValidationError {
                message: "Scenario name cannot be empty".to_string(),
                path: "name".to_string(),
            });
        }

        // Validate terminal config
        if self.terminal.cols == 0 {
            errors.push(ValidationError {
                message: "Terminal columns must be > 0".to_string(),
                path: "terminal.cols".to_string(),
            });
        }
        if self.terminal.rows == 0 {
            errors.push(ValidationError {
                message: "Terminal rows must be > 0".to_string(),
                path: "terminal.rows".to_string(),
            });
        }

        // Validate steps
        if self.steps.is_empty() {
            errors.push(ValidationError {
                message: "Scenario must have at least one step".to_string(),
                path: "steps".to_string(),
            });
        }

        for (i, step) in self.steps.iter().enumerate() {
            self.validate_step(step, &format!("steps[{}]", i), &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_step(&self, step: &Step, path: &str, errors: &mut Vec<ValidationError>) {
        match step {
            Step::WaitFor { pattern, .. } => {
                if pattern.is_empty() {
                    errors.push(ValidationError {
                        message: "Pattern cannot be empty".to_string(),
                        path: format!("{}.pattern", path),
                    });
                }
            }
            Step::WaitTicks { ticks } => {
                if *ticks == 0 {
                    errors.push(ValidationError {
                        message: "Ticks must be > 0".to_string(),
                        path: format!("{}.ticks", path),
                    });
                }
            }
            Step::Resize { cols, rows } => {
                if *cols == 0 {
                    errors.push(ValidationError {
                        message: "Resize cols must be > 0".to_string(),
                        path: format!("{}.cols", path),
                    });
                }
                if *rows == 0 {
                    errors.push(ValidationError {
                        message: "Resize rows must be > 0".to_string(),
                        path: format!("{}.rows", path),
                    });
                }
            }
            Step::Snapshot { name } => {
                if name.is_empty() {
                    errors.push(ValidationError {
                        message: "Snapshot name cannot be empty".to_string(),
                        path: format!("{}.name", path),
                    });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yaml_scenario() {
        let yaml = r#"
name: "test scenario"
description: "A test"
command: "echo hello"
steps:
  - action: wait_for
    pattern: "hello"
  - action: send_keys
    keys: "exit\n"
"#;

        let scenario = Scenario::_from_yaml(yaml).unwrap();
        assert_eq!(scenario.name, "test scenario");
        assert_eq!(scenario.steps.len(), 2);
    }

    #[test]
    fn parse_json_scenario() {
        let json = r#"{
  "name": "test scenario",
  "command": "echo hello",
  "steps": [
    {"action": "wait_for", "pattern": "hello"},
    {"action": "send_keys", "keys": "exit\n"}
  ]
}"#;

        let scenario = Scenario::_from_json(json).unwrap();
        assert_eq!(scenario.name, "test scenario");
        assert_eq!(scenario.steps.len(), 2);
    }

    #[test]
    fn validate_empty_name() {
        let scenario = Scenario {
            name: "".to_string(),
            description: String::new(),
            command: Command::Simple("echo".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitTicks { ticks: 1 }],
            invariants: vec![],
            seed: None,
            timeout_ms: None,
        };

        let result = scenario.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.path == "name"));
    }

    #[test]
    fn validate_empty_steps() {
        let scenario = Scenario {
            name: "test".to_string(),
            description: String::new(),
            command: Command::Simple("echo".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![],
            invariants: vec![],
            seed: None,
            timeout_ms: None,
        };

        let result = scenario.validate();
        assert!(result.is_err());
    }

    #[test]
    fn validate_valid_scenario() {
        let scenario = Scenario {
            name: "test".to_string(),
            description: String::new(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![
                Step::WaitFor {
                    pattern: "hello".to_string(),
                    timeout_ms: None,
                },
                Step::SendKeys {
                    keys: KeySequence::Text("exit\n".to_string()),
                },
            ],
            invariants: vec![InvariantRef::CursorBounds],
            seed: Some(42),
            timeout_ms: Some(5000),
        };

        assert!(scenario.validate().is_ok());
    }

    #[test]
    fn invalid_scenario_rejected() {
        let yaml = r#"
name: ""
command: "echo"
steps:
  - action: wait_ticks
    ticks: 0
"#;

        let scenario = Scenario::_from_yaml(yaml).unwrap();
        let result = scenario.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.len() >= 2); // Empty name and zero ticks
    }

    #[test]
    fn schema_is_diffable() {
        let scenario1 = Scenario {
            name: "test1".to_string(),
            description: String::new(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitTicks { ticks: 1 }],
            invariants: vec![],
            seed: None,
            timeout_ms: None,
        };

        let scenario2 = Scenario {
            name: "test2".to_string(),
            description: String::new(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig::default(),
            env: HashMap::new(),
            steps: vec![Step::WaitTicks { ticks: 1 }],
            invariants: vec![],
            seed: None,
            timeout_ms: None,
        };

        // Both should serialize to readable YAML that can be diffed
        let yaml1 = scenario1._to_yaml().unwrap();
        let yaml2 = scenario2._to_yaml().unwrap();

        // They should be similar but with different names
        assert!(yaml1.contains("test1"));
        assert!(yaml2.contains("test2"));
        assert!(!yaml1.contains("test2"));
        assert!(!yaml2.contains("test1"));
    }

    #[test]
    fn no_imperative_scripting() {
        // The schema is purely declarative - no functions, no conditionals
        // This test verifies the structure is data-only
        let yaml = r#"
name: "declarative only"
command: "echo"
steps:
  - action: send_keys
    keys: "hello"
  - action: wait_for
    pattern: "hello"
  - action: assert_screen
    pattern: "hello"
    anywhere: true
"#;

        let scenario = Scenario::_from_yaml(yaml).unwrap();
        // All steps are pure data declarations
        for step in &scenario.steps {
            match step {
                Step::SendKeys { .. }
                | Step::WaitFor { .. }
                | Step::AssertScreen { .. }
                | Step::WaitTicks { .. }
                | Step::SendSignal { .. }
                | Step::Resize { .. }
                | Step::AssertCursor { .. }
                | Step::Snapshot { .. }
                | Step::CheckInvariant { .. } => {}
            }
        }
    }

    #[test]
    fn special_keys_work() {
        assert_eq!(SpecialKey::Enter.to_bytes(), vec![b'\r']);
        assert_eq!(SpecialKey::Ctrl('c').to_bytes(), vec![3]); // Ctrl+C is 0x03
        assert_eq!(SpecialKey::Up.to_bytes(), vec![0x1b, b'[', b'A']);
    }

    #[test]
    fn roundtrip_yaml() {
        let scenario = Scenario {
            name: "roundtrip".to_string(),
            description: "Test roundtrip".to_string(),
            command: Command::Simple("echo hello".to_string()),
            terminal: TerminalConfig {
                cols: 120,
                rows: 40,
            },
            env: {
                let mut m = HashMap::new();
                m.insert("TERM".to_string(), "xterm".to_string());
                m
            },
            steps: vec![
                Step::WaitFor {
                    pattern: "hello".to_string(),
                    timeout_ms: Some(1000),
                },
                Step::SendKeys {
                    keys: KeySequence::Text("test".to_string()),
                },
            ],
            invariants: vec![InvariantRef::CursorBounds],
            seed: Some(12345),
            timeout_ms: Some(5000),
        };

        let yaml = scenario._to_yaml().unwrap();
        let parsed = Scenario::_from_yaml(&yaml).unwrap();

        assert_eq!(scenario.name, parsed.name);
        assert_eq!(scenario.terminal.cols, parsed.terminal.cols);
        assert_eq!(scenario.seed, parsed.seed);
    }
}
