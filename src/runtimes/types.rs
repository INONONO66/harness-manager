use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DetectedRuntime {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub binary_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub auth_sources: Vec<AuthStatus>,
}

#[derive(Debug, Clone)]
pub enum AuthStatus {
    Valid { detail: String },
    ExpiresSoon { detail: String },
    Expired { detail: String },
    NotConfigured,
}

impl AuthStatus {
    pub fn status_icon(&self) -> &str {
        match self {
            Self::Valid { .. } => "\u{2705}",
            Self::ExpiresSoon { .. } => "\u{26a0}\u{fe0f}",
            Self::Expired { .. } => "\u{274c}",
            Self::NotConfigured => "\u{274c}",
        }
    }

    pub fn status_text(&self) -> String {
        match self {
            Self::Valid { detail } => format!("Valid ({})", detail),
            Self::ExpiresSoon { detail } => format!("Expires soon ({})", detail),
            Self::Expired { detail } => format!("Expired ({})", detail),
            Self::NotConfigured => "Not configured".to_string(),
        }
    }
}

impl fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status_icon(), self.status_text())
    }
}
