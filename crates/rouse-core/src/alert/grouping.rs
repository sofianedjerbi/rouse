use chrono::Duration;

use super::group::AlertGroup;
use super::Alert;

/// Deterministic grouping key from source + service label.
pub fn compute_grouping_key(alert: &Alert) -> String {
    let source = alert.source().as_str();
    match alert.labels().get("service") {
        Some(service) => format!("{source}:{service}"),
        None => source.to_string(),
    }
}

/// Pure time-window check: is the new alert within the group's window?
pub fn should_group(
    existing_group: &AlertGroup,
    new_alert_created_at: chrono::DateTime<chrono::Utc>,
    window: Duration,
) -> bool {
    new_alert_created_at < existing_group.last_added_at() + window
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::{Severity, Source};
    use crate::ids::AlertId;
    use chrono::{DateTime, Utc};
    use std::collections::BTreeMap;

    fn ts(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn make_alert(source: &str, service: &str) -> Alert {
        let labels = BTreeMap::from([("service".into(), service.into())]);
        let (alert, _) = Alert::new(
            "ext-1".into(),
            Source::new(source),
            Severity::Critical,
            labels,
            "test".into(),
            ts("2025-01-15T10:00:00Z"),
        );
        alert
    }

    #[test]
    fn grouping_key_deterministic() {
        let a1 = make_alert("alertmanager", "api");
        let a2 = make_alert("alertmanager", "api");
        assert_eq!(compute_grouping_key(&a1), compute_grouping_key(&a2));
    }

    #[test]
    fn different_sources_different_keys() {
        let a1 = make_alert("alertmanager", "api");
        let a2 = make_alert("datadog", "api");
        assert_ne!(compute_grouping_key(&a1), compute_grouping_key(&a2));
    }

    #[test]
    fn different_services_different_keys() {
        let a1 = make_alert("alertmanager", "api");
        let a2 = make_alert("alertmanager", "payments");
        assert_ne!(compute_grouping_key(&a1), compute_grouping_key(&a2));
    }

    #[test]
    fn alert_within_window_groups() {
        let group = AlertGroup::new(
            AlertId::new(),
            "am:api".into(),
            Duration::seconds(30),
            ts("2025-01-15T10:00:00Z"),
        );
        // 10 seconds later — within 30s window
        assert!(should_group(
            &group,
            ts("2025-01-15T10:00:10Z"),
            Duration::seconds(30)
        ));
    }

    #[test]
    fn alert_outside_window_does_not_group() {
        let group = AlertGroup::new(
            AlertId::new(),
            "am:api".into(),
            Duration::seconds(30),
            ts("2025-01-15T10:00:00Z"),
        );
        // 60 seconds later — outside 30s window
        assert!(!should_group(
            &group,
            ts("2025-01-15T10:01:00Z"),
            Duration::seconds(30)
        ));
    }
}
