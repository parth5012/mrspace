use std::fmt;

#[derive(Debug)]
pub enum MsError {
    Win32(String),
    Com(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    NotFound(String),
    Invalid(String),
}

impl fmt::Display for MsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MsError::Win32(s) => write!(f, "win32: {s}"),
            MsError::Com(s) => write!(f, "com: {s}"),
            MsError::Io(e) => write!(f, "io: {e}"),
            MsError::Json(e) => write!(f, "json: {e}"),
            MsError::NotFound(s) => write!(f, "not found: {s}"),
            MsError::Invalid(s) => write!(f, "invalid: {s}"),
        }
    }
}

impl std::error::Error for MsError {}

impl From<std::io::Error> for MsError {
    fn from(e: std::io::Error) -> Self { MsError::Io(e) }
}
impl From<serde_json::Error> for MsError {
    fn from(e: serde_json::Error) -> Self { MsError::Json(e) }
}
pub type Result<T> = std::result::Result<T, MsError>;