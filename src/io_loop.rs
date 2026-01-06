//! Non-blocking IO loop for PTY communication
//!
//! This module provides an epoll-based IO loop that handles
//! reading and writing to the PTY without deadlocks.

#![allow(dead_code)]

use crate::process::{ProcessError, PtyProcess};
use crate::pty::PtyError;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::collections::VecDeque;
use std::os::fd::BorrowedFd;

/// Default buffer size for reading from PTY
const DEFAULT_READ_BUFFER_SIZE: usize = 4096;

/// Default maximum buffer size before backpressure kicks in
const DEFAULT_MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

/// Error type for IO loop operations
#[derive(Debug)]
pub enum IoError {
    /// Process error
    Process(ProcessError),
    /// Poll error
    PollFailed(nix::Error),
    /// Buffer overflow (backpressure)
    BufferOverflow,
    /// Read error
    ReadFailed(nix::Error),
    /// Write error
    WriteFailed(nix::Error),
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::Process(e) => write!(f, "Process error: {}", e),
            IoError::PollFailed(e) => write!(f, "Poll failed: {}", e),
            IoError::BufferOverflow => write!(f, "Buffer overflow (backpressure triggered)"),
            IoError::ReadFailed(e) => write!(f, "Read failed: {}", e),
            IoError::WriteFailed(e) => write!(f, "Write failed: {}", e),
        }
    }
}

impl std::error::Error for IoError {}

impl From<ProcessError> for IoError {
    fn from(e: ProcessError) -> Self {
        IoError::Process(e)
    }
}

impl From<PtyError> for IoError {
    fn from(e: PtyError) -> Self {
        IoError::Process(ProcessError::Pty(e))
    }
}

/// Bounded buffer for accumulating data
#[derive(Debug)]
pub struct BoundedBuffer {
    /// Internal buffer
    data: VecDeque<u8>,
    /// Maximum size before backpressure
    max_size: usize,
}

impl BoundedBuffer {
    /// Create a new bounded buffer
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_size.min(DEFAULT_READ_BUFFER_SIZE * 4)),
            max_size,
        }
    }

    /// Create with default max size (1MB)
    pub fn default_size() -> Self {
        Self::new(DEFAULT_MAX_BUFFER_SIZE)
    }

    /// Push data into the buffer
    ///
    /// Returns Err if buffer is full (backpressure)
    pub fn push(&mut self, data: &[u8]) -> Result<(), IoError> {
        if self.data.len() + data.len() > self.max_size {
            return Err(IoError::BufferOverflow);
        }
        self.data.extend(data);
        Ok(())
    }

    /// Push data, dropping oldest if necessary (lossy mode)
    pub fn push_lossy(&mut self, data: &[u8]) -> usize {
        let space_needed = (self.data.len() + data.len()).saturating_sub(self.max_size);
        let dropped = space_needed.min(self.data.len());

        // Drop oldest data to make room
        self.data.drain(..dropped);

        // Add new data
        self.data.extend(data);
        dropped
    }

    /// Take all data from the buffer
    pub fn take_all(&mut self) -> Vec<u8> {
        self.data.drain(..).collect()
    }

    /// Take up to n bytes from the buffer
    pub fn take(&mut self, n: usize) -> Vec<u8> {
        let to_take = n.min(self.data.len());
        self.data.drain(..to_take).collect()
    }

    /// Peek at the buffer contents without consuming
    pub fn peek(&self) -> &VecDeque<u8> {
        &self.data
    }

    /// Get the current buffer length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.max_size
    }

    /// Get available space
    pub fn available(&self) -> usize {
        self.max_size.saturating_sub(self.data.len())
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

/// Result of a poll operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollResult {
    /// Data is available to read
    pub readable: bool,
    /// Ready to accept writes
    pub writable: bool,
    /// Error or hangup occurred
    pub error: bool,
    /// Process exited (hangup)
    pub hangup: bool,
}

/// IO loop for PTY communication
pub struct IoLoop {
    /// Read buffer size
    read_buffer_size: usize,
    /// Output buffer (data read from PTY)
    output_buffer: BoundedBuffer,
    /// Input buffer (data to write to PTY)
    input_buffer: BoundedBuffer,
    /// Whether to use lossy mode (drop data on overflow)
    lossy_mode: bool,
    /// Total bytes read
    bytes_read: u64,
    /// Total bytes written
    bytes_written: u64,
    /// Bytes dropped due to backpressure
    bytes_dropped: u64,
}

impl IoLoop {
    /// Create a new IO loop with default settings
    pub fn new() -> Self {
        Self {
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            output_buffer: BoundedBuffer::default_size(),
            input_buffer: BoundedBuffer::default_size(),
            lossy_mode: true, // Default to lossy to prevent deadlocks
            bytes_read: 0,
            bytes_written: 0,
            bytes_dropped: 0,
        }
    }

    /// Set the read buffer size
    pub fn with_read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size;
        self
    }

    /// Set the maximum output buffer size
    pub fn with_max_output_size(mut self, size: usize) -> Self {
        self.output_buffer = BoundedBuffer::new(size);
        self
    }

    /// Set lossy mode (drop data on overflow instead of erroring)
    pub fn with_lossy_mode(mut self, lossy: bool) -> Self {
        self.lossy_mode = lossy;
        self
    }

    /// Poll the PTY for readiness
    pub fn poll(&self, process: &PtyProcess, timeout_ms: i32) -> Result<PollResult, IoError> {
        let master_fd = process.pty().master_fd()?;

        // Safety: We're borrowing the fd for the duration of poll
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(master_fd) };

        let mut poll_flags = PollFlags::POLLIN;
        if !self.input_buffer.is_empty() {
            poll_flags |= PollFlags::POLLOUT;
        }

        let mut poll_fds = [PollFd::new(borrowed_fd, poll_flags)];

        // Convert timeout_ms to PollTimeout
        let timeout = if timeout_ms < 0 {
            PollTimeout::NONE // Block forever
        } else if timeout_ms == 0 {
            PollTimeout::ZERO // Non-blocking
        } else {
            PollTimeout::try_from(timeout_ms as u16).unwrap_or(PollTimeout::MAX)
        };

        poll(&mut poll_fds, timeout).map_err(IoError::PollFailed)?;

        let revents = poll_fds[0].revents().unwrap_or(PollFlags::empty());

        Ok(PollResult {
            readable: revents.contains(PollFlags::POLLIN),
            writable: revents.contains(PollFlags::POLLOUT),
            error: revents.contains(PollFlags::POLLERR),
            hangup: revents.contains(PollFlags::POLLHUP),
        })
    }

    /// Read available data from the PTY into the output buffer
    pub fn read_available(&mut self, process: &PtyProcess) -> Result<usize, IoError> {
        let mut temp_buf = vec![0u8; self.read_buffer_size];
        let mut total_read = 0;

        loop {
            match process.read(&mut temp_buf) {
                Ok(0) => break, // No more data (non-blocking returned EAGAIN)
                Ok(n) => {
                    if self.lossy_mode {
                        let dropped = self.output_buffer.push_lossy(&temp_buf[..n]);
                        self.bytes_dropped += dropped as u64;
                    } else {
                        self.output_buffer.push(&temp_buf[..n])?;
                    }
                    total_read += n;
                    self.bytes_read += n as u64;
                }
                Err(e) => return Err(IoError::Process(e)),
            }
        }

        Ok(total_read)
    }

    /// Write pending data from the input buffer to the PTY
    pub fn write_pending(&mut self, process: &PtyProcess) -> Result<usize, IoError> {
        let mut total_written = 0;

        while !self.input_buffer.is_empty() {
            // Get data to write (without consuming yet)
            let data: Vec<u8> = self.input_buffer.peek().iter().copied().collect();
            if data.is_empty() {
                break;
            }

            match process.write(&data) {
                Ok(0) => break, // Would block
                Ok(n) => {
                    // Consume the written bytes
                    self.input_buffer.take(n);
                    total_written += n;
                    self.bytes_written += n as u64;
                }
                Err(e) => return Err(IoError::Process(e)),
            }
        }

        Ok(total_written)
    }

    /// Queue data to be written to the PTY
    pub fn queue_input(&mut self, data: &[u8]) -> Result<(), IoError> {
        if self.lossy_mode {
            self.input_buffer.push_lossy(data);
            Ok(())
        } else {
            self.input_buffer.push(data)
        }
    }

    /// Take all output data from the buffer
    pub fn take_output(&mut self) -> Vec<u8> {
        self.output_buffer.take_all()
    }

    /// Get a reference to the output buffer
    pub fn output_buffer(&self) -> &BoundedBuffer {
        &self.output_buffer
    }

    /// Get total bytes read
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Get total bytes written
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Get bytes dropped due to backpressure
    pub fn bytes_dropped(&self) -> u64 {
        self.bytes_dropped
    }

    /// Run one iteration of the IO loop
    ///
    /// Returns (bytes_read, bytes_written)
    pub fn tick(
        &mut self,
        process: &PtyProcess,
        timeout_ms: i32,
    ) -> Result<(usize, usize), IoError> {
        let poll_result = self.poll(process, timeout_ms)?;

        let mut read = 0;
        let mut written = 0;

        if poll_result.readable {
            read = self.read_available(process)?;
        }

        if poll_result.writable && !self.input_buffer.is_empty() {
            written = self.write_pending(process)?;
        }

        Ok((read, written))
    }

    /// Run the IO loop until the process exits or a condition is met
    ///
    /// The callback is called after each tick with (bytes_read, bytes_written).
    /// Return false from the callback to stop the loop early.
    pub fn run_until<F>(
        &mut self,
        process: &mut PtyProcess,
        timeout_ms: i32,
        mut callback: F,
    ) -> Result<(), IoError>
    where
        F: FnMut(usize, usize) -> bool,
    {
        loop {
            // Check if process has exited
            if let Ok(Some(_)) = process.try_wait() {
                // Process exited, do final read
                let _ = self.read_available(process);
                break;
            }

            let (read, written) = self.tick(process, timeout_ms)?;

            if !callback(read, written) {
                break;
            }
        }

        Ok(())
    }
}

impl Default for IoLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessConfig;
    use std::thread;

    #[test]
    fn bounded_buffer_basics() {
        let mut buf = BoundedBuffer::new(100);

        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.available(), 100);

        buf.push(b"hello").unwrap();
        assert_eq!(buf.len(), 5);
        assert!(!buf.is_empty());

        let data = buf.take_all();
        assert_eq!(&data, b"hello");
        assert!(buf.is_empty());
    }

    #[test]
    fn bounded_buffer_overflow() {
        let mut buf = BoundedBuffer::new(10);

        buf.push(b"12345").unwrap();
        buf.push(b"12345").unwrap();

        // This should fail - would exceed 10 bytes
        let result = buf.push(b"x");
        assert!(matches!(result, Err(IoError::BufferOverflow)));
    }

    #[test]
    fn bounded_buffer_lossy() {
        let mut buf = BoundedBuffer::new(10);

        buf.push(b"12345").unwrap();
        let dropped = buf.push_lossy(b"67890123"); // 8 bytes, need to drop 3

        assert!(dropped >= 3);
        assert!(buf.len() <= 10);
    }

    #[test]
    fn io_loop_reads_output() {
        let config = ProcessConfig::shell("echo hello_world");
        let process = PtyProcess::spawn(&config).unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let mut io = IoLoop::new();
        let _ = io.read_available(&process);

        let output = io.take_output();
        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("hello_world"),
            "Expected 'hello_world', got: {:?}",
            output_str
        );
    }

    #[test]
    fn io_loop_writes_input() {
        let config = ProcessConfig::shell("cat");
        let process = PtyProcess::spawn(&config).unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let mut io = IoLoop::new();

        // Queue input
        io.queue_input(b"test_data\n").unwrap();

        // Write it
        let written = io.write_pending(&process).unwrap();
        assert!(written > 0);

        // Give time for echo
        thread::sleep(std::time::Duration::from_millis(100));

        // Read response
        let _ = io.read_available(&process);
        let output = io.take_output();
        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("test_data"),
            "Expected 'test_data', got: {:?}",
            output_str
        );
    }

    #[test]
    fn io_loop_handles_rapid_output() {
        // Generate lots of output quickly
        let config = ProcessConfig::shell("seq 1 10000");
        let process = PtyProcess::spawn(&config).unwrap();

        let mut io = IoLoop::new().with_lossy_mode(true);

        // Read until process exits
        let mut iterations = 0;
        loop {
            let result = io.tick(&process, 10);
            if result.is_err() {
                break;
            }

            iterations += 1;
            if iterations > 1000 {
                break; // Safety limit
            }

            thread::sleep(std::time::Duration::from_millis(1));
        }

        // Should have read something
        assert!(io.bytes_read() > 0, "Should have read some data");
    }

    #[test]
    fn io_loop_no_deadlock_on_flood() {
        // This tests that flooding the PTY doesn't deadlock
        let config = ProcessConfig::shell("yes | head -c 100000");
        let process = PtyProcess::spawn(&config).unwrap();

        let mut io = IoLoop::new()
            .with_max_output_size(10000) // Small buffer
            .with_lossy_mode(true); // Drop data if needed

        let start = std::time::Instant::now();

        // Read with timeout
        while start.elapsed().as_secs() < 2 {
            let result = io.tick(&process, 10);
            if result.is_err() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(1));
        }

        // If we get here without hanging, the test passes
        assert!(io.bytes_read() > 0);
    }

    #[test]
    fn poll_detects_hangup() {
        let config = ProcessConfig::shell("echo done");
        let mut process = PtyProcess::spawn(&config).unwrap();

        // Wait for process to exit
        thread::sleep(std::time::Duration::from_millis(200));
        let _ = process.try_wait();

        let io = IoLoop::new();
        let result = io.poll(&process, 0);

        // After process exits, we should see hangup
        if let Ok(poll_result) = result {
            // Either readable (there's output) or hangup (process exited)
            assert!(
                poll_result.readable || poll_result.hangup,
                "Expected readable or hangup after process exit"
            );
        }
    }
}
