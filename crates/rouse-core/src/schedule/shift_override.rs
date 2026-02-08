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

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn make_override() -> ScheduleOverride {
        ScheduleOverride::new(
            UserId::new(),
            ts("2025-01-14T00:00:00Z"),
            ts("2025-01-15T00:00:00Z"),
        )
    }

    #[test]
    fn is_active_during_period() {
        let ovr = make_override();
        assert!(ovr.is_active_at(ts("2025-01-14T12:00:00Z")));
    }

    #[test]
    fn is_active_at_start_inclusive() {
        let ovr = make_override();
        assert!(ovr.is_active_at(ts("2025-01-14T00:00:00Z")));
    }

    #[test]
    fn is_not_active_at_end_exclusive() {
        let ovr = make_override();
        assert!(!ovr.is_active_at(ts("2025-01-15T00:00:00Z")));
    }

    #[test]
    fn is_not_active_before_start() {
        let ovr = make_override();
        assert!(!ovr.is_active_at(ts("2025-01-13T23:59:59Z")));
    }

    #[test]
    fn is_not_active_after_end() {
        let ovr = make_override();
        assert!(!ovr.is_active_at(ts("2025-01-15T00:00:01Z")));
    }
}
