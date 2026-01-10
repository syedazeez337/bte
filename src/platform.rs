//! Platform abstraction layer for cross-platform terminal testing support.
//
// This module provides abstractions over platform-specific terminal operations,
// enabling support for Linux, macOS, and (in the future) Windows.
//
// Current State:
// - Linux: Full support via nix crate
// - macOS: Partial support (see implementation notes)
// - Windows: Not yet implemented (ConPTY support planned)
//
// Design Principles:
// 1. Trait-based abstraction for terminal operations
// 2. Platform-specific implementations behind a unified interface
// 3. Graceful degradation with clear error messages for unsupported features

use std::path::PathBuf;

/// Signal types supported across platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Interrupt (Ctrl+C)
    Sigint,
    /// Termination request
    Sigterm,
    /// Force kill (cannot be caught)
    Sigkill,
    /// Window size change
    Sigwinch,
    /// Stop process (can be trapped)
    Sigstop,
    /// Continue stopped process
    Sigcont,
    /// Hangup (useful for daemon testing)
    Sighup,
    /// User-defined signal 1
    Sigusr1,
    /// User-defined signal 2
    Sigusr2,
}

impl Signal {
    /// Get the signal name as a string
    pub fn name(&self) -> &'static str {
        match self {
            Signal::Sigint => "SIGINT",
            Signal::Sigterm => "SIGTERM",
            Signal::Sigkill => "SIGKILL",
            Signal::Sigwinch => "SIGWINCH",
            Signal::Sigstop => "SIGSTOP",
            Signal::Sigcont => "SIGCONT",
            Signal::Sighup => "SIGHUP",
            Signal::Sigusr1 => "SIGUSR1",
            Signal::Sigusr2 => "SIGUSR2",
        }
    }

    /// Check if this signal can be trapped/handled
    pub fn is_trappable(&self) -> bool {
        match self {
            Signal::Sigkill | Signal::Sigstop => false,
            _ => true,
        }
    }
}

/// Terminal dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

/// Environment variable
#[derive(Debug, Clone)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// Platform-specific capabilities
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlatformCapabilities {
    /// Supports mouse tracking (SGR 1006)
    pub mouse_tracking: bool,
    /// Supports truecolor (24-bit color)
    pub truecolor: bool,
    /// Supports 256 colors
    pub256_colors: bool,
    /// Supports hyperlinks (OSC 8)
    pub hyperlinks: bool,
    /// Supports-bracketed paste
    pub bracketed_paste: bool,
    /// Supports focus reporting
    pub focus_reporting: bool,
    /// Supports synchronized output (DECSET 2026)
    pub synchronized_output: bool,
}

impl PlatformCapabilities {
    /// Get capabilities for the current platform
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                mouse_tracking: true,
                truecolor: true,
                pub256_colors: true,
                hyperlinks: true,
                bracketed_paste: true,
                focus_reporting: true,
                synchronized_output: true,
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS Terminal and iTerm2 have good support
            Self {
                mouse_tracking: true,
                truecolor: true,
                pub256_colors: true,
                hyperlinks: true,
                bracketed_paste: true,
                focus_reporting: false, // Limited support
                synchronized_output: false, // Limited support
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self::default()
        }
    }
}

/// Configuration for spawning a terminal process
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Program to execute
    pub program: PathBuf,
    /// Command-line arguments
    pub args: Vec<String>,
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// Environment variables
    pub env: Vec<EnvVar>,
    /// Terminal size
    pub size: TerminalSize,
    /// Enable raw mode
    pub raw_mode: bool,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            program: PathBuf::from("/bin/sh"),
            args: vec!["sh".to_string()],
            cwd: None,
            env: vec![],
            size: TerminalSize::default(),
            raw_mode: true,
        }
    }
}

/// Result of a process spawn operation
pub struct SpawnResult {
    /// Handle to the spawned process
    pub process: Box<dyn TerminalProcess>,
}

/// Trait for platform-agnostic terminal process operations
pub trait TerminalProcess: Send {
    /// Write data to the terminal input
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error>;

    /// Read available data from terminal output
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;

    /// Check if the process is still running
    fn is_running(&self) -> bool;

    /// Send a signal to the process
    fn send_signal(&mut self, signal: Signal) -> Result<(), std::io::Error>;

    /// Resize the terminal
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), std::io::Error>;

    /// Wait for the process to exit and get exit status
    fn wait(&mut self) -> Result<ExitStatus, std::io::Error>;

    /// Try to get exit status without blocking
    fn try_wait(&mut self) -> Result<Option<ExitStatus>, std::io::Error>;

    /// Get the process ID
    fn pid(&self) -> Option<u32>;

    /// Check if EOF has been reached on output
    fn eof(&self) -> bool;
}

/// Exit status of a terminal process
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitStatus {
    /// Process exited with a code
    Exited(i32),
    /// Process was terminated by a signal
    Signaled(i32),
    /// Process is still running
    Running,
}

impl ExitStatus {
    /// Check if the process exited successfully
    pub fn success(&self) -> bool {
        matches!(self, ExitStatus::Exited(0))
    }

    /// Get the signal name if terminated by signal
    pub fn signal_name(&self) -> Option<&'static str> {
        match self {
            ExitStatus::Signaled(sig) => Some(Signal::from_errno(*sig)?.name()),
            _ => None,
        }
    }
}

impl Signal {
    /// Create a signal from errno number (Unix signal number)
    #[cfg(unix)]
    pub fn from_errno(errno: i32) -> Option<Self> {
        match errno {
            libc::SIGINT => Some(Signal::Sigint),
            libc::SIGTERM => Some(Signal::Sigterm),
            libc::SIGKILL => Some(Signal::Sigkill),
            libc::SIGWINCH => Some(Signal::Sigwinch),
            libc::SIGSTOP => Some(Signal::Sigstop),
            libc::SIGCONT => Some(Signal::Sigcont),
            libc::SIGHUP => Some(Signal::Sighup),
            libc::SIGUSR1 => Some(Signal::Sigusr1),
            libc::SIGUSR2 => Some(Signal::Sigusr2),
            _ => None,
        }
    }

    /// Get the errno number for this signal
    #[cfg(unix)]
    pub fn to_errno(&self) -> i32 {
        match self {
            Signal::Sigint => libc::SIGINT,
            Signal::Sigterm => libc::SIGTERM,
            Signal::Sigkill => libc::SIGKILL,
            Signal::Sigwinch => libc::SIGWINCH,
            Signal::Sigstop => libc::SIGSTOP,
            Signal::Sigcont => libc::SIGCONT,
            Signal::Sighup => libc::SIGHUP,
            Signal::Sigusr1 => libc::SIGUSR1,
            Signal::Sigusr2 => libc::SIGUSR2,
        }
    }
}

/// Trait for platform-specific terminal backend operations
pub trait TerminalBackend: Send {
    /// Create a new terminal backend
    fn new() -> Result<Self, PlatformError>
    where
        Self: Sized;

    /// Spawn a new terminal process
    fn spawn(&self, config: &SpawnConfig) -> Result<SpawnResult, PlatformError>;

    /// Get platform capabilities
    fn capabilities(&self) -> PlatformCapabilities;

    /// Get platform name
    fn name(&self) -> &'static str;
}

/// Errors that can occur in platform operations
#[derive(Debug, thiserror::Error)]
#[error("{kind}: {source}")]
pub struct PlatformError {
    source: Box<dyn std::error::Error + Send + Sync>,
    kind: PlatformErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformErrorKind {
    #[error("PTY allocation failed")]
    PtyAllocation,

    #[error("Process spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Signal delivery failed: {0}")]
    SignalFailed(String),

    #[error("Terminal resize failed: {0}")]
    ResizeFailed(String),

    #[error("Process wait failed: {0}")]
    WaitFailed(String),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Timeout waiting for process")]
    Timeout,
}

/// Get the current platform's terminal backend
///
/// Returns a platform-specific implementation
pub fn get_backend() -> Result<Box<dyn TerminalBackend>, PlatformError> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::LinuxTerminalBackend;
        LinuxTerminalBackend::new().map(|b| Box::new(b) as Box<dyn TerminalBackend>)
    }

    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::MacOSTerminalBackend;
        MacOSTerminalBackend::new().map(|b| Box::new(b) as Box<dyn TerminalBackend>)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(PlatformError {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("Platform {} is not yet supported", std::env::consts::OS),
            )),
            kind: PlatformErrorKind::UnsupportedPlatform(
                std::env::consts::OS.to_string(),
            ),
        })
    }
}

/// Check if running on a supported platform
pub fn is_supported() -> bool {
    cfg!(target_os = "linux") || cfg!(target_os = "macos")
}

/// Get a human-readable platform description
pub fn platform_description() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "Linux (full support)"
    }

    #[cfg(target_os = "macos")]
    {
        "macOS (experimental support)"
    }

    #[cfg(target_os = "windows")]
    {
        "Windows (not yet implemented - ConPTY support planned)"
    }

    #[cfg(target_os = "freebsd")]
    {
        "FreeBSD (not yet implemented)"
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "freebsd"
    )))]
    {
        "Unknown platform"
    }
}

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

// Re-export platform types for library users
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use {
    linux::{LinuxTerminalBackend, LinuxTerminalProcess},
    macos::{MacOSTerminalBackend, MacOSTerminalProcess},
};
