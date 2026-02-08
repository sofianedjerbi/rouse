use rouse_core::error::DomainError;
use rouse_ports::error::{ParseError, PortError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("domain error: {0}")]
    Domain(#[from] DomainError),
    #[error("port error: {0}")]
    Port(#[from] PortError),
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("routing error: {0}")]
    Routing(String),
}
