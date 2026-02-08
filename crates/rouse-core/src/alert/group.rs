use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AlertId, GroupId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertGroup {
    id: GroupId,
    root_alert_id: AlertId,
    member_alert_ids: Vec<AlertId>,
    grouping_key: String,
    window_secs: i64,
    created_at: DateTime<Utc>,
    last_added_at: DateTime<Utc>,
}

impl AlertGroup {
    pub fn new(
        root_alert_id: AlertId,
        grouping_key: String,
        window: Duration,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: GroupId::new(),
            root_alert_id: root_alert_id.clone(),
            member_alert_ids: vec![root_alert_id],
            grouping_key,
            window_secs: window.num_seconds(),
            created_at: now,
            last_added_at: now,
        }
    }

    pub fn add_member(&mut self, alert_id: AlertId, now: DateTime<Utc>) {
        self.member_alert_ids.push(alert_id);
        self.last_added_at = now;
    }

    pub fn member_count(&self) -> usize {
        self.member_alert_ids.len()
    }

    pub fn id(&self) -> &GroupId {
        &self.id
    }

    pub fn root_alert_id(&self) -> &AlertId {
        &self.root_alert_id
    }

    pub fn grouping_key(&self) -> &str {
        &self.grouping_key
    }

    pub fn window(&self) -> Duration {
        Duration::seconds(self.window_secs)
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn last_added_at(&self) -> DateTime<Utc> {
        self.last_added_at
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

    #[test]
    fn new_group_has_root_as_first_member() {
        let root = AlertId::new();
        let group = AlertGroup::new(
            root.clone(),
            "src:api".into(),
            Duration::seconds(30),
            ts("2025-01-15T10:00:00Z"),
        );
        assert_eq!(group.member_count(), 1);
        assert_eq!(group.root_alert_id(), &root);
    }

    #[test]
    fn add_member_increments_count_and_updates_last_added() {
        let mut group = AlertGroup::new(
            AlertId::new(),
            "src:api".into(),
            Duration::seconds(30),
            ts("2025-01-15T10:00:00Z"),
        );

        group.add_member(AlertId::new(), ts("2025-01-15T10:00:05Z"));
        assert_eq!(group.member_count(), 2);
        assert_eq!(group.last_added_at(), ts("2025-01-15T10:00:05Z"));

        group.add_member(AlertId::new(), ts("2025-01-15T10:00:10Z"));
        assert_eq!(group.member_count(), 3);
        assert_eq!(group.last_added_at(), ts("2025-01-15T10:00:10Z"));
    }
}
