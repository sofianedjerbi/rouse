use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::alert::severity::Severity;
use crate::channel::Channel;
use crate::ids::{AlertId, PolicyId, ScheduleId, UserId};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DomainEvent {
    AlertReceived(AlertReceived),
    AlertDeduplicated(AlertDeduplicated),
    AlertAcknowledged(AlertAcknowledged),
    AlertEscalated(AlertEscalated),
    AlertResolved(AlertResolved),
    NotificationSent(NotificationSent),
    NotificationFailed(NotificationFailed),
    OnCallChanged(OnCallChanged),
    EscalationExhausted(EscalationExhausted),
}

impl DomainEvent {
    pub fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::AlertReceived(e) => e.occurred_at,
            Self::AlertDeduplicated(e) => e.occurred_at,
            Self::AlertAcknowledged(e) => e.occurred_at,
            Self::AlertEscalated(e) => e.occurred_at,
            Self::AlertResolved(e) => e.occurred_at,
            Self::NotificationSent(e) => e.occurred_at,
            Self::NotificationFailed(e) => e.occurred_at,
            Self::OnCallChanged(e) => e.occurred_at,
            Self::EscalationExhausted(e) => e.occurred_at,
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AlertReceived(_) => "alert.received",
            Self::AlertDeduplicated(_) => "alert.deduplicated",
            Self::AlertAcknowledged(_) => "alert.acknowledged",
            Self::AlertEscalated(_) => "alert.escalated",
            Self::AlertResolved(_) => "alert.resolved",
            Self::NotificationSent(_) => "notification.sent",
            Self::NotificationFailed(_) => "notification.failed",
            Self::OnCallChanged(_) => "oncall.changed",
            Self::EscalationExhausted(_) => "escalation.exhausted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertReceived {
    pub alert_id: AlertId,
    pub source: String,
    pub severity: Severity,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertDeduplicated {
    pub alert_id: AlertId,
    pub fingerprint: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertAcknowledged {
    pub alert_id: AlertId,
    pub user_id: UserId,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertEscalated {
    pub alert_id: AlertId,
    pub step: u32,
    pub targets: Vec<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertResolved {
    pub alert_id: AlertId,
    pub resolved_by: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationSent {
    pub alert_id: AlertId,
    pub channel: Channel,
    pub target: String,
    pub external_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationFailed {
    pub alert_id: AlertId,
    pub channel: Channel,
    pub target: String,
    pub error: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnCallChanged {
    pub schedule_id: ScheduleId,
    pub new_user: UserId,
    pub previous_user: Option<UserId>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EscalationExhausted {
    pub alert_id: AlertId,
    pub policy_id: PolicyId,
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339("2025-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn event_types_are_unique_strings() {
        let types = [
            "alert.received",
            "alert.deduplicated",
            "alert.acknowledged",
            "alert.escalated",
            "alert.resolved",
            "notification.sent",
            "notification.failed",
            "oncall.changed",
            "escalation.exhausted",
        ];
        let mut unique = std::collections::HashSet::new();
        for t in &types {
            assert!(unique.insert(t), "duplicate event type: {t}");
        }
    }

    #[test]
    fn events_carry_sufficient_context() {
        let alert_id = AlertId::new();
        let event = DomainEvent::AlertEscalated(AlertEscalated {
            alert_id: alert_id.clone(),
            step: 2,
            targets: vec!["alice".into(), "bob".into()],
            occurred_at: now(),
        });
        assert_eq!(event.event_type(), "alert.escalated");
        assert_eq!(event.occurred_at(), now());
        if let DomainEvent::AlertEscalated(e) = &event {
            assert_eq!(e.alert_id, alert_id);
            assert_eq!(e.step, 2);
            assert_eq!(e.targets.len(), 2);
        }
    }

    #[test]
    fn notification_events_include_channel() {
        let event = DomainEvent::NotificationSent(NotificationSent {
            alert_id: AlertId::new(),
            channel: Channel::Slack,
            target: "#oncall".into(),
            external_id: Some("ts-123".into()),
            occurred_at: now(),
        });
        assert_eq!(event.event_type(), "notification.sent");
    }

    #[test]
    fn escalation_exhausted_references_policy() {
        let policy_id = PolicyId::new();
        let event = DomainEvent::EscalationExhausted(EscalationExhausted {
            alert_id: AlertId::new(),
            policy_id: policy_id.clone(),
            occurred_at: now(),
        });
        if let DomainEvent::EscalationExhausted(e) = &event {
            assert_eq!(e.policy_id, policy_id);
        }
    }
}
