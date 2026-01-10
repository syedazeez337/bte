//! macOS terminal backend implementation
//!
//! Note: macOS uses BSD-style APIs which are similar to Linux but have
//! some differences.

use super::{
    ExitStatus, PlatformCapabilities, PlatformError, PlatformErrorKind, Signal, SpawnConfig,
    SpawnResult, TerminalBackend, TerminalProcess, TerminalSize,
};
use nix::errno::Errno;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::libc;
use nix::pty::{openpty, OpenptyResult, Winsize};
use nix::sys::signal::{kill, Signal as NixSignal};
use nix::sys::termios::{self, LocalFlags, SetArg, Termios};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{close, dup2, execvpe, fork, setsid, ForkResult, Pid};
use std::ffi::CString;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd, RawFd};

/// macOS-specific terminal process handle
pub struct MacOSTerminalProcess {
    pid: Option<Pid>,
    master_fd: Option<OwnedFd>,
    original_termios: Option<Termios>,
    size: TerminalSize,
    eof: bool,
}

impl MacOSTerminalProcess {
    fn new(
        pid: Pid,
        master_fd: OwnedFd,
        original_termios: Option<Termios>,
        size: TerminalSize,
    ) -> Self {
        Self {
            pid: Some(pid),
            master_fd: Some(master_fd),
            original_termios,
            size,
            eof: false,
        }
    }
}

impl TerminalProcess for MacOSTerminalProcess {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        let fd = self
            .master_fd
            .as_ref()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "PTY is closed"))?;

        let fd_raw = fd.as_raw_fd();
        let result =
            unsafe { libc::write(fd_raw, data.as_ptr() as *const libc::c_void, data.len()) };

        if result < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(result as usize)
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let fd = self
            .master_fd
            .as_ref()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "PTY is closed"))?;

        let fd_raw = fd.as_raw_fd();
        let result =
            unsafe { libc::read(fd_raw, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

        match result {
            0 => {
                self.eof = true;
                Ok(0)
            }
            n if n < 0 => {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    Err(err)
                }
            }
            n => Ok(n as usize),
        }
    }

    fn is_running(&self) -> bool {
        if let Some(pid) = self.pid {
            match waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => return true,
                Ok(_) => return false,
                Err(_) => return false,
            }
        }
        false
    }

    fn send_signal(&mut self, signal: Signal) -> Result<(), std::io::Error> {
        let pid = self.pid.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "No process to signal")
        })?;

        let nix_signal = match signal {
            Signal::Sigint => NixSignal::SIGINT,
            Signal::Sigterm => NixSignal::SIGTERM,
            Signal::Sigkill => NixSignal::SIGKILL,
            Signal::Sigwinch => NixSignal::SIGWINCH,
            Signal::Sigstop => NixSignal::SIGSTOP,
            Signal::Sigcont => NixSignal::SIGCONT,
            Signal::Sighup => NixSignal::SIGHUP,
            Signal::Sigusr1 => NixSignal::SIGUSR1,
            Signal::Sigusr2 => NixSignal::SIGUSR2,
        };

        kill(pid, nix_signal).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send signal: {}", e),
            )
        })
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), std::io::Error> {
        let fd = self
            .master_fd
            .as_ref()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "PTY is closed"))?;

        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // macOS uses a different ioctl number for TIOCSWINSZ
        unsafe {
            let ret = libc::ioctl(fd.as_raw_fd(), 0x80087467 as _, &winsize);
            if ret < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }

        self.size = TerminalSize { cols, rows };
        Ok(())
    }

    fn wait(&mut self) -> Result<ExitStatus, std::io::Error> {
        let pid = self.pid.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "No process to wait for")
        })?;

        match waitpid(pid, None) {
            Ok(WaitStatus::Exited(_, code)) => Ok(ExitStatus::Exited(code)),
            Ok(WaitStatus::Signaled(_, sig, _)) => Ok(ExitStatus::Signaled(sig as i32)),
            Ok(_) => Ok(ExitStatus::Running),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Wait failed: {}", e),
            )),
        }
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, std::io::Error> {
        let pid = match self.pid {
            Some(pid) => pid,
            None => return Ok(None),
        };

        match waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::Exited(_, code)) => {
                self.pid = None;
                Ok(Some(ExitStatus::Exited(code)))
            }
            Ok(WaitStatus::Signaled(_, sig, _)) => {
                self.pid = None;
                Ok(Some(ExitStatus::Signaled(sig as i32)))
            }
            Ok(WaitStatus::StillAlive) => Ok(None),
            Ok(_) => Ok(None),
            Err(Errno::ECHILD) => {
                self.pid = None;
                Ok(None)
            }
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Try wait failed: {}", e),
            )),
        }
    }

    fn pid(&self) -> Option<u32> {
        self.pid.map(|p| p.as_raw() as u32)
    }

    fn eof(&self) -> bool {
        self.eof
    }
}

impl Drop for MacOSTerminalProcess {
    fn drop(&mut self) {
        self.master_fd.take();
    }
}

/// macOS terminal backend
pub struct MacOSTerminalBackend;

impl MacOSTerminalBackend {
    fn set_raw_mode(fd: std::os::fd::RawFd) -> Result<Option<Termios>, PlatformError> {
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        let mut termios = termios::tcgetattr(&borrowed).map_err(|e| PlatformError {
            source: Box::new(e),
            kind: PlatformErrorKind::PtyAllocation,
        })?;

        let original = termios.clone();

        termios.local_flags.remove(LocalFlags::ICANON);
        termios.local_flags.remove(LocalFlags::ECHO);
        termios.local_flags.remove(LocalFlags::ECHOE);
        termios.local_flags.remove(LocalFlags::ECHOK);
        termios.local_flags.remove(LocalFlags::ECHONL);
        termios.local_flags.remove(LocalFlags::ISIG);

        termios::tcsetattr(&borrowed, SetArg::TCSANOW, &termios).map_err(|e| PlatformError {
            source: Box::new(e),
            kind: PlatformErrorKind::PtyAllocation,
        })?;

        Ok(Some(original))
    }

    fn set_non_blocking(fd: std::os::fd::RawFd) -> Result<(), PlatformError> {
        let flags = fcntl(fd, FcntlArg::F_GETFL).map_err(|e| PlatformError {
            source: Box::new(e),
            kind: PlatformErrorKind::IoError("Failed to get flags".to_string()),
        })?;

        let new_flags = OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK;

        fcntl(fd, FcntlArg::F_SETFL(new_flags)).map_err(|e| PlatformError {
            source: Box::new(e),
            kind: PlatformErrorKind::IoError("Failed to set non-blocking".to_string()),
        })?;

        Ok(())
    }
}

impl TerminalBackend for MacOSTerminalBackend {
    fn new() -> Result<Self, PlatformError> {
        Ok(Self)
    }

    fn spawn(&self, config: &SpawnConfig) -> Result<SpawnResult, PlatformError> {
        let winsize = Winsize {
            ws_row: config.size.rows,
            ws_col: config.size.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let OpenptyResult { master, slave } =
            openpty(&winsize, None).map_err(|e| PlatformError {
                source: Box::new(e),
                kind: PlatformErrorKind::PtyAllocation,
            })?;

        let master_fd = master.as_raw_fd();
        let slave_fd = slave.as_raw_fd();

        let original_termios = if config.raw_mode {
            Self::set_raw_mode(slave_fd)?
        } else {
            None
        };

        Self::set_non_blocking(master_fd)?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                drop(slave);

                let process =
                    MacOSTerminalProcess::new(child, master, original_termios, config.size);

                Ok(SpawnResult {
                    process: Box::new(process),
                })
            }
            Ok(ForkResult::Child) => {
                if setsid().is_err() {
                    std::process::exit(1);
                }

                unsafe {
                    libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0);
                }

                dup2(slave_fd, libc::STDIN_FILENO);
                dup2(slave_fd, libc::STDOUT_FILENO);
                dup2(slave_fd, libc::STDERR_FILENO);

                close(slave_fd);

                for env in &config.env {
                    std::env::set_var(&env.name, &env.value);
                }

                if let Some(cwd) = &config.cwd {
                    std::env::set_current_dir(cwd).ok();
                }

                let program =
                    CString::new(config.program.to_string_lossy().as_bytes()).map_err(|_| {
                        PlatformError {
                            source: Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Invalid program path",
                            )),
                            kind: PlatformErrorKind::SpawnFailed(
                                "Invalid program path".to_string(),
                            ),
                        }
                    })?;

                let mut args: Vec<CString> = Vec::new();
                for arg in &config.args {
                    args.push(CString::new(arg.as_bytes()).map_err(|_| PlatformError {
                        source: Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid argument",
                        )),
                        kind: PlatformErrorKind::SpawnFailed("Invalid argument".to_string()),
                    })?);
                }

                let envs: Vec<CString> = std::env::vars()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .map(|s| CString::new(s).unwrap())
                    .collect();

                execvpe(&program, &args, &envs).map_err(|e| PlatformError {
                    source: Box::new(e),
                    kind: PlatformErrorKind::SpawnFailed("Exec failed".to_string()),
                })?;

                std::process::exit(1);
            }
            Err(e) => Err(PlatformError {
                source: Box::new(e),
                kind: PlatformErrorKind::SpawnFailed("Fork failed".to_string()),
            }),
        }
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::current()
    }

    fn name(&self) -> &'static str {
        "macos"
    }
}
