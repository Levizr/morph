use thiserror::Error;

#[derive(Debug, Error)]
pub enum MorphJsError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("emit error: {0}")]
    Emit(String),
    #[error("io error: {0}")]
    Io(String),
}

impl From<anyhow::Error> for MorphJsError {
    fn from(e: anyhow::Error) -> Self {
        Self::Emit(e.to_string())
    }
}
