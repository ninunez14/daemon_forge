use crate::daemon::ForgeDaemon;
use crate::error::{DaemonError, DaemonResult};
use crate::stdio::Stdio;
use crate::types::{Group, User};
use std::ffi::CString;
use std::io;
use std::path::Path;
use std::process::exit;

#[cfg(target_os = "linux")]
use sd_notify::NotifyState;

/// Main entry point for Unix systems.
/// 
/// It automatically detects if the process is being managed by Systemd (via `NOTIFY_SOCKET`).
/// - **Systemd Detected:** Runs in the foreground, notifies `READY=1`, and executes the payload.
/// - **Manual Start:** Performs the classic double-fork machination to daemonize into the background.
pub fn start<T>(daemon: ForgeDaemon<T>) -> DaemonResult<T> {
    
    #[cfg(target_os = "linux")]
    {
        // If NOTIFY_SOCKET is present, Systemd expects us to stay in the foreground
        if std::env::var("NOTIFY_SOCKET").is_ok() {
            return start_systemd_mode(daemon);
        }
    }

    start_background_mode(daemon)
}


#[cfg(target_os = "linux")]
fn start_systemd_mode<T>(daemon: ForgeDaemon<T>) -> DaemonResult<T> {

    apply_io_redirection(&daemon)?;

    // Notify Systemd that the service is ready.
    // 'true' tells the library to unset the env var so it doesn't leak to children.
    let _ = sd_notify::notify(true, &[NotifyState::Ready]);

    execute_daemon_logic(daemon)
}

/// Double-Fork to detach from terminal and run in background.
fn start_background_mode<T>(daemon: ForgeDaemon<T>) -> DaemonResult<T> {
    unsafe {
        // Fork 1
        if perform_fork()? > 0 {
            exit(0);
        }

        // New Session
        if libc::setsid() < 0 {
            return Err(DaemonError::SyscallError {
                call: "setsid",
                errno: io::Error::last_os_error().raw_os_error().unwrap_or(0),
            });
        }

        // IO Redirection
        apply_io_redirection(&daemon)?;

        // Fork 2
        if perform_fork()? > 0 {
            exit(0);
        }

        // Execute the main daemon logic in the grandchild process
        execute_daemon_logic(daemon)
    }
}

/// The core execution logic common to both Systemd and Background modes.
/// Handles environment, chroot, PID files, privileges, and the user action.
fn execute_daemon_logic<T>(daemon: ForgeDaemon<T>) -> DaemonResult<T> {
    unsafe {
        // --- Environment Management ---
        if daemon.clear_env {
            #[cfg(target_os = "linux")]
            libc::clearenv();
        }
        for (k, v) in &daemon.env_vars {
            std::env::set_var(k, v);
        }

        // --- System Configuration ---
        if let Some(mask) = daemon.umask {
            libc::umask(mask as libc::mode_t);
        }

        let cwd = CString::new(daemon.directory.to_str().unwrap()).map_err(|_| {
            DaemonError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid CWD path",
            ))
        })?;
        if libc::chdir(cwd.as_ptr()) < 0 {
            return Err(DaemonError::Io(io::Error::last_os_error()));
        }

        // --- Chroot Logic ---
        if let Some(root) = &daemon.root {
            let root_c = CString::new(root.to_str().unwrap()).map_err(|_| {
                DaemonError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid chroot path",
                ))
            })?;
            if libc::chroot(root_c.as_ptr()) < 0 {
                return Err(DaemonError::PrivilegeError(format!(
                    "chroot failed: {}",
                    io::Error::last_os_error()
                )));
            }
            // Always change dir to "/" after chroot
            if libc::chdir(b"/\0".as_ptr() as *const i8) < 0 {
                return Err(DaemonError::Io(io::Error::last_os_error()));
            }
        }

        // --- Locking & PID File Logic ---
        let effective_lock_path = if let Some(path) = &daemon.pid_file {
            Some(path.clone())
        } else if let Some(name) = &daemon.name {
            Some(std::env::temp_dir().join(format!("daemon-{}.pid", name)))
        } else {
            None
        };

        if let Some(path) = effective_lock_path {
            write_pid_file_unix(&path)?;
            if daemon.chown_pid {
                apply_chown(&path, &daemon.user, &daemon.group)?;
            }
        }

        // --- Privileged Action (Payload) ---
        // This is where the user's loop runs
        let action = daemon.privileged_action.unwrap();
        let result = action()?;

        // --- Drop Privileges ---
        // (Only executed if the action returns, usually cleanup)
        if let Some(group) = &daemon.group {
            set_group(group)?;
        }
        if let Some(user) = &daemon.user {
            set_user(user)?;
        }

        Ok(result)
    }
}

// =========================================================================
// Helpers
// =========================================================================

fn apply_io_redirection<T>(daemon: &ForgeDaemon<T>) -> DaemonResult<()> {
    unsafe {
        redirect_stream(&daemon.stdin, libc::STDIN_FILENO)?;
        redirect_stream(&daemon.stdout, libc::STDOUT_FILENO)?;
        redirect_stream(&daemon.stderr, libc::STDERR_FILENO)?;
    }
    Ok(())
}

unsafe fn perform_fork() -> DaemonResult<libc::pid_t> {
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        Err(DaemonError::SyscallError {
            call: "fork",
            errno: io::Error::last_os_error().raw_os_error().unwrap_or(0),
        })
    } else {
        Ok(pid)
    }
}

unsafe fn redirect_stream(stdio: &Stdio, target_fd: libc::c_int) -> DaemonResult<()> {
    use std::os::unix::io::AsRawFd;

    match stdio {
        Stdio::RedirectToFile(f) => {
            if unsafe { libc::dup2(f.as_raw_fd(), target_fd) } < 0 {
                return Err(DaemonError::Io(io::Error::last_os_error()));
            }
        }
        Stdio::Devnull => {
            let path = CString::new("/dev/null").unwrap();
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR) };
            if fd < 0 {
                return Err(DaemonError::Io(io::Error::last_os_error()));
            }

            if unsafe { libc::dup2(fd, target_fd) } < 0 {
                return Err(DaemonError::Io(io::Error::last_os_error()));
            }

            unsafe { libc::close(fd) };
        }
        Stdio::Keep => {}
    }
    Ok(())
}

unsafe fn write_pid_file_unix(path: &Path) -> DaemonResult<()> {
    use std::io::Write;
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    let fd = file.as_raw_fd();

    // LOCK_NB ensures we don't block if another instance is running
    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err(DaemonError::TargetLocked);
    }

    let mut file = file;
    let pid = unsafe { libc::getpid() };
    write!(file, "{}", pid)?;
    
    // Intentionally leak the file handle to maintain the OS lock 
    // for the lifetime of the process.
    std::mem::forget(file);

    Ok(())
}

unsafe fn set_user(user: &User) -> DaemonResult<()> {
    let cname = CString::new(user.0.as_str()).unwrap();
    let pwd = unsafe { libc::getpwnam(cname.as_ptr()) };
    if pwd.is_null() {
        return Err(DaemonError::PrivilegeError(format!(
            "User '{}' not found",
            user.0
        )));
    }

    if unsafe { libc::setuid((*pwd).pw_uid) } < 0 {
        return Err(DaemonError::PrivilegeError(format!(
            "Failed to setuid: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

unsafe fn set_group(group: &Group) -> DaemonResult<()> {
    let cname = CString::new(group.0.as_str()).unwrap();
    let grp = unsafe { libc::getgrnam(cname.as_ptr()) };
    if grp.is_null() {
        return Err(DaemonError::PrivilegeError(format!(
            "Group '{}' not found",
            group.0
        )));
    }
    if unsafe { libc::setgid((*grp).gr_gid) } < 0 {
        return Err(DaemonError::PrivilegeError(format!(
            "Failed to setgid: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

unsafe fn apply_chown(path: &Path, user: &Option<User>, group: &Option<Group>) -> DaemonResult<()> {
    let uid = if let Some(u) = user {
        let cname = CString::new(u.0.as_str()).unwrap();
        let pwd = unsafe { libc::getpwnam(cname.as_ptr()) };
        if pwd.is_null() {
            return Err(DaemonError::PrivilegeError(format!(
                "User '{}' not found",
                u.0
            )));
        }
        unsafe { (*pwd).pw_uid }
    } else {
        u32::MAX
    };

    let gid = if let Some(g) = group {
        let cname = CString::new(g.0.as_str()).unwrap();
        let grp = unsafe { libc::getgrnam(cname.as_ptr()) };
        if grp.is_null() {
            return Err(DaemonError::PrivilegeError(format!(
                "Group '{}' not found",
                g.0
            )));
        }
        unsafe { (*grp).gr_gid }
    } else {
        u32::MAX
    };

    let cpath = CString::new(path.to_str().unwrap()).unwrap();
    if unsafe { libc::chown(cpath.as_ptr(), uid, gid) } < 0 {
        return Err(DaemonError::PrivilegeError(format!(
            "chown failed: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}