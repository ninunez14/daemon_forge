use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io::Write; 
use std::fmt; 
use crate::stdio::Stdio;
use crate::error::{DaemonResult, DaemonError};

/// Main constructor to configure and launch the daemon process.
///
/// `SetupOutput` represents the return type of the privileged setup action.
/// The daemonized process will return this value wrapped in a `DaemonResult`.
pub struct ForgeDaemon<SetupOutput> {
    pub(crate) name: Option<String>,
    pub(crate) directory: PathBuf,
    pub(crate) pid_file: Option<PathBuf>,
    pub(crate) stdin: Stdio,
    pub(crate) stdout: Stdio,
    pub(crate) stderr: Stdio,
    
    // Environment Configuration
    pub(crate) clear_env: bool,
    pub(crate) env_vars: HashMap<String, String>,

    // Unix specific configuration
    #[cfg(unix)] pub(crate) user: Option<User>,
    #[cfg(unix)] pub(crate) group: Option<Group>,
    #[cfg(unix)] pub(crate) umask: Option<u32>,
    #[cfg(unix)] pub(crate) root: Option<PathBuf>,
    #[cfg(unix)] pub(crate) chown_pid: bool,

    // The action now returns a Result
    pub(crate) privileged_action: Option<Box<dyn FnOnce() -> DaemonResult<SetupOutput>>>,
}

// C-DEBUG: Implementación manual de Debug porque el closure no lo soporta.
impl<T> fmt::Debug for ForgeDaemon<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("ForgeDaemon");
        ds.field("name", &self.name)
          .field("directory", &self.directory)
          .field("pid_file", &self.pid_file)
          .field("stdin", &self.stdin)
          .field("stdout", &self.stdout)
          .field("stderr", &self.stderr)
          .field("clear_env", &self.clear_env)
          .field("env_vars", &self.env_vars);

        #[cfg(unix)]
        {
            ds.field("user", &self.user)
              .field("group", &self.group)
              .field("umask", &self.umask)
              .field("root", &self.root)
              .field("chown_pid", &self.chown_pid);
        }

        // Indicamos que existe una acción, pero opaca
        ds.field("privileged_action", &if self.privileged_action.is_some() { "Some(FnOnce)" } else { "None" })
          .finish()
    }
}

impl Default for ForgeDaemon<()> {
    fn default() -> Self { Self::new() }
}

impl ForgeDaemon<()> {
    /// Creates a new default configuration.
    /// 
    /// # Defaults
    /// - Working directory: `/` (Unix) or `C:\` (Windows)
    /// - Stdio: `/dev/null`
    /// - Umask: `0o027` (Unix)
    pub fn new() -> Self {
        ForgeDaemon {
            name: None,
            #[cfg(unix)]
            directory: PathBuf::from("/"),
            #[cfg(windows)]
            directory: PathBuf::from("C:\\"),

            pid_file: None,
            stdin: Stdio::devnull(),
            stdout: Stdio::devnull(),
            stderr: Stdio::devnull(),
            clear_env: false,
            env_vars: HashMap::new(),

            #[cfg(unix)] user: None,
            #[cfg(unix)] group: None,
            #[cfg(unix)] umask: Some(0o027),
            #[cfg(unix)] root: None,
            #[cfg(unix)] chown_pid: false,

            privileged_action: Some(Box::new(|| Ok(()))),
        }
    }
}

impl<SetupOutput> ForgeDaemon<SetupOutput> {
    
    // --- Public Getters ---

    /// Returns the daemon name if set.
    pub fn get_name(&self) -> Option<&str> { self.name.as_deref() }
    
    /// Returns the configured PID file path, if any.
    pub fn pid_file_path(&self) -> Option<&Path> { self.pid_file.as_deref() }
    
    /// Returns a reference to the environment variables map.
    pub fn environment(&self) -> &HashMap<String, String> { &self.env_vars }
    
    /// Returns the configured working directory.
    pub fn working_directory_path(&self) -> &Path { &self.directory }

    // --- Builder Methods ---

    /// Sets the internal name of the daemon.
    /// 
    /// On Windows, this name is used to create a Global Mutex for single-instance locking.
    pub fn name(mut self, name: &str) -> Self { self.name = Some(name.to_owned()); self }
    
    /// Sets the path to the PID file.
    /// This file is used for locking to ensure only one instance runs.
    pub fn pid_file<P: Into<PathBuf>>(mut self, path: P) -> Self { self.pid_file = Some(path.into()); self }
    
    /// Sets the working directory for the daemon.
    pub fn working_directory<P: Into<PathBuf>>(mut self, path: P) -> Self { self.directory = path.into(); self }
    
    /// Configures the standard input stream.
    pub fn stdin<S: Into<Stdio>>(mut self, stdio: S) -> Self { self.stdin = stdio.into(); self }
    
    /// Configures the standard output stream.
    pub fn stdout<S: Into<Stdio>>(mut self, stdio: S) -> Self { self.stdout = stdio.into(); self }
    
    /// Configures the standard error stream.
    pub fn stderr<S: Into<Stdio>>(mut self, stdio: S) -> Self { self.stderr = stdio.into(); self }
    
    /// If `true`, clears all inherited environment variables for security.
    pub fn clear_env(mut self, clear: bool) -> Self { self.clear_env = clear; self }
    
    /// Adds or overwrites an environment variable.
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.insert(key.to_owned(), value.to_owned());
        self
    }

    /// Adds an environment variable only if `value` is `Some`.
    pub fn env_opt(mut self, key: &str, value: Option<&str>) -> Self {
        if let Some(v) = value {
            self.env_vars.insert(key.to_owned(), v.to_owned());
        }
        self
    }

    /// Inherits current environment variables into the configuration.
    /// 
    /// Useful when combined with `clear_env(true)` to selectively keep variables,
    /// or to ensure specific variables are captured before cleaning.
    pub fn inherit_env(mut self) -> Self {
        for (k, v) in std::env::vars() {
            self.env_vars.entry(k).or_insert(v);
        }
        self
    }

    /// Validates configuration without starting the daemon.
    /// Checks if the PID file directory exists.
    pub fn build(self) -> DaemonResult<Self> {
        if let Some(pid) = &self.pid_file {
            if pid.parent().map(|p| !p.exists()).unwrap_or(false) {
                return Err(DaemonError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound, 
                    "PID file directory does not exist"
                )));
            }
        }
        Ok(self)
    }

    /// Executes an action before dropping privileges (Unix) or before entering the main loop.
    /// 
    /// The action MUST return a `DaemonResult`. If it returns `Err`, the daemon will abort startup.
    /// This consumes the current builder and returns a new one with the updated generic type `N`.
    pub fn privileged_action<N, F>(self, action: F) -> ForgeDaemon<N> 
    where 
        F: FnOnce() -> DaemonResult<N> + 'static 
    {
        ForgeDaemon {
            name: self.name,
            directory: self.directory,
            pid_file: self.pid_file,
            stdin: self.stdin,
            stdout: self.stdout,
            stderr: self.stderr,
            clear_env: self.clear_env,
            env_vars: self.env_vars,
            #[cfg(unix)] user: self.user,
            #[cfg(unix)] group: self.group,
            #[cfg(unix)] umask: self.umask,
            #[cfg(unix)] root: self.root,
            #[cfg(unix)] chown_pid: self.chown_pid,
            privileged_action: Some(Box::new(action)),
        }
    }

    // --- Unix exclusive methods ---
    
    /// (Unix) Sets the user to run the daemon as (privilege dropping).
    #[cfg(unix)] pub fn user<U: Into<User>>(mut self, user: U) -> Self { self.user = Some(user.into()); self }
    #[cfg(not(unix))] pub fn user<U>(self, _: U) -> Self { self }

    /// (Unix) Sets the group to run the daemon as.
    #[cfg(unix)] pub fn group<G: Into<Group>>(mut self, group: G) -> Self { self.group = Some(group.into()); self }
    #[cfg(not(unix))] pub fn group<G>(self, _: G) -> Self { self }

    /// (Unix) Sets the umask for the daemon process.
    #[cfg(unix)] pub fn umask(mut self, mask: u32) -> Self { self.umask = Some(mask); self }
    #[cfg(not(unix))] pub fn umask(self, _: u32) -> Self { self }

    /// (Unix) Sets a chroot directory for the daemon.
    #[cfg(unix)] pub fn chroot<P: Into<PathBuf>>(mut self, path: P) -> Self { self.root = Some(path.into()); self }
    #[cfg(not(unix))] pub fn chroot<P>(self, _: P) -> Self { self }

    /// (Unix) If true, changes ownership of the PID file to the target user/group.
    #[cfg(unix)] pub fn chown_pid_file(mut self, chown: bool) -> Self { self.chown_pid = chown; self }
    #[cfg(not(unix))] pub fn chown_pid_file(self, _: bool) -> Self { self }

    /// Starts the daemonization process.
    pub fn start(self) -> DaemonResult<SetupOutput> {
        #[cfg(unix)]
        return crate::sys::unix::start(self);

        #[cfg(windows)]
        return crate::sys::windows::start(self);
    }

    pub(crate) fn log_error(&mut self, msg: &str) {
        let msg_formatted = format!("[DaemonForge Critical] {}", msg);
        if let Stdio::RedirectToFile(ref mut f) = self.stderr {
             let _ = writeln!(f, "{}", msg_formatted);
             let _ = f.sync_all();
        } 
        else if let Stdio::RedirectToFile(ref mut f) = self.stdout {
             let _ = writeln!(f, "{}", msg_formatted);
             let _ = f.sync_all();
        }
    }
}