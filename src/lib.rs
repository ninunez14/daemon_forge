//! # DaemonForge
//!
//! **DaemonForge** is a cross-platform library for creating system daemons (background services) in Rust.
//! It abstracts away the low-level complexities of operating system process management, providing a safe, idiomatic, and ergonomic builder API.
//! 
//! This crate is suitable for learning and experimentation, but not recommended for serious or production projects.
//!


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
