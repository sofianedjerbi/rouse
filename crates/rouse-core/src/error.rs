use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("alert is already resolved")]
    AlertAlreadyResolved,
    #[error("schedule requires at least one participant")]
    ScheduleRequiresParticipant,
    #[error("invalid phone format")]
    InvalidPhoneFormat,
    #[error("invalid override period")]
    InvalidOverridePeriod,
    #[error("invalid id: {0}")]
    InvalidId(String),
    #[error("policy requires at least one step")]
    PolicyRequiresStep,
    #[error("step requires at least one target")]
    StepRequiresTarget,
    #[error("step requires a channel")]
    StepRequiresChannel,
}
