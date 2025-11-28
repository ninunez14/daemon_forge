use std::fmt;
use std::io;

/// Custom error type for DaemonForge.
/// Provides specific details about why the daemonization failed.
#[derive(Debug)]
pub enum DaemonError {
    /// Standard IO errors (file creation, piping, etc.)
    Io(io::Error),
    /// The PID lock file or Mutex is already locked by another instance.
    TargetLocked,
    /// Failed to drop privileges (User/Group not found or permission denied).
    PrivilegeError(String),
    /// Environment variable error (e.g., failed to set or clear).
    EnvError(String),
    /// (Windows) Specific Win32 API error code.
    #[cfg(not(unix))]
    Win32Error(u32),
    /// (Unix) Specific system call failure (fork, setsid).
    #[cfg(unix)]
    SyscallError { call: &'static str, errno: i32 },
}

impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonError::Io(err) => write!(f, "IO Error: {}", err),
            DaemonError::TargetLocked => write!(f, "Daemon is already running (Target Locked)"),
            DaemonError::PrivilegeError(msg) => write!(f, "Privilege Drop Error: {}", msg),
            DaemonError::EnvError(msg) => write!(f, "Environment Error: {}", msg),
            #[cfg(not(unix))]
            DaemonError::Win32Error(code) => write!(f, "Win32 API Error Code: {}", code),
            #[cfg(unix)]
            DaemonError::SyscallError { call, errno } => {
                write!(f, "Syscall '{}' failed with errno {}", call, errno)
            }
        }
    }
}

impl std::error::Error for DaemonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DaemonError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for DaemonError {
    fn from(err: io::Error) -> Self {
        DaemonError::Io(err)
    }
}

/// A specialized Result type for DaemonForge operations.
pub type DaemonResult<T> = Result<T, DaemonError>;
