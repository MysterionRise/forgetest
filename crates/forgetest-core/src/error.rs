//! Provider error types.
//!
//! These error types represent failures when interacting with LLM providers.
//! Defined in `forgetest-core` so the eval engine can downcast and classify
//! errors for retry decisions without string matching.

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

impl ProviderError {
    /// Returns `true` if this error is permanent and should not be retried.
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            ProviderError::AuthenticationFailed(_) | ProviderError::ModelNotFound(_)
        )
    }

    /// Returns the retry-after delay in milliseconds, if applicable.
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            _ => None,
        }
    }
}
