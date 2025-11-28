use crate::daemon::ForgeDaemon;
use crate::error::{DaemonError, DaemonResult};
use crate::stdio::Stdio;
use crate::types::{Group, User};
use std::ffi::CString;
use std::io;
use std::path::Path;
use std::process::exit;

pub fn start<T>(daemon: ForgeDaemon<T>) -> DaemonResult<T> {
    unsafe {
        // Initial Fork
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
        redirect_stream(&daemon.stdin, libc::STDIN_FILENO)?;
        redirect_stream(&daemon.stdout, libc::STDOUT_FILENO)?;
        redirect_stream(&daemon.stderr, libc::STDERR_FILENO)?;

        // Second Fork
        if perform_fork()? > 0 {
            exit(0);
        }

        // --- DAEMON CONTEXT ESTABLISHED ---

        // Environment Management
        if daemon.clear_env {
            #[cfg(target_os = "linux")]
            libc::clearenv();
        }
        for (k, v) in &daemon.env_vars {
            std::env::set_var(k, v);
        }

        //System Configuration
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
            if libc::chdir(b"/\0".as_ptr() as *const i8) < 0 {
                return Err(DaemonError::Io(io::Error::last_os_error()));
            }
        }

        // Locking & PID File Logic
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

        // Privileged Action
        let action = daemon.privileged_action.unwrap();
        let result = action()?;

        // Drop Privileges
        if let Some(group) = &daemon.group {
            set_group(group)?;
        }
        if let Some(user) = &daemon.user {
            set_user(user)?;
        }

        Ok(result)
    }
}

// --- Helpers ---

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

    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err(DaemonError::TargetLocked);
    }

    let mut file = file;
    let pid = unsafe { libc::getpid() };
    write!(file, "{}", pid)?;
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
