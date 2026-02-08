pub mod fingerprint;
pub mod severity;
pub mod source;
pub mod status;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::events::{AlertAcknowledged, AlertReceived, AlertResolved, DomainEvent};
use crate::ids::{AlertId, UserId};

pub use fingerprint::Fingerprint;
pub use severity::Severity;
pub use source::Source;
pub use status::Status;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    id: AlertId,
    external_id: String,
    source: Source,
    severity: Severity,
    status: Status,
    fingerprint: Fingerprint,
    labels: BTreeMap<String, String>,
    summary: String,
    created_at: DateTime<Utc>,
    acknowledged_at: Option<DateTime<Utc>>,
    acknowledged_by: Option<UserId>,
    resolved_at: Option<DateTime<Utc>>,
}

impl Alert {
    pub fn new(
        external_id: String,
        source: Source,
        severity: Severity,
        labels: BTreeMap<String, String>,
        summary: String,
        now: DateTime<Utc>,
    ) -> (Self, Vec<DomainEvent>) {
        let id = AlertId::new();
        let fingerprint = Fingerprint::from_labels(&labels);
        let alert = Self {
            id: id.clone(),
            external_id,
            source: source.clone(),
            severity,
            status: Status::Firing,
            fingerprint,
            labels,
            summary,
            created_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
        };
        let events = vec![DomainEvent::AlertReceived(AlertReceived {
            alert_id: id,
            source: source.as_str().to_string(),
            severity,
            occurred_at: now,
        })];
        (alert, events)
    }

    pub fn acknowledge(
        &mut self,
        user_id: UserId,
        now: DateTime<Utc>,
    ) -> Result<Vec<DomainEvent>, DomainError> {
        match self.status {
            Status::Resolved => Err(DomainError::AlertAlreadyResolved),
            Status::Acknowledged => Ok(vec![]),
            Status::Firing => {
                self.status = Status::Acknowledged;
                self.acknowledged_at = Some(now);
                self.acknowledged_by = Some(user_id.clone());
                Ok(vec![DomainEvent::AlertAcknowledged(AlertAcknowledged {
                    alert_id: self.id.clone(),
                    user_id,
                    occurred_at: now,
                })])
            }
        }
    }

    pub fn resolve(
        &mut self,
        resolved_by: String,
        now: DateTime<Utc>,
    ) -> Result<Vec<DomainEvent>, DomainError> {
        match self.status {
            Status::Resolved => Ok(vec![]),
            Status::Firing | Status::Acknowledged => {
                self.status = Status::Resolved;
                self.resolved_at = Some(now);
                Ok(vec![DomainEvent::AlertResolved(AlertResolved {
                    alert_id: self.id.clone(),
                    resolved_by,
                    occurred_at: now,
                })])
            }
        }
    }

    pub fn id(&self) -> &AlertId {
        &self.id
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn labels(&self) -> &BTreeMap<String, String> {
        &self.labels
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn acknowledged_by(&self) -> Option<&UserId> {
        self.acknowledged_by.as_ref()
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_labels() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("alertname".into(), "HighCPU".into()),
            ("instance".into(), "web-01".into()),
        ])
    }

    fn now() -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339("2025-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn make_alert() -> Alert {
        let (alert, _) = Alert::new(
            "ext-123".into(),
            Source::new("alertmanager"),
            Severity::Critical,
            make_labels(),
            "CPU is high".into(),
            now(),
        );
        alert
    }

    #[test]
    fn new_alert_status_is_firing() {
        let alert = make_alert();
        assert_eq!(alert.status(), Status::Firing);
    }

    #[test]
    fn new_alert_fingerprint_is_deterministic() {
        let labels = make_labels();
        let (a1, _) = Alert::new(
            "ext-1".into(),
            Source::new("src"),
            Severity::Info,
            labels.clone(),
            "s".into(),
            now(),
        );
        let (a2, _) = Alert::new(
            "ext-2".into(),
            Source::new("src"),
            Severity::Info,
            labels,
            "s".into(),
            now(),
        );
        assert_eq!(a1.fingerprint(), a2.fingerprint());
    }

    #[test]
    fn acknowledge_from_firing_succeeds() {
        let mut alert = make_alert();
        let result = alert.acknowledge(UserId::new(), now());
        assert!(result.is_ok());
        assert_eq!(alert.status(), Status::Acknowledged);
    }

    #[test]
    fn acknowledge_from_resolved_fails() {
        let mut alert = make_alert();
        alert.resolve("source".into(), now()).unwrap();
        let result = alert.acknowledge(UserId::new(), now());
        assert_eq!(result, Err(DomainError::AlertAlreadyResolved));
    }

    #[test]
    fn acknowledge_returns_event() {
        let mut alert = make_alert();
        let user = UserId::new();
        let events = alert.acknowledge(user, now()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "alert.acknowledged");
    }

    #[test]
    fn resolve_from_firing_succeeds() {
        let mut alert = make_alert();
        let events = alert.resolve("operator".into(), now()).unwrap();
        assert_eq!(alert.status(), Status::Resolved);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "alert.resolved");
    }

    #[test]
    fn resolve_from_acknowledged_succeeds() {
        let mut alert = make_alert();
        alert.acknowledge(UserId::new(), now()).unwrap();
        let events = alert.resolve("operator".into(), now()).unwrap();
        assert_eq!(alert.status(), Status::Resolved);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn resolve_already_resolved_is_noop() {
        let mut alert = make_alert();
        alert.resolve("a".into(), now()).unwrap();
        let events = alert.resolve("b".into(), now()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn fingerprint_ignores_label_order() {
        // BTreeMap is inherently sorted, so insertion order doesn't matter.
        let mut labels_a = BTreeMap::new();
        labels_a.insert("z".into(), "1".into());
        labels_a.insert("a".into(), "2".into());

        let mut labels_b = BTreeMap::new();
        labels_b.insert("a".into(), "2".into());
        labels_b.insert("z".into(), "1".into());

        let fp_a = Fingerprint::from_labels(&labels_a);
        let fp_b = Fingerprint::from_labels(&labels_b);
        assert_eq!(fp_a, fp_b);
    }
}
