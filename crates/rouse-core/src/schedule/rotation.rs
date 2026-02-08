use chrono::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rotation {
    Daily,
    Weekly,
    Custom(i64), // duration in seconds
}

impl Rotation {
    pub fn duration(&self) -> Duration {
        match self {
            Self::Daily => Duration::days(1),
            Self::Weekly => Duration::weeks(1),
            Self::Custom(secs) => Duration::seconds(*secs),
        }
    }
}
