```markdown
[![Crates.io](https://img.shields.io/crates/v/daemon_forge.svg)](https://crates.io/crates/daemon_forge)
[![Documentation](https://docs.rs/daemon_forge/badge.svg)](https://docs.rs/daemon_forge)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

# DaemonForge

**DaemonForge** is a cross-platform library for creating system daemons (background services) in Rust.
It abstracts away the low-level complexities of operating system process management, providing a safe, idiomatic, and ergonomic builder API.

This crate is suitable for learning and experimentation, but not recommended for serious or production projects.

## Key Features

* **True Cross-Platform Support**:
    * **Unix/Linux (Hybrid Mode)**:
        * **Systemd Native**: Automatically detects if running under Systemd (`NOTIFY_SOCKET`). If detected, it stays in the foreground and sends `READY=1` notifications via `sd-notify`.
        * **Legacy Daemonization**: If run manually, it falls back to the canonical `double-fork` routine (`setsid`, `umask`, standard IO redirection) to run in the background.
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

### Linux/Unix Example (Systemd & Manual Compatible)

The library now uses an "Inversion of Control" pattern. You pass your main loop closure to `privileged_action`.

> **Note:** For proper Systemd support, your loop must handle termination signals (like `SIGTERM`) to exit cleanly.

```rust
use daemon_forge::ForgeDaemon;
use std::fs::OpenOptions;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use signal_hook::consts::signal::*;
use signal_hook::flag;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pwd = std::env::current_dir().unwrap();
    let log_path = pwd.join("service.log");
    let err_path = pwd.join("service.err");
    let pid_path = pwd.join("service.pid");

    // Open logs in append mode
    let stdout_file = OpenOptions::new().create(true).append(true).open(&log_path)?;
    let stderr_file = OpenOptions::new().create(true).append(true).open(&err_path)?;

    let daemon = ForgeDaemon::new()
        .name("my_service")
        .pid_file(&pid_path)
        .working_directory(&pwd)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .privileged_action(move || {
            // --- MAIN DAEMON LOGIC ---
            
            // 1. Setup Signal Handling
            let term = Arc::new(AtomicBool::new(false));
            flag::register(SIGTERM, Arc::clone(&term))?; // Systemd Stop
            flag::register(SIGINT, Arc::clone(&term))?;  // Manual Ctrl+C

            println!("Service started. PID: {}", std::process::id());

            // 2. Main Loop
            while !term.load(Ordering::Relaxed) {
                // Your logic here...
                println!("Working...");
                
                // Reactive sleep (check for stop signal frequently)
                for _ in 0..10 {
                    if term.load(Ordering::Relaxed) { break; }
                    thread::sleep(Duration::from_millis(100));
                }
            }

            println!("Shutdown signal received.");
            Ok(())
        });

    // Automatically decides between Systemd (foreground) or Fork (background)
    daemon_forge::unix::start(daemon)?;

    Ok(())
}

```

### Windows Example

On Windows, it is highly recommended to set a `.name()` for your daemon. This creates a global mutex to ensure uniqueness.

```rust
use daemon_forge::ForgeDaemon;
use std::fs::OpenOptions;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        .privileged_action(|| {
            // Your Windows Service logic here
            println!("I am running in the background on Windows!");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            Ok(())
        });

    // Launches the detached process
    daemon.start();

    println!("Launcher finished. Service is running in background.");
    Ok(())
}

```

## Systemd Integration (Linux)

To use the **Systemd** features, create a service file at `/etc/systemd/system/my_service.service`.

**Crucial Settings:**

1. `Type=notify`: Tells Systemd to wait for the `READY=1` signal sent by DaemonForge.
2. `ExecStart`: Must be the **absolute path** to your binary.

```ini
[Unit]
Description=My Rust Daemon

[Service]
# Important: This allows DaemonForge to handshake with Systemd
Type=notify

# Replace with your actual path
ExecStart=/path/to/your/release/binary

# Logs will appear in `journalctl -u my_service`
StandardOutput=journal
StandardError=journal

Restart=on-failure

[Install]
WantedBy=multi-user.target

```

## Advanced Configuration

You can execute a `privileged_action` before the process fully daemonizes. This is useful for tasks that require higher permissions (like binding to port 80) or for checking environment prerequisites.

```rust
# use daemon_forge::ForgeDaemon;
ForgeDaemon::new()
    .clear_env(true) // Clears inherited environment variables
    .env("API_KEY", "secret_value") // Sets explicit variables
    .privileged_action(|| {
        println!("Initializing resources...");
        // If this returns Err, the daemon will abort startup.
        Ok("Initialization Done")
    });

```

```

```