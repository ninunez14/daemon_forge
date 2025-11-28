// =========================================================================
// Compatibility Types (User/Group)
// =========================================================================

// --- WINDOWS (Dummy Types for API compatibility) ---
#[cfg(not(unix))]
#[derive(Debug, Clone)]
#[allow(dead_code)] // We ignore warning because fields are not read on Windows
pub struct User(String);

#[cfg(not(unix))]
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Group(String);

#[cfg(not(unix))]
impl From<&str> for User {
    fn from(s: &str) -> Self {
        User(s.to_owned())
    }
}
#[cfg(not(unix))]
impl From<&str> for Group {
    fn from(s: &str) -> Self {
        Group(s.to_owned())
    }
}
#[cfg(not(unix))]
impl From<u32> for User {
    fn from(id: u32) -> Self {
        User(id.to_string())
    }
}
#[cfg(not(unix))]
impl From<u32> for Group {
    fn from(id: u32) -> Self {
        Group(id.to_string())
    }
}

// --- UNIX (Real Types) ---
#[cfg(unix)]
pub use unix_types::{Group, User};

#[cfg(unix)]
mod unix_types {
    /// Represents a system user (Unix).
    #[derive(Debug, Clone)]
    pub struct User(pub String);
    /// Represents a system group (Unix).
    #[derive(Debug, Clone)]
    pub struct Group(pub String);

    impl From<&str> for User {
        fn from(s: &str) -> Self {
            User(s.to_owned())
        }
    }
    impl From<&str> for Group {
        fn from(s: &str) -> Self {
            Group(s.to_owned())
        }
    }
    impl From<u32> for User {
        fn from(id: u32) -> Self {
            User(id.to_string())
        }
    }
    impl From<u32> for Group {
        fn from(id: u32) -> Self {
            Group(id.to_string())
        }
    }
}
