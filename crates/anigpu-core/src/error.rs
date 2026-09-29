#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid selector: {0}")]
    Selector(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Extraction failed: {0}")]
    Extraction(String),
}

pub type Result<T> = std::result::Result<T, Error>;
