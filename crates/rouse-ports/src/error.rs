use thiserror::Error;

#[derive(Debug, Error)]
pub enum PortError {
    #[error("not found")]
    NotFound,
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("connection error: {0}")]
    Connection(String),
}

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("channel unavailable")]
    ChannelUnavailable,
    #[error("rate limited")]
    RateLimited,
    #[error("invalid target")]
    InvalidTarget,
    #[error("delivery failed: {0}")]
    DeliveryFailed(String),
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
}
