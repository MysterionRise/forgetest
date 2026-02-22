//! Provider error types.

use thiserror::Error;

/// Errors that can occur when interacting with an LLM provider.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// The API returned a 429 rate limit response.
    #[error("rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    /// Authentication failed (invalid API key).
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// The requested model was not found.
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// The API returned an error response.
    #[error("API error (HTTP {status}): {message}")]
    ApiError { status: u16, message: String },

    /// The request timed out.
    #[error("request timed out after {0}s")]
    Timeout(u64),

    /// A network error occurred.
    #[error("network error: {0}")]
    NetworkError(String),
}
