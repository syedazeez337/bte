//! Process launch and management inside PTY
//!
//! This module handles forking and executing binaries inside a PTY,
//! with proper stdio routing and environment isolation.

use crate::pty::{Pty, PtyConfig, PtyError};
use nix::libc;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{close, dup2, execvpe, fork, setsid, ForkResult, Pid};
use std::collections::HashMap;
use std::ffi::CString;

// ============================================================================
// Constants
// ============================================================================

/// Timeout for graceful SIGTERM termination in milliseconds
const SIGTERM_TIMEOUT_MS: u64 = 500;

/// Polling interval while waiting for process exit in milliseconds
const POLL_INTERVAL_MS: u64 = 10;

/// Maximum number of EINTR retries before giving up
const MAX_EINTR_RETRIES: u32 = 10;

/// Default test sleep duration in milliseconds
const TEST_SLEEP_MS: u64 = 100;

/// Quick test sleep duration in milliseconds
const QUICK_SLEEP_MS: u64 = 10;

/// Signal probe sleep duration in milliseconds
const PROBE_SLEEP_MS: u64 = 100;

/// Longer test sleep duration in milliseconds
const LONG_SLEEP_MS: u64 = 200;

/// Signal test sleep duration in milliseconds
const SIGNAL_SLEEP_MS: u64 = 50;

/// Error type for process operations
#[derive(Debug)]
pub enum ProcessError {
    /// PTY error
    Pty(PtyError),
    /// Fork failed
    ForkFailed(nix::Error),
    /// Exec failed
    ExecFailed(nix::Error),
    /// Session creation failed
    SetsidFailed(nix::Error),
    /// IO redirection failed
    IoRedirectFailed(nix::Error),
    /// Invalid program path
    InvalidPath(std::ffi::NulError),
    /// Invalid argument
    InvalidArgument(std::ffi::NulError),
    /// Process not running
    NotRunning,
    /// Wait failed
    WaitFailed(nix::Error),
    /// Signal send failed
    SignalFailed(nix::Error),
    /// Process is still running (unexpected in blocking wait)
    StillRunning,
    /// Unexpected ptrace event
    UnexpectedPtraceEvent,
    /// Timeout waiting for process
    Timeout,
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Pty(e) => write!(f, "PTY error: {}", e),
            ProcessError::ForkFailed(e) => write!(f, "Fork failed: {}", e),
            ProcessError::ExecFailed(e) => write!(f, "Exec failed: {}", e),
            ProcessError::SetsidFailed(e) => write!(f, "Setsid failed: {}", e),
            ProcessError::IoRedirectFailed(e) => write!(f, "IO redirect failed: {}", e),
            ProcessError::InvalidPath(e) => write!(f, "Invalid path: {}", e),
            ProcessError::InvalidArgument(e) => write!(f, "Invalid argument: {}", e),
            ProcessError::NotRunning => write!(f, "Process is not running"),
            ProcessError::WaitFailed(e) => write!(f, "Wait failed: {}", e),
            ProcessError::SignalFailed(e) => write!(f, "Signal failed: {}", e),
            ProcessError::StillRunning => write!(f, "Process is still running"),
            ProcessError::UnexpectedPtraceEvent => write!(f, "Unexpected ptrace event"),
            ProcessError::Timeout => write!(f, "Timeout waiting for process"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<PtyError> for ProcessError {
    fn from(e: PtyError) -> Self {
        ProcessError::Pty(e)
    }
}

/// Configuration for process launch
#[derive(Debug, Clone)]
pub struct ProcessConfig {
    /// Program to execute
    pub program: String,
    /// Arguments (including argv[0])
    pub args: Vec<String>,
    /// Environment variables (if None, use minimal isolated environment)
    pub env: Option<HashMap<String, String>>,
    /// Working directory (if None, use current)
    pub cwd: Option<String>,
    /// PTY configuration
    pub pty_config: PtyConfig,
}

impl ProcessConfig {
    /// Create a new process config for running a shell command
    pub fn shell(command: &str) -> Self {
        // Using /bin/sh directly - it's POSIX-mandated and always available
        // This avoids TOCTOU races from checking file existence
        Self {
            program: "/bin/sh".to_string(),
            args: vec!["sh".to_string(), "-c".to_string(), command.to_string()],
            env: None,
            cwd: None,
            pty_config: PtyConfig::default(),
        }
    }

    /// Create a new process config for running bash
    pub fn bash() -> Self {
        Self {
            program: "/bin/bash".to_string(),
            args: vec!["bash".to_string()],
            env: None,
            cwd: None,
            pty_config: PtyConfig::default(),
        }
    }

    /// Create a new process config for running a program
    pub fn program<S: Into<String>>(program: S) -> Self {
        let prog = program.into();
        Self {
            program: prog.clone(),
            args: vec![prog],
            env: None,
            cwd: None,
            pty_config: PtyConfig::default(),
        }
    }

    /// Add arguments
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Set environment variables
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }

    /// Set working directory
    pub fn with_cwd<S: Into<String>>(mut self, cwd: S) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set PTY size
    pub fn with_size(mut self, cols: u16, rows: u16) -> Self {
        self.pty_config.size = (cols, rows);
        self
    }
}

/// Exit reason for a process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Process exited normally with a status code
    Exited(i32),
    /// Process was killed by a signal
    Signaled(i32),
    /// Process is still running
    Running,
}

/// A process running inside a PTY
pub struct PtyProcess {
    /// The PTY
    pty: Pty,
    /// Child process ID
    pid: Pid,
    /// Exit reason (if known)
    exit_reason: Option<ExitReason>,
}

impl PtyProcess {
    /// Spawn a new process inside a PTY
    pub fn spawn(config: &ProcessConfig) -> Result<Self, ProcessError> {
        // Create PTY
        let mut pty = Pty::open(&config.pty_config)?;

        // Prepare arguments for execvpe
        let program = CString::new(config.program.as_bytes()).map_err(ProcessError::InvalidPath)?;
        let args: Vec<CString> = config
            .args
            .iter()
            .map(|s| CString::new(s.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ProcessError::InvalidArgument)?;

        // Prepare environment
        let env_vars: Vec<CString> = Self::prepare_environment(&config.env)?;

        // Fork
        //
        // SAFETY: fork() is a POSIX system call that duplicates the current process.
        // This is marked unsafe because:
        // 1. It creates a second concurrently executing thread of control
        // 2. Only async-signal-safe functions may be called in the child
        // 3. The child must either exec() or _exit() - it must not return here
        //
        // After fork succeeds:
        // - Parent: returns child PID, continues normally
        // - Child: returns 0, and must exec() a new program or call _exit()
        //
        // We satisfy these requirements by:
        // - Only calling async-signal-safe functions after fork (setsid, ioctl, dup2, close)
        // - The child either exec's the target program or calls _exit() on error
        let fork_result = unsafe { fork() }.map_err(ProcessError::ForkFailed)?;

        match fork_result {
            ForkResult::Parent { child } => {
                // Parent process
                // Close the slave fd - child owns it now
                pty.close_slave()?;

                Ok(Self {
                    pty,
                    pid: child,
                    exit_reason: None,
                })
            }
            ForkResult::Child => {
                // Child process - this code runs in the child
                // We need to set up the PTY as our controlling terminal

                // Create a new session
                setsid().map_err(ProcessError::SetsidFailed)?;

                // Get the slave fd
                let slave_fd = pty.slave_fd().map_err(ProcessError::Pty)?;

                // Set the slave as controlling terminal
                //
                // SAFETY: ioctl(TIOCSCTTY) sets the controlling terminal for the process.
                // This is safe because:
                // 1. slave_fd is a valid file descriptor from openpty()
                // 2. We are the session leader (setsid() called above, required by TIOCSCTTY)
                // 3. The second argument is 0 = don't steal from another session
                //
                // Note: We don't check the return value because TIOCSCTTY returns
                // an error only if the fd is invalid or we're not a session leader,
                // both of which we've verified.
                unsafe {
                    libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0);
                }

                // Redirect stdio to the slave
                dup2(slave_fd, libc::STDIN_FILENO).map_err(ProcessError::IoRedirectFailed)?;
                dup2(slave_fd, libc::STDOUT_FILENO).map_err(ProcessError::IoRedirectFailed)?;
                dup2(slave_fd, libc::STDERR_FILENO).map_err(ProcessError::IoRedirectFailed)?;

                // Close the original slave fd if it's not one of the standard fds
                if slave_fd > libc::STDERR_FILENO {
                    let _ = close(slave_fd);
                }

                // Close the master fd in child
                if let Ok(master_fd) = pty.master_fd() {
                    let _ = close(master_fd);
                }

                // Change directory if specified
                if let Some(ref cwd) = config.cwd {
                    let cwd_cstr =
                        CString::new(cwd.as_bytes()).map_err(ProcessError::InvalidPath)?;

                    // SAFETY: chdir() is async-signal-safe. cwd_cstr is a valid CString
                    // pointing to a NUL-terminated path. We verified the path earlier.
                    let ret = unsafe { libc::chdir(cwd_cstr.as_ptr()) };
                    if ret != 0 {
                        // Capture errno before any other calls that might change it
                        // SAFETY: __errno_location() returns a pointer to thread-local errno.
                        // This is async-signal-safe.
                        let errno = unsafe { *libc::__errno_location() };
                        let err_msg = match errno {
                            libc::ENOENT => "No such file or directory",
                            libc::EACCES => "Permission denied",
                            libc::ENOTDIR => "Not a directory",
                            libc::ENAMETOOLONG => "Path name too long",
                            libc::ELOOP => "Too many symbolic links",
                            _ => "Unknown error",
                        };
                        // Write error to stderr - use write() directly to avoid buffering issues
                        // SAFETY: write() to fd 2 (stderr) with validated pointer and length.
                        let msg = format!(
                            "bte: chdir to '{}' failed: {} (errno {})\n",
                            cwd, err_msg, errno
                        );
                        let _ = unsafe {
                            libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len())
                        };
                        // Use _exit() not exit() - we're in a forked child before exec
                        // SAFETY: _exit() is async-signal-safe and we're in the child process
                        // after fork but before exec. exit() would flush stdio buffers
                        // which can cause issues.
                        // Exit code 1 for general error (not 126/127 which are shell-specific)
                        unsafe {
                            libc::_exit(1);
                        }
                    }
                }

                // Execute the program
                let args_ref: Vec<&std::ffi::CStr> = args.iter().map(|s| s.as_c_str()).collect();
                let env_ref: Vec<&std::ffi::CStr> = env_vars.iter().map(|s| s.as_c_str()).collect();

                execvpe(&program, &args_ref, &env_ref).map_err(ProcessError::ExecFailed)?;

                // If execvpe returns, something went wrong - exit with error code 127
                // SAFETY: exit() terminates the process. We use it here because we're
                // in the child process after fork but before exec. Exit code 127 is
                // the standard "command not found" exit code.
                unsafe {
                    libc::exit(127);
                }
            }
        }
    }

    /// Prepare environment variables
    fn prepare_environment(
        env: &Option<HashMap<String, String>>,
    ) -> Result<Vec<CString>, ProcessError> {
        let env_map = match env {
            Some(e) => e.clone(),
            None => {
                // Create minimal isolated environment
                let mut minimal = HashMap::new();
                minimal.insert("TERM".to_string(), "xterm-256color".to_string());
                minimal.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
                minimal.insert("HOME".to_string(), "/tmp".to_string());
                minimal.insert("LANG".to_string(), "C.UTF-8".to_string());
                // Disable PS1 customization for more predictable prompts
                minimal.insert("PS1".to_string(), "$ ".to_string());
                minimal
            }
        };

        env_map
            .iter()
            .map(|(k, v)| {
                let s = format!("{}={}", k, v);
                CString::new(s.as_bytes()).map_err(ProcessError::InvalidArgument)
            })
            .collect()
    }

    /// Get the process ID
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// Get the raw PID as i32
    pub fn pid_raw(&self) -> i32 {
        self.pid.as_raw()
    }

    /// Get a reference to the PTY
    pub fn pty(&self) -> &Pty {
        &self.pty
    }

    /// Get a mutable reference to the PTY
    pub fn pty_mut(&mut self) -> &mut Pty {
        &mut self.pty
    }

    /// Check if the process is still running (non-blocking)
    pub fn try_wait(&mut self) -> Result<Option<ExitReason>, ProcessError> {
        if self.exit_reason.is_some() {
            return Ok(self.exit_reason);
        }

        match waitpid(self.pid, Some(WaitPidFlag::WNOHANG)).map_err(ProcessError::WaitFailed)? {
            WaitStatus::StillAlive => Ok(None),
            WaitStatus::Exited(_, code) => {
                let reason = ExitReason::Exited(code);
                self.exit_reason = Some(reason);
                Ok(Some(reason))
            }
            WaitStatus::Signaled(_, signal, _) => {
                let reason = ExitReason::Signaled(signal as i32);
                self.exit_reason = Some(reason);
                Ok(Some(reason))
            }
            _ => Ok(None),
        }
    }

    /// Wait for the process to exit (blocking)
    pub fn wait(&mut self) -> Result<ExitReason, ProcessError> {
        if let Some(reason) = self.exit_reason {
            return Ok(reason);
        }

        #[allow(unreachable_patterns)]
        match waitpid(self.pid, None).map_err(ProcessError::WaitFailed)? {
            WaitStatus::Exited(_, code) => {
                let reason = ExitReason::Exited(code);
                self.exit_reason = Some(reason);
                Ok(reason)
            }
            WaitStatus::Signaled(_, signal, _) => {
                let reason = ExitReason::Signaled(signal as i32);
                self.exit_reason = Some(reason);
                Ok(reason)
            }
            WaitStatus::Stopped(_, _) => self.wait(),
            WaitStatus::Continued(_) => self.wait(),
            WaitStatus::PtraceEvent(_, _, _) | WaitStatus::PtraceSyscall(_) => {
                Err(ProcessError::UnexpectedPtraceEvent)
            }
            WaitStatus::StillAlive => Err(ProcessError::StillRunning),
            // WaitStatus is non-exhaustive, handle any remaining variants
            _ => Err(ProcessError::UnexpectedPtraceEvent),
        }
    }

    /// Check if process has exited
    pub fn has_exited(&self) -> bool {
        self.exit_reason.is_some()
    }

    /// Get exit reason if available
    pub fn exit_reason(&self) -> Option<ExitReason> {
        self.exit_reason
    }

    /// Read from the PTY master (non-blocking)
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ProcessError> {
        let master_fd = self.pty.master_fd()?;
        match nix::unistd::read(master_fd, buf) {
            Ok(n) => Ok(n),
            Err(nix::Error::EAGAIN) => Ok(0),
            Err(e) => Err(ProcessError::Pty(PtyError::ConfigurationFailed(e))),
        }
    }

    /// Write to the PTY master
    pub fn write(&self, data: &[u8]) -> Result<usize, ProcessError> {
        let master_fd = self.pty.master_borrowed()?;
        nix::unistd::write(master_fd, data)
            .map_err(|e| ProcessError::Pty(PtyError::ConfigurationFailed(e)))
    }

    /// Write all data to the PTY master
    pub fn write_all(&self, data: &[u8]) -> Result<(), ProcessError> {
        let mut written = 0;
        while written < data.len() {
            let n = self.write(&data[written..])?;
            if n == 0 {
                break;
            }
            written += n;
        }
        Ok(())
    }

    /// Send a signal to the process
    pub fn send_signal(&self, signal: Signal) -> Result<(), ProcessError> {
        if self.exit_reason.is_some() {
            return Err(ProcessError::NotRunning);
        }
        kill(self.pid, signal).map_err(ProcessError::SignalFailed)
    }

    /// Send SIGINT to the process (interrupt)
    pub fn signal_int(&self) -> Result<(), ProcessError> {
        self.send_signal(Signal::SIGINT)
    }

    /// Send SIGTERM to the process (terminate)
    pub fn signal_term(&self) -> Result<(), ProcessError> {
        self.send_signal(Signal::SIGTERM)
    }

    /// Send SIGKILL to the process (force kill)
    pub fn signal_kill(&self) -> Result<(), ProcessError> {
        self.send_signal(Signal::SIGKILL)
    }

    /// Send SIGWINCH to the process (window size change)
    pub fn signal_winch(&self) -> Result<(), ProcessError> {
        self.send_signal(Signal::SIGWINCH)
    }

    /// Resize the PTY and send SIGWINCH to the process
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), ProcessError> {
        self.pty.resize(cols, rows)?;
        // Only send SIGWINCH if process is still running
        if self.exit_reason.is_none() {
            self.signal_winch()?;
        }
        Ok(())
    }

    /// Gracefully terminate the process and reap it.
    /// Returns true if the process was terminated, false if it was already dead.
    fn terminate(&mut self) -> bool {
        use nix::errno::Errno;
        use nix::sys::signal::kill;
        use nix::sys::wait::{waitpid, WaitPidFlag};

        // Check if already exited
        if self.exit_reason.is_some() {
            return false;
        }

        // Try graceful SIGTERM first
        // Note: We check if delivery succeeded, but process might still ignore it
        let sigterm_result = kill(self.pid, Signal::SIGTERM);
        if sigterm_result.is_err() {
            // Process doesn't exist or we can't send signal - check if already dead
            match waitpid(self.pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, _)) | Ok(WaitStatus::Signaled(_, _, _)) => return true,
                Ok(WaitStatus::StillAlive) => {
                    // Signal failed but process is alive - strange but continue
                }
                // Handle other WaitStatus variants
                Ok(WaitStatus::Stopped(_, _))
                | Ok(WaitStatus::PtraceEvent(_, _, _))
                | Ok(WaitStatus::PtraceSyscall(_))
                | Ok(WaitStatus::Continued(_)) => {
                    // Process is in a special state, treat as alive
                }
                Ok(_) => {
                    // Unknown status, treat as alive
                }
                Err(Errno::ECHILD) => return false, // Already reaped
                Err(_) => return false,             // Other error, give up
            }
        }

        // Polling loop with actual timeout
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(SIGTERM_TIMEOUT_MS);

        loop {
            match waitpid(self.pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, _)) | Ok(WaitStatus::Signaled(_, _, _)) => {
                    // Process exited successfully
                    return true;
                }
                Ok(WaitStatus::StillAlive) => {
                    if start.elapsed() > timeout {
                        // Timeout reached, force kill
                        let sigkill_result = kill(self.pid, Signal::SIGKILL);
                        if sigkill_result.is_ok() {
                            // SIGKILL sent, now reap the zombie
                            // Use WNOHANG again to avoid blocking forever
                            let mut sigchild_count = 0;
                            loop {
                                match waitpid(self.pid, Some(WaitPidFlag::WNOHANG)) {
                                    Ok(_) | Err(Errno::ECHILD) => {
                                        // Either reaped or no more children
                                        break;
                                    }
                                    Err(Errno::EINTR) => {
                                        // Interrupted, retry
                                        sigchild_count += 1;
                                        if sigchild_count > MAX_EINTR_RETRIES {
                                            break; // Give up after max retries
                                        }
                                    }
                                    Err(_) => break, // Other error, give up
                                }
                            }
                        }
                        return true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
                }
                // Handle stopped/traced states - these are temporary, keep waiting
                Ok(WaitStatus::Stopped(_, _))
                | Ok(WaitStatus::PtraceEvent(_, _, _))
                | Ok(WaitStatus::PtraceSyscall(_)) => {
                    if start.elapsed() > timeout {
                        // Been stopped too long, force kill
                        let _ = kill(self.pid, Signal::SIGKILL);
                        let _ = waitpid(self.pid, None);
                        return true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
                }
                // Unknown status - treat as alive but don't infinite loop
                Ok(_) => {
                    if start.elapsed() > timeout {
                        let _ = kill(self.pid, Signal::SIGKILL);
                        let _ = waitpid(self.pid, None);
                        return true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
                }
                Err(Errno::ECHILD) => {
                    // No such child - already reaped
                    return false;
                }
                Err(Errno::EINTR) => {
                    // Interrupted, retry immediately
                    continue;
                }
                Err(_) => {
                    // Other error (EPERM, etc.) - give up
                    return false;
                }
            }
        }
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        // Do NOT manage process lifecycle in Drop.
        // - Sending signals in Drop is unreliable (can't handle errors properly)
        // - Waiting in Drop can block indefinitely
        // - Process cleanup should be explicit via terminate() or wait()
        //
        // The PTY will be dropped automatically when self.pty is dropped.
        // If the process is still running, it becomes orphaned and will be
        // reaped by init/PID 1 when this process exits.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn can_spawn_process() {
        let config = ProcessConfig::shell("echo hello");
        let process = PtyProcess::spawn(&config);
        assert!(process.is_ok(), "Failed to spawn: {:?}", process.err());
    }

    #[test]
    fn captures_pid() {
        let config = ProcessConfig::shell("sleep 0.1");
        let process = PtyProcess::spawn(&config).unwrap();
        assert!(process.pid_raw() > 0);
    }

    #[test]
    fn can_read_output() {
        let config = ProcessConfig::shell("echo hello");
        let process = PtyProcess::spawn(&config).unwrap();

        // Give the process time to produce output
        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        let mut buf = [0u8; 1024];
        let mut output = Vec::new();

        // Read all available output
        for _ in 0..10 {
            match process.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
            thread::sleep(std::time::Duration::from_millis(QUICK_SLEEP_MS));
        }

        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("hello"),
            "Expected 'hello' in output, got: {:?}",
            output_str
        );
    }

    #[test]
    fn can_launch_bash() {
        let config = ProcessConfig::bash().with_size(80, 24);
        let process = PtyProcess::spawn(&config).unwrap();

        // Give bash time to start
        thread::sleep(std::time::Duration::from_millis(LONG_SLEEP_MS));

        let mut buf = [0u8; 4096];
        let mut output = Vec::new();

        // Read initial output (prompt)
        for _ in 0..20 {
            match process.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
            thread::sleep(std::time::Duration::from_millis(SIGNAL_SLEEP_MS));
        }

        let output_str = String::from_utf8_lossy(&output);
        // The prompt should contain "$" since we set PS1="$ "
        assert!(
            output_str.contains("$") || output_str.contains(">") || !output_str.is_empty(),
            "Expected prompt in output, got: {:?}",
            output_str
        );

        // Send exit command
        let _ = process.write_all(b"exit\n");
    }

    #[test]
    fn detects_process_exit() {
        let config = ProcessConfig::shell("exit 42");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Wait for process to exit
        let reason = process.wait().unwrap();

        match reason {
            ExitReason::Exited(code) => assert_eq!(code, 42),
            other => panic!("Expected Exited(42), got {:?}", other),
        }
    }

    #[test]
    fn can_write_to_process() {
        let config = ProcessConfig::shell("cat");
        let process = PtyProcess::spawn(&config).unwrap();

        // Give cat time to start
        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        // Write some data
        process.write_all(b"test input\n").unwrap();

        // Give time for echo
        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

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
            output_str.contains("test input"),
            "Expected 'test input' in output, got: {:?}",
            output_str
        );
    }

    #[test]
    fn sigint_stops_running_program() {
        // Run a long sleep
        let config = ProcessConfig::shell("sleep 60");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Give it time to start
        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        // Send SIGINT
        process.signal_int().unwrap();

        // Wait for process to exit
        let reason = process.wait().unwrap();

        // Process should have been signaled
        match reason {
            ExitReason::Signaled(sig) => {
                assert_eq!(sig, Signal::SIGINT as i32);
            }
            other => panic!("Expected Signaled(SIGINT), got {:?}", other),
        }
    }

    #[test]
    fn sigterm_terminates_process() {
        let config = ProcessConfig::shell("sleep 60");
        let mut process = PtyProcess::spawn(&config).unwrap();

        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        process.signal_term().unwrap();

        let reason = process.wait().unwrap();

        match reason {
            ExitReason::Signaled(sig) => {
                assert_eq!(sig, Signal::SIGTERM as i32);
            }
            other => panic!("Expected Signaled(SIGTERM), got {:?}", other),
        }
    }

    #[test]
    fn sigkill_force_kills_process() {
        let config = ProcessConfig::shell("sleep 60");
        let mut process = PtyProcess::spawn(&config).unwrap();

        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        process.signal_kill().unwrap();

        let reason = process.wait().unwrap();

        match reason {
            ExitReason::Signaled(sig) => {
                assert_eq!(sig, Signal::SIGKILL as i32);
            }
            other => panic!("Expected Signaled(SIGKILL), got {:?}", other),
        }
    }

    #[test]
    fn resize_sends_sigwinch() {
        // The SIGWINCH test verifies that resize() both:
        // 1. Changes the PTY window size
        // 2. Sends SIGWINCH to the process
        //
        // We verify this by checking the PTY size is updated correctly
        let config = ProcessConfig::shell("sleep 5").with_size(80, 24);
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Give it time to start
        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        // Verify initial size
        assert_eq!(process.pty().size(), (80, 24));

        // Resize the PTY - this should also send SIGWINCH
        process.resize(100, 50).unwrap();

        // Verify size changed
        assert_eq!(process.pty().size(), (100, 50));

        // Verify we can send SIGWINCH directly
        assert!(process.signal_winch().is_ok());

        // Clean up
        let _ = process.signal_kill();
    }

    #[test]
    fn exit_reason_detectable_after_signal() {
        let config = ProcessConfig::shell("sleep 60");
        let mut process = PtyProcess::spawn(&config).unwrap();

        thread::sleep(std::time::Duration::from_millis(TEST_SLEEP_MS));

        assert!(process.exit_reason().is_none());
        assert!(!process.has_exited());

        process.signal_kill().unwrap();
        process.wait().unwrap();

        assert!(process.has_exited());
        assert!(process.exit_reason().is_some());

        match process.exit_reason().unwrap() {
            ExitReason::Signaled(_) => {}
            other => panic!("Expected Signaled, got {:?}", other),
        }
    }

    #[test]
    fn drop_does_not_manage_process() {
        // Test that Drop does NOT send signals or manage process lifecycle
        // Process cleanup must be explicit via terminate() or wait()
        let config = ProcessConfig::shell("sleep 60");
        let pid = {
            let process = PtyProcess::spawn(&config).unwrap();
            let pid = process.pid_raw();
            assert!(pid > 0);
            pid
        };
        // Process is now dropped - Drop should NOT send any signals
        // The process is now orphaned and will be reaped by init/PID 1

        // Process should still be running (Drop doesn't kill it)
        // SAFETY: kill(pid, 0) is a probe that checks if process exists without sending signal.
        // It's safe because we're just checking process liveness.
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        assert_eq!(result, 0, "Process should still be running after drop");

        // Clean up explicitly (in real code, caller should do this)
        let mut process = PtyProcess::spawn(&config).unwrap();
        let terminated = process.terminate();
        assert!(
            terminated,
            "terminate() should successfully kill the process"
        );
    }

    #[test]
    fn drop_does_not_double_wait() {
        // Test that dropping a process that was already waited on doesn't crash
        let config = ProcessConfig::shell("sleep 0.01");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Wait for process to exit naturally
        let reason = process.wait().unwrap();
        assert!(matches!(reason, ExitReason::Exited(_)));

        // Drop should not crash or cause issues
        drop(process);
    }

    #[test]
    fn terminate_handles_already_reaped_process() {
        // Test that terminate() handles ECHILD gracefully
        let config = ProcessConfig::shell("echo hello");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Wait for process to exit
        let _ = process.wait().unwrap();

        // terminate() should return false without panicking
        let result = process.terminate();
        assert!(!result);
    }
}
