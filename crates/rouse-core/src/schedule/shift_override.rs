use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{OverrideId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleOverride {
    id: OverrideId,
    user_id: UserId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl ScheduleOverride {
    pub fn new(user_id: UserId, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            id: OverrideId::new(),
            user_id,
            start,
            end,
        }
    }

    pub fn id(&self) -> &OverrideId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn is_active_at(&self, at: DateTime<Utc>) -> bool {
        at >= self.start && at < self.end
    }

    pub fn start(&self) -> DateTime<Utc> {
        self.start
    }

    pub fn end(&self) -> DateTime<Utc> {
        self.end
    }
}
