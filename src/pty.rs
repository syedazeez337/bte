//! PTY (Pseudo-Terminal) creation and ownership
//!
//! This module provides low-level PTY operations for controlling terminal applications.
//! It owns the terminal rather than simulating it.

use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::pty::{openpty, OpenptyResult, Winsize};
use nix::sys::termios::{self, LocalFlags, SetArg, Termios};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

/// Error type for PTY operations
#[derive(Debug)]
pub enum PtyError {
    /// Failed to allocate PTY pair
    AllocationFailed(nix::Error),
    /// Failed to configure terminal settings
    ConfigurationFailed(nix::Error),
    /// Failed to set non-blocking mode
    NonBlockingFailed(nix::Error),
    /// Failed to close file descriptor
    CloseFailed(nix::Error),
    /// PTY has been closed
    Closed,
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyError::AllocationFailed(e) => write!(f, "PTY allocation failed: {}", e),
            PtyError::ConfigurationFailed(e) => write!(f, "PTY configuration failed: {}", e),
            PtyError::NonBlockingFailed(e) => write!(f, "Failed to set non-blocking mode: {}", e),
            PtyError::CloseFailed(e) => write!(f, "Failed to close PTY: {}", e),
            PtyError::Closed => write!(f, "PTY has been closed"),
        }
    }
}

impl std::error::Error for PtyError {}

/// Configuration for PTY creation
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Initial window size (columns, rows)
    pub size: (u16, u16),
    /// Enable raw mode (disable line buffering, echo, etc.)
    pub raw_mode: bool,
    /// Set non-blocking I/O on the master fd
    pub non_blocking: bool,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            size: (80, 24),
            raw_mode: true,
            non_blocking: true,
        }
    }
}

/// A PTY master/slave pair
///
/// The master side is used by the test harness to read/write to the terminal.
/// The slave side is given to the child process as its controlling terminal.
pub struct Pty {
    /// Master file descriptor (our end)
    master: Option<OwnedFd>,
    /// Slave file descriptor (child's end)
    slave: Option<OwnedFd>,
    /// Original termios settings (for restoration)
    original_termios: Option<Termios>,
    /// Current window size
    size: (u16, u16),
}

impl Pty {
    /// Create a new PTY pair with the given configuration.
    pub fn open(config: &PtyConfig) -> Result<Self, PtyError> {
        let winsize = Winsize {
            ws_row: config.size.1,
            ws_col: config.size.0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // Allocate PTY pair - nix 0.29+ returns OwnedFd directly
        let OpenptyResult { master, slave } =
            openpty(&winsize, None).map_err(PtyError::AllocationFailed)?;

        let mut pty = Self {
            master: Some(master),
            slave: Some(slave),
            original_termios: None,
            size: config.size,
        };

        // Configure raw mode if requested
        if config.raw_mode {
            pty.set_raw_mode()?;
        }

        // Set non-blocking if requested
        if config.non_blocking {
            pty.set_non_blocking()?;
        }

        Ok(pty)
    }

    /// Create a PTY with default configuration.
    pub fn open_default() -> Result<Self, PtyError> {
        Self::open(&PtyConfig::default())
    }

    /// Get the master file descriptor (for reading/writing).
    pub fn master_fd(&self) -> Result<RawFd, PtyError> {
        self.master
            .as_ref()
            .map(|fd| fd.as_raw_fd())
            .ok_or(PtyError::Closed)
    }

    /// Get a borrowed reference to the master fd.
    pub fn master_borrowed(&self) -> Result<BorrowedFd<'_>, PtyError> {
        self.master
            .as_ref()
            .map(|fd| fd.as_fd())
            .ok_or(PtyError::Closed)
    }

    /// Get the slave file descriptor (for the child process).
    pub fn slave_fd(&self) -> Result<RawFd, PtyError> {
        self.slave
            .as_ref()
            .map(|fd| fd.as_raw_fd())
            .ok_or(PtyError::Closed)
    }

    /// Get a borrowed reference to the slave fd.
    pub fn slave_borrowed(&self) -> Result<BorrowedFd<'_>, PtyError> {
        self.slave
            .as_ref()
            .map(|fd| fd.as_fd())
            .ok_or(PtyError::Closed)
    }

    /// Take ownership of the slave fd (for forking to child).
    ///
    /// After calling this, the slave fd is no longer owned by this Pty.
    /// This is used when forking a child process.
    pub fn take_slave(&mut self) -> Result<OwnedFd, PtyError> {
        self.slave.take().ok_or(PtyError::Closed)
    }

    /// Close the slave fd from the parent process.
    ///
    /// Should be called after forking, in the parent process.
    pub fn close_slave(&mut self) -> Result<(), PtyError> {
        // Simply dropping the OwnedFd will close it
        self.slave = None;
        Ok(())
    }

    /// Set raw mode on the slave terminal.
    ///
    /// In raw mode:
    /// - No input processing (no line buffering)
    /// - No echo
    /// - No signal generation from special characters
    fn set_raw_mode(&mut self) -> Result<(), PtyError> {
        let slave_fd = self.slave.as_ref().ok_or(PtyError::Closed)?;

        // Get current termios
        let mut termios = termios::tcgetattr(slave_fd).map_err(PtyError::ConfigurationFailed)?;

        // Save original for potential restoration
        self.original_termios = Some(termios.clone());

        // Disable canonical mode and echo
        termios.local_flags.remove(LocalFlags::ICANON); // No line buffering
        termios.local_flags.remove(LocalFlags::ECHO); // No echo
        termios.local_flags.remove(LocalFlags::ECHOE); // No echo erase
        termios.local_flags.remove(LocalFlags::ECHOK); // No echo kill
        termios.local_flags.remove(LocalFlags::ECHONL); // No echo newline
        termios.local_flags.remove(LocalFlags::ISIG); // No signal generation

        // Apply settings - need to get reference again after mutation
        let slave_fd = self.slave.as_ref().ok_or(PtyError::Closed)?;
        termios::tcsetattr(slave_fd, SetArg::TCSANOW, &termios)
            .map_err(PtyError::ConfigurationFailed)?;

        Ok(())
    }

    /// Set non-blocking I/O on the master fd.
    fn set_non_blocking(&self) -> Result<(), PtyError> {
        let master_fd = self.master_fd()?;

        let flags = fcntl(master_fd, FcntlArg::F_GETFL).map_err(PtyError::NonBlockingFailed)?;

        let new_flags = OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK;

        fcntl(master_fd, FcntlArg::F_SETFL(new_flags)).map_err(PtyError::NonBlockingFailed)?;

        Ok(())
    }

    /// Check if the master fd is still valid (PTY hasn't been closed).
    pub fn is_open(&self) -> bool {
        self.master.is_some()
    }

    /// Get the current window size.
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// Resize the PTY window.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let master_fd = self.master_fd()?;

        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // TIOCSWINSZ ioctl
        unsafe {
            let ret = libc::ioctl(master_fd, libc::TIOCSWINSZ, &winsize);
            if ret < 0 {
                return Err(PtyError::ConfigurationFailed(nix::Error::last()));
            }
        }

        self.size = (cols, rows);
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // OwnedFd handles closing automatically
        // Just clear our references
        self.master = None;
        self.slave = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_allocate_pty() {
        let pty = Pty::open_default();
        assert!(pty.is_ok(), "Failed to allocate PTY: {:?}", pty.err());
    }

    #[test]
    fn pty_has_valid_fds() {
        let pty = Pty::open_default().unwrap();
        assert!(pty.master_fd().is_ok());
        assert!(pty.slave_fd().is_ok());
        assert!(pty.master_fd().unwrap() >= 0);
        assert!(pty.slave_fd().unwrap() >= 0);
    }

    #[test]
    fn pty_survives_operations() {
        let mut pty = Pty::open_default().unwrap();

        // Resize should work
        assert!(pty.resize(120, 40).is_ok());
        assert_eq!(pty.size(), (120, 40));

        // Should still be open
        assert!(pty.is_open());
    }

    #[test]
    fn pty_custom_config() {
        let config = PtyConfig {
            size: (132, 50),
            raw_mode: true,
            non_blocking: true,
        };

        let pty = Pty::open(&config).unwrap();
        assert_eq!(pty.size(), (132, 50));
    }

    #[test]
    fn non_blocking_read_returns_eagain() {
        let pty = Pty::open_default().unwrap();
        let master_fd = pty.master_fd().unwrap();

        // Try to read from master - should return EAGAIN since there's no data
        let mut buf = [0u8; 128];
        let result = nix::unistd::read(master_fd, &mut buf);

        match result {
            Err(nix::Error::EAGAIN) => {
                // Expected - non-blocking with no data
            }
            Ok(0) => {
                // Also acceptable - EOF
            }
            other => {
                // Some systems might behave differently, but this is the expected path
                panic!("Unexpected read result: {:?}", other);
            }
        }
    }
}
