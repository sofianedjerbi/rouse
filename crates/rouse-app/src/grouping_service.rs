use chrono::Duration;

use rouse_core::alert::group::AlertGroup;
use rouse_core::alert::grouping::{compute_grouping_key, should_group};
use rouse_core::alert::Alert;
use rouse_core::ids::GroupId;
use rouse_ports::outbound::AlertGroupRepository;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingResult {
    Grouped(GroupId),
    NewGroup(GroupId),
}

pub struct GroupingService<GR>
where
    GR: AlertGroupRepository,
{
    groups: GR,
    window: Duration,
}

impl<GR> GroupingService<GR>
where
    GR: AlertGroupRepository,
{
    pub fn new(groups: GR, window: Duration) -> Self {
        Self { groups, window }
    }

    pub async fn process(&self, alert: &Alert) -> Result<GroupingResult, AppError> {
        let key = compute_grouping_key(alert);

        if let Some(mut group) = self.groups.find_active_by_key(&key).await? {
            if should_group(&group, alert.created_at(), self.window) {
                group.add_member(alert.id().clone(), alert.created_at());
                self.groups.save(&group).await?;
                return Ok(GroupingResult::Grouped(group.id().clone()));
            }
        }

        let group = AlertGroup::new(alert.id().clone(), key, self.window, alert.created_at());
        let group_id = group.id().clone();
        self.groups.save(&group).await?;
        Ok(GroupingResult::NewGroup(group_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rouse_core::alert::{Severity, Source};
    use rouse_ports::error::PortError;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockGroupRepo {
        groups: Mutex<Vec<AlertGroup>>,
    }

    #[async_trait]
    impl AlertGroupRepository for MockGroupRepo {
        async fn save(&self, group: &AlertGroup) -> Result<(), PortError> {
            let mut groups = self.groups.lock().unwrap();
            if let Some(pos) = groups.iter().position(|g| g.id() == group.id()) {
                groups[pos] = group.clone();
            } else {
                groups.push(group.clone());
            }
            Ok(())
        }
        async fn find_active_by_key(&self, key: &str) -> Result<Option<AlertGroup>, PortError> {
            let groups = self.groups.lock().unwrap();
            Ok(groups.iter().find(|g| g.grouping_key() == key).cloned())
        }
    }

    fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn make_alert(source: &str, service: &str, at: chrono::DateTime<chrono::Utc>) -> Alert {
        let labels = BTreeMap::from([("service".into(), service.into())]);
        let (alert, _) = Alert::new(
            "ext-1".into(),
            Source::new(source),
            Severity::Critical,
            labels,
            "test".into(),
            at,
        );
        alert
    }

    fn make_service() -> GroupingService<MockGroupRepo> {
        GroupingService::new(MockGroupRepo::default(), Duration::seconds(30))
    }

    #[tokio::test]
    async fn first_alert_creates_new_group() {
        let svc = make_service();
        let alert = make_alert("am", "api", ts("2025-01-15T10:00:00Z"));

        let result = svc.process(&alert).await.unwrap();
        assert!(matches!(result, GroupingResult::NewGroup(_)));

        let groups = svc.groups.groups.lock().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].member_count(), 1);
    }

    #[tokio::test]
    async fn alerts_within_window_grouped() {
        let svc = make_service();
        let a1 = make_alert("am", "api", ts("2025-01-15T10:00:00Z"));
        let a2 = make_alert("am", "api", ts("2025-01-15T10:00:10Z"));

        let r1 = svc.process(&a1).await.unwrap();
        let r2 = svc.process(&a2).await.unwrap();

        assert!(matches!(r1, GroupingResult::NewGroup(_)));
        assert!(matches!(r2, GroupingResult::Grouped(_)));

        let groups = svc.groups.groups.lock().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].member_count(), 2);
    }

    #[tokio::test]
    async fn alert_outside_window_creates_new_group() {
        let svc = make_service();
        let a1 = make_alert("am", "api", ts("2025-01-15T10:00:00Z"));
        let a2 = make_alert("am", "api", ts("2025-01-15T10:01:00Z")); // 60s > 30s window

        svc.process(&a1).await.unwrap();
        let r2 = svc.process(&a2).await.unwrap();

        assert!(matches!(r2, GroupingResult::NewGroup(_)));

        let groups = svc.groups.groups.lock().unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn different_services_separate_groups() {
        let svc = make_service();
        let a1 = make_alert("am", "api", ts("2025-01-15T10:00:00Z"));
        let a2 = make_alert("am", "payments", ts("2025-01-15T10:00:05Z"));

        svc.process(&a1).await.unwrap();
        svc.process(&a2).await.unwrap();

        let groups = svc.groups.groups.lock().unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn five_alerts_within_window_single_group() {
        let svc = make_service();
        for i in 0..5 {
            let at = ts("2025-01-15T10:00:00Z") + Duration::seconds(i * 2); // 0, 2, 4, 6, 8 seconds
            let alert = make_alert("am", "api", at);
            svc.process(&alert).await.unwrap();
        }

        let groups = svc.groups.groups.lock().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].member_count(), 5);
    }
}
