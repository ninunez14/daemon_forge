//! # DaemonForge
//!
//! **DaemonForge** is a modern, cross-platform library for creating system daemons (background services) in Rust.
//! It abstracts away the low-level complexities of operating system process management, providing a safe, idiomatic, and ergonomic builder API.
//!
//! ## Key Features
//!
//! * **True Cross-Platform Support**:
//!     * **Unix/Linux**: Implements the canonical daemonization routine (`double-fork`, `setsid`, `umask`, and signal handling).
//!     * **Windows**: Uses native "Detached Processes" and manages creation flags for true background execution without a console window.
//! * **Locking Mechanism**:
//!     * Automatically prevents multiple instances of the same service from running simultaneously.
//!     * Utilizes `flock` (Unix) and **Global Named Mutexes** (Windows) for reliable exclusion.
//! * **Security First**:
//!     * Secure environment variable clearing.
//!     * Support for privilege dropping (User/Group switching) and `chroot` jail on Unix systems.
//! * **Other features**:
//!     * **Panic Capture**: Redirects `stdout`/`stderr` to log files, ensuring that panics and crashes are recorded instead of being lost.
//!
//! ## Usage Examples
//!
//! ### Linux/Unix Example
//!
//! ```no_run
//! use daemon_forge::ForgeDaemon;
//! use std::fs::File;
//!
//! fn main() {
//!     let stdout = File::create("/tmp/daemon.out").unwrap();
//!     let stderr = File::create("/tmp/daemon.err").unwrap();
//!
//!     let daemon = ForgeDaemon::new()
//!         .pid_file("/tmp/test.pid")
//!         .working_directory("/tmp")
//!         .user("www-data") // Unix specific: drop privileges
//!         .group("www-data")
//!         .stdout(stdout)
//!         .stderr(stderr)
//!         .start();
//!
//!     match daemon {
//!         Ok(_) => println!("Daemon started successfully"),
//!         Err(e) => eprintln!("Error starting daemon: {}", e),
//!     }
//! }
//! ```
//!
//! ### 🪟 Windows Example
//!
//! On Windows, it is highly recommended to set a `.name()` for your daemon. This creates a global mutex to ensure uniqueness.
//!
//! ```no_run
//! use daemon_forge::ForgeDaemon;
//! use std::fs::File;
//!
//! fn main() {
//!     // Use absolute paths on Windows for safety
//!     let stdout = File::create("C:\\Logs\\service.out").unwrap();
//!     let stderr = File::create("C:\\Logs\\service.err").unwrap();
//!
//!     let daemon = ForgeDaemon::new()
//!         .name("MyUniqueService") // Creates "Global\DaemonForge_MyUniqueService" Mutex
//!         .pid_file("C:\\Logs\\service.pid")
//!         .working_directory("C:\\Logs")
//!         .stdout(stdout)
//!         .stderr(stderr)
//!         .start();
//!
//!     match daemon {
//!         Ok(_) => println!("Service launched in background"),
//!         Err(e) => eprintln!("Critical Failure: {}", e),
//!     }
//! }
//! ```
//!
//! ## Advanced Configuration
//!
//! You can execute a `privileged_action` before the process fully daemonizes. This is useful for tasks that require higher permissions (like binding to port 80) or for checking environment prerequisites.
//!
//! ```rust
//! # use daemon_forge::ForgeDaemon;
//! ForgeDaemon::new()
//!     .clear_env(true) // Clears inherited environment variables
//!     .env("API_KEY", "secret_value") // Sets explicit variables
//!     .privileged_action(|| {
//!         println!("Initializing resources before forking...");
//!         // If this returns Err, the daemon will abort startup.
//!         Ok("Initialization Done")
//!     });
//! ```

mod daemon;
mod error;
mod stdio;
mod sys;
mod types;

// Re-export public types to keeping the API flat
pub use daemon::ForgeDaemon;
pub use error::{DaemonError, DaemonResult};
pub use stdio::Stdio;
pub use types::{Group, User};
