use thiserror::Error;

pub type CliResult<T> = Result<T, CliError>;

/// Stable operational failures from `pdcan`.
///
/// Clap reports command-line syntax failures before this boundary. These
/// variants describe failures which occur after a valid command was accepted.
#[derive(Debug, Error)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum CliError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    Artifact(String),
    #[error("{0}")]
    Transport(String),
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Codec(String),
    #[error("{0}")]
    NodeRejected(String),
    #[error("node reports interrupt update impact; pass --allow-interruption to activate")]
    InterruptionRequired,
    #[error("{0}")]
    Internal(String),
}

impl CliError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid_input",
            Self::Artifact(_) => "artifact",
            Self::Transport(_) => "transport",
            Self::Timeout(_) => "timeout",
            Self::Codec(_) => "codec",
            Self::NodeRejected(_) => "node_rejected",
            Self::InterruptionRequired => "interruption_required",
            Self::Internal(_) => "internal",
        }
    }

    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::InvalidInput(_) => 2,
            Self::Artifact(_) => 3,
            Self::Transport(_) => 4,
            Self::Timeout(_) => 5,
            Self::Codec(_) => 6,
            Self::NodeRejected(_) => 7,
            Self::InterruptionRequired => 8,
            Self::Internal(_) => 1,
        }
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "error": {
                "code": self.code(),
                "message": self.to_string(),
            }
        })
    }
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::InvalidInput(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::InvalidInput(message.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_error_contract_uses_stable_code() {
        let error = CliError::InterruptionRequired;
        assert_eq!(error.code(), "interruption_required");
        assert_eq!(error.exit_code(), 8);
        assert_eq!(error.json()["ok"], false);
        assert_eq!(error.json()["error"]["code"], "interruption_required");
    }
}
