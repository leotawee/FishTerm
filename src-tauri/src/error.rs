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
    #[error(
        "HOST KEY CHANGED: the server now presents {0}. This can mean a man-in-the-middle attack, \
         so the connection was refused. If the server was reinstalled, verify the new fingerprint \
         with its admin, then remove the old line from ~/.ssh/known_hosts."
    )]
    HostKeyChanged(String),
    #[error("host key was not trusted; connection cancelled")]
    HostKeyRejected,
    #[error("could not read or update ~/.ssh/known_hosts")]
    KnownHosts,
    #[error("authentication failed")]
    AuthFailed,
    #[error("connection timed out")]
    Timeout,
    #[error("SSH connection failed")]
    Ssh,
    #[error("no Cloudflare Access application answered on that host; check the tunnel and the URL")]
    CfProbe,
    #[error("Cloudflare Access sign-in was not completed in time; try connecting again")]
    CfLogin,
    #[error("could not fetch the Cloudflare Access token from the transfer service")]
    CfTransfer,
    #[error("could not reach the server through Cloudflare Tunnel")]
    CfWs,
}

impl From<russh::Error> for AppError {
    fn from(e: russh::Error) -> Self {
        tracing::warn!(error = ?e, "ssh error");
        AppError::Ssh
    }
}

impl AppError {
    /// Stable machine-readable code so the UI never parses messages.
    fn code(&self) -> &'static str {
        match self {
            AppError::Invalid(_) => "invalid",
            AppError::NotFound => "not_found",
            AppError::CorruptConfig(_) => "corrupt_config",
            AppError::Io(_) | AppError::Json(_) | AppError::Internal => "internal",
            AppError::Keyring(_) => "keychain",
            AppError::HostKeyChanged(_) => "host_key_changed",
            AppError::HostKeyRejected => "host_key_rejected",
            AppError::KnownHosts => "known_hosts",
            AppError::AuthFailed => "auth_failed",
            AppError::Timeout => "timeout",
            AppError::Ssh => "ssh",
            AppError::CfProbe => "cf_probe",
            AppError::CfLogin => "cf_login",
            AppError::CfTransfer => "cf_transfer",
            AppError::CfWs => "cf_ws",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
