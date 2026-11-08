use serde::{Serialize, Serializer};

/// Errors returned to the webview.
///
/// `Display` is what the UI sees, so variants that wrap internal errors use a generic
/// message. Log the `Debug` form (`tracing::error!(error = ?e)`) for details.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid input: {0}")]
    Invalid(&'static str),
    #[error("host not found")]
    NotFound,
    #[error("saved hosts file was unreadable; it was moved to {0} and an empty list was started")]
    CorruptConfig(String),
    #[error("could not read or write app data")]
    Io(#[from] std::io::Error),
    #[error("system keychain is unavailable or denied access")]
    Keyring(#[from] keyring::Error),
    #[error("could not encode app data")]
    Json(#[from] serde_json::Error),
    #[error("internal error")]
    Internal,
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
