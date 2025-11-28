use crate::daemon::ForgeDaemon;
use crate::error::{DaemonError, DaemonResult};
use crate::stdio::Stdio;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, exit};

mod win_api {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn CreateMutexW(
            lpMutexAttributes: *const c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> *mut c_void;

        pub fn CloseHandle(hObject: *mut c_void) -> i32;
    }

    pub const ERROR_ALREADY_EXISTS: i32 = 183;
}

struct ScopedHandle(*mut std::ffi::c_void);

impl Drop for ScopedHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                win_api::CloseHandle(self.0);
            }
        }
    }
}

pub fn start<T>(mut daemon: ForgeDaemon<T>) -> DaemonResult<T> {
    const DETACHED_PROCESS: u32 = 0x00000008;
    const ENV_VAR_NAME: &str = "__DAEMONIZED_INTERNAL_FLAG";

    if env::var(ENV_VAR_NAME).is_ok() {
        // =========================================================
        // ---> CHILD PROCESS (The Daemon) <---
        // =========================================================

        // Ensure Single Instance (Robust Locking)
        // Try to lock if we have either a name OR a pid_file
        let _lock = if daemon.name.is_some() || daemon.pid_file.is_some() {
            match ensure_single_instance_windows(&daemon.pid_file, &daemon.name) {
                Ok(l) => Some(l),
                Err(e) => {
                    daemon.log_error(&format!("Failed to acquire instance lock. {}", e));
                    return Err(e);
                }
            }
        } else {
            None
        };

        // Change Directory
        if let Err(e) = env::set_current_dir(&daemon.directory) {
            daemon.log_error(&format!("Failed to change directory. {}", e));
            return Err(DaemonError::Io(e));
        }

        // Write PID File
        if let Some(path) = &daemon.pid_file
            && let Err(e) = File::create(path).and_then(|mut f| write!(f, "{}", std::process::id()))
        {
            daemon.log_error(&format!("Failed to write PID file. {}", e));
            return Err(DaemonError::Io(e));
        }

        if let Some(lock) = _lock {
            std::mem::forget(lock);
        }

        // Run the privileged action
        let action = daemon.privileged_action.unwrap();
        action()
    } else {
        // =========================================================
        // ---> PARENT PROCESS (The Launcher) <---
        // =========================================================
        let exe_path = env::current_exe().map_err(DaemonError::Io)?;
        let mut cmd = Command::new(exe_path);

        cmd.args(env::args().skip(1));
        cmd.env(ENV_VAR_NAME, "1");
        cmd.creation_flags(DETACHED_PROCESS);

        if daemon.clear_env {
            cmd.env_clear();
        }
        cmd.envs(&daemon.env_vars);

        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(map_stdio(&daemon.stdout).map_err(DaemonError::Io)?);
        cmd.stderr(map_stdio(&daemon.stderr).map_err(DaemonError::Io)?);

        cmd.spawn().map_err(DaemonError::Io)?;

        exit(0);
    }
}

fn map_stdio(stdio: &Stdio) -> io::Result<std::process::Stdio> {
    match stdio {
        Stdio::Devnull => Ok(std::process::Stdio::null()),
        Stdio::RedirectToFile(file) => {
            let f = file.try_clone()?;
            Ok(std::process::Stdio::from(f))
        }
        Stdio::Keep => Ok(std::process::Stdio::inherit()),
    }
}

fn ensure_single_instance_windows(
    pid_file_path: &Option<PathBuf>,
    name: &Option<String>,
) -> DaemonResult<ScopedHandle> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let unique_name = if let Some(n) = name {
        format!("Global\\DaemonForge_{}", n)
    } else if let Some(p) = pid_file_path {
        let path_cow = p.to_string_lossy();
        let path_bytes = path_cow.as_bytes();
        let hex_name: String = path_bytes.iter().map(|b| format!("{:02X}", b)).collect();
        format!("Global\\DaemonForge_{}", hex_name)
    } else {
        return Err(DaemonError::TargetLocked); // O quizás un error de config, pero TargetLocked es lo más cercano
    };

    let mut wide_name: Vec<u16> = OsStr::new(&unique_name).encode_wide().collect();
    wide_name.push(0);

    unsafe {
        let handle = win_api::CreateMutexW(std::ptr::null(), 1, wide_name.as_ptr());

        if handle.is_null() {
            return Err(DaemonError::Win32Error(
                io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32,
            ));
        }

        let last_err = io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if last_err == win_api::ERROR_ALREADY_EXISTS {
            win_api::CloseHandle(handle);
            return Err(DaemonError::TargetLocked);
        }

        Ok(ScopedHandle(handle))
    }
}
