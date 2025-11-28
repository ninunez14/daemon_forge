use std::fs::File;

/// Defines the behavior of input/output streams (stdin, stdout, stderr).
#[derive(Debug)]
pub enum Stdio {
    /// Redirects the stream to `/dev/null` (or equivalent on Windows). Default option.
    Devnull,
    /// Redirects the stream to the specified file.
    RedirectToFile(File),
    /// Keeps the original stream (useful for debugging, but not recommended for production).
    Keep,
}

impl Stdio {
    /// Creates a configuration that discards all output.
    pub fn devnull() -> Self {
        Stdio::Devnull
    }
}

impl From<File> for Stdio {
    fn from(f: File) -> Self {
        Stdio::RedirectToFile(f)
    }
}
