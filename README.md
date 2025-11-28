[![Crates.io](https://img.shields.io/crates/v/daemon_forge.svg)](https://crates.io/crates/daemon_forge)
[![Documentation](https://docs.rs/daemon_forge/badge.svg)](https://docs.rs/daemon_forge)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
# DaemonForge

**DaemonForge** is a cross-platform library for creating system daemons (background services) in Rust.
It abstracts away the low-level complexities of operating system process management, providing a safe, idiomatic, and ergonomic builder API.

This crate is suitable for learning and experimentation, but not recommended for serious or production projects.

## Key Features

* **True Cross-Platform Support**:
    * **Unix/Linux**: Implements the canonical daemonization routine (`double-fork`, `setsid`, `umask`, and signal handling).
    * **Windows**: Uses native "Detached Processes" and manages creation flags for true background execution without a console window (NOT a Windows Service).
* **Locking Mechanism**:
    * Automatically prevents multiple instances of the same service from running simultaneously.
    * Utilizes `flock` (Unix) and **Global Named Mutexes** (Windows) for reliable exclusion.
* **Security First**:
    * Secure environment variable clearing.
    * Support for privilege dropping (User/Group switching) and `chroot` jail on Unix systems.
* **Other features**:
    * **Panic Capture**: Redirects `stdout`/`stderr` to log files, ensuring that panics and crashes are recorded instead of being lost.

## Usage Examples

### Linux/Unix Example

```rust
use daemon_forge::ForgeDaemon;
use std::fs::File;

fn main() {
    let pwd = env::current_dir().unwrap();
    let log_path = pwd.join("log.log");
    let err_path = pwd.join("error.err");
    let pid_path = pwd.join("pid.pid");

    // (Optional) We open them in append mode so we dont erase the history
    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Couldn't open stdout");

    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&err_path)
        .expect("Couldn't open stderr");

    let daemon = ForgeDaemon::new()
        .pid_file(&pid_path)
        .working_directory(&pwd)
        .user("www-data") // Unix specific: drop privileges
        .group("www-data")
        .stdout(stdout_file)
        .stderr(stderr_file)
        .start();

    match daemon {
        Ok(_) => println!("Daemon started successfully"),
        Err(e) => eprintln!("Error starting daemon: {}", e),
    }
}
````

### Windows Example

On Windows, it is highly recommended to set a `.name()` for your daemon. This creates a global mutex to ensure uniqueness.

```rust
use daemon_forge::ForgeDaemon;
use std::fs::File;

fn main() {
    let pwd = env::current_dir().unwrap();
    let log_path = pwd.join("log.log");
    let err_path = pwd.join("error.err");
    let pid_path = pwd.join("pid.pid");

    // (Optional) We open them in append mode so we dont erase the history
    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Couldn't open stdout");

    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&err_path)
        .expect("Couldn't open stderr");

    let daemon = ForgeDaemon::new()
        .name("MyUniqueService") // Creates "Global\DaemonForge_MyUniqueService" Mutex
        .pid_file(&pid_path)
        .working_directory(&pwd)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .start();

    match daemon {
        Ok(_) => println!("Service launched in background"),
        Err(e) => eprintln!("Critical Failure: {}", e),
    }
}
```

## Advanced Configuration

You can execute a `privileged_action` before the process fully daemonizes. This is useful for tasks that require higher permissions (like binding to port 80) or for checking environment prerequisites.

```rust
# use daemon_forge::ForgeDaemon;
ForgeDaemon::new()
    .clear_env(true) // Clears inherited environment variables
    .env("API_KEY", "secret_value") // Sets explicit variables
    .privileged_action(|| {
        println!("Initializing resources before forking...");
        // If this returns Err, the daemon will abort startup.
        Ok("Initialization Done")
    });
```

